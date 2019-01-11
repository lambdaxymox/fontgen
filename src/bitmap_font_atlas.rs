use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::path::Path;


///
/// A `GlyphMetadata` struct stores the parameters necessary to represent
/// the glyph in a bitmap font atlas.
///
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct GlyphMetadata {
    /// The unicode code point.
    pub code_point: usize,
    ///
    pub x_min: f32,
    /// The width of the glyph, stored in [0,1].
    pub width: f32,
    /// The height of the glyph, represented in the interval [0,1].
    pub height: f32,
    /// The maximum depth of the glyph that falls below the baseline for the font.
    pub y_min: f32,
    pub y_offset: f32,
}

impl GlyphMetadata {
    pub fn new(
        code_point: usize,
        width: f32, height: f32,
        x_min: f32, y_min: f32, y_offset: f32) -> GlyphMetadata {

        GlyphMetadata {
            code_point: code_point,
            width: width,
            height: height,
            x_min: x_min,
            y_min: y_min,
            y_offset: y_offset,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BitmapFontAtlasMetadata {
    pub dimensions: usize,
    pub columns: usize,
    pub rows: usize,
    pub padding: usize,
    pub slot_glyph_size: usize,
    pub glyph_size: usize,
    pub glyph_metadata: HashMap<usize, GlyphMetadata>,
}

///
/// A `BitmapFontAtlas` is a bitmapped font sheet. It contains the glyph parameters necessary to
/// index into the bitmap image as well as the bitmap image.
///
pub struct BitmapFontAtlas {
    pub metadata: BitmapFontAtlasMetadata,
    pub buffer: Vec<u8>,
}

///
/// Write the metadata file that accompanies the atlas image to a file.
///
pub fn write_metadata<P: AsRef<Path>>(atlas: &BitmapFontAtlas, path: P) -> io::Result<()> {
    let mut file = match File::create(path) {
        Ok(val) => val,
        Err(e) => return Err(e),
    };

    serde_json::to_writer_pretty(file, &atlas.metadata)?;

    Ok(())
}

///
/// Write the atlas bitmap image to a file.
///
pub fn write_atlas_buffer<P: AsRef<Path>>(atlas: &BitmapFontAtlas, path: P) -> io::Result<()> {
    image::save_buffer(
        path, &atlas.buffer,
        atlas.metadata.dimensions as u32, atlas.metadata.dimensions as u32, image::RGBA(8)
    )
}

///
/// Write the bitmap font atlas to the disk.
///
pub fn write_font_atlas<P: AsRef<Path>>(atlas: &BitmapFontAtlas, path: P) -> io::Result<()> {
    write_metadata(atlas, &path)?;
    write_atlas_buffer(atlas, &path)?;

    Ok(())
}
