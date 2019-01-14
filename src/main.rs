extern crate bmfa;
extern crate freetype;
extern crate image;
extern crate structopt;


use bmfa::{BitmapFontAtlas, BitmapFontAtlasMetadata, GlyphMetadata};
use freetype::Library;
use std::collections::HashMap;
use std::error;
use std::fmt;
use std::mem;
use std::path::PathBuf;
use std::process;
use structopt::StructOpt;


///
/// The atlas specification is a description of the dimensions of the atlas
/// and the dimensions of each glyph in the atlas. This comes in as input at
/// runtime.
///
#[derive(Copy, Clone)]
struct AtlasSpec {
    /// The size of the atlas, in pixels.
    dimensions: usize,
    /// The number of glyphs per row in the atlas.
    columns: usize,
    /// The amount of padding available for outlines in the glyph, in pixels.
    padding: usize,
    /// The maximum size of a glyph slot, in pixels.
    slot_glyph_size: usize,
    /// The size of a glyph inside the slot, leaving room for padding for outlines.
    glyph_size: usize,
}

impl AtlasSpec {
    fn new(
        dimensions: usize, columns: usize,
        padding: usize, slot_glyph_size: usize, glyph_size: usize) -> AtlasSpec {

        AtlasSpec {
            dimensions: dimensions,
            columns: columns,
            padding: padding,
            slot_glyph_size: slot_glyph_size,
            glyph_size: glyph_size,
        }
    }
}

///
/// A `GlyphImage` is a bitmapped representation of a single font glyph.
///
#[derive(Clone)]
struct GlyphImage {
    data: Vec<u8>,
}

impl GlyphImage {
    fn new(data: Vec<u8>) -> GlyphImage {
        GlyphImage {
            data: data,
        }
    }
}

///
/// A `GlyphTable` is an intermediate date structure storing all the typeface parameters
/// for each glyph to be used in the construction of the final bitmap atlas.
///
struct GlyphTable {
    /// the height of a glyph in pixels.
    rows: Vec<i32>,
    /// the width of a row in a glyph in pixels.
    width: Vec<i32>,
    /// The number of bytes per row in a glyph.
    pitch: Vec<i32>,
    /// The offset in pixels of a character from the baseline.
    y_min: Vec<i64>,
    /// A table holding the individual bitmap images for each glyph.
    buffer: HashMap<usize, GlyphImage>,
}

///
/// Sample a single bitmap image for a single glyph from a font. The FreeType library interns
/// each sampled glyph image one at a time internally. Each time the library samples a new glyph,
/// the old glyph gets overwritten, so the data must be copied out before each subsequent
/// sampling of a new glyph.
///
fn create_glyph_image(glyph: &freetype::glyph_slot::GlyphSlot) -> GlyphImage {
    let bitmap = glyph.bitmap();
    let rows = bitmap.rows() as usize;
    let pitch = bitmap.pitch() as usize;

    let mut glyph_data = vec![0 as u8; rows * pitch];
    glyph_data.clone_from_slice(bitmap.buffer());

    GlyphImage::new(glyph_data)
}


#[derive(Copy, Clone, Debug)]
enum SampleTypefaceError {
    SetPixelSize(freetype::error::Error, usize, usize),
    LoadCharacter(freetype::error::Error, usize),
    RenderCharacter(freetype::error::Error, usize),
    GetGlyphImage(freetype::error::Error, usize),
}

impl fmt::Display for SampleTypefaceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            SampleTypefaceError::SetPixelSize(_, code_point, pixels) => {
                write!(
                    f, "The FreeType library failed to set the size of glyph {} to {} pixels.",
                    code_point, pixels
                )
            }
            SampleTypefaceError::LoadCharacter(_, code_point) => {
                write!(
                    f, "The FreeType library failed to load the character with code point {}.",
                    code_point
                )
            }
            SampleTypefaceError::RenderCharacter(_, code_point) => {
                write!(
                    f, "The FreeType library could not render the code point {}.",
                    code_point
                )
            }
            SampleTypefaceError::GetGlyphImage(_, code_point) => {
                write!(
                    f, "The FreeType library extract the glyph image for the code point {}.",
                    code_point
                )
            }
        }
    }
}

impl error::Error for SampleTypefaceError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            &SampleTypefaceError::SetPixelSize(ref e,_,_) => Some(e),
            &SampleTypefaceError::LoadCharacter(ref e,_) => Some(e),
            &SampleTypefaceError::RenderCharacter(ref e, _) => Some(e),
            &SampleTypefaceError::GetGlyphImage(ref e,_) => Some(e),
        }
    }
}

///
/// Generate the glyph image for each individual glyph slot in the typeface to be
/// mapped into the final atlas image.
///
fn sample_typeface(
    face: freetype::face::Face, spec: AtlasSpec) -> Result<GlyphTable, SampleTypefaceError> {

    // Tell FreeType the maximum size of each glyph, in pixels.
    // The glyph height in pixels.
    let mut glyph_rows = vec![0 as i32; 256];
    // The glyph width in pixels.
    let mut glyph_width = vec![0 as i32; 256];
    // The bytes to per row of pixels per glyph.
    let mut glyph_pitch = vec![0 as i32; 256];
    // The offset for letters that dip below the baseline like 'g' and 'y', for example.
    let mut glyph_ymin = vec![0 as i64; 256];
    // A table for storing the sampled glyph images.
    let mut glyph_buffer = HashMap::new();

    // Set the height in pixels width 0 height 48 (48x48).
    face.set_pixel_sizes(0, spec.glyph_size as u32).map_err(|e| {
        SampleTypefaceError::SetPixelSize(e, 0, spec.glyph_size)
    })?;

    for i in 33..256 {
        face.load_char(i, freetype::face::LoadFlag::RENDER).map_err(|e| {
            SampleTypefaceError::LoadCharacter(e, i)
        })?;

        // Draw a glyph image anti-aliased.
        let glyph_handle = face.glyph();

        glyph_handle.render_glyph(freetype::render_mode::RenderMode::Normal).map_err(|e| {
            SampleTypefaceError::RenderCharacter(e, i)
        })?;

        // Get the dimensions of the bitmap.
        glyph_rows[i] = glyph_handle.bitmap().rows();
        glyph_width[i] = glyph_handle.bitmap().width();
        glyph_pitch[i] = glyph_handle.bitmap().pitch();

        let glyph_image_i = create_glyph_image(glyph_handle);
        glyph_buffer.insert(i, glyph_image_i);

        // Get the y-offset to place glyphs on baseline. This data lies in the bounding box.
        let glyph = match glyph_handle.get_glyph() {
            Ok(val) => val,
            Err(e) => {
                return Err(SampleTypefaceError::GetGlyphImage(e, i));
            }
        };

        // Get the bounding box. Here "truncated" mode specifies that the dimensions
        // of the bounding box are given in pixels.
        let bbox = glyph.get_cbox(freetype::ffi::FT_GLYPH_BBOX_TRUNCATE);
        glyph_ymin[i] = bbox.yMin;
    }

    Ok(GlyphTable {
        rows: glyph_rows,
        width: glyph_width,
        pitch: glyph_pitch,
        y_min: glyph_ymin,
        buffer: glyph_buffer,
    })
}

///
/// Calculate the metadata for indexing into the atlas bitmap image.
///
fn create_bitmap_metadata(glyph_tab: &GlyphTable, spec: AtlasSpec) -> HashMap<usize, GlyphMetadata> {
    let mut metadata = HashMap::new();
    let glyph_metadata_space = GlyphMetadata::new(32, 0.0, 0.5, 0.0, 1.0, 0.0);
    metadata.insert(32, glyph_metadata_space);
    for i in glyph_tab.buffer.keys() {
        let order = i - 32;
        let col = order % spec.columns;
        let row = order % spec.columns;

        // Glyph metadata parameters.
        let x_min = (col * spec.slot_glyph_size) as f32 / spec.dimensions as f32;
        let y_min = (row * spec.slot_glyph_size) as f32 / spec.dimensions as f32;
        let width = (glyph_tab.width[*i] + spec.padding as i32) as f32 / spec.slot_glyph_size as f32;
        let height = (glyph_tab.rows[*i] + spec.padding as i32) as f32 / spec.slot_glyph_size as f32;
        let y_offset = -(spec.padding as f32 - glyph_tab.y_min[*i] as f32) / spec.slot_glyph_size as f32;

        let glyph_metadata_i = GlyphMetadata::new(*i, width, height, x_min, y_min, y_offset);
        metadata.insert(*i, glyph_metadata_i);
    }

    metadata
}

///
/// Pack the glyph bitmap images sampled from the typeface into a single bitmap image.
///
fn create_bitmap_buffer(glyph_tab: &GlyphTable, spec: AtlasSpec) -> Vec<u8> {
    // Next we can open a file stream to write our atlas image to
    let mut atlas_buffer = vec![
        0 as u8; spec.dimensions * spec.dimensions * 4 * mem::size_of::<u8>()
    ];
    let mut atlas_buffer_index = 0;
    for y in 0..spec.dimensions {
        for x in 0..spec.dimensions {
            // work out which grid slot (col, row) we are in i.e. out of 16 glyphs x 16 glyphs.
            let col = x / spec.slot_glyph_size;
            let row = y / spec.slot_glyph_size;
            let order = row * spec.columns + col;
            let glyph_index = order + 32;

            if (glyph_index > 32) && (glyph_index < 256) {
                // A glyph exists for this code point in the bitmap.
                // Pixel indices within padded glyph slot area.
                let x_loc = ((x % spec.slot_glyph_size) as i32) - ((spec.padding / 2) as i32);
                let y_loc = ((y % spec.slot_glyph_size) as i32) - ((spec.padding / 2) as i32);
                // Outside of the glyph dimensions we use as default value a
                // transparent black pixel (0,0,0,0).
                if x_loc < 0 || y_loc < 0 || x_loc >= glyph_tab.width[glyph_index] ||
                    y_loc >= glyph_tab.rows[glyph_index] {
                    atlas_buffer[atlas_buffer_index] = 0;
                    atlas_buffer_index += 1;
                    atlas_buffer[atlas_buffer_index] = 0;
                    atlas_buffer_index += 1;
                    atlas_buffer[atlas_buffer_index] = 0;
                    atlas_buffer_index += 1;
                    atlas_buffer[atlas_buffer_index] = 0;
                    atlas_buffer_index += 1;
                } else {
                    // this is 1, but it's safer to put it in anyway
                    // int bytes_per_pixel = gwidth[glyph_index] / gpitch[glyph_index];
                    // int bytes_in_glyph = grows[glyph_index] * gpitch[glyph_index];
                    let byte_order_in_glyph = y_loc * glyph_tab.width[glyph_index] + x_loc;
                    let mut colour = [0 as u8; 4];
                    colour[0] = glyph_tab.buffer[&glyph_index].data[byte_order_in_glyph as usize];
                    colour[1] = colour[0];
                    colour[2] = colour[0];
                    colour[3] = colour[0];

                    atlas_buffer[atlas_buffer_index] = glyph_tab.buffer[&glyph_index].data[byte_order_in_glyph as usize];
                    atlas_buffer_index += 1;
                    atlas_buffer[atlas_buffer_index] = glyph_tab.buffer[&glyph_index].data[byte_order_in_glyph as usize];
                    atlas_buffer_index += 1;
                    atlas_buffer[atlas_buffer_index] = glyph_tab.buffer[&glyph_index].data[byte_order_in_glyph as usize];
                    atlas_buffer_index += 1;
                    atlas_buffer[atlas_buffer_index] = glyph_tab.buffer[&glyph_index].data[byte_order_in_glyph as usize];
                    atlas_buffer_index += 1;
                }
            } else {
                // A glyph does not exist for this code point in the bitmap. We choose to use a
                // a transparent black pixel value (0,0,0,0).
                atlas_buffer[atlas_buffer_index] = 0;
                atlas_buffer_index += 1;
                atlas_buffer[atlas_buffer_index] = 0;
                atlas_buffer_index += 1;
                atlas_buffer[atlas_buffer_index] = 0;
                atlas_buffer_index += 1;
                atlas_buffer[atlas_buffer_index] = 0;
                atlas_buffer_index += 1;
            }
        }
    }

    atlas_buffer
}

///
/// Create a bitmapped atlas from a vector based font atlas.
///
fn create_bitmap_atlas(
    face: freetype::face::Face, spec: AtlasSpec) -> Result<BitmapFontAtlas, SampleTypefaceError> {

    let glyph_tab = match sample_typeface(face, spec) {
        Ok(val) => val,
        Err(e) => return Err(e),
    };
    let glyph_metadata = create_bitmap_metadata(&glyph_tab, spec);
    let atlas_image = create_bitmap_buffer(&glyph_tab, spec);

    let metadata = BitmapFontAtlasMetadata {
        dimensions: spec.dimensions,
        columns: spec.columns,
        rows: spec.columns,
        padding: spec.padding,
        slot_glyph_size: spec.slot_glyph_size,
        glyph_size: spec.glyph_size,
        glyph_metadata: glyph_metadata,
    };

    Ok(BitmapFontAtlas::new(metadata, atlas_image))
}

///
/// The shell input options for `fontgen`.
///
#[derive(Debug, StructOpt)]
#[structopt(
    name = "fontgen",
    about = "A shell utility for converting TrueType or OpenType fonts into bitmapped fonts."
)]
struct Opt {
    /// The path to the input file.
    #[structopt(parse(from_os_str))]
    #[structopt(short = "i", long = "input")]
    input_path: PathBuf,
    #[structopt(parse(from_os_str))]
    #[structopt(short = "o", long = "output")]
    /// The path to the output file.
    output_path: PathBuf,
    #[structopt(long = "slot-glyph-size", default_value = "64")]
    /// The size, in pixels, of a glyph slot in the font sheet. The slot glyph
    /// is not necessarily the same as the glyph size because a glyph slot can contain padding.
    slot_glyph_size: usize,
    #[structopt(short = "p", long = "padding", default_value = "0")]
    /// The glyph slot padding size, in pixels. This is the number of pixels away from the
    /// boundary of a glyph slot a glyph will be placed.
    padding: usize,
}

#[derive(Clone, Debug)]
enum OptError {
    InputFileDoesNotExist(PathBuf),
    InputFileIsNotAFile(PathBuf),
    OutputFileExists(PathBuf),
    SlotGlyphSizeCannotBeZero(usize),
    PaddingLargerThanSlotGlyphSize(usize, usize),
}

///
/// Verify the input options.
///
fn verify_opt(opt: &Opt) -> Result<(), OptError> {
    if !opt.input_path.exists() {
        return Err(OptError::InputFileDoesNotExist(opt.input_path.clone()));
    }
    if !opt.input_path.is_file() {
        return Err(OptError::InputFileIsNotAFile(opt.input_path.clone()));
    }
    if opt.output_path.exists() {
        return Err(OptError::OutputFileExists(opt.output_path.clone()));
    }
    if !(opt.slot_glyph_size > 0) {
        return Err(OptError::SlotGlyphSizeCannotBeZero(opt.slot_glyph_size));
    }
    if opt.padding > opt.slot_glyph_size {
        return Err(OptError::PaddingLargerThanSlotGlyphSize(opt.padding, opt.slot_glyph_size));
    }

    Ok(())
}

///
/// Run the application.
///
fn run_app(opt: &Opt) -> Result<(), String> {
    let ft = Library::init().expect("Failed to initialize FreeType library.");
    let face = match ft.new_face(&opt.input_path, 0) {
        Ok(val) => val,
        Err(_) => {
            return Err(format!("Could not open font file: {}.", &opt.input_path.display()));
        }
    };

    let slot_glyph_size = opt.slot_glyph_size;
    let atlas_columns = 16;
    let atlas_dimensions_px = slot_glyph_size * atlas_columns;
    let padding_px = opt.padding;
    let atlas_glyph_px = slot_glyph_size - padding_px;
    let mut atlas_file = opt.output_path.clone();
    atlas_file.set_extension("bmfa");

    let atlas_spec = AtlasSpec::new(
        atlas_dimensions_px, atlas_columns, padding_px, slot_glyph_size, atlas_glyph_px
    );
    let atlas = match create_bitmap_atlas(face, atlas_spec) {
        Ok(val) => val,
        Err(e) => {
            return Err(format!("Could create bitmap font. Got error: {}", e));
        }
    };

    if bmfa::write_to_file(&atlas_file, &atlas).is_err() {
        return Err(format!(
            "Could not create atlas file: {}.", atlas_file.display()
        ));
    }

    Ok(())
}

fn main() {
    let opt = Opt::from_args();
    match verify_opt(&opt) {
        Err(e) => {
            eprintln!("Error: {:?}", e);
            process::exit(1);
        }
        Ok(_) => {}
    }

    process::exit(match run_app(&opt) {
        Ok(_) => {
            0
        },
        Err(e) => {
            eprintln!("{:?}", e);
            1
        }
    });
}
