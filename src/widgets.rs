//! Small presentational components. Each one renders a value; none of them
//! own state or talk to the device, so they are safe to reuse and easy to read.

use crate::theme;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

/// Horizontal LED meter with a held peak marker, in the style of a hardware
/// level meter: lit segments coloured by where they sit on the scale, unlit
/// segments left as faint ticks.
pub fn led_meter(width: u16, level: f64, peak: f64, muted: bool) -> Line<'static> {
    let w = width.max(1) as usize;
    let lit = ((level.clamp(0.0, 1.0)) * w as f64).round() as usize;
    let peak_col = ((peak.clamp(0.0, 1.0)) * (w.saturating_sub(1)) as f64).round() as usize;

    let mut spans = Vec::with_capacity(w);
    for i in 0..w {
        let frac = i as f64 / w as f64;
        if muted {
            spans.push(Span::styled("╌", Style::default().fg(theme::FAINT)));
        } else if i == peak_col && peak > 0.02 {
            spans.push(Span::styled(
                "┃",
                Style::default()
                    .fg(theme::TEXT)
                    .add_modifier(Modifier::BOLD),
            ));
        } else if i < lit {
            spans.push(Span::styled("█", Style::default().fg(theme::zone(frac))));
        } else {
            spans.push(Span::styled("╌", Style::default().fg(theme::FAINT)));
        }
    }
    Line::from(spans)
}

/// Ruler under the meter. Labels are placed at their true positions rather
/// than spaced evenly, so they stay honest if the width changes.
pub fn meter_scale(width: u16) -> Line<'static> {
    let w = width.max(1) as usize;
    let marks: [(f64, &str); 6] = [
        (0.0, "0"),
        (0.2, "20"),
        (0.4, "40"),
        (0.6, "60"),
        (0.85, "85"),
        (1.0, "100"),
    ];
    let mut row = vec![' '; w];
    for (frac, label) in marks {
        let centre = (frac * (w.saturating_sub(1)) as f64).round() as usize;
        let start = centre.saturating_sub(label.len() / 2).min(w.saturating_sub(label.len()));
        for (k, ch) in label.chars().enumerate() {
            if start + k < w {
                row[start + k] = ch;
            }
        }
    }
    Line::from(Span::styled(
        row.into_iter().collect::<String>(),
        Style::default().fg(theme::FAINT),
    ))
}

/// Slider track with a grab handle. `hot` brightens it while hovered or dragged.
pub fn slider(width: u16, ratio: f64, accent: ratatui::style::Color, hot: bool) -> Line<'static> {
    let w = width.max(1) as usize;
    let pos = ((ratio.clamp(0.0, 1.0) * w.saturating_sub(1) as f64).round() as usize).min(w - 1);
    let mut spans = Vec::new();
    if pos > 0 {
        spans.push(Span::styled("━".repeat(pos), Style::default().fg(accent)));
    }
    spans.push(Span::styled(
        "⬤",
        Style::default()
            .fg(if hot { theme::TEXT } else { accent })
            .add_modifier(Modifier::BOLD),
    ));
    if pos + 1 < w {
        spans.push(Span::styled(
            "━".repeat(w - pos - 1),
            Style::default().fg(theme::LINE),
        ));
    }
    Line::from(spans)
}

/// A key hint rendered as a small keycap, for the status bar.
pub fn keycap(key: &str, what: &str) -> Vec<Span<'static>> {
    vec![
        Span::styled(
            format!(" {key} "),
            Style::default().bg(theme::LINE).fg(theme::TEXT),
        ),
        Span::styled(format!(" {what}   "), Style::default().fg(theme::MUTE_TEXT)),
    ]
}

/// Column graph with eighth-block resolution and per-column colouring.
///
/// `Sparkline` draws every bar in one colour, which flattens a level history
/// into a green slab and hides the part that matters — where it got loud.
/// Grading each column by its own height makes peaks read at a glance, and
/// eighth blocks give eight times the vertical resolution of whole cells.
pub fn bar_graph(width: u16, height: u16, data: &[u64], muted: bool) -> Vec<Line<'static>> {
    const EIGHTHS: [&str; 9] = [" ", "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
    let (w, h) = (width.max(1) as usize, height.max(1) as usize);

    // right-align: the newest sample sits at the right edge
    let start = data.len().saturating_sub(w);
    let recent = &data[start..];
    let pad = w.saturating_sub(recent.len());

    let mut rows = Vec::with_capacity(h);
    for row in 0..h {
        let mut spans = Vec::with_capacity(w);
        for col in 0..w {
            if col < pad {
                spans.push(Span::raw(" "));
                continue;
            }
            let v = recent[col - pad].min(100) as f64 / 100.0;
            let total = (v * (h * 8) as f64).round() as usize;
            // eighths belonging to this row, counting up from the bottom
            let below = (h - 1 - row) * 8;
            let here = total.saturating_sub(below).min(8);
            if here == 0 {
                // faint guides at the hot and clipping thresholds, so the graph
                // can be read without counting rows
                let row_top = (h - row) as f64 / h as f64;
                let guide = (0.60, 0.85);
                let on_guide = (row_top - guide.0).abs() < 0.5 / h as f64
                    || (row_top - guide.1).abs() < 0.5 / h as f64;
                spans.push(if on_guide && col % 3 == 0 {
                    Span::styled("╌", Style::default().fg(theme::LINE))
                } else {
                    Span::raw(" ")
                });
                continue;
            }
            let colour = if muted { theme::FAINT } else { theme::zone(v) };
            spans.push(Span::styled(EIGHTHS[here], Style::default().fg(colour)));
        }
        rows.push(Line::from(spans));
    }
    rows
}
