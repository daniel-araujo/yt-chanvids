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
            let mut crawler = YtUploadsCrawler::channel(channel);

            while let Some(link) = crawler.next() {
                println!("{}", link);
            }

            if let &Some(ref error) = crawler.error() {
                match error {
                    &FetchLinksError::RequestError(ref err) => match err {
                        &RequestError::NotFound => {
                            print_error("Channel does not seem to be reachable.");
                            exit(1);
                        },
                        &RequestError::ParseJsonDataError(ParseJsonDataError::Html(ref html)) => {
                            let message = "A JSON request returned html:\n".to_string() + &html;
                            print_error(&message);
                            exit(1);
                        },
                        _ => {
                            panic!(format!("{:?}", err))
                        },
                    },
                    &FetchLinksError::MissingUploadsPage => {
                        print_error("This channel does not have an Uploads page.");
                        exit(1);
                    },
                }
            }

            exit(0);
        },
        _ => {
            print_usage_mistake("Unexpected number of arguments.");
            exit(1);
        }
    }

    fn print_error(message: &str) {
        write!(&mut std::io::stderr(), "{}\n", message).unwrap();
    };
}

struct YtUploadsCrawler {
    channel: String,

    links: Vec<String>,

    started: bool,

    next_continuation: serde_json::Value,

    error: Option<FetchLinksError>,

    tokio_core: tokio_core::reactor::Core,

    http_client: hyper::Client<hyper_tls::HttpsConnector<hyper::client::HttpConnector>>,
}

impl YtUploadsCrawler {
    /*
     * Creates a crawler for the given channel.
     */
    fn channel(channel: &str) -> YtUploadsCrawler {
        // HTTP client setup.
        let tokio_core = tokio_core::reactor::Core::new().unwrap();
        let http_client = hyper::Client::configure()
            .connector(hyper_tls::HttpsConnector::new(4, &tokio_core.handle()).unwrap())
            .build(&tokio_core.handle());

        YtUploadsCrawler {
            channel: String::from(channel),
            links: Vec::new(),
            started: false,
            next_continuation: serde_json::Value::default(),
            error: None,
            tokio_core: tokio_core,
            http_client: http_client,
        }
    }

    /*
     * Indicates whether an error occurred.
     */
    fn error(&self) -> &Option<FetchLinksError> {
        &self.error
    }

    /*
     * Advances to the next link and returns it.
     */
    fn next(&mut self) -> Option<String> {
        if !self.started {
            self.start();
            self.started = true;
        }

        if self.links.is_empty() {
            if self.next_continuation.is_object() {
                self.follow_continuation();
            }
        }

        self.links.pop()
    }

    /*
     * Gets the first batch of links.
     */
    fn start(&mut self) {
        match self.uploads_page_data() {
            Ok(data) => {
                let section_list_renderer = &data[1]["response"]["contents"]["twoColumnBrowseResultsRenderer"]["tabs"][1]["tabRenderer"]["content"]["sectionListRenderer"];
                let item_section_renderer = &section_list_renderer["contents"][0]["itemSectionRenderer"];
                let grid_renderer = &item_section_renderer["contents"][0]["gridRenderer"];

                let items = &grid_renderer["items"];
                let next_continuation = &grid_renderer["continuations"][0]["nextContinuationData"];

                if !items.is_array() {
                    self.error = Some(FetchLinksError::MissingUploadsPage);
                    return;
                }

                self.collect_links(&items);
                self.next_continuation = next_continuation.clone();
            },
            Err(error) => {
                self.error = Some(FetchLinksError::from(error));
            },
        }
    }

    /*
     * Follows the continuation to get more links.
     */
    fn follow_continuation(&mut self) {
        match self.uploads_continuation_data() {
            Ok(data) => {
                let continuation_contents = &data[1]["response"]["continuationContents"];
                let grid_continuation = &continuation_contents["gridContinuation"];

                let items = &grid_continuation["items"];
                let next_continuation = &grid_continuation["continuations"][0]["nextContinuationData"];

                self.collect_links(&items);
                self.next_continuation = next_continuation.clone();
            },
            Err(error) => {
                self.error = Some(FetchLinksError::from(error));
            },
        }
    }

    /*
     * Collects links from a response from YouTube.
     */
    fn collect_links(&mut self, items: &serde_json::Value) {
        for item in items.as_array().unwrap().iter() {
            let video_id = item["gridVideoRenderer"]["videoId"].as_str().unwrap();
            self.links.push(make_youtube_video_url(video_id));
        }
    }

    /*
     * Returns YouTube's response for the uploads page.
     */
    fn uploads_page_data(&mut self) -> Result<serde_json::Value, RequestError> {
        let candidates = [request_channel_uploads, request_user_uploads];

        for worker in candidates.iter() {
            let request = worker(&self.channel);

            let response = self.tokio_core.run(self.http_client.request(request)).unwrap();

            if let hyper::StatusCode::Ok = response.status() {
                let data = hyper_response_body_as_string(&mut self.tokio_core, response)?;

                return Ok(parse_json_data(&data)?)
            }
        }

        Err(RequestError::NotFound)
    }

    fn uploads_continuation_data(&mut self) -> Result<serde_json::Value, RequestError> {
        let request = request_browse(
            self.next_continuation["continuation"].as_str().unwrap(),
            self.next_continuation["clickTrackingParams"].as_str().unwrap());

        let work = self.http_client.request(request);

        let response = self.tokio_core.run(work).unwrap();

        if let hyper::StatusCode::Ok = response.status() {
            let data = hyper_response_body_as_string(&mut self.tokio_core, response)?;

            return Ok(parse_json_data(&data)?)
        } else {
            Err(RequestError::NotFound)
        }
    }
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
