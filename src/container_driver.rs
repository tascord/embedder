use crate::{types::OgType, utils::*};
use async_process::Command;
use fantoccini::ClientBuilder;
use futures::AsyncWriteExt;
use http::Method;
use http_body_util::BodyExt;
use std::{
    fmt::Debug,
    net::TcpListener,
    ops::{Deref, Not},
    path::PathBuf,
    process::Stdio,
    time::Duration,
};

pub use fantoccini::{wd::Capabilities, Locator};

use crate::types::WebData;

const DOCKERFILE: &str = include_str!("../Dockerfile");
const BUILD_LOCK: &str = "/tmp/embedder-build.lock";
const PORT_LOCK: &str = "/tmp/embedder-port.lock";
#[derive(Debug)]
pub struct Driver(fantoccini::Client, String, u16);

impl Driver {
    pub fn name(&self) -> &str {
        &self.1
    }
    pub fn port(&self) -> u16 {
        self.2
    }

    pub async fn new(
        port: Option<u16>,
        capabilities: Option<Capabilities>,
        name: impl AsRef<str>,
    ) -> anyhow::Result<Driver> {
        let name = name.as_ref();

        let runner = which::which("podman")
            .or(which::which("docker"))
            .map_err(|_| anyhow::anyhow!("No `podman` or `docker` installed!"))?;

        let port = build_container(port, &runner, name).await?;

        let address = format!("http://127.0.0.1:{}", port);

        // println!("\n\nRunning {} on: {address}\n\n", name);
        let driver = ClientBuilder::native()
            .capabilities(capabilities.unwrap_or_default())
            .connect(&address)
            .await?;

        Ok(Driver(driver, name.into(), port))
    }

    /// Fetches the data from the specified url.
    pub async fn fetch(&self, url: &str) -> anyhow::Result<WebData> {
        self.goto(url)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to navigate to url: {:?}", e))?;

        let mut data = WebData::default();

        // <title>
        data.title = self.title().await.unwrap_or_default();

        // <meta name="description" />
        data.description = get_single(self, "meta[property=\"og:description\"]").await;

        // <meta property="og:type" />
        data.r#type = match get_single(self, "meta[property=\"og:type\"]").await {
            Some(t) => OgType::from_meta(t.as_str()),
            None => OgType::Website,
        };

        // <meta property="og:image" />
        data.image = get_single(self, "meta[property=\"og:image\"]").await;

        // <meta property="book:author" />, <meta property="article:author" />
        data.author = get_multiple(self, "meta[property$=\":author\"]").await;

        // <meta name="theme-color" />
        data.colour = match find(self, "meta[name=\"theme-color\"]").await.first() {
            Some(e) => Some(
                e.attr("value")
                    .await
                    .unwrap_or_default()
                    .unwrap_or_default(),
            ),
            None => None,
        };

        Ok(data)
    }

    /// Downloads the bytes from the specified file download url.
    pub async fn download_file_from(
        &self,
        url: &str,
        locator: Locator<'_>,
        link_attr_name: &str,
        override_dl_link: Option<&str>,
    ) -> anyhow::Result<Vec<u8>> {
        self.goto(url)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to navigate to url: {:?}", e))?;

        let elem = self.find(locator).await?;
        let dl_link = elem
            .attr(link_attr_name)
            .await?
            .unwrap_or_else(|| panic!("Element should have a {link_attr_name}!"));

        let dl_link = override_dl_link.unwrap_or(dl_link.as_str());

        let response = elem.client().raw_client_for(Method::GET, dl_link).await?;

        let mut body = response.into_body();

        let mut result = Vec::new();
        while let Some(frame) = body.frame().await {
            match frame {
                Ok(frame) if frame.is_data() => {
                    result.extend_from_slice(&frame.into_data().unwrap())
                }
                _ => continue,
            }
        }
        Ok(result)
    }
}
impl Deref for Driver {
    type Target = fantoccini::Client;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl Drop for Driver {
    fn drop(&mut self) {
        let runner = which::which("podman")
            .or(which::which("docker"))
            .map_err(|_| "No `podman` or `docker` installed!")
            .unwrap();

        std::process::Command::new(&runner)
            .arg("stop")
            .arg(format!("embedder-{}", self.1))
            .output()
            .unwrap();
        std::process::Command::new(&runner)
            .arg("rm")
            .arg(format!("embedder-{}", self.1))
            .output()
            .unwrap();
    }
}

fn is_port_in_use(port: u16) -> bool {
    match TcpListener::bind(("127.0.0.1", port)) {
        Ok(_listener) => false,
        Err(_) => true,
    }
}

async fn build_container(port: Option<u16>, runner: &PathBuf, name: &str) -> anyhow::Result<u16> {
    loop {
        if tokio::fs::try_exists(BUILD_LOCK).await? {
            println!("{} exists, waiting.", BUILD_LOCK);
            tokio::time::sleep(Duration::from_millis(500)).await;
        } else {
            break;
        }
    }
    tokio::fs::write(BUILD_LOCK, Vec::new()).await?;

    let built = Command::new(runner)
        .arg("image")
        .arg("exists")
        .arg("embedder-container:latest")
        .output()
        .await?
        .status
        .success();

    if built.not() {
        let mut build = Command::new(runner)
            .arg("build")
            .arg("-f")
            .arg("-")
            .arg("-t")
            .arg("embedder-container")
            .arg(".")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let stdin = build.stdin.as_mut().expect("Failed to open stdin");
        stdin.write_all(DOCKERFILE.as_bytes()).await?;
        let _ = build.output().await?;
    }
    tokio::fs::remove_file(BUILD_LOCK).await?;

    let open_port = {
        loop {
            if tokio::fs::try_exists(PORT_LOCK).await? {
                println!("{} exists, waiting.", PORT_LOCK);
                tokio::time::sleep(Duration::from_millis(500)).await;
            } else {
                break;
            }
        }
        tokio::fs::write(PORT_LOCK, Vec::new()).await?;

        let mut p = 4444;
        for i in 0.. {
            let p2 = 4444u16 + i;

            if is_port_in_use(p2) {
                continue;
            } else {
                p = p2;
                break;
            }
        }
        p
    };
    tokio::fs::remove_file(PORT_LOCK).await?;

    let port = port.unwrap_or(open_port);
    Command::new(runner)
        .arg("run")
        .arg("-p")
        .arg(format!("{port}:{port}"))
        .arg("--name")
        .arg(format!("embedder-{name}"))
        .arg("-d")
        .arg("embedder-container")
        .arg("/usr/bin/geckodriver")
        .arg("--host")
        .arg("0.0.0.0")
        .arg("-p")
        .arg(port.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .await?;
    Ok(port)
}

#[cfg(test)]
pub mod test {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[tokio::test]
    async fn test_fetch_and_download() {
        let driver = Driver::new(
            None,
            None,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                .to_string(),
        )
        .await
        .unwrap();

        let data =  driver.fetch("https://reneweconomy.com.au/aemos-jaw-dropping-prediction-for-coal-power-all-but-gone-from-the-grid-in-a-decade")
            .await
            .unwrap();

        println!("{:?}", data);

        let data = driver
            .download_file_from(
                "https://hil-speed.hetzner.com/",
                Locator::Css("body > p:nth-child(2) > a"),
                "href",
                None,
            )
            .await
            .unwrap();

        println!("{:?}", data.len());
    }
}
