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

    let usage = format!("Usage: {} [OPTIONS] [--] CHANNEL-ID|USERNAME", program);

    let print_help = || {
        write!(&mut std::io::stdout(), "{}", opts.usage(&usage)).unwrap();
    };

    let print_usage_mistake = |mistake| {
        write!(&mut std::io::stderr(), "{}\n\n", mistake).unwrap();
        write!(&mut std::io::stderr(), "{}", opts.usage(&usage)).unwrap();
    };

    if matches.opt_present("h") {
        print_help();
        exit(0);
    }

    match matches.free.len() {
        0 => {
            print_usage_mistake("Channel id or username not provided.");
            exit(1);
        },
        1 => {
            let channel = &matches.free[0];
            print_links(channel);
            exit(0);
        },
        _ => {
            print_usage_mistake("Unexpected number of arguments.");
            exit(1);
        }
    }

    fn print_links(channel: &str) {
        match youtube_video_links(channel) {
            Ok(mut links) => {
                while let Some(link) = links.pop() {
                    println!("{}", link);
                }
            },
            Err(err) => {
                let FetchLinksError::RequestError(err) = err;

                match err {
                    RequestError::NotFound => {
                        print_error("Channel does not seem to be reachable.");
                        exit(1);
                    },
                    _ => panic!(err),
                }
            },
        }
    }

    fn print_error(message: &str) {
        write!(&mut std::io::stderr(), "{}\n", message).unwrap();
    };
}

fn youtube_video_links(channel: &str) -> Result<Vec<String>, FetchLinksError> {
    let mut links: Vec<String> = Vec::new();

    let data = try!(do_start_request(channel)
        .map_err(FetchLinksError::RequestError));
    let data = data.as_object()
        .unwrap();

    let content = data.get("body")
        .and_then(|i| i.as_object())
        .and_then(|i| i.get("content"))
        .and_then(|i| i.as_string())
        .unwrap();

    let content_parser = parse_document(RcDom::default(), Default::default())
        .one(content);

    find_video_links(&mut links, &content_parser.document);

    if let Some(mut next_page_link) = find_next_page_link(&content_parser.document) {
        loop {
            let data = try!(do_next_page_request(&next_page_link)
                .map_err(FetchLinksError::RequestError));
            let data = data.as_object()
                .unwrap();

            let content_html = data.get("content_html")
                .and_then(|i| i.as_string())
                .unwrap();

            let content_parser = parse_document(RcDom::default(), Default::default())
                .one(content_html);

            find_video_links(&mut links, &content_parser.document);

            let load_more_widget_html = data.get("load_more_widget_html")
                .and_then(|i| i.as_string())
                .unwrap();

            let load_more_widget_parser = parse_document(RcDom::default(), Default::default())
                .one(load_more_widget_html);

            match find_next_page_link(&load_more_widget_parser.document) {
                Some(url) => next_page_link = url,
                None => break,
            }
        }
    }

    return Ok(links);

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

fn do_start_request(id: &str) -> Result<Json, RequestError> {
    do_start_request_channel(id)
        .or_else(|_| do_start_request_username(id))
}

fn do_start_request_channel(channel: &str) -> Result<Json, RequestError> {
    let start_url = String::from("https://www.youtube.com/channel/")
        + channel
        + "/videos?live_view=500&flow=grid&view=0&sort=dd&spf=navigate";

    let client = Client::new();

    let res = try!(client.get(&start_url)
        .send()
        .map_err(RequestError::HyperError));

    if let StatusCode::Ok = res.status {
        return parse_json_response(res)
            .map_err(RequestError::ParseJsonError)
    } else {
        return Err(RequestError::NotFound);
    }
}

fn do_start_request_username(username: &str) -> Result<Json, RequestError> {
    let start_url = String::from("https://www.youtube.com/user/")
        + username
        + "/videos?live_view=500&flow=grid&view=0&sort=dd&spf=navigate";

    let client = Client::new();

    let res = try!(client.get(&start_url)
        .send()
        .map_err(RequestError::HyperError));

    if let StatusCode::Ok = res.status {
        parse_json_response(res)
            .map_err(RequestError::ParseJsonError)
    } else {
        return Err(RequestError::NotFound);
    }
}

fn do_next_page_request(next_url: &str) -> Result<Json, RequestError> {
    let client = Client::new();

    let res = try!(client.get(next_url)
        .send()
        .map_err(RequestError::HyperError));

    parse_json_response(res)
        .map_err(RequestError::ParseJsonError)
}

fn parse_json_response(mut res: Response) -> Result<Json, ParseJsonError> {
    let mut body = String::new();

    try!(res.read_to_string(&mut body).map_err(ParseJsonError::Io));
    
    return Json::from_str(&body).map_err(ParseJsonError::Parse)
}

fn canonicalize_video_url(url: &str) -> String {
    if url.starts_with("/") {
        return String::from("https://www.youtube.com") + url;
    }

    return String::from(url);
}

#[derive(Debug)]
enum FetchLinksError {
    RequestError(RequestError),
}

#[derive(Debug)]
enum RequestError {
    HyperError(hyper::Error),
    ParseJsonError(ParseJsonError),
    NotFound,
}

#[derive(Debug)]
enum ParseJsonError {
    Io(std::io::Error),
    Parse(rustc_serialize::json::ParserError),
}
