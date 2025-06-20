use crate::types::{OgType, WebData};
use crate::utils::*;
use async_process::{Child, Command};
use fantoccini::{elements::Element, Client, ClientBuilder};
use futures::{future::try_join_all, lock::Mutex};
use http::Method;
use http_body_util::BodyExt;
use std::process;

pub use fantoccini::{wd::Capabilities, Locator};

lazy_static::lazy_static! {
    static ref DRIVER: Mutex<Option<Client>> = Mutex::new(None);
    static ref CHILD: Mutex<Option<Child>> = Mutex::new(None);
}

pub async fn get_driver() -> Client {
    DRIVER
        .lock()
        .await
        .as_ref()
        .expect("Client not initialized")
        .clone()
}

/// Starts a geckodriver instance on the specified port, and initializes the driver.
/// If no port is specified, the default port of 4444 is used.
/// If no binary is specified, 'which' is used to find the firefox binary.
pub async fn start(
    port: Option<usize>,
    binary: Option<&str>,
    capabilities: Option<Capabilities>,
) -> anyhow::Result<()> {
    if DRIVER.lock().await.is_some() {
        eprintln!("Driver already initialized, skipping start");
        return Ok(());
    }

    let mut command = Command::new("geckodriver");
    command.env("MOZ_HEADLESS", "1");

    if let Some(binary) = binary {
        command.arg("-b").arg(binary);
    } else {
        let bin_location = which::which("firefox")
            .map_err(|_| anyhow::anyhow!("Failed to find firefox binary"))?;
        command.arg("-b").arg(bin_location);
    }

    if let Some(port) = port {
        command.arg("-p").arg(port.to_string());
    }

    if std::env::var("EMBEDDER_DEBUG").is_err() {
        command.stdin(process::Stdio::null());
        command.stdout(process::Stdio::null());
        command.stderr(process::Stdio::null());
    }

    *CHILD.lock().await = Some(
        command
            .spawn()
            .map_err(|_| anyhow::anyhow!("Failed to start geckodriver"))?,
    );

    let address = format!("http://localhost:{}", port.unwrap_or(4444));
    init(&address, capabilities).await;

    Ok(())
}

/// Initializes the driver with the specified address.
pub async fn init(address: &str, capabilities: Option<Capabilities>) {
    if DRIVER.lock().await.is_some() {
        eprintln!("Driver already initialized, skipping connection");
        return;
    }

    let driver = ClientBuilder::native()
        .capabilities(capabilities.unwrap_or_default())
        .connect(address)
        .await
        .expect("Failed to connect to driver");

    *DRIVER.lock().await = Some(driver);
}

/// Closes the driver and geckodriver instance.
/// Without calling this, the geckodriver instance will remain open.
pub async fn close() {
    if DRIVER.lock().await.is_none() {
        eprintln!("Driver not initialized, skipping close");
        return;
    }

    // fix this clippy lint?
    DRIVER.lock().await.take().unwrap().close().await.unwrap();
    CHILD.lock().await.take().unwrap().kill().unwrap();
}

/// Fetches the data from the specified url.
pub async fn fetch(url: &str) -> anyhow::Result<WebData> {
    let driver = get_driver().await;
    driver
        .goto(url)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to navigate to url: {:?}", e))?;

    let mut data = WebData::default();

    // <title>
    data.title = driver.title().await.unwrap_or_default();

    // <meta name="description" />
    data.description = get_single(driver.clone(), "meta[property=\"og:description\"]").await;

    // <meta property="og:type" />
    data.r#type = match get_single(driver.clone(), "meta[property=\"og:type\"]").await {
        Some(t) => OgType::from_meta(t.as_str()),
        None => OgType::Website,
    };

    // <meta property="og:image" />
    data.image = get_single(driver.clone(), "meta[property=\"og:image\"]").await;

    // <meta property="book:author" />, <meta property="article:author" />
    data.author = get_multiple(driver.clone(), "meta[property$=\":author\"]").await;

    // <meta name="theme-color" />
    data.colour = match find(driver.clone(), "meta[name=\"theme-color\"]")
        .await
        .first()
    {
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
    url: &str,
    locator: Locator<'_>,
    link_attr_name: &str,
    override_dl_link: Option<&str>,
) -> anyhow::Result<Vec<u8>> {
    let driver = get_driver().await;

    driver
        .goto(url)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to navigate to url: {:?}", e))?;

    let elem = driver.find(locator).await?;
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
            Ok(frame) if frame.is_data() => result.extend_from_slice(&frame.into_data().unwrap()),
            _ => continue,
        }
    }
    Ok(result)
}

#[cfg(test)]
pub mod test {
    use super::*;

    #[tokio::test]
    async fn test_fetch_and_download() {
        start(None, None, None).await.unwrap();

        let data = fetch("https://reneweconomy.com.au/aemos-jaw-dropping-prediction-for-coal-power-all-but-gone-from-the-grid-in-a-decade")
            .await
            .unwrap();

        println!("{:?}", data);

        let data = download_file_from(
            "https://hil-speed.hetzner.com/",
            Locator::Css("body > p:nth-child(2) > a"),
            "href",
            None,
        )
        .await
        .unwrap();

        println!("{:?}", data.len());

        close().await;
    }
}
