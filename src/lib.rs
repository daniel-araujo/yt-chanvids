pub struct ChannelDetail {
    pub title: String,
    pub author_thumbnail: String,
    pub description: String,
}

pub struct YtChannelDetailScraper {
    id: String,
}

impl YtChannelDetailScraper {
    pub fn from_id(id: &str) -> YtChannelDetailScraper {
        YtChannelDetailScraper { id: id.into() }
    }

    pub fn get(&self) -> ChannelDetail {
        let url_chan = format!(
            "https://youtube.com/channel/{}/about?flow=grid&view=0&pbj=1",
            self.id
        );
        let url_user = format!(
            "https://youtube.com/user/{}/about?flow=grid&view=0&pbj=1",
            self.id
        );
        let mut response = None;
        for url in vec![url_chan, url_user] {
            let res = attohttpc::get(&url)
                .header("x-youtube-client-name", "1")
                .header("x-youtube-client-version", "2.20170927")
                .send()
                .unwrap();
            if res.is_success() {
                response = Some(res);
                break;
            }
        }

        let text = response.unwrap().text().unwrap();
        let data: serde_json::Value = serde_json::from_str(&text).unwrap();

        fn dec(x: &serde_json::Value) -> String {
            x.as_str().unwrap().into()
        }

        let title = dec(&data[1]["response"]["metadata"]["channelMetadataRenderer"]["title"]);
        let desc = dec(&data[1]["response"]["metadata"]["channelMetadataRenderer"]["description"]);
        let thumb = dec(
            &data[1]["response"]["metadata"]["channelMetadataRenderer"]["avatar"]["thumbnails"][0]
                ["url"],
        );
        ChannelDetail {
            author_thumbnail: thumb,
            title: title,
            description: desc,
        }
    }
}

pub struct VideoDetail {
    pub title: String,
    pub description: String,
    pub duration_seconds: i32,
    pub publish_date: String,
}

pub struct YtVideoDetailScraper {
    id: String,
}

impl YtVideoDetailScraper {
    pub fn from_id(id: &str) -> YtVideoDetailScraper {
        YtVideoDetailScraper { id: id.into() }
    }

    pub fn get(&self) -> VideoDetail {
        let url = format!(
            "https://www.youtube.com/get_video_info?video_id={}",
            self.id
        );
        let response = attohttpc::get(url).send().unwrap();
        let text = response.text().unwrap();
        for chunk in text.split("&") {
            if chunk.starts_with("player_response=") {
                let clean = chunk.replace("player_response=", "");
                let json_src = &urlencoding::decode(&clean).unwrap().replace("+", " ");

                let j: serde_json::Value = serde_json::from_str(&json_src).unwrap();
                fn dec(x: &serde_json::Value) -> String {
                    urlencoding::decode(x.as_str().unwrap()).unwrap()
                }
                return VideoDetail {
                    title: dec(&j["videoDetails"]["title"]),
                    description: dec(&j["videoDetails"]["shortDescription"]),
                    duration_seconds: dec(&j["videoDetails"]["lengthSeconds"])
                        .parse::<i32>()
                        .unwrap(),
                    publish_date: dec(&j["microformat"]["playerMicroformatRenderer"]["uploadDate"]),
                };
            }
        }
        unreachable!();
    }
}

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
}

impl YtUploadsCrawler {
    /*
     * Creates a crawler for the given channel.
     */
    pub fn channel(channel: &str) -> YtUploadsCrawler {
        YtUploadsCrawler {
            channel: String::from(channel),
            links: Vec::new(),
            started: false,
            next_continuation: serde_json::Value::default(),
            error: None,
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
        for item in items.as_array().unwrap().iter().rev() {
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
            let response = worker(&self.channel)?;
            if response.is_success() {
                let data = response.text()?;
                return Ok(parse_json_data(&data)?);
            }
        }

        Err(RequestError::NotFound)
    }

    fn uploads_continuation_data(&mut self) -> Result<serde_json::Value, RequestError> {
        let response = request_browse(
            self.next_continuation["continuation"].as_str().unwrap(),
            self.next_continuation["clickTrackingParams"]
                .as_str()
                .unwrap(),
        )?;

        if response.is_success() {
            let data = response.text()?;
            return Ok(parse_json_data(&data)?);
        } else {
            Err(RequestError::NotFound)
        }
    }
}

fn request_channel_uploads(channel: &str) -> Result<attohttpc::Response, attohttpc::Error> {
    let url = format!(
        "https://www.youtube.com/channel/{}/videos?view=0&flow=grid&pbj=1&sort=dd",
        channel
    );

    attohttpc::get(url)
        .header("x-youtube-client-name", "1")
        .header("x-youtube-client-version", "2.20170927")
        .send()
}

fn request_user_uploads(user: &str) -> Result<attohttpc::Response, attohttpc::Error> {
    let url = format!(
        "https://www.youtube.com/user/{}/videos?view=0&flow=grid&pbj=1&sort=dd",
        user
    );

    attohttpc::get(url)
        .header("x-youtube-client-name", "1")
        .header("x-youtube-client-version", "2.20170927")
        .send()
}

fn request_browse(ctoken: &str, itct: &str) -> Result<attohttpc::Response, attohttpc::Error> {
    let url = format!(
        "https://www.youtube.com/browse_ajax?ctoken={}&itct={}",
        urlencoding::encode(ctoken),
        urlencoding::encode(itct)
    );

    let request = attohttpc::get(url)
        .header("x-youtube-client-name", "1")
        .header("x-youtube-client-version", "2.20170927")
        .send();

    request
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
    HTTPError(attohttpc::Error),
    Io(std::io::Error),
    ParseJsonDataError(ParseJsonDataError),
    NotFound,
}

impl From<attohttpc::Error> for RequestError {
    fn from(err: attohttpc::Error) -> RequestError {
        RequestError::HTTPError(err)
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
