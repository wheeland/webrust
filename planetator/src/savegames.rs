

#[derive(Serialize)]
#[derive(Deserialize)]
pub enum Savegame {
    Version0 {
        generator: String,
        colorator: String,
        select_channels: Vec<(String, i32)>,
        active_textures: Vec<(String, (i32, i32), Vec<u8>)>,
    }
}
