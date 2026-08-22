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
