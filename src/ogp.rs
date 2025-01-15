use tl::{NodeHandle, Parser};

#[derive(Debug)]
pub struct WebPage {
    url: String,
    title: String,
    desc: String,
    thumbnail_url: Option<String>,
}

async fn read_open_graph(url: String) -> Option<WebPage> {
    let body = reqwest::get(url).await.ok()?.text().await.ok()?;
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
            .next()?
            .get_attr(parser),
    })
}

trait ElementHandle {
    fn get_attr(&self, parser: &Parser) -> Option<String>;
}

impl ElementHandle for NodeHandle {
    fn get_attr(&self, parser: &Parser) -> Option<String> {
        Some(
            self.get(parser)?
                .as_tag()?
                .attributes()
                .get("content")??
                .try_as_utf8_str()?
                .to_string(),
        )
    }
}

#[cfg(test)]
mod ogp_test {
    use super::*;

    #[tokio::test]
    async fn read_open_graph_test() {
        println!(
            "{:?}",
            read_open_graph("https://youtu.be/g-pG79LOtMw".to_string()).await
        );
    }
}
