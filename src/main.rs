extern crate freetype;
extern crate image;

use freetype::Library;
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::Write;
use std::mem;
use std::path::Path;
use std::process;


const FONT_FILE: &str = "assets/FreeMono.ttf";
const PNG_OUTPUT_IMAGE: &str = "atlas.png";
const ATLAS_META_FILE: &str = "atlas.meta";



#[derive(Copy, Clone)]
struct AtlasSpec {
    dimensions_px: usize,   // atlas size in pixels
    columns: usize,         // number of glyphs across atlas
    padding_px: usize,      // total space in glyph size for outlines
    slot_glyph_size: usize, // glyph maximum size in pixels
    glyph_px: usize,        // leave some padding for outlines
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

#[derive(Clone)]
enum GlyphSlot {
    Occupied(GlyphImage),
    Unoccupied,
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
    metadata: HashMap<usize, GlyphMetadata>,
    glyphs: HashMap<usize, GlyphImage>,
    buffer: Vec<u8>,
}

fn sample_typeface(face: freetype::face::Face, spec: AtlasSpec) -> BitmapAtlas {
    // Tell FreeType the maximum size of each glyph, in pixels.
    let mut grows = vec![0 as i32; 256];   // glyph height in pixels
    let mut gwidth = vec![0 as i32; 256];  // glyph width in pixels
    let mut gpitch = vec![0 as i32; 256];  // bytes per row of pixels
    let mut gymin = vec![0 as i64; 256];   // offset for letters that dip below baseline like g and y
    let mut glyph_buffer = HashMap::new(); // stored glyph images

    // set height in pixels width 0 height 48 (48x48)
    match face.set_pixel_sizes(0, spec.glyph_px as u32) {
        Ok(_) => {}
        Err(_) => {
            eprintln!("Could not set pixel size for font");
            panic!(); // process::exit(1);
        }
    };

    for i in 33..256 {
        if face.load_char(i, freetype::face::LoadFlag::RENDER).is_err() {
            eprintln!("Could not load character {:x}", i);
            panic!(); // process::exit(1);
        }

        // draw glyph image anti-aliased
        let glyph_handle = face.glyph();
        if glyph_handle.render_glyph(freetype::render_mode::RenderMode::Normal).is_err() {
            eprintln!("Could not render character {:x}", i);
            panic!(); // process::exit(1);
        }

        // get dimensions of bitmap
        grows[i] = glyph_handle.bitmap().rows();
        gwidth[i] = glyph_handle.bitmap().width();
        gpitch[i] = glyph_handle.bitmap().pitch();

        // copy glyph data into memory because it seems to be overwritten/lost later
        let glyph_image_i = create_glyph_image(glyph_handle);
        glyph_buffer.insert(i, glyph_image_i);

        // get y-offset to place glyphs on baseline. this is in the bounding box
        let glyph = match glyph_handle.get_glyph() {
            Ok(val) => val,
            Err(_) => {
                eprintln!("Could not get glyph handle {}", i);
                panic!(); //process::exit(1);
            }
        };


        // get bbox. "truncated" mode means get dimensions in pixels
        let bbox = glyph.get_cbox(freetype::ffi::FT_GLYPH_BBOX_TRUNCATE);
        gymin[i] = bbox.yMin;
    }

    let mut metadata = HashMap::new();
    let glyph_metadata_space = GlyphMetadata::new(32, 0.0, 0.5, 0.0, 1.0, 0.0);
    metadata.insert(32, glyph_metadata_space);
    for i in 33..256 {
        let order = i - 32;
        let col = order % spec.columns;
        let row = order % spec.columns;

        // Glyph metadata parameters.
        let x_min = (col * spec.slot_glyph_size) as f32 / spec.dimensions_px as f32;
        let y_min = (row * spec.slot_glyph_size) as f32 / spec.dimensions_px as f32;
        let width = (gwidth[i] + spec.padding_px as i32) as f32 / spec.slot_glyph_size as f32;
        let height = (grows[i] + spec.padding_px as i32) as f32 / spec.slot_glyph_size as f32;
        let y_offset = -(spec.padding_px as f32 - gymin[i] as f32) / spec.slot_glyph_size as f32;

        let glyph_metadata_i = GlyphMetadata::new(i, width, height, x_min, y_min, y_offset);
        metadata.insert(i, glyph_metadata_i);
    }

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
                if x_loc < 0 || y_loc < 0 || x_loc >= gwidth[glyph_index] ||
                    y_loc >= grows[glyph_index] {
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
                    let byte_order_in_glyph = y_loc * gwidth[glyph_index] + x_loc;
                    let mut colour = [0 as u8; 4];
                    colour[0] = glyph_buffer[&glyph_index].data[byte_order_in_glyph as usize];
                    colour[1] = colour[0];
                    colour[2] = colour[0];
                    colour[3] = colour[0];
                    // print byte from glyph
                    atlas_buffer[atlas_buffer_index] = glyph_buffer[&glyph_index].data[byte_order_in_glyph as usize];
                    atlas_buffer_index += 1;
                    atlas_buffer[atlas_buffer_index] = glyph_buffer[&glyph_index].data[byte_order_in_glyph as usize];
                    atlas_buffer_index += 1;
                    atlas_buffer[atlas_buffer_index] = glyph_buffer[&glyph_index].data[byte_order_in_glyph as usize];
                    atlas_buffer_index += 1;
                    atlas_buffer[atlas_buffer_index] = glyph_buffer[&glyph_index].data[byte_order_in_glyph as usize];
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

    BitmapAtlas {
        metadata: metadata,
        glyphs: glyph_buffer,
        buffer: atlas_buffer,
    }
}

fn create_glyph_image(glyph: &freetype::glyph_slot::GlyphSlot) -> GlyphImage {
    let bitmap = glyph.bitmap();
    let rows = bitmap.rows() as usize;
    let pitch = bitmap.pitch() as usize;
    let mut glyph_data = vec![0 as u8; rows * pitch];
    glyph_data.clone_from_slice(bitmap.buffer());

    GlyphImage::new(glyph_data)
}

fn write_metadata(metadata: &HashMap<usize, GlyphMetadata>, path: &Path) -> io::Result<()> {
    // write meta-data file to go with atlas image
    let mut file = match File::create(path) {
        Ok(val) => val,
        Err(e) => return Err(e),
    };

    // comment, reminding me what each column is
    writeln!(file, "// ascii_code prop_xMin prop_width prop_yMin prop_height prop_y_offset").unwrap();
    // write a line for each regular character
    for glyph in metadata.values() {
        writeln!(
            file, "{} {} {} {} {} {}",
            glyph.code_point, glyph.x_min,
            glyph.width, glyph.y_min, glyph.height, glyph.y_offset
        ).unwrap();
    }

    Ok(())
}

fn main() {
    let ft = match Library::init() {
        Ok(val) => val,
        Err(_) => {
            eprintln!("Failed to initialize FreeType library.");
            panic!(); // process::exit(1);
        }
    };

    let face = match ft.new_face(FONT_FILE, 0) {
        Ok(val) => val,
        Err(_) => {
            eprintln!("Could not open font file.");
            panic!(); // process::exit(1);
        }
    };

    let atlas_dimensions_px = 1024;       // atlas size in pixels
    let atlas_columns = 16;               // number of glyphs across atlas
    let padding_px = 6;                   // total space in glyph size for outlines
    let slot_glyph_size = 64;             // glyph maximum size in pixels
    let atlas_glyph_px = 64 - padding_px; // leave some padding for outlines

    // *******************************************************************************
    // BEGIN BITMAP FONT ATLAS
    // *******************************************************************************
    let atlas_spec = AtlasSpec::new(
        atlas_dimensions_px, atlas_columns, padding_px, slot_glyph_size, atlas_glyph_px
    );
    let atlas = sample_typeface(face, atlas_spec);
    // ********************************************************************************
    // END BITMAP FONT ATLAS
    // ********************************************************************************

    let path = Path::new(ATLAS_META_FILE);
    match write_metadata(&atlas.metadata, path) {
        Err(_) => {
            eprintln!("Failed to create atlas metadata file {}", ATLAS_META_FILE);
            panic!(); // process::exit(1);
        }
        _ => {}
    }

    // Write out the image.
    // use stb_image_write to write directly to png
    if image::save_buffer(
        PNG_OUTPUT_IMAGE, &atlas.buffer,
        atlas_dimensions_px as u32, atlas_dimensions_px as u32, image::RGBA(8)).is_err() {

        eprintln!("ERROR: Could not write file {}", PNG_OUTPUT_IMAGE);
        panic!(); // process::exit(1);
    }
    // End write out the image.
}

