extern crate hyper;
extern crate rustc_serialize;
extern crate html5ever;
extern crate regex;
extern crate getopts;

use rustc_serialize::json::Json;
use std::io::Read;
use hyper::Client;
use hyper::status::StatusCode;
use hyper::client::response::Response;
use std::env;
use html5ever::parse_document;
use html5ever::rcdom::{Element, RcDom, Handle};
use html5ever::tendril::TendrilSink;
use regex::Regex;
use getopts::Options;
use std::process::exit;
use std::io::Write;

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = &args[0];

    let mut opts = Options::new();
    opts.optflag("h", "help", "Print this help menu.");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => panic!(f.to_string()),
    };

    if matches.opt_present("h") {
        print_usage(&program, opts);
        exit(0);
    }

    match matches.free.len() {
        0 => {
            writeln!(&mut std::io::stderr(), "Missing channel argument.")
                .unwrap();
            exit(1);
        },
        1 => {
            let channel = &matches.free[0];
            print_links(channel);
            exit(0);
        },
        _ => {
            writeln!(&mut std::io::stderr(), "Unexpected number of arguments.")
                .unwrap();
            exit(1);
        }
    }
}

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} [OPTIONS] CHANNEL-ID|USERNAME", program);
    print!("{}", opts.usage(&brief));
}

fn print_links(channel: &str) {
    let mut links = youtube_video_links(channel);

    while let Some(link) = links.pop() {
        println!("{}", link);
    }
}

fn youtube_video_links(channel: &str) -> Vec<String> {
    let mut links: Vec<String> = Vec::new();

    let data = do_start_request(channel);
    let data = data.as_object().unwrap();

    let content = data.get("body")
        .unwrap()
        .as_object()
        .unwrap()
        .get("content")
        .unwrap()
        .as_string()
        .unwrap();

    let content_parser = parse_document(RcDom::default(), Default::default())
        .one(content);

    find_video_links(&mut links, &content_parser.document);

    if let Some(mut next_page_link) = find_next_page_link(&content_parser.document) {
        loop {
            let data = do_next_page_request(&next_page_link);
            let data = data.as_object().unwrap();

            let content_html = data.get("content_html")
                .unwrap()
                .as_string()
                .unwrap();

            let content_parser = parse_document(RcDom::default(), Default::default())
                .one(content_html);

            find_video_links(&mut links, &content_parser.document);

            let load_more_widget_html = data.get("load_more_widget_html")
                .unwrap()
                .as_string()
                .unwrap();

            let load_more_widget_parser = parse_document(RcDom::default(), Default::default())
                .one(load_more_widget_html);

            match find_next_page_link(&load_more_widget_parser.document) {
                Some(url) => next_page_link = url,
                None => break,
            }
        }
    }

    return links;

    fn find_video_links(links: &mut Vec<String>, handle: &Handle) {
        let watch_link_regex = Regex::new(r"^/watch").unwrap();
        let node = handle.borrow();

        for child in node.children.iter() {
            find_video_links(links, &child.clone());
        }

        if let Element(ref name, _, ref attrs) = node.node {
            if let Some(ref parent) = node.parent {
                let parent = parent.upgrade().unwrap();
                let parent = parent.borrow();

                if let Element(ref name, _, _) = parent.node {
                    if !name.local.eq_str_ignore_ascii_case("h3") {
                        return;
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

                links.push(canonicalize_video_url(&attr.value));
            }
        }
    }

    fn find_next_page_link(handle: &Handle) -> Option<String> {
        let browse_link_regex = Regex::new(r"^/browse_ajax").unwrap();
        let node = handle.borrow();

        for child in node.children.iter() {
            if let Some(link) = find_next_page_link(&child.clone()) {
                return Some(link);
            }
        }

        if let Element(_, _, ref attrs) = node.node {
            for attr in attrs.iter() {
                if !attr.name.local.eq_str_ignore_ascii_case("data-uix-load-more-href") {
                    continue;
                }

                if !browse_link_regex.is_match(&attr.value) {
                    continue;
                }

                return Some(canonicalize_video_url(&attr.value));
            }
        }

        return None;
    }
}

fn do_start_request(channel: &str) -> Json {
    let start_url = String::from("https://www.youtube.com/channel/")
        + channel
        + "/videos?live_view=500&flow=grid&view=0&sort=dd&spf=navigate";

    let client = Client::new();

    let res = client.get(&start_url).send().unwrap();

    if let StatusCode::Ok = res.status {
        parse_json_response(res)
    } else {
        let start_url = String::from("https://www.youtube.com/user/")
            + channel
            + "/videos?live_view=500&flow=grid&view=0&sort=dd&spf=navigate";

        let res = client.get(&start_url).send().unwrap();

        if let StatusCode::Ok = res.status {
            parse_json_response(res)
        } else {
            panic!("Channel does not seem to be reachable");
        }
    }
}

fn do_next_page_request(next_url: &str) -> Json {
    let client = Client::new();

    parse_json_response(client.get(next_url).send().unwrap())
}

fn parse_json_response(mut res: Response) -> Json {
    let mut body = String::new();

    res.read_to_string(&mut body).unwrap();

    return Json::from_str(&body).unwrap();
}

fn canonicalize_video_url(url: &str) -> String {
    if url.starts_with("/") {
        return String::from("https://www.youtube.com") + url;
    }

    return String::from(url);
}
