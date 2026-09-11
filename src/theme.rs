//! One palette for the whole UI, so nothing picks a colour on its own.
//!
//! Raw ANSI names land wherever the terminal theme puts them, which makes a
//! layout look accidental. These are fixed RGB values chosen to sit well on a
//! dark background and to keep the meter's green/amber/red reading obvious.

use ratatui::style::Color;

pub const TEXT: Color = Color::Rgb(192, 202, 245);
pub const MUTE_TEXT: Color = Color::Rgb(120, 130, 170);
pub const FAINT: Color = Color::Rgb(62, 68, 94);
pub const LINE: Color = Color::Rgb(52, 58, 80);
pub const FOCUS: Color = Color::Rgb(122, 162, 247);

pub const GREEN: Color = Color::Rgb(158, 206, 106);
pub const AMBER: Color = Color::Rgb(224, 175, 104);
pub const RED: Color = Color::Rgb(247, 118, 142);
pub const VIOLET: Color = Color::Rgb(187, 154, 247);
pub const CYAN: Color = Color::Rgb(125, 207, 255);
pub const INK: Color = Color::Rgb(26, 27, 38);

/// Meter colour for a position along the scale: the last 15% reads as clipping,
/// the 15% before that as hot.
pub fn zone(fraction: f64) -> Color {
    if fraction >= 0.85 {
        RED
    } else if fraction >= 0.60 {
        AMBER
    } else {
        GREEN
    }
}

pub fn battery(pct: u16) -> Color {
    match pct {
        0..=15 => RED,
        16..=35 => AMBER,
        _ => GREEN,
    }
}
