extern crate freetype;
extern crate image;
extern crate structopt;

use freetype::Library;
use std::collections::HashMap;
use std::error;
use std::fmt;
use std::fs::File;
use std::io;
use std::io::Write;
use std::mem;
use std::path::{Path, PathBuf};
use std::process;
use structopt::StructOpt;


const FONT_FILE: &str = "assets/FreeMono.ttf";
const PNG_OUTPUT_IMAGE: &str = "atlas.png";
const ATLAS_META_FILE: &str = "atlas.meta";


///
/// The atlas specification is a description of the dimensions of the atlas
/// and the dimensions of each glyph in the atlas. This comes in as input at
/// runtime.
///
#[derive(Copy, Clone)]
struct AtlasSpec {
    /// The size of the atlas, in pixels.
    dimensions_px: usize,
    /// The number of glyphs per row in the atlas.
    columns: usize,
    /// The amount of padding available for outlines in the glyph.
    padding_px: usize,
    /// The maximum size of a glyph slot, in pixels.
    slot_glyph_size: usize,
    /// The size of a glyph inside the slot, leaving room for padding for outlines.
    glyph_px: usize,
}

impl AtlasSpec {
    fn new(
        dimensions_px: usize, columns: usize,
        padding_px: usize, slot_glyph_size: usize, glyph_px: usize) -> AtlasSpec {

        AtlasSpec {
            dimensions_px: dimensions_px,
            columns: columns,
            padding_px: padding_px,
            slot_glyph_size: slot_glyph_size,
            glyph_px: glyph_px,
        }
    }
}

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

struct GlyphMetadata {
    code_point: usize,
    x_min: f32,
    width: f32,
    height: f32,
    y_min: f32,
    y_offset: f32,
}

impl GlyphMetadata {
    fn new(
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

struct BitmapAtlas {
    dimensions_px: usize,
    columns: usize,
    padding_px: usize,
    slot_glyph_size: usize,
    glyph_px: usize,
    metadata: HashMap<usize, GlyphMetadata>,
    buffer: Vec<u8>,
}

struct GlyphTable {
    rows: Vec<i32>,
    width: Vec<i32>,
    pitch: Vec<i32>,
    y_min: Vec<i64>,
    buffer: HashMap<usize, GlyphImage>,
}

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
    SetPixelSize(usize, usize),
    LoadCharacter(usize),
    RenderCharacter(usize),
    GetGlyphImage(usize),
}

impl fmt::Display for SampleTypefaceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            SampleTypefaceError::SetPixelSize(code_point, pixels) => {
                write!(
                    f, "The FreeType library failed to set the size of glyph {} to {} pixels.",
                    code_point, pixels
                )
            }
            SampleTypefaceError::LoadCharacter(code_point) => {
                write!(
                    f, "The FreeType library failed to load the character with code point {}.",
                    code_point
                )
            }
            SampleTypefaceError::RenderCharacter(code_point) => {
                write!(
                    f, "The FreeType library could not render the code point {}.",
                    code_point
                )
            }
            SampleTypefaceError::GetGlyphImage(code_point) => {
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
        None
    }
}


fn sample_typeface(
    face: freetype::face::Face, spec: AtlasSpec) -> Result<GlyphTable, SampleTypefaceError> {

    // Tell FreeType the maximum size of each glyph, in pixels.
    let mut glyph_rows = vec![0 as i32; 256];   // glyph height in pixels
    let mut glyph_width = vec![0 as i32; 256];  // glyph width in pixels
    let mut glyph_pitch = vec![0 as i32; 256];  // bytes per row of pixels
    let mut glyph_ymin = vec![0 as i64; 256];   // offset for letters that dip below baseline like g and y
    let mut glyph_buffer = HashMap::new(); // stored glyph images

    // set height in pixels width 0 height 48 (48x48)
    face.set_pixel_sizes(0, spec.glyph_px as u32).map_err(|e| {
        SampleTypefaceError::SetPixelSize(0, spec.glyph_px)
    })?;

    for i in 33..256 {
        face.load_char(i, freetype::face::LoadFlag::RENDER).map_err(|e| {
            SampleTypefaceError::LoadCharacter(i)
        })?;

        // draw glyph image anti-aliased
        let glyph_handle = face.glyph();

        glyph_handle.render_glyph(freetype::render_mode::RenderMode::Normal).map_err(|e| {
            SampleTypefaceError::RenderCharacter(i)
        })?;

        // get dimensions of bitmap
        glyph_rows[i] = glyph_handle.bitmap().rows();
        glyph_width[i] = glyph_handle.bitmap().width();
        glyph_pitch[i] = glyph_handle.bitmap().pitch();

        // copy glyph data into memory because it seems to be overwritten/lost later
        let glyph_image_i = create_glyph_image(glyph_handle);
        glyph_buffer.insert(i, glyph_image_i);

        // get y-offset to place glyphs on baseline. this is in the bounding box
        let glyph = match glyph_handle.get_glyph() {
            Ok(val) => val,
            Err(_) => {
                return Err(SampleTypefaceError::GetGlyphImage(i));
            }
        };

        // get bbox. "truncated" mode means get dimensions in pixels
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

fn create_bitmap_metadata(glyph_tab: &GlyphTable, spec: AtlasSpec) -> HashMap<usize, GlyphMetadata> {
    let mut metadata = HashMap::new();
    let glyph_metadata_space = GlyphMetadata::new(32, 0.0, 0.5, 0.0, 1.0, 0.0);
    metadata.insert(32, glyph_metadata_space);
    for i in glyph_tab.buffer.keys() {
        let order = i - 32;
        let col = order % spec.columns;
        let row = order % spec.columns;

        // Glyph metadata parameters.
        let x_min = (col * spec.slot_glyph_size) as f32 / spec.dimensions_px as f32;
        let y_min = (row * spec.slot_glyph_size) as f32 / spec.dimensions_px as f32;
        let width = (glyph_tab.width[*i] + spec.padding_px as i32) as f32 / spec.slot_glyph_size as f32;
        let height = (glyph_tab.rows[*i] + spec.padding_px as i32) as f32 / spec.slot_glyph_size as f32;
        let y_offset = -(spec.padding_px as f32 - glyph_tab.y_min[*i] as f32) / spec.slot_glyph_size as f32;

        let glyph_metadata_i = GlyphMetadata::new(*i, width, height, x_min, y_min, y_offset);
        metadata.insert(*i, glyph_metadata_i);
    }

    metadata
}

fn create_bitmap_buffer(glyph_tab: &GlyphTable, spec: AtlasSpec) -> Vec<u8> {
    // Next we can open a file stream to write our atlas image to
    let mut atlas_buffer = vec![
        0 as u8; spec.dimensions_px * spec.dimensions_px * 4 * mem::size_of::<u8>()
    ];
    let mut atlas_buffer_index = 0;
    for y in 0..spec.dimensions_px {
        for x in 0..spec.dimensions_px {
            // work out which grid slot[col][row] we are in e.g out of 16x16
            let col = x / spec.slot_glyph_size;
            let row = y / spec.slot_glyph_size;
            let order = row * spec.columns + col;
            let glyph_index = order + 32;

            // an actual glyph bitmap exists for these indices
            if (glyph_index > 32) && (glyph_index < 256) {
                // pixel indices within padded glyph slot area
                let x_loc = ((x % spec.slot_glyph_size) as i32) - ((spec.padding_px / 2) as i32);
                let y_loc = ((y % spec.slot_glyph_size) as i32) - ((spec.padding_px / 2) as i32);
                // outside of glyph dimensions use a transparent, black pixel (0,0,0,0)
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
                    // print byte from glyph
                    atlas_buffer[atlas_buffer_index] = glyph_tab.buffer[&glyph_index].data[byte_order_in_glyph as usize];
                    atlas_buffer_index += 1;
                    atlas_buffer[atlas_buffer_index] = glyph_tab.buffer[&glyph_index].data[byte_order_in_glyph as usize];
                    atlas_buffer_index += 1;
                    atlas_buffer[atlas_buffer_index] = glyph_tab.buffer[&glyph_index].data[byte_order_in_glyph as usize];
                    atlas_buffer_index += 1;
                    atlas_buffer[atlas_buffer_index] = glyph_tab.buffer[&glyph_index].data[byte_order_in_glyph as usize];
                    atlas_buffer_index += 1;
                }
                // write black in non-graphical ASCII boxes
            } else {
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

fn create_bitmap_atlas(
    face: freetype::face::Face, spec: AtlasSpec) -> Result<BitmapAtlas, SampleTypefaceError> {

    let glyph_tab = match sample_typeface(face, spec) {
        Ok(val) => val,
        Err(e) => return Err(e),
    };
    let metadata = create_bitmap_metadata(&glyph_tab, spec);
    let atlas_buffer = create_bitmap_buffer(&glyph_tab, spec);

    Ok(BitmapAtlas {
        dimensions_px: spec.dimensions_px,
        columns: spec.columns,
        padding_px: spec.padding_px,
        slot_glyph_size: spec.slot_glyph_size,
        glyph_px: spec.glyph_px,
        metadata: metadata,
        buffer: atlas_buffer,
    })
}

fn write_metadata<P: AsRef<Path>>(atlas: &BitmapAtlas, path: P) -> io::Result<()> {
    // write meta-data file to go with atlas image
    let mut file = match File::create(path) {
        Ok(val) => val,
        Err(e) => return Err(e),
    };

    // comment, reminding me what each column is
    writeln!(file, "// ascii_code prop_xMin prop_width prop_yMin prop_height prop_y_offset").unwrap();
    // write a line for each regular character
    for glyph in atlas.metadata.values() {
        writeln!(
            file, "{} {} {} {} {} {}",
            glyph.code_point, glyph.x_min,
            glyph.width, glyph.y_min, glyph.height, glyph.y_offset
        ).unwrap();
    }

    Ok(())
}

fn write_atlas_buffer<P: AsRef<Path>>(atlas: &BitmapAtlas, path: P) -> io::Result<()> {
    image::save_buffer(
        path, &atlas.buffer,
        atlas.dimensions_px as u32, atlas.dimensions_px as u32, image::RGBA(8)
    )
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
    OutputPathIsNotAFile(PathBuf),
    SlotGlyphSizeCannotBeZero(usize),
    PaddingLargerThanSlotGlyphSize(usize, usize),
}

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
    if !opt.output_path.is_file() {
        return Err(OptError::OutputPathIsNotAFile(opt.output_path.clone()));
    }
    if !(opt.slot_glyph_size > 0) {
        return Err(OptError::SlotGlyphSizeCannotBeZero(opt.slot_glyph_size));
    }
    if opt.padding > opt.slot_glyph_size {
        return Err(OptError::PaddingLargerThanSlotGlyphSize(opt.padding, opt.slot_glyph_size));
    }

    Ok(())
}

fn run_app(opt: &Opt) -> Result<(), String> {
    let ft = Library::init().expect("Failed to initialize FreeType library.");
    let face = match ft.new_face(&opt.input_path, 0) {
        Ok(val) => val,
        Err(_) => {
            return Err(format!("Could not open font file: {}.", &opt.input_path.display()));
        }
    };

    let slot_glyph_size = opt.slot_glyph_size;         // glyph maximum size in pixels
    let atlas_columns = 16;                            // number of glyphs across atlas
    let atlas_dimensions_px = slot_glyph_size * atlas_columns;       // atlas size in pixels
    let padding_px = opt.padding;                      // total space in glyph size for outlines
    let atlas_glyph_px = slot_glyph_size - padding_px; // leave some padding for outlines
    let mut atlas_meta_file = opt.output_path.clone();
    atlas_meta_file.set_extension("meta");
    let mut atlas_image_file = opt.output_path.clone();
    atlas_image_file.set_extension("png");

    let atlas_spec = AtlasSpec::new(
        atlas_dimensions_px, atlas_columns, padding_px, slot_glyph_size, atlas_glyph_px
    );
    let atlas = match create_bitmap_atlas(face, atlas_spec) {
        Ok(val) => val,
        Err(e) => {
            return Err(format!("Could create bitmap font. Got error: {}", e));
        }
    };

    if write_metadata(&atlas, &atlas_meta_file).is_err() {
        return Err(format!(
            "Could not create atlas metadata file: {}.", atlas_meta_file.display()
        ));
    }

    if write_atlas_buffer(&atlas, &atlas_image_file).is_err() {
        return Err(format!(
            "Could not create atlas font sheet file: {}.", atlas_image_file.display()
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
