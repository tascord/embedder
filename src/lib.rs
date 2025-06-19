#![allow(clippy::field_reassign_with_default)]

use scraper::{ElementRef, Html, Selector};
use types::{OgType, WebData};
use url::Url;

pub mod types;
pub use types as ty;

#[cfg(all(feature = "driver", not(feature = "container")))]
pub mod driver;
#[cfg(all(feature = "driver", not(feature = "container")))]
pub use driver as dr;
#[cfg(feature = "container")]
pub mod container_driver;
#[cfg(feature = "container")]
pub use container_driver as dr;

/// Fetches the data from the given url.
pub async fn fetch(url: &str) -> anyhow::Result<WebData> {
    let document = Html::parse_document(
        &reqwest::get(url)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch url: {:?}", e))?
            .text()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to read response: {:?}", e))?,
    );

    let find = |id: &str| {
        document
            .select(
                &Selector::parse(id).unwrap_or_else(|_| panic!("Failed to build selector: {id}")),
            )
            .collect::<Vec<ElementRef>>()
    };

    let mut data = WebData::default();

    // <title>
    data.title = find("title").first().unwrap().text().collect();

    // <meta name="description" />
    data.description = find("meta[property=\"og:description\"]")
        .first()
        .map(|e| e.value().attr("content").unwrap().to_string());

    // <meta property="og:type" />
    data.r#type = find("meta[property=\"og:type\"]")
        .first()
        .map(|e| OgType::from_meta(e.value().attr("content").unwrap()))
        .unwrap_or_default();

    // <meta property="og:image" />
    data.image = find("meta[property=\"og:image\"]")
        .first()
        .map(|e| resolve_url(e.value().attr("content").unwrap(), url));

    // <meta property="book:author" />, <meta property="article:author" />
    data.author = find("meta[property$=\":author\"]")
        .iter()
        .map(|e| e.value().attr("content").unwrap().to_string())
        .collect();

    // <meta name="theme-color" />
    data.colour = find("meta[name=\"theme-color\"]")
        .first()
        .map(|e| e.value().attr("content").unwrap().to_string());

    Ok(data)
}

/// Resolves the given url to an absolute url.
pub fn resolve_url(url: &str, base: &str) -> String {
    if url.starts_with('/') || url.starts_with("./") {
        let base = Url::parse(base).unwrap().origin().unicode_serialization();
        return Url::parse(&base).unwrap().join(url).unwrap().to_string();
    }

    url.to_string()
}

#[cfg(test)]
pub mod test {
    use crate::fetch;

    #[tokio::test]
    async fn a() {
        let url = "https://reneweconomy.com.au/market-operator-ticks-off-one-of-major-challenges-of-meeting-100-pct-renewables/";
        println!("{:?}", fetch(url).await.unwrap());
    }

    #[tokio::test]
    async fn b() {
        let url = "https://oilprice.com/Latest-Energy-News/World-News/US-Natural-Gas-Prices-Tumble-10-on-Mild-Weather.html";
        println!("{:?}", fetch(url).await.unwrap());
    }
}
