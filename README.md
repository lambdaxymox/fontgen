# Bitmapped Image Font Sheet Generator

## Introduction
The program `fontgen` is a shell utility for converting a TrueType or OpenType file into a bitmapped atlas file. 
See [https://github.com/lambdaxymox/bmfa](repo) for details. The primary use case for for this program is for 
generating fonts for use in game development.

## Usage
The primary input usage for `fontgen` has the form
```bash
fontgen --input <input_path> --output <output_path> --padding <padding> --slot-glyph-size <slot_glyph_size>
```
where `--input` denotes the input font file to be converted to a bitmapped font sheet, `--output` is the name
of the output `png` image, `--slot-glyph-size` is the desired maximum size of each glyph in the final output image,
and `--padding` denotes the amount of pixels of padding you want to place each glyph from the boundaries of the glyph slot.
Padding out the glyph slots is handy if you want to add some outlines to the font glyphs in some kind of post-processing 
in your image editor, for example.

## Installation
Fork this repository and enter
```bash
cargo install
```
to install the program.

## Dependencies
The main dependency is the [https://github.com/lambdaxymox/bmfa](bmfa) file format for bitmapped font atlases. 
