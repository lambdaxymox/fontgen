extern crate freetype;


fn main() {
    use freetype::Library;

    // Init the library
    let lib = Library::init().unwrap();
    // Load a font face
    let face = lib.new_face("assets/FreeMono.ttf", 0).unwrap();
    // Set the font size
    face.set_char_size(40 * 64, 0, 50, 0).unwrap();
    // Load a character
    face.load_char('A' as usize, freetype::face::LoadFlag::RENDER).unwrap();
    // Get the glyph instance
    let glyph = face.glyph();
}
