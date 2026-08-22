use crate::errors::RequestError;

/// A cookie that skips YouTube's cookie consent interstitial, which would
/// otherwise redirect requests from EU-region IPs to consent.youtube.com
/// instead of returning the requested page.
pub(crate) const CONSENT_COOKIE: &str = "CONSENT=YES+1; SOCS=CAI";

/// Fetches a URL and returns its body, treating a non-2xx response as
/// `RequestError::NotFound`.
pub(crate) fn fetch_html(url: &str) -> Result<String, RequestError> {
    let response = attohttpc::get(url).header("cookie", CONSENT_COOKIE).send()?;

    if !response.is_success() {
        return Err(RequestError::NotFound);
    }

    Ok(response.text()?)
}

/// The URLs to try in order to reach a channel identified by either a raw
/// channel id (e.g. `UC...`) or a handle/username (with or without a
/// leading `@`).
pub(crate) fn channel_url_candidates(channel: &str, suffix: &str) -> [String; 2] {
    [
        format!("https://www.youtube.com/channel/{}{}", channel, suffix),
        format!(
            "https://www.youtube.com/{}{}",
            channel.trim_start_matches('@'),
            suffix
        ),
    ]
}

/// Fetches a channel page, trying each candidate URL in turn until one
/// succeeds.
pub(crate) fn fetch_channel_html(channel: &str, suffix: &str) -> Result<String, RequestError> {
    for url in channel_url_candidates(channel, suffix) {
        match fetch_html(&url) {
            Ok(html) => return Ok(html),
            Err(RequestError::NotFound) => continue,
            Err(error) => return Err(error),
        }
    }

    Err(RequestError::NotFound)
}

/// Finds `marker` in `html` and returns the JSON object that immediately
/// follows it, by counting braces (while skipping over quoted strings) until
/// the one that closes the object starting the search is found. Used to pull
/// things like `var ytInitialData = {...};` out of a page without a full JS
/// parser.
pub(crate) fn extract_balanced_json<'a>(html: &'a str, marker: &str) -> Option<&'a str> {
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
pub(crate) fn extract_quoted_value(html: &str, marker: &str) -> Option<String> {
    let after = &html[html.find(marker)? + marker.len()..];
    let end = after.find('"')?;
    Some(after[..end].to_string())
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
    fn channel_url_candidates_tries_channel_id_then_stripped_handle() {
        let candidates = channel_url_candidates("@SomeHandle", "/videos");

        assert_eq!(
            candidates,
            [
                "https://www.youtube.com/channel/@SomeHandle/videos".to_string(),
                "https://www.youtube.com/SomeHandle/videos".to_string(),
            ]
        );
    }

    #[test]
    fn channel_url_candidates_leaves_plain_names_untouched() {
        let candidates = channel_url_candidates("UC-lHJZR3Gqxm24_Vd_AJ5Yw", "");

        assert_eq!(
            candidates,
            [
                "https://www.youtube.com/channel/UC-lHJZR3Gqxm24_Vd_AJ5Yw".to_string(),
                "https://www.youtube.com/UC-lHJZR3Gqxm24_Vd_AJ5Yw".to_string(),
            ]
        );
    }
}
