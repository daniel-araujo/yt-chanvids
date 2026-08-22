use crate::errors::{ParseJsonDataError, RequestError};
use crate::youtube_page::{extract_balanced_json, fetch_channel_html};

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
        let html = fetch_channel_html(&self.id, "").unwrap();
        parse_channel_detail(&html).unwrap()
    }
}

/// Extracts a channel's title, description and avatar from the embedded
/// `ytInitialData` on any of its pages (this metadata isn't tied to a
/// particular tab).
fn parse_channel_detail(html: &str) -> Result<ChannelDetail, RequestError> {
    let json_src = extract_balanced_json(html, "var ytInitialData = ")
        .ok_or_else(|| ParseJsonDataError::Html(html.to_string()))?;
    let data: serde_json::Value =
        serde_json::from_str(json_src).map_err(ParseJsonDataError::from)?;

    let metadata = &data["metadata"]["channelMetadataRenderer"];

    let title = metadata["title"]
        .as_str()
        .ok_or_else(|| ParseJsonDataError::Html(html.to_string()))?
        .to_string();
    let description = metadata["description"].as_str().unwrap_or("").to_string();
    let author_thumbnail = metadata["avatar"]["thumbnails"][0]["url"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(ChannelDetail {
        title,
        author_thumbnail,
        description,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_channel_detail_extracts_fields() {
        let html = concat!(
            r#"<script>var ytInitialData = {"metadata":{"channelMetadataRenderer":"#,
            r#"{"title":"PewDiePie","description":"I make videos.","avatar":"#,
            r#"{"thumbnails":[{"url":"https://example.com/avatar.jpg"}]}}}};</script>"#,
        );

        let detail = parse_channel_detail(html).unwrap();

        assert_eq!(detail.title, "PewDiePie");
        assert_eq!(detail.description, "I make videos.");
        assert_eq!(detail.author_thumbnail, "https://example.com/avatar.jpg");
    }

    #[test]
    fn parse_channel_detail_defaults_missing_description_and_thumbnail() {
        let html = concat!(
            r#"<script>var ytInitialData = {"metadata":{"channelMetadataRenderer":"#,
            r#"{"title":"PewDiePie"}}};</script>"#,
        );

        let detail = parse_channel_detail(html).unwrap();

        assert_eq!(detail.title, "PewDiePie");
        assert_eq!(detail.description, "");
        assert_eq!(detail.author_thumbnail, "");
    }

    #[test]
    fn parse_channel_detail_errors_when_ytinitialdata_missing() {
        let html = "<html>not a channel page</html>";

        assert!(matches!(
            parse_channel_detail(html),
            Err(RequestError::ParseJsonDataError(ParseJsonDataError::Html(_)))
        ));
    }

    #[test]
    fn parse_channel_detail_errors_when_title_missing() {
        let html = r#"<script>var ytInitialData = {"metadata":{}};</script>"#;

        assert!(matches!(
            parse_channel_detail(html),
            Err(RequestError::ParseJsonDataError(ParseJsonDataError::Html(_)))
        ));
    }
}
