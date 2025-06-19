use async_process::Command;
use fantoccini::ClientBuilder;
use futures::AsyncWriteExt;
use std::{ops::Deref, path::PathBuf, process::Stdio};

pub use fantoccini::{wd::Capabilities, Locator};

const DOCKERFILE: &str = include_str!("../Dockerfile");

pub struct Driver(fantoccini::Client, String);

impl Driver {
    pub fn name(&self) -> &str {
        &self.1
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
    }
}

async fn is_port_in_use(port: u16) -> bool {
    String::from_utf8_lossy(
        &Command::new("ss")
            .arg("-tulpn")
            .arg("|")
            .arg("grep")
            .arg(format!("':{port}'"))
            .output()
            .await
            .unwrap()
            .stdout,
    )
    .lines()
    .count()
        > 1
}

async fn build_container(port: Option<u16>, runner: &PathBuf, name: &str) -> anyhow::Result<u16> {
    // start container
    let mut build = Command::new(runner)
        .arg("build")
        .arg("-f")
        .arg("-")
        .arg("-t")
        .arg("embedder-container")
        .arg(".")
        .stdin(Stdio::piped())
        .spawn()?;
    let stdin = build.stdin.as_mut().expect("Failed to open stdin");
    stdin.write_all(DOCKERFILE.as_bytes()).await?;
    let _ = build.output().await?;

    let open_port = {
        let mut p = 4444;
        for i in 0.. {
            let p2 = 4444u16 + i;
            if is_port_in_use(p2).await {
                continue;
            }
            p = p2;
        }
        p
    };
    let port = port.unwrap_or(open_port);
    Command::new(runner)
        .arg("run")
        .arg("-p")
        .arg(format!("{port}:4444"))
        .arg("--name")
        .arg(format!("embedder-{name}"))
        .arg("embedder-container")
        .output()
        .await?;

    Ok(port)
}

/// Starts a geckodriver instance on the specified port, and initializes the driver.
/// If no port is specified, the default port of 4444 is used.
/// If no binary is specified, 'which' is used to find the firefox binary.
pub async fn start(
    port: Option<u16>,
    capabilities: Option<Capabilities>,
    name: impl AsRef<str>,
) -> anyhow::Result<Driver> {
    let name = name.as_ref();
    let runner = which::which("podman")
        .or(which::which("docker"))
        .map_err(|_| anyhow::anyhow!("No `podman` or `docker` installed!"))?;

    let port = build_container(port, &runner, name).await?;

    let mut command = Command::new(&runner);
    command
        .arg("run")
        .arg("--rm")
        .arg(format!("embedder-{name}"))
        .arg("geckodriver")
        .arg("-b")
        .arg(
            which::which("firefox")
                .map_err(|_| anyhow::anyhow!("Failed to find firefox binary"))?,
        );
    command
        .output()
        .await
        .map_err(|_| anyhow::anyhow!("Failed to start geckodriver"))?;

    let address = format!("http://localhost:{}", port);

    let driver = ClientBuilder::native()
        .capabilities(capabilities.unwrap_or_default())
        .connect(&address)
        .await
        .expect("Failed to connect to driver");

    Ok(Driver(driver, name.into()))
}
