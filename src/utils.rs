use fantoccini::{elements::Element, Client, Locator};
use futures::future::try_join_all;

pub(crate) async fn find(d: &Client, id: &str) -> Vec<Element> {
    d.find_all(Locator::Css(id)).await.unwrap_or_default()
}

pub(crate) async fn get_single(d: &Client, q: &str) -> Option<String> {
    let e = find(d, q).await;
    let e = e.first()?;

    let r = e.attr("content").await;

    r.unwrap_or_default()
}
pub(crate) async fn get_multiple(d: &Client, q: &str) -> Vec<String> {
    let e = find(d, q).await;
    let r = try_join_all(e.iter().map(|e| e.attr("content"))).await;

    if r.is_err() {
        return vec![];
    }

    let r = r.unwrap();
    let v: Vec<String> = r
        .iter()
        .filter(|v| v.is_some())
        .map(|v| v.as_ref().unwrap().clone())
        .collect();

    v
}
