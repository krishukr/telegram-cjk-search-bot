use cached::proc_macro::cached;
use reqwest::{IntoUrl, Url};
use tl::{NodeHandle, Parser};

#[derive(Debug, Clone)]
pub struct WebPage {
    pub url: Url,
    pub title: String,
    pub desc: String,
    pub thumbnail_url: Option<Url>,
}

const APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

#[cached(
    time = 300,
    sync_writes = true,
    key = "Url",
    convert = r#"{ url.clone().into_url().ok()? }"#
)]
pub async fn read_open_graph(url: impl IntoUrl + Clone) -> Option<WebPage> {
    let client = reqwest::Client::builder()
        .user_agent(APP_USER_AGENT)
        .timeout(std::time::Duration::from_millis(5000))
        .build()
        .ok()?;
    let body = client.get(url).send().await.ok()?.text().await.ok()?;

    let dom = tl::parse(&body, tl::ParserOptions::default()).ok()?;
    let parser = dom.parser();
    Some(WebPage {
        url: dom
            .query_selector("meta[property='og:url']")?
            .next()?
            .get_attr(parser)?,
        title: dom
            .query_selector("meta[property='og:title']")?
            .next()?
            .get_attr(parser)?,
        desc: dom
            .query_selector("meta[property='og:description']")?
            .next()?
            .get_attr(parser)?,
        thumbnail_url: dom
            .query_selector("meta[property='og:image']")?
            .next()
            .map(|x| x.get_attr(parser))
            .flatten(),
    })
}

trait ElementHandle {
    fn get_attr<T>(&self, parser: &Parser) -> Option<T>
    where
        T: std::str::FromStr;
}

impl ElementHandle for NodeHandle {
    fn get_attr<T>(&self, parser: &Parser) -> Option<T>
    where
        T: std::str::FromStr,
    {
        self.get(parser)?
            .as_tag()?
            .attributes()
            .get("content")??
            .try_as_utf8_str()?
            .parse()
            .ok()
    }
}

#[cfg(feature = "private_tests")]
#[cfg(test)]
#[path = "./private_tests/ogp_test.rs"]
mod ogp_test;
