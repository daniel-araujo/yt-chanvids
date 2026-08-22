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
        [channel] => list_links(&parse_channel_arg(channel), false),
        [subcommand, id] if subcommand == "list" => list_links(&parse_channel_arg(id), json),
        [subcommand, id] if subcommand == "channel" => show_channel(&parse_channel_arg(id), json),
        [subcommand, id] if subcommand == "video" => show_video(&parse_video_arg(id), json),
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

/// Extracts a channel id, handle or username out of a pasted YouTube
/// channel URL. Anything that isn't a recognized YouTube URL (including a
/// bare id, handle or username) is returned unchanged.
fn parse_channel_arg(input: &str) -> String {
    let url = match url::Url::parse(input) {
        Ok(url) => url,
        Err(_) => return input.to_string(),
    };

    if !is_youtube_host(&url) {
        return input.to_string();
    }

    match path_segments(&url).as_slice() {
        ["channel", id, ..] => id.to_string(),
        ["c", name, ..] => name.to_string(),
        ["user", name, ..] => name.to_string(),
        [first, ..] => first.to_string(),
        [] => input.to_string(),
    }
}

/// Extracts a video id out of a pasted YouTube video URL (a `watch` link,
/// a `youtu.be` short link, or a `shorts`/`embed`/`live` link). Anything
/// that isn't one of those is returned unchanged, which includes a bare
/// video id.
fn parse_video_arg(input: &str) -> String {
    let url = match url::Url::parse(input) {
        Ok(url) => url,
        Err(_) => return input.to_string(),
    };

    if url.host_str() == Some("youtu.be") {
        if let Some(id) = path_segments(&url).first() {
            return id.to_string();
        }
        return input.to_string();
    }

    if !is_youtube_host(&url) {
        return input.to_string();
    }

    if let Some((_, id)) = url.query_pairs().find(|(key, _)| key == "v") {
        return id.into_owned();
    }

    match path_segments(&url).as_slice() {
        ["shorts", id, ..] => id.to_string(),
        ["embed", id, ..] => id.to_string(),
        ["live", id, ..] => id.to_string(),
        _ => input.to_string(),
    }
}

fn is_youtube_host(url: &url::Url) -> bool {
    match url.host_str() {
        Some(host) => host == "youtube.com" || host.ends_with(".youtube.com"),
        None => false,
    }
}

fn path_segments(url: &url::Url) -> Vec<&str> {
    url.path_segments()
        .map(|segments| segments.filter(|segment| !segment.is_empty()).collect())
        .unwrap_or_default()
}

fn print_error(message: &str) {
    write!(&mut std::io::stderr(), "{}\n", message).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_channel_arg_leaves_bare_id_untouched() {
        assert_eq!(parse_channel_arg("UC-lHJZR3Gqxm24_Vd_AJ5Yw"), "UC-lHJZR3Gqxm24_Vd_AJ5Yw");
    }

    #[test]
    fn parse_channel_arg_leaves_bare_handle_untouched() {
        assert_eq!(parse_channel_arg("@PewDiePie"), "@PewDiePie");
    }

    #[test]
    fn parse_channel_arg_extracts_from_channel_url() {
        assert_eq!(
            parse_channel_arg("https://www.youtube.com/channel/UC-lHJZR3Gqxm24_Vd_AJ5Yw"),
            "UC-lHJZR3Gqxm24_Vd_AJ5Yw"
        );
    }

    #[test]
    fn parse_channel_arg_extracts_from_channel_url_with_tab() {
        assert_eq!(
            parse_channel_arg("https://www.youtube.com/channel/UC-lHJZR3Gqxm24_Vd_AJ5Yw/videos"),
            "UC-lHJZR3Gqxm24_Vd_AJ5Yw"
        );
    }

    #[test]
    fn parse_channel_arg_extracts_from_handle_url() {
        assert_eq!(
            parse_channel_arg("https://www.youtube.com/@PewDiePie/videos"),
            "@PewDiePie"
        );
    }

    #[test]
    fn parse_channel_arg_extracts_from_legacy_user_url() {
        assert_eq!(
            parse_channel_arg("https://www.youtube.com/user/pewdiepie"),
            "pewdiepie"
        );
    }

    #[test]
    fn parse_channel_arg_extracts_from_legacy_custom_url() {
        assert_eq!(
            parse_channel_arg("https://www.youtube.com/c/PewDiePie"),
            "PewDiePie"
        );
    }

    #[test]
    fn parse_channel_arg_extracts_from_bare_custom_url() {
        assert_eq!(
            parse_channel_arg("https://www.youtube.com/pewdiepie"),
            "pewdiepie"
        );
    }

    #[test]
    fn parse_channel_arg_ignores_non_youtube_url() {
        assert_eq!(
            parse_channel_arg("https://example.com/channel/UC-lHJZR3Gqxm24_Vd_AJ5Yw"),
            "https://example.com/channel/UC-lHJZR3Gqxm24_Vd_AJ5Yw"
        );
    }

    #[test]
    fn parse_video_arg_leaves_bare_id_untouched() {
        assert_eq!(parse_video_arg("cNYSOHWbkJU"), "cNYSOHWbkJU");
    }

    #[test]
    fn parse_video_arg_extracts_from_watch_url() {
        assert_eq!(
            parse_video_arg("https://www.youtube.com/watch?v=cNYSOHWbkJU"),
            "cNYSOHWbkJU"
        );
    }

    #[test]
    fn parse_video_arg_extracts_from_watch_url_with_extra_params() {
        assert_eq!(
            parse_video_arg("https://www.youtube.com/watch?v=cNYSOHWbkJU&t=42s&list=PL123"),
            "cNYSOHWbkJU"
        );
    }

    #[test]
    fn parse_video_arg_extracts_from_short_url() {
        assert_eq!(
            parse_video_arg("https://youtu.be/cNYSOHWbkJU"),
            "cNYSOHWbkJU"
        );
    }

    #[test]
    fn parse_video_arg_extracts_from_short_url_with_query() {
        assert_eq!(
            parse_video_arg("https://youtu.be/cNYSOHWbkJU?si=abc123"),
            "cNYSOHWbkJU"
        );
    }

    #[test]
    fn parse_video_arg_extracts_from_shorts_url() {
        assert_eq!(
            parse_video_arg("https://www.youtube.com/shorts/cNYSOHWbkJU"),
            "cNYSOHWbkJU"
        );
    }

    #[test]
    fn parse_video_arg_extracts_from_embed_url() {
        assert_eq!(
            parse_video_arg("https://www.youtube.com/embed/cNYSOHWbkJU"),
            "cNYSOHWbkJU"
        );
    }

    #[test]
    fn parse_video_arg_ignores_non_youtube_url() {
        assert_eq!(
            parse_video_arg("https://example.com/watch?v=cNYSOHWbkJU"),
            "https://example.com/watch?v=cNYSOHWbkJU"
        );
    }

    #[test]
    fn parse_video_arg_falls_back_when_no_v_param() {
        assert_eq!(
            parse_video_arg("https://www.youtube.com/watch?list=PL123"),
            "https://www.youtube.com/watch?list=PL123"
        );
    }
}
