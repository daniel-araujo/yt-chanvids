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
