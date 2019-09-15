
/// Generates a texture coordinates vertex buffer for plate rendering
///
/// A plate has 2^N+3 edge vertices, a texture may have 2^M+3 texels,
/// where M >= N.
///
/// This method creates a buffer with texture coords, bound to the
/// [1..2^M+1] region of the actual plate, excluding the ribbon space
/// inside the texture.
///
/// The texture coords are in the UInt16 format
pub fn generate_tex_coords_buffer(plate_depth: u32, texture_base_depth: u32, texture_delta: u32) -> Vec<u16> {
    let plate_size = 2u32.pow(plate_depth);
    let texture_size = (2u32.pow(texture_base_depth) + 2) * 2u32.pow(texture_delta as _) + 1;
    let texture_size = texture_size as f32;
    let mut tile_coords = Vec::new();
    let ofs = 2.0f32.powi(texture_delta as _);
    let mult = 2.0f32.powi(texture_delta as i32 + texture_base_depth as i32 - plate_depth as i32);

    let to_tex_space = |vert: u32| {
        let vert = vert.max(1).min(plate_size + 1) as f32;  // constrain to [1..size+1]
        let px = ofs + mult * (vert - 1.0);
        let tc = (px + 0.5) * 65535.0 / texture_size;       // add 0.5, convert to [0..1] range
        tc
    };

    for j in 0..(plate_size + 3) {
        let tc_y = to_tex_space(j);

        for i in 0..(plate_size + 3) {
            let tc_x = to_tex_space(i);

            tile_coords.push(tc_x as u16);
            tile_coords.push(tc_y as u16);
        }
    }

    tile_coords
}