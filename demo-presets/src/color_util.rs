use palette_core::color::Color;
use palette_core::resolved::ResolvedPalette;

const CHROMATIC_SLOTS: &[&str] = &[
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "bright_red",
    "bright_green",
    "bright_yellow",
    "bright_blue",
    "bright_magenta",
    "bright_cyan",
];

/// The 12 chromatic ANSI colors from a resolved palette, skipping black/white variants.
pub fn chromatic_colors(resolved: &ResolvedPalette) -> [&Color; 12] {
    let mut out: [&Color; 12] = [&Color { r: 0, g: 0, b: 0 }; 12];
    for (slot_name, color) in resolved.terminal.all_slots() {
        let pos = CHROMATIC_SLOTS.iter().position(|&s| s == slot_name);
        if let Some(i) = pos {
            out[i] = color;
        }
    }
    out
}

fn is_light(bg: &Color) -> bool {
    bg.relative_luminance() > 0.179
}

pub fn text_on_color(color: &Color) -> &'static str {
    match is_light(color) {
        true => "#000",
        false => "#fff",
    }
}
