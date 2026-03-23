use demo_presets::{chromatic_colors, text_on_color};
use palette_core::color::Color;
use palette_core::registry::Registry;

#[test]
fn text_on_white_is_black() {
    let white = Color::from_hex("#ffffff").unwrap();
    assert_eq!(text_on_color(&white), "#000");
}

#[test]
fn text_on_black_is_white() {
    let black = Color::from_hex("#000000").unwrap();
    assert_eq!(text_on_color(&black), "#fff");
}

#[test]
fn chromatic_colors_has_twelve_distinct() {
    let registry = Registry::new();
    let info = registry.list().next().unwrap();
    let palette = registry.load(&info.id).unwrap();
    let resolved = palette.resolve();
    let colors = chromatic_colors(&resolved);
    assert_eq!(colors.len(), 12);
    // No two adjacent chromatic colors should be identical
    for pair in colors.windows(2) {
        assert_ne!(pair[0], pair[1]);
    }
}
