use futures::future::join_all;
use reqwest::Url;
use teloxide::types::{Message, MessageEntityKind};
use tokio::{spawn, task::JoinHandle};
use utf16string::{WStr, WString, LE};

use crate::{
    db::Db,
    ogp::{read_open_graph, WebPage},
};

const WHITELISTED_DOMAINS: [&str; 9] = [
    "fixupx.com",
    "www.fixupx.com",
    "fxtwitter.com",
    "www.fxtwitter.com",
    // "www.fxzhihu.com",
    // "zhuanlan.fxzhihu.com",
    "www.youtube.com",
    "youtube.com",
    "youtu.be",
    "github.com",
    "www.github.com",
];

const REDIRECT_DOMAINS: [(&str, &str); 4] = [
    ("x.com", "fixupx.com"),
    ("www.x.com", "www.fixupx.com"),
    ("twitter.com", "fxtwitter.com"),
    ("www.twitter.com", "www.fxtwitter.com"),
    // ("www.zhihu.com", "www.fxzhihu.com"),
    // ("zhuanlan.zhihu.com", "zhuanlan.fxzhihu.com"),
];

pub async fn web_page_handler(msg: Message) {
    if msg.entities().is_none() {
        return;
    }
    let e = msg.entities().unwrap_or_default();

    let mut handles: Vec<JoinHandle<Option<WebPage>>> = vec![];
    let text = msg.text().or(msg.caption()).unwrap().to_string();

    for ele in e {
        if let Some(url) = match &ele.kind {
            MessageEntityKind::Url => get_url_from_text(&text, ele.offset, ele.length),
            MessageEntityKind::TextLink { url } => Some(url.clone()),
            _ => None,
        }
        .and_then(|u| get_url_in_whitelist(&u))
        {
            handles.push(spawn(read_open_graph(url)));
        };
    }

    let web_pages = join_all(handles)
        .await
        .into_iter()
        .filter_map(|x| x.ok().flatten())
        .collect::<Vec<_>>();

    Db::new()
        .insert(
            &web_pages
                .iter()
                .map(|p| crate::types::Message::from(&msg).set_web_page(p))
                .collect::<Vec<_>>(),
        )
        .await;
}

fn get_url_from_text(text: &String, offset: usize, length: usize) -> Option<Url> {
    let text_w: WString<LE> = WString::from(text);
    let u16raw = &text_w.as_bytes()[offset..(offset + length * 2)];
    let u16str: &WStr<LE> = WStr::from_utf16(u16raw).ok()?;
    Url::parse(u16str.to_utf8().as_str()).ok()
}

pub fn get_url_in_whitelist(url: &Url) -> Option<Url> {
    if url.scheme() != "https" {
        return None;
    }

    let mut url = url.clone();
    url.set_host(Some(&get_domain_in_whitelist(url.domain()?)?))
        .unwrap();
    Some(url)
}

fn get_domain_in_whitelist(domain: &str) -> Option<String> {
    if let Some(&d) = WHITELISTED_DOMAINS.iter().find(|&&x| x == domain) {
        Some(d.to_string())
    } else if let Some(&d) = REDIRECT_DOMAINS.iter().find(|&&x| x.0 == domain) {
        Some(d.1.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod web_page_handler_test {
    use super::*;

    #[test]
    fn get_url_from_text_test() {
        assert_eq!(
            Url::parse("https://fixupx.com/mofu_sand/status/1879499797981458689").unwrap(),
            get_url_from_text(
                &"https://fixupx.com/mofu_sand/status/1879499797981458689".to_string(),
                0,
                55
            )
            .unwrap()
        )
    }

    #[test]
    fn non_https_test() {
        assert_eq!(
            get_url_in_whitelist(&Url::parse("mailto:hi@example.com").unwrap()),
            None
        );
    }

    #[test]
    fn get_url_redirect_test() {
        let before = Url::parse("https://x.com/mofu_sand/status/1879499797981458689").unwrap();
        let after = Url::parse("https://fixupx.com/mofu_sand/status/1879499797981458689").unwrap();

        assert_eq!(get_url_in_whitelist(&before).unwrap(), after);
    }

    #[test]
    fn domain_whitelist_test() {
        let domain = "www.fxtwitter.com";
        assert_eq!(get_domain_in_whitelist(domain).unwrap(), domain.to_string());
    }

    #[test]
    fn invalid_domain_test() {
        assert_eq!(
            get_url_in_whitelist(&Url::parse("https://example.com").unwrap()),
            None
        )
    }
}

#[cfg(feature = "private_tests")]
#[cfg(test)]
#[path = "../private_tests/web_page_test.rs"]
mod web_page_test;
