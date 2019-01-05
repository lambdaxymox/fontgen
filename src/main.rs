extern crate freetype;

use freetype::Library;
use std::process;


const FONT_FILE: &str = "assets/FreeMono.ttf";


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
    let face = match ft.new_face(FONT_FILE, 0) {
        Ok(val) => val,
        Err(_) => {
            eprintln!("Could not open font file.");
            process::exit(1);
        }
    };
    // Set the font size
    face.set_char_size(40 * 64, 0, 50, 0).unwrap();
    // Load a character
    face.load_char('A' as usize, freetype::face::LoadFlag::RENDER).unwrap();
    // Get the glyph instance
    let glyph = face.glyph();
}
