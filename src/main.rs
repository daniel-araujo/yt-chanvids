extern crate getopts;
extern crate hyper;
extern crate hyper_tls;
extern crate tokio_core;
extern crate futures;
extern crate serde;
extern crate serde_json;
extern crate serde_qs;
#[macro_use]
extern crate serde_derive;

use std::env;
use std::process::exit;
use std::io::Write;
use std::str::FromStr;

use getopts::Options;

use futures::Future;
use futures::Stream;

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
            Err(err) => match err {
                FetchLinksError::RequestError(err) => match err {
                    RequestError::NotFound => {
                        print_error("Channel does not seem to be reachable.");
                        exit(1);
                    },
                    RequestError::ParseJsonDataError(ParseJsonDataError::Html(html)) => {
                        let message = "A JSON request returned html:\n".to_string() + &html;
                        print_error(&message);
                        exit(1);
                    },
                    _ => {
                        panic!(format!("{:?}", err))
                    },
                },
                FetchLinksError::MissingUploadsPage => {
                    print_error("This channel does not have an Uploads page.");
                    exit(1);
                },
            },
        }
    }

    fn print_error(message: &str) {
        write!(&mut std::io::stderr(), "{}\n", message).unwrap();
    };
}

fn youtube_video_links(channel: &str) -> Result<Vec<String>, FetchLinksError> {
    let mut links: Vec<String> = Vec::new();

    // HTTP client setup.
    let mut core = tokio_core::reactor::Core::new().unwrap();
    let http_client = hyper::Client::configure()
        .connector(hyper_tls::HttpsConnector::new(4, &core.handle()).unwrap())
        .build(&core.handle());

    {
        let mut items;
        let mut next_continuation;

        let mut collect_links = |items: &serde_json::Value| {
            for item in items.as_array().unwrap().iter() {
                let video_id = item["gridVideoRenderer"]["videoId"].as_str().unwrap();
                links.push(make_youtube_video_url(video_id));
            }
        };

        let data = uploads_page_data(&mut core, &http_client, channel)?;

        let section_list_renderer = &data[1]["response"]["contents"]["twoColumnBrowseResultsRenderer"]["tabs"][1]["tabRenderer"]["content"]["sectionListRenderer"];
        let item_section_renderer = &section_list_renderer["contents"][0]["itemSectionRenderer"];
        let grid_renderer = &item_section_renderer["contents"][0]["gridRenderer"];

        items = grid_renderer["items"].clone();
        next_continuation = grid_renderer["continuations"][0]["nextContinuationData"].clone();

        if !items.is_array() {
            return Err(FetchLinksError::MissingUploadsPage);
        }

        {
            let mut cycle = |items: &serde_json::Value, next_continuation: &serde_json::Value|
                    -> Result<(serde_json::Value, serde_json::Value), RequestError> {
                let request = request_browse(
                    next_continuation["continuation"].as_str().unwrap(),
                    next_continuation["clickTrackingParams"].as_str().unwrap());

                let work = http_client.request(request);

                collect_links(items);

                let response = core.run(work).unwrap();

                if let hyper::StatusCode::Ok = response.status() {
                    let data = parse_json_data(&hyper_response_body_as_string(&mut core, response)?)?;
                    let continuation_contents = &data[1]["response"]["continuationContents"];
                    let grid_continuation = &continuation_contents["gridContinuation"];

                    let items = grid_continuation["items"].clone();
                    let next_continuation = grid_continuation["continuations"][0]["nextContinuationData"].clone();

                    Ok((items, next_continuation))
                } else {
                    Ok((items.clone(), serde_json::Value::default()))
                }
            };

            while next_continuation.is_object() {
                let result = cycle(&items, &next_continuation)?;

                items = result.0;
                next_continuation = result.1;
            }
        }

        collect_links(&items);
    }

    Ok(links)
}

fn uploads_page_data(
        core: &mut tokio_core::reactor::Core,
        http_client: &hyper::Client<hyper_tls::HttpsConnector<hyper::client::HttpConnector>>,
        id: &str) -> Result<serde_json::Value, RequestError> {
    let candidates = [request_channel_uploads, request_user_uploads];

    for worker in candidates.iter() {
        let request = worker(id);

        let response = core.run(http_client.request(request)).unwrap();

        if let hyper::StatusCode::Ok = response.status() {
            let data = hyper_response_body_as_string(core, response)?;

            return Ok(parse_json_data(&data)?)
        }
    }

    Err(RequestError::NotFound)
}

fn request_channel_uploads(channel: &str) -> hyper::Request {
    let url = hyper::Uri::from_str(
        &(String::from("https://www.youtube.com/channel/")
            + channel
            + "/videos?view=0&flow=grid&pbj=1"))
        .unwrap();

    let mut request = hyper::Request::new(hyper::Method::Get, url);
    request.headers_mut().set_raw("x-youtube-client-name", "1");
    request.headers_mut().set_raw("x-youtube-client-version", "2.20170927");

    request
}

fn request_user_uploads(user: &str) -> hyper::Request {
    let url = hyper::Uri::from_str(
        &(String::from("https://www.youtube.com/user/")
            + user
            + "/videos?view=0&flow=grid&pbj=1"))
        .unwrap();

    let mut request = hyper::Request::new(hyper::Method::Get, url);
    request.headers_mut().set_raw("x-youtube-client-name", "1");
    request.headers_mut().set_raw("x-youtube-client-version", "2.20170927");

    request
}

fn request_browse(ctoken: &str, itct: &str) -> hyper::Request {
    #[derive(Deserialize, Serialize)]
    struct Query {
        ctoken: String,
        itct: String,
    }

    let query = Query {
        ctoken: ctoken.to_owned(),
        itct: itct.to_owned(),
    };
    let url = hyper::Uri::from_str(
        &(String::from("https://www.youtube.com/browse_ajax?")
            + &serde_qs::to_string(&query).unwrap()))
        .unwrap();

    let mut request = hyper::Request::new(hyper::Method::Get, url);
    request.headers_mut().set_raw("x-youtube-client-name", "1");
    request.headers_mut().set_raw("x-youtube-client-version", "2.20170927");

    request
}

fn hyper_response_body_as_string(
        core: &mut tokio_core::reactor::Core,
        response: hyper::Response) -> Result<String, hyper::Error> {
    let work = response.body().concat2()
        .and_then(|chunk| {
            Ok(String::from_utf8(chunk.to_vec()).unwrap())
        });

    core.run(work)
}

fn parse_json_data(string: &str) -> Result<serde_json::Value, ParseJsonDataError> {
    let s = String::from(string.trim_left());

    if s.starts_with("<!") {
        return Err(ParseJsonDataError::Html(s));
    }

    serde_json::from_str(string).map_err(|err| ParseJsonDataError::from(err))
}

fn make_youtube_video_url(id: &str) -> String {
    String::from("https://www.youtube.com/watch?v=") + id
}

#[allow(dead_code)]
fn canonicalize_video_url(url: &str) -> String {
    if url.starts_with("/") {
        return String::from("https://www.youtube.com") + url;
    }

    String::from(url)
}

#[derive(Debug)]
enum FetchLinksError {
    RequestError(RequestError),
    MissingUploadsPage,
}

impl From<RequestError> for FetchLinksError {
    fn from(err: RequestError) -> FetchLinksError {
        FetchLinksError::RequestError(err)
    }
}

#[derive(Debug)]
enum RequestError {
    HyperError(hyper::Error),
    HyperUriError(hyper::error::UriError),
    Io(std::io::Error),
    ParseJsonDataError(ParseJsonDataError),
    NotFound,
}

impl From<hyper::Error> for RequestError {
    fn from(err: hyper::Error) -> RequestError {
        RequestError::HyperError(err)
    }
}

impl From<ParseJsonDataError> for RequestError {
    fn from(err: ParseJsonDataError) -> RequestError {
        RequestError::ParseJsonDataError(err)
    }
}

impl From<std::io::Error> for RequestError {
    fn from(err: std::io::Error) -> RequestError {
        RequestError::Io(err)
    }
}

impl From<hyper::error::UriError> for RequestError {
    fn from(err: hyper::error::UriError) -> RequestError {
        RequestError::HyperUriError(err)
    }
}

#[derive(Debug)]
enum ParseJsonDataError {
    Io(std::io::Error),
    Parse(serde_json::Error),
    Html(String),
}

impl From<std::io::Error> for ParseJsonDataError {
    fn from(err: std::io::Error) -> ParseJsonDataError {
        ParseJsonDataError::Io(err)
    }
}

impl From<serde_json::Error> for ParseJsonDataError {
    fn from(err: serde_json::Error) -> ParseJsonDataError {
        ParseJsonDataError::Parse(err)
    }
}
