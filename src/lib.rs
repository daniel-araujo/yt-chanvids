mod channel_detail;
mod errors;
mod uploads_crawler;
mod video_detail;

pub use channel_detail::{ChannelDetail, YtChannelDetailScraper};
pub use errors::{FetchLinksError, ParseJsonDataError, RequestError};
pub use uploads_crawler::{VideoInfo, YtUploadsCrawler};
pub use video_detail::{VideoDetail, YtVideoDetailScraper};
