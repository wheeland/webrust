

#[derive(Serialize)]
#[derive(Deserialize)]
pub enum Savegame {
    Version0 {
        generator: String,
        colorator: String,
        select_channels: Vec<(String, i32)>,
    }
}

