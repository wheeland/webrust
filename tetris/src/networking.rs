

pub fn encode<T: serde::Serialize>(message: &T) -> String {
    // let message = bincode::serialize(message).unwrap();
    // base64::encode(&message)
    base64::encode(&serde_json::to_string(message).unwrap())
}

pub fn decode<T: serde::de::DeserializeOwned>(message: &str) -> Option<T> {
    let ret = match base64::decode(message).ok() {
        None => None,
        Some(data) => {
            // bincode::deserialize(&data).ok()
            serde_json::from_str(&String::from_utf8(data).unwrap()).ok()
        }
    };

    ret
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    UploadReplay {
        name: String,
        idtag: String,
        replay: super::replay::Replay,
    },
    RequestHighscores {
        by_score: bool, // else by time
        idtag: Option<String>,
        from: usize,
        to: usize,
    },
    RequestReplays {
        ids: Vec<usize>,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerAnswer {
    InvalidMessage(String),
    ServerError(String),
    HighscoreList {
        by_score: bool, // else by time
        idtagged: bool,
        from: usize,
        to: usize,
        data: Vec<super::PlayedGame>,
    },
    ReplayList {
        data: Vec<(usize, super::replay::Replay)>,
    },
    UploadResult(
        Option<super::PlayedGame>
    )
}