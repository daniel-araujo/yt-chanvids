use std::str::FromStr;

use serde_derive::{Deserialize, Serialize};

pub struct VideoInfo {
    pub id: String,
    pub url: String,
    pub title: String,
    pub thumbnail: String,
}

pub struct YtUploadsCrawler {
    channel: String,

    links: Vec<VideoInfo>,

    started: bool,

    next_continuation: serde_json::Value,

    error: Option<FetchLinksError>,

    tokio_runtime: tokio::runtime::Runtime,

    http_client: hyper::Client<hyper_tls::HttpsConnector<hyper::client::HttpConnector>>,
}

impl YtUploadsCrawler {
    /*
     * Creates a crawler for the given channel.
     */
    pub fn channel(channel: &str) -> YtUploadsCrawler {
        // HTTP client setup.
        let tokio_runtime = tokio::runtime::Runtime::new().unwrap();
        let https_connector = hyper_tls::HttpsConnector::new();
        let http_client = hyper::Client::builder().build(https_connector);

        YtUploadsCrawler {
            channel: String::from(channel),
            links: Vec::new(),
            started: false,
            next_continuation: serde_json::Value::default(),
            error: None,
            tokio_runtime: tokio_runtime,
            http_client: http_client,
        }
    }

    /*
     * Indicates whether an error occurred.
     */
    pub fn error(&self) -> &Option<FetchLinksError> {
        &self.error
    }

    /*
     * Advances to the next link and returns it.
     */
    pub fn next(&mut self) -> Option<VideoInfo> {
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
                let section_list_renderer = &data[1]["response"]["contents"]
                    ["twoColumnBrowseResultsRenderer"]["tabs"][1]["tabRenderer"]["content"]
                    ["sectionListRenderer"];
                let item_section_renderer =
                    &section_list_renderer["contents"][0]["itemSectionRenderer"];
                let grid_renderer = &item_section_renderer["contents"][0]["gridRenderer"];

                let items = &grid_renderer["items"];
                let next_continuation = &grid_renderer["continuations"][0]["nextContinuationData"];

                if !items.is_array() {
                    self.error = Some(FetchLinksError::MissingUploadsPage);
                    return;
                }

                self.collect_links(&items);
                self.next_continuation = next_continuation.clone();
            }
            Err(error) => {
                self.error = Some(FetchLinksError::from(error));
            }
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
                let next_continuation =
                    &grid_continuation["continuations"][0]["nextContinuationData"];

                self.collect_links(&items);
                self.next_continuation = next_continuation.clone();
            }
            Err(error) => {
                self.error = Some(FetchLinksError::from(error));
            }
        }
    }

    /*
     * Collects links from a response from YouTube.
     */
    fn collect_links(&mut self, items: &serde_json::Value) {
        for item in items.as_array().unwrap().iter() {
            // dbg!(item);
            let video_id = item["gridVideoRenderer"]["videoId"].as_str().unwrap();
            let info = VideoInfo {
                id: video_id.into(),
                url: make_youtube_video_url(video_id),
                title: item["gridVideoRenderer"]["title"]["runs"][0]["text"]
                    .as_str()
                    .unwrap_or(
                        item["gridVideoRenderer"]["title"]["simpleText"]
                            .as_str()
                            .unwrap_or(""),
                    )
                    .into(),
                thumbnail: item["gridVideoRenderer"]["thumbnail"]["thumbnails"][0]["url"]
                    .as_str()
                    .unwrap_or("")
                    .into(),
            };
            self.links.push(info);
        }
    }

    /*
     * Returns YouTube's response for the uploads page.
     */
    fn uploads_page_data(&mut self) -> Result<serde_json::Value, RequestError> {
        let candidates = [request_channel_uploads, request_user_uploads];

        for worker in candidates.iter() {
            let request = worker(&self.channel);

            let mut response = self
                .tokio_runtime
                .block_on(self.http_client.request(request))
                .unwrap();

            if let hyper::StatusCode::OK = response.status() {
                let data = hyper_response_body_as_string(&mut self.tokio_runtime, &mut response)?;

                return Ok(parse_json_data(&data)?);
            }
        }

        Err(RequestError::NotFound)
    }

    fn uploads_continuation_data(&mut self) -> Result<serde_json::Value, RequestError> {
        let request = request_browse(
            self.next_continuation["continuation"].as_str().unwrap(),
            self.next_continuation["clickTrackingParams"]
                .as_str()
                .unwrap(),
        );

        let mut response = self
            .tokio_runtime
            .block_on(self.http_client.request(request))
            .unwrap();

        if let hyper::StatusCode::OK = response.status() {
            let data = hyper_response_body_as_string(&mut self.tokio_runtime, &mut response)?;

            return Ok(parse_json_data(&data)?);
        } else {
            Err(RequestError::NotFound)
        }
    }
}

fn request_channel_uploads(channel: &str) -> hyper::Request<hyper::Body> {
    let url = hyper::Uri::from_str(
        &(String::from("https://www.youtube.com/channel/")
            + channel
            + "/videos?view=0&flow=grid&pbj=1"),
    )
    .unwrap();

    let request = hyper::Request::builder()
        .method("GET")
        .uri(url)
        .header("x-youtube-client-name", "1")
        .header("x-youtube-client-version", "2.20170927")
        .body(hyper::Body::empty())
        .unwrap();

    request
}

fn request_user_uploads(user: &str) -> hyper::Request<hyper::Body> {
    let url = hyper::Uri::from_str(
        &(String::from("https://www.youtube.com/user/") + user + "/videos?view=0&flow=grid&pbj=1"),
    )
    .unwrap();

    let request = hyper::Request::builder()
        .method("GET")
        .uri(url)
        .header("x-youtube-client-name", "1")
        .header("x-youtube-client-version", "2.20170927")
        .body(hyper::Body::empty())
        .unwrap();

    request
}

fn request_browse(ctoken: &str, itct: &str) -> hyper::Request<hyper::Body> {
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
            + &serde_qs::to_string(&query).unwrap()),
    )
    .unwrap();

    let request = hyper::Request::builder()
        .method("GET")
        .uri(url)
        .header("x-youtube-client-name", "1")
        .header("x-youtube-client-version", "2.20170927")
        .body(hyper::Body::empty())
        .unwrap();

    request
}

fn hyper_response_body_as_string(
    tokio_runtime: &mut tokio::runtime::Runtime,
    response: &mut hyper::Response<hyper::Body>,
) -> Result<String, hyper::Error> {
    let bytes = tokio_runtime
        .block_on(hyper::body::to_bytes(response.body_mut()))
        .unwrap();

    Ok(String::from_utf8(bytes.to_vec()).unwrap())
}

fn parse_json_data(string: &str) -> Result<serde_json::Value, ParseJsonDataError> {
    let s = String::from(string.trim_start());

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
pub enum FetchLinksError {
    RequestError(RequestError),
    MissingUploadsPage,
}

impl From<RequestError> for FetchLinksError {
    fn from(err: RequestError) -> FetchLinksError {
        FetchLinksError::RequestError(err)
    }
}

#[derive(Debug)]
pub enum RequestError {
    HyperError(hyper::Error),
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

#[derive(Debug)]
pub enum ParseJsonDataError {
    Io(std::io::Error),
    /// JSON parse error
    Parse(serde_json::Error),
    /// Unexpected HTML response from URL which should respond with JSON data
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
