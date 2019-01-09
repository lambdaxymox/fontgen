# Run `fontgen` on the FontMono GNU font in the assets directory with 
# a slot padding of 6 pixels and a slot glyph size of 128 pixels.
cargo run  -- --input "assets/FontMono.ttf" --output "FontMono.ttf" --padding 6 --slot-glyph-size 128
