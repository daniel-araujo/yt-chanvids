extern crate hyper;
extern crate rustc_serialize;
extern crate html5ever;
extern crate regex;

use rustc_serialize::json::Json;
use std::io::Read;
use hyper::Client;
use hyper::client::response::Response;
use std::env;
use html5ever::parse_document;
use html5ever::rcdom::{Element, RcDom, Handle};
use html5ever::tendril::TendrilSink;
use regex::Regex;

fn main() {
    let args: Vec<String> = env::args().collect();
    let ref channel: String = args[1];

    let mut links = youtube_video_links(channel);

    while let Some(link) = links.pop() {
        println!("{}", link);
    }
}

fn youtube_video_links(channel: &str) -> Vec<String> {
    let mut links: Vec<String> = Vec::new();

    let data = do_start_request(channel);
    let data = data.as_object().unwrap();

    let body = data.get("body")
        .unwrap()
        .as_object()
        .unwrap()
        .get("content")
        .unwrap()
        .as_string()
        .unwrap();

    let parser = parse_document(RcDom::default(), Default::default())
        .one(body);
    let mut document = parser.document;

    find_links(&mut links, &document);

    while let Some(next_link) = find_next_link(&document) {
        let data = do_next_request(&next_link);
        let data = data.as_object().unwrap();

        let body = data.get("content_html")
            .unwrap()
            .as_string()
            .unwrap();

        let parser = parse_document(RcDom::default(), Default::default())
            .one(body);
        document = parser.document;

        find_links(&mut links, &document);
    }

    return links;

    fn find_links(links: &mut Vec<String>, handle: &Handle) {
        let watch_link_regex = Regex::new(r"^/watch").unwrap();
        let node = handle.borrow();

        for child in node.children.iter() {
            find_links(links, &child.clone());
        }

        match node.node {
            Element(ref name, _, ref attrs) => {
                if let Some(ref parent) = node.parent {
                    if let Some(parent) = parent.upgrade() {
                        let ref parent = *parent;
                        let parent = parent.borrow();

                        match parent.node {
                            Element(ref name, _, _) => {
                                if !name.local.eq_str_ignore_ascii_case("h3") {
                                    return;
                                }
                            },
                            _ => return,
                        }
                    } else {
                        return;
                    }
                } else {
                    return;
                }

                if !name.local.eq_str_ignore_ascii_case("a") {
                    return;
                }

                for attr in attrs.iter() {
                    if !attr.name.local.eq_str_ignore_ascii_case("href") {
                        continue;
                    }

                    if !watch_link_regex.is_match(&attr.value) {
                        continue;
                    }

                    links.push(prettify_link_url(&attr.value));
                }
            },
            _ => {},
        }
    }

    fn find_next_link(handle: &Handle) -> Option<String> {
        let browse_link_regex = Regex::new(r"^/browse_ajax").unwrap();
        let node = handle.borrow();

        for child in node.children.iter() {
            if let Some(link) = find_next_link(&child.clone()) {
                return Some(link);
            }
        }

        match node.node {
            Element(_, _, ref attrs) => {
                for attr in attrs.iter() {
                    if !attr.name.local.eq_str_ignore_ascii_case("data-uix-load-more-href") {
                        continue;
                    }

                    if !browse_link_regex.is_match(&attr.value) {
                        continue;
                    }

                    return Some(prettify_link_url(&attr.value));
                }
            },
            _ => {},
        };

        return None;
    }
}

fn do_start_request(channel: &str) -> Json {
    let start_url = String::from("https://www.youtube.com/user/")
        + channel
        + "/videos?live_view=500&flow=grid&view=0&sort=dd&spf=navigate";

    let client = Client::new();

    parse_json_response(client.get(&start_url).send().unwrap())
}

fn do_next_request(next_url: &str) -> Json {
    let client = Client::new();

    parse_json_response(client.get(next_url).send().unwrap())
}

fn parse_json_response(mut res: Response) -> Json {
    let mut body = String::new();

    res.read_to_string(&mut body).unwrap();

    return Json::from_str(&body).unwrap();
}

fn prettify_link_url(l: &str) -> String {
    if l.starts_with("/") {
        return String::from("https://www.youtube.com") + l;
    }

    return String::from(l);
}

#[allow(dead_code)]
fn unused_function() {}
