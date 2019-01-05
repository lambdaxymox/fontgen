extern crate freetype;

use freetype::Library;
use std::process;


fn main() {
    // Init the library
    let ft = match Library::init() {
        Ok(val) => val,
        Err(_) => {
            eprintln!("Failed to initialize FreeType library.");
            process::exit(1);
        }
    };
    // Load a font face
    let face = ft.new_face("assets/FreeMono.ttf", 0).unwrap();
    // Set the font size
    face.set_char_size(40 * 64, 0, 50, 0).unwrap();
    // Load a character
    face.load_char('A' as usize, freetype::face::LoadFlag::RENDER).unwrap();
    // Get the glyph instance
    let glyph = face.glyph();
}
