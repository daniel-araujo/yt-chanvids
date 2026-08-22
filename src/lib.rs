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
                    urlencoding::decode(x.as_str().unwrap()).unwrap().into_owned()
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

/// A cookie that skips YouTube's cookie consent interstitial, which would
/// otherwise redirect requests from EU-region IPs to consent.youtube.com
/// instead of returning the channel page.
const CONSENT_COOKIE: &str = "CONSENT=YES+1; SOCS=CAI";

pub struct YtUploadsCrawler {
    channel: String,
    links: Vec<VideoInfo>,
    started: bool,
    api_key: String,
    client_version: String,
    next_continuation: Option<String>,
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
            api_key: String::new(),
            client_version: String::new(),
            next_continuation: None,
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
            if self.next_continuation.is_some() {
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
            Ok(page) => {
                let items = find_videos_tab_content(&page.data)
                    .and_then(|content| content["richGridRenderer"]["contents"].as_array());

                match items {
                    Some(items) => {
                        let grid = parse_grid_items(items);
                        self.collect_links(grid.items);
                        self.next_continuation = grid.next_continuation;
                        self.api_key = page.api_key;
                        self.client_version = page.client_version;
                    }
                    None => {
                        self.error = Some(FetchLinksError::MissingUploadsPage);
                    }
                }
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
                let items = data["onResponseReceivedActions"][0]["appendContinuationItemsAction"]
                    ["continuationItems"]
                    .as_array();

                match items {
                    Some(items) => {
                        let grid = parse_grid_items(items);
                        self.collect_links(grid.items);
                        self.next_continuation = grid.next_continuation;
                    }
                    None => {
                        self.next_continuation = None;
                    }
                }
            }
            Err(error) => {
                self.error = Some(FetchLinksError::from(error));
            }
        }
    }

    /*
     * Collects links from a response from YouTube.
     */
    fn collect_links(&mut self, items: Vec<VideoInfo>) {
        for info in items.into_iter().rev() {
            self.links.push(info);
        }
    }

    /*
     * Returns YouTube's response for the uploads page.
     */
    fn uploads_page_data(&mut self) -> Result<PageData, RequestError> {
        let candidates: [fn(&str) -> Result<PageData, RequestError>; 2] =
            [request_channel_uploads, request_user_uploads];

        for worker in candidates.iter() {
            match worker(&self.channel) {
                Ok(page) => return Ok(page),
                Err(RequestError::NotFound) => continue,
                Err(error) => return Err(error),
            }
        }

        Err(RequestError::NotFound)
    }

    fn uploads_continuation_data(&mut self) -> Result<serde_json::Value, RequestError> {
        let token = match &self.next_continuation {
            Some(token) => token.clone(),
            None => return Err(RequestError::NotFound),
        };

        let response = request_browse(&token, &self.api_key, &self.client_version)?;

        if !response.is_success() {
            return Err(RequestError::NotFound);
        }

        let text = response.text()?;
        serde_json::from_str(&text).map_err(|err| RequestError::from(ParseJsonDataError::from(err)))
    }
}

/// The channel "Videos" tab content, plus the InnerTube API key and client
/// version scraped from the same page (needed to fetch further pages).
struct PageData {
    data: serde_json::Value,
    api_key: String,
    client_version: String,
}

/// A batch of videos parsed out of a grid (either the initial page load or a
/// continuation response), plus the continuation token for the next batch,
/// if there is one.
struct GridPage {
    items: Vec<VideoInfo>,
    next_continuation: Option<String>,
}

/// Finds the content of whichever tab is currently selected. Requesting a
/// channel's "/videos" URL makes YouTube pre-select the Videos tab.
fn find_videos_tab_content(data: &serde_json::Value) -> Option<&serde_json::Value> {
    let tabs = data["contents"]["twoColumnBrowseResultsRenderer"]["tabs"].as_array()?;

    tabs.iter()
        .find(|tab| tab["tabRenderer"]["selected"].as_bool() == Some(true))
        .map(|tab| &tab["tabRenderer"]["content"])
}

/// Parses the videos and next continuation token out of a grid's list of
/// items. Items are either a video (`richItemRenderer`) or, at the end of
/// the list, the token needed to fetch the next batch
/// (`continuationItemRenderer`).
fn parse_grid_items(items: &[serde_json::Value]) -> GridPage {
    let mut videos = Vec::new();
    let mut next_continuation = None;

    for item in items {
        let lockup = &item["richItemRenderer"]["content"]["lockupViewModel"];

        if let Some(video_id) = lockup["contentId"].as_str() {
            let metadata = &lockup["metadata"]["lockupMetadataViewModel"];
            let title = metadata["title"]["content"].as_str().unwrap_or("").into();
            let thumbnail = lockup["contentImage"]["thumbnailViewModel"]["image"]["sources"][0]
                ["url"]
                .as_str()
                .unwrap_or("")
                .into();

            videos.push(VideoInfo {
                id: video_id.into(),
                url: make_youtube_video_url(video_id),
                title,
                thumbnail,
            });
        } else if let Some(token) = item["continuationItemRenderer"]["continuationEndpoint"]
            ["continuationCommand"]["token"]
            .as_str()
        {
            next_continuation = Some(token.to_string());
        }
    }

    GridPage {
        items: videos,
        next_continuation,
    }
}

fn request_channel_uploads(channel: &str) -> Result<PageData, RequestError> {
    let url = format!("https://www.youtube.com/channel/{}/videos", channel);
    fetch_channel_videos_page(&url)
}

fn request_user_uploads(user: &str) -> Result<PageData, RequestError> {
    let url = format!(
        "https://www.youtube.com/{}/videos",
        user.trim_start_matches('@')
    );
    fetch_channel_videos_page(&url)
}

/// Fetches a channel's "Videos" tab page and extracts the embedded
/// `ytInitialData`, along with the InnerTube API key and client version
/// used to fetch further pages of results.
fn fetch_channel_videos_page(url: &str) -> Result<PageData, RequestError> {
    let response = attohttpc::get(url).header("cookie", CONSENT_COOKIE).send()?;

    if !response.is_success() {
        return Err(RequestError::NotFound);
    }

    let html = response.text()?;

    let data = match extract_balanced_json(&html, "var ytInitialData = ") {
        Some(json_src) => {
            serde_json::from_str(json_src).map_err(|err| ParseJsonDataError::from(err))?
        }
        None => return Err(ParseJsonDataError::Html(html).into()),
    };

    let api_key = match extract_quoted_value(&html, "\"INNERTUBE_API_KEY\":\"") {
        Some(key) => key,
        None => return Err(ParseJsonDataError::Html(html).into()),
    };

    let client_version = match extract_quoted_value(&html, "\"INNERTUBE_CONTEXT_CLIENT_VERSION\":\"")
    {
        Some(version) => version,
        None => return Err(ParseJsonDataError::Html(html).into()),
    };

    Ok(PageData {
        data,
        api_key,
        client_version,
    })
}

fn request_browse(
    continuation: &str,
    api_key: &str,
    client_version: &str,
) -> Result<attohttpc::Response, attohttpc::Error> {
    let url = format!("https://www.youtube.com/youtubei/v1/browse?key={}", api_key);

    let body = serde_json::json!({
        "context": {
            "client": {
                "clientName": "WEB",
                "clientVersion": client_version,
            }
        },
        "continuation": continuation,
    });

    attohttpc::post(url)
        .header("cookie", CONSENT_COOKIE)
        .json(&body)?
        .send()
}

/// Finds `marker` in `html` and returns the JSON object that immediately
/// follows it, by counting braces (while skipping over quoted strings) until
/// the one that closes the object starting the search is found. Used to pull
/// `var ytInitialData = {...};` out of a page without a full JS parser.
fn extract_balanced_json<'a>(html: &'a str, marker: &str) -> Option<&'a str> {
    let after_marker = &html[html.find(marker)? + marker.len()..];
    let brace_start = after_marker.find('{')?;
    let bytes = after_marker.as_bytes();

    let mut depth = 0u32;
    let mut in_string = false;
    let mut escaped = false;

    for (i, &b) in bytes.iter().enumerate().skip(brace_start) {
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }

        match b {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&after_marker[brace_start..=i]);
                }
            }
            _ => {}
        }
    }

    None
}

/// Finds `marker` (expected to end right before an opening quote, e.g.
/// `"someKey":"`) and returns the following string value.
fn extract_quoted_value(html: &str, marker: &str) -> Option<String> {
    let after = &html[html.find(marker)? + marker.len()..];
    let end = after.find('"')?;
    Some(after[..end].to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_balanced_json_finds_object_after_marker() {
        let html = r#"<script>var ytInitialData = {"a":1,"b":{"c":"}not a brace{"}};</script>"#;

        let json = extract_balanced_json(html, "var ytInitialData = ").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(json).unwrap();

        assert_eq!(parsed["a"], 1);
        assert_eq!(parsed["b"]["c"], "}not a brace{");
    }

    #[test]
    fn extract_balanced_json_stops_at_matching_brace() {
        let html = r#"var ytInitialData = {"x":1}; var somethingElse = {"y":2};"#;

        let json = extract_balanced_json(html, "var ytInitialData = ").unwrap();

        assert_eq!(json, r#"{"x":1}"#);
    }

    #[test]
    fn extract_balanced_json_missing_marker_returns_none() {
        let html = "<script>nothing here</script>";

        assert!(extract_balanced_json(html, "var ytInitialData = ").is_none());
    }

    #[test]
    fn extract_quoted_value_finds_value() {
        let html = r#"blah,"INNERTUBE_API_KEY":"AIzaTest123","other":true"#;

        let value = extract_quoted_value(html, "\"INNERTUBE_API_KEY\":\"").unwrap();

        assert_eq!(value, "AIzaTest123");
    }

    #[test]
    fn extract_quoted_value_missing_marker_returns_none() {
        let html = "no key here";

        assert!(extract_quoted_value(html, "\"INNERTUBE_API_KEY\":\"").is_none());
    }

    #[test]
    fn find_videos_tab_content_returns_selected_tab() {
        let data = serde_json::json!({
            "contents": {
                "twoColumnBrowseResultsRenderer": {
                    "tabs": [
                        {"tabRenderer": {"title": "Home", "selected": false, "content": {"marker": "home"}}},
                        {"tabRenderer": {"title": "Videos", "selected": true, "content": {"marker": "videos"}}},
                        {"tabRenderer": {"title": "Playlists", "selected": false, "content": {"marker": "playlists"}}}
                    ]
                }
            }
        });

        let content = find_videos_tab_content(&data).unwrap();

        assert_eq!(content["marker"], "videos");
    }

    #[test]
    fn find_videos_tab_content_none_when_no_tab_selected() {
        let data = serde_json::json!({
            "contents": {
                "twoColumnBrowseResultsRenderer": {
                    "tabs": [
                        {"tabRenderer": {"title": "Home", "selected": false, "content": {}}}
                    ]
                }
            }
        });

        assert!(find_videos_tab_content(&data).is_none());
    }

    #[test]
    fn find_videos_tab_content_none_when_shape_unexpected() {
        let data = serde_json::json!({"contents": {}});

        assert!(find_videos_tab_content(&data).is_none());
    }

    fn lockup_item(video_id: &str, title: &str, thumbnail_url: &str) -> serde_json::Value {
        serde_json::json!({
            "richItemRenderer": {
                "content": {
                    "lockupViewModel": {
                        "contentId": video_id,
                        "metadata": {
                            "lockupMetadataViewModel": {
                                "title": {"content": title}
                            }
                        },
                        "contentImage": {
                            "thumbnailViewModel": {
                                "image": {"sources": [{"url": thumbnail_url}]}
                            }
                        }
                    }
                }
            }
        })
    }

    fn continuation_item(token: &str) -> serde_json::Value {
        serde_json::json!({
            "continuationItemRenderer": {
                "continuationEndpoint": {
                    "continuationCommand": {"token": token}
                }
            }
        })
    }

    #[test]
    fn parse_grid_items_extracts_videos_and_continuation() {
        let items = vec![
            lockup_item("abc123", "First video", "https://example.com/1.jpg"),
            lockup_item("def456", "Second video", "https://example.com/2.jpg"),
            continuation_item("next-page-token"),
        ];

        let grid = parse_grid_items(&items);

        assert_eq!(grid.items.len(), 2);
        assert_eq!(grid.items[0].id, "abc123");
        assert_eq!(grid.items[0].title, "First video");
        assert_eq!(grid.items[0].thumbnail, "https://example.com/1.jpg");
        assert_eq!(grid.items[0].url, "https://www.youtube.com/watch?v=abc123");
        assert_eq!(grid.items[1].id, "def456");
        assert_eq!(grid.next_continuation, Some("next-page-token".to_string()));
    }

    #[test]
    fn parse_grid_items_last_page_has_no_continuation() {
        let items = vec![lockup_item("abc123", "Only video", "https://example.com/1.jpg")];

        let grid = parse_grid_items(&items);

        assert_eq!(grid.items.len(), 1);
        assert_eq!(grid.next_continuation, None);
    }

    #[test]
    fn parse_grid_items_defaults_missing_title_and_thumbnail() {
        let item = serde_json::json!({
            "richItemRenderer": {
                "content": {
                    "lockupViewModel": {
                        "contentId": "abc123"
                    }
                }
            }
        });

        let grid = parse_grid_items(&[item]);

        assert_eq!(grid.items.len(), 1);
        assert_eq!(grid.items[0].title, "");
        assert_eq!(grid.items[0].thumbnail, "");
    }

    #[test]
    fn parse_grid_items_skips_unrecognized_entries() {
        let items = vec![serde_json::json!({"someOtherRenderer": {}})];

        let grid = parse_grid_items(&items);

        assert!(grid.items.is_empty());
        assert_eq!(grid.next_continuation, None);
    }

    fn video_info(id: &str) -> VideoInfo {
        VideoInfo {
            id: id.into(),
            url: make_youtube_video_url(id),
            title: String::new(),
            thumbnail: String::new(),
        }
    }

    #[test]
    fn collect_links_preserves_youtube_order() {
        let mut crawler = YtUploadsCrawler::channel("some-channel");

        crawler.collect_links(vec![
            video_info("newest"),
            video_info("middle"),
            video_info("oldest"),
        ]);

        assert_eq!(crawler.links.pop().unwrap().id, "newest");
        assert_eq!(crawler.links.pop().unwrap().id, "middle");
        assert_eq!(crawler.links.pop().unwrap().id, "oldest");
        assert!(crawler.links.is_empty());
    }

    #[test]
    fn make_youtube_video_url_builds_watch_url() {
        assert_eq!(
            make_youtube_video_url("abc123"),
            "https://www.youtube.com/watch?v=abc123"
        );
    }

    #[test]
    fn canonicalize_video_url_prefixes_relative_paths() {
        assert_eq!(
            canonicalize_video_url("/watch?v=abc123"),
            "https://www.youtube.com/watch?v=abc123"
        );
    }

    #[test]
    fn canonicalize_video_url_leaves_absolute_urls_untouched() {
        assert_eq!(
            canonicalize_video_url("https://www.youtube.com/watch?v=abc123"),
            "https://www.youtube.com/watch?v=abc123"
        );
    }
}
