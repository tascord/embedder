use scraper::{ElementRef, Html, Selector};
use types::{WebData, OgType};
use url::Url;

pub mod types;

pub async fn fetch(url: &str) -> WebData {
    let document = Html::parse_document(
        &reqwest::get(url)
            .await
            .expect("Failed to fetch url")
            .text()
            .await
            .expect("Failed to read response"),
    );

    let find = |id: &str| {
        document
            .select(&Selector::parse(id).expect(&format!("Failed to build selector: {id}")))
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
        .unwrap();

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

    data
}

pub fn resolve_url(url: &str, base: &str) -> String {
    if url.starts_with("/") || url.starts_with("./") {
        let base = Url::parse(base).unwrap().origin().unicode_serialization();
        return Url::parse(&base).unwrap().join(url).unwrap().to_string();
    }

    return url.to_string();
}



#[cfg(test)]
pub mod tests {
    use std::env::args;
    use super::*;

    #[tokio::test]
    async fn main() {
        let url = args().nth(1).expect("No url provided");
        let data = fetch(&url).await;
        println!("{:#?}", data);
    }
}