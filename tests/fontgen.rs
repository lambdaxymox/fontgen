use assert_cmd::prelude::*;
use std::fs;
use std::process::Command;
use std::path::Path;


/// Generate a font sheet from a TrueType font. The font sheet and its
/// corresponding metadata file should appear in the root directory of the source tree.
#[test]
fn generate_a_font_sheet_from_a_ttf_file() -> Result<(), Box<std::error::Error>> {
    let mut cmd = Command::cargo_bin("fontgen")?;
    cmd.arg("--input")
        .arg("assets/FreeMono.ttf")
        .arg("--output")
        .arg("FontMono.png")
        .arg("--padding")
        .arg("6")
        .arg("--slot-glyph-size")
        .arg("128");
    cmd.assert().success();

    let path = Path::new("FontMono.bmfa");

    assert!(path.exists());

    fs::remove_file(path)?;

    Ok(())
}

/// Attempt to generate a font sheet from a file that does not exist.
#[test]
fn generate_a_font_sheet_that_does_not_exist() -> Result<(), Box<std::error::Error>> {
    let mut cmd = Command::cargo_bin("fontgen")?;
    cmd.arg("--input")
        .arg("assets/DoesNotExist.ttf")
        .arg("--output")
        .arg("DoesNotExist.png")
        .arg("--padding")
        .arg("6")
        .arg("--slot-glyph-size")
        .arg("128");
    cmd.assert().failure();

    Ok(())
}

/// The application should reject any padding that's larger than the slot glyph size.
#[test]
fn fontgen_should_reject_padding_larger_than_slot_glyph_size() -> Result<(), Box<std::error::Error>> {
    let mut cmd = Command::cargo_bin("fontgen")?;
    cmd.arg("--input")
        .arg("assets/FreeMono.ttf")
        .arg("--output")
        .arg("FreeMono.bmfa")
        .arg("--padding")
        .arg("129")
        .arg("--slot-glyph-size")
        .arg("128");
    cmd.assert().failure();

    Ok(())
}
