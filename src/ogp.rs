use cached::proc_macro::cached;
use reqwest::{IntoUrl, Url};
use tl::{NodeHandle, Parser};

#[derive(Debug)]
pub struct WebPage {
    pub url: Url,
    pub title: String,
    pub desc: String,
    pub thumbnail_url: Option<Url>,
}

const APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

pub async fn read_open_graph<T>(url: T) -> Option<WebPage>
where
    T: IntoUrl,
{
    let body = get_url_body(url.into_url().ok()?).await?;
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

#[cached(time = 300)]
async fn get_url_body(url: Url) -> Option<String> {
    let client = reqwest::Client::builder()
        .user_agent(APP_USER_AGENT)
        .build()
        .ok()?;
    client.get(url).send().await.ok()?.text().await.ok()
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
