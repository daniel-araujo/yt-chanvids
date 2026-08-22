use std::env;
use std::io::Write;
use std::process::exit;

use getopts::Options;

use yt_chanvids::{
    ChannelDetail, FetchLinksError, ParseJsonDataError, RequestError, VideoDetail,
    YtChannelDetailScraper, YtUploadsCrawler, YtVideoDetailScraper,
};

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = &args[0];

    let mut opts = Options::new();
    opts.optflag("h", "help", "Print this help menu.");
    opts.optflag(
        "",
        "json",
        "Print output as JSON (list, channel and video subcommands only).",
    );

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => panic!("{}", f.to_string()),
    };

    let usage = format!(
        "Usage: {program} [OPTIONS] [--] CHANNEL-ID|USERNAME\n   or: {program} list [OPTIONS] [--] CHANNEL-ID|USERNAME\n   or: {program} channel [OPTIONS] [--] CHANNEL-ID|USERNAME\n   or: {program} video [OPTIONS] [--] VIDEO-ID",
        program = program
    );

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

    let json = matches.opt_present("json");

    match matches.free.as_slice() {
        [channel] => list_links(channel, false),
        [subcommand, id] if subcommand == "list" => list_links(id, json),
        [subcommand, id] if subcommand == "channel" => show_channel(id, json),
        [subcommand, id] if subcommand == "video" => show_video(id, json),
        [] => {
            print_usage_mistake("Channel id or username not provided.");
            exit(1);
        }
        _ => {
            print_usage_mistake("Unexpected number of arguments.");
            exit(1);
        }
    }
}

fn list_links(channel: &str, json: bool) {
    let mut crawler = YtUploadsCrawler::channel(channel);

    if json {
        let mut videos = Vec::new();

        while let Some(link) = crawler.next() {
            videos.push(serde_json::json!({
                "id": link.id,
                "url": link.url,
                "title": link.title,
                "thumbnail": link.thumbnail,
            }));
        }

        println!("{}", serde_json::to_string_pretty(&videos).unwrap());
    } else {
        while let Some(link) = crawler.next() {
            println!("{}", link.url);
        }
    }

    if let &Some(ref error) = crawler.error() {
        match error {
            &FetchLinksError::RequestError(ref err) => match err {
                &RequestError::NotFound => {
                    print_error("Channel does not seem to be reachable.");
                    exit(1);
                }
                &RequestError::ParseJsonDataError(ParseJsonDataError::Html(ref html)) => {
                    let message = "A JSON request returned html:\n".to_string() + &html;
                    print_error(&message);
                    exit(1);
                }
                _ => panic!("{:?}", err),
            },
            &FetchLinksError::MissingUploadsPage => {
                print_error("This channel does not have an Uploads page.");
                exit(1);
            }
        }
    }

    exit(0);
}

fn show_channel(id: &str, json: bool) {
    let ChannelDetail {
        title,
        description,
        author_thumbnail,
    } = YtChannelDetailScraper::from_id(id).get();

    if json {
        let value = serde_json::json!({
            "title": title,
            "thumbnail": author_thumbnail,
            "description": description,
        });
        println!("{}", serde_json::to_string_pretty(&value).unwrap());
    } else {
        println!("Title: {}", title);
        println!("Thumbnail: {}", author_thumbnail);
        println!("Description: {}", description);
    }
    exit(0);
}

fn show_video(id: &str, json: bool) {
    let VideoDetail {
        title,
        description,
        duration_seconds,
        publish_date,
    } = YtVideoDetailScraper::from_id(id).get();

    if json {
        let value = serde_json::json!({
            "title": title,
            "published": publish_date,
            "duration_seconds": duration_seconds,
            "description": description,
        });
        println!("{}", serde_json::to_string_pretty(&value).unwrap());
    } else {
        println!("Title: {}", title);
        println!("Published: {}", publish_date);
        println!("Duration: {}s", duration_seconds);
        println!("Description: {}", description);
    }
    exit(0);
}

fn print_error(message: &str) {
    write!(&mut std::io::stderr(), "{}\n", message).unwrap();
}
