use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum OgType {
    MusicSong,
    MusicAlbum,
    MusicPlaylist,
    MusicRadioStation,

    VideoMovie,
    VideoEpisode,
    VideoTvShow,
    VideoOther,

    Article,
    Book,
    Profile,

    #[default]
    Website,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebData {
    // Website title
    pub title: String,
    // Open-Graph media type
    pub r#type: OgType,
    // Open-Graph provided description
    pub description: Option<String>,
    // Open-Graph banner image
    pub image: Option<String>,
    // Open-Graph author
    pub author: Vec<String>,
    // Accent colour of the website
    pub colour: Option<String>,
}

impl OgType {
    pub fn from_meta(s: &str) -> OgType {
        match s {
            "music.song" => OgType::MusicSong,
            "music.album" => OgType::MusicAlbum,
            "music.playlist" => OgType::MusicPlaylist,
            "music.radio_station" => OgType::MusicRadioStation,

            "video.movie" => OgType::VideoMovie,
            "video.episode" => OgType::VideoEpisode,
            "video.tv_show" => OgType::VideoTvShow,
            "video.other" => OgType::VideoOther,

            "article" => OgType::Article,
            "book" => OgType::Book,
            "profile" => OgType::Profile,

            "website" => OgType::Website,
            _ => OgType::Website,
        }
    }
}

impl Default for WebData {
    fn default() -> Self {
        WebData {
            title: String::new(),
            r#type: OgType::Website,
            description: None,
            image: None,
            author: Vec::new(),
            colour: None,
        }
    }
}
