

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
    CreateAccount {
        namepass: Vec<u8>,
    },
    SaltRequest {
        name: Vec<u8>,
    },
    Login {
        namesalthashpass: Vec<u8>,
    },
    UploadReplay {
        name: String,
        replay: super::replay::Replay,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerAnswer {
    InvalidMessage(String),
    ServerError(String),
    CreateAccountResult {
        error: Option<String>,
    },
    Salt {
        salt: Vec<u8>,
    },
    LoginResult {
        error: Option<String>,
    },
    UploadResult(bool)
}