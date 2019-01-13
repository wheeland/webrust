use webutil::httpclient;
use webutil::curve25519;

use tetris::networking::*;

pub struct ServerConfig {
    idtag: String,
    publickey: [u8; 32],
}

impl ServerConfig {
    pub fn new() -> Self {
        ServerConfig {
            publickey: [
                0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0,
            ],
            idtag: String::new(),
        }
    }
}

pub struct Request {
    fetch: webutil::httpclient::Fetch,
}

#[derive(Debug)]
pub enum Response {
    Waiting,
    HttpError(String),
    ParseError(String),
    Success(ServerAnswer),
}

impl Request {
    pub fn response(&self) -> Response {
        let is_done = match self.fetch.state() {
            webutil::httpclient::State::Done | webutil::httpclient::State::Error => true,
            _ => false,
        };

        if !is_done {
            return Response::Waiting;
        }

        let data = self.fetch.data();
        if data.is_none() {
            return Response::Waiting;
        }

        let mut data = data.unwrap();
        if data.len() == 0 {
            // return Response::HttpError("Empty response".to_string());
            return Response::Waiting;
        }

        // TODO: timeout?

        if *data.last().unwrap() != 0 {
            data.push(0);
        }

        let cstr = unsafe { std::ffi::CStr::from_ptr(data.as_ptr() as *const i8) };
        match cstr.to_str().ok().map(|s| s.to_string()) {
            None => Response::ParseError("No String buildable from data".to_string()),

            Some(cstr) => {
                let cstr = cstr.trim();
                match decode::<ServerAnswer>(cstr) {
                    None => Response::ParseError(format!("Parse Error for '{}'", cstr)),
                    Some(msg) => Response::Success(msg)
                }
            }
        }
    }
}

impl ServerConfig {
    fn encode<T: serde::Serialize>(&self, v: &T) -> Vec<u8> {
        let encoded = bincode::serialize(v).unwrap();
        curve25519::encrypt(&self.publickey, &encoded).ok().unwrap()
    }

    fn post(&self, message: ServerMessage) -> Request {
        let message = encode(&message);
        Request {
            fetch: httpclient::Fetch::post("action.php", &message)
        }
    }

    fn get(&self, message: ServerMessage) -> Request {
        let message = encode(&message);
        let request = format!("action.php?msg={}", message);
        Request {
            fetch: httpclient::Fetch::get(&request)
        }
    }

    pub fn request_scores(&self, by_score: bool, local: bool) -> Request {
        let idtag = if local { Some(self.idtag.clone()) } else { None };
        self.post(ServerMessage::RequestHighscores {
            by_score,
            idtag,
            from: 0,
            to: 0,
        })
    }

    pub fn upload_replay(&self, name: &str, replay: &tetris::replay::Replay) -> Request {
        self.post(ServerMessage::UploadReplay {
            name: name.to_string(),
            idtag: self.idtag.clone(),
            replay: replay.clone(),
        })
    }

    pub fn request_replay(&self, id: usize) -> Request {
        self.post(ServerMessage::RequestReplays {
            ids: vec!(id)
        })
    }
}