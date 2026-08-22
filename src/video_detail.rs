use crate::errors::{ParseJsonDataError, RequestError};
use crate::youtube_page::{extract_balanced_json, fetch_html};

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
        let url = format!("https://www.youtube.com/watch?v={}", self.id);
        let html = fetch_html(&url).unwrap();
        parse_video_detail(&html).unwrap()
    }
}

/// Extracts a video's details from the `ytInitialPlayerResponse` embedded in
/// its watch page. A video id that doesn't resolve to a playable video (e.g.
/// it was deleted) still returns a 200 response but with no `videoDetails`,
/// which is reported as `RequestError::NotFound`.
fn parse_video_detail(html: &str) -> Result<VideoDetail, RequestError> {
    let json_src = extract_balanced_json(html, "var ytInitialPlayerResponse = ")
        .ok_or_else(|| ParseJsonDataError::Html(html.to_string()))?;
    let data: serde_json::Value =
        serde_json::from_str(json_src).map_err(ParseJsonDataError::from)?;

    let video_details = &data["videoDetails"];
    let title = match video_details["title"].as_str() {
        Some(title) => title.to_string(),
        None => return Err(RequestError::NotFound),
    };

    let description = video_details["shortDescription"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let duration_seconds = video_details["lengthSeconds"]
        .as_str()
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    let publish_date = data["microformat"]["playerMicroformatRenderer"]["uploadDate"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(VideoDetail {
        title,
        description,
        duration_seconds,
        publish_date,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_video_detail_extracts_fields() {
        let html = concat!(
            r#"<script>var ytInitialPlayerResponse = {"videoDetails":"#,
            r#"{"title":"Earthquake by Mount Fuji","shortDescription":"desc here","#,
            r#""lengthSeconds":"1936"},"microformat":{"playerMicroformatRenderer":"#,
            r#"{"uploadDate":"2026-07-30T01:30:17-07:00"}}};</script>"#,
        );

        let detail = parse_video_detail(html).unwrap();

        assert_eq!(detail.title, "Earthquake by Mount Fuji");
        assert_eq!(detail.description, "desc here");
        assert_eq!(detail.duration_seconds, 1936);
        assert_eq!(detail.publish_date, "2026-07-30T01:30:17-07:00");
    }

    #[test]
    fn parse_video_detail_not_found_when_video_details_missing() {
        let html = concat!(
            r#"<script>var ytInitialPlayerResponse = {"playabilityStatus":"#,
            r#"{"status":"ERROR"}};</script>"#,
        );

        assert!(matches!(
            parse_video_detail(html),
            Err(RequestError::NotFound)
        ));
    }

    #[test]
    fn parse_video_detail_errors_when_player_response_missing() {
        let html = "<html>not a watch page</html>";

        assert!(matches!(
            parse_video_detail(html),
            Err(RequestError::ParseJsonDataError(ParseJsonDataError::Html(_)))
        ));
    }
}
