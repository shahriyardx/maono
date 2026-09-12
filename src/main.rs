//! Control a Maono PD100W wireless microphone from the command line or a TUI.

use maono::mic;
mod theme;
mod widgets;
mod tui;
mod shell;

use mic::{
    light_mode_from_name, light_mode_name, Mic, BATTERY, GAIN, GAIN_MAX, LIGHT, LIGHT_MODE,
    LIGHT_MODE_MAX, MUTE, NR, NR_LEVEL, NR_NAMES,
};
use std::io::Write;
use std::process::ExitCode;

const USAGE: &str = "\
maono - control a Maono PD100W wireless microphone

    maono                     live terminal UI (default)
    maono status [--json]     battery, mute, gain, noise reduction
    maono mute | unmute | toggle
    maono gain [n | +n | -n]  0-20
    maono nr [off | low | mid | high]
    maono light [on | off | next | 0-8 | <colour>]

    maono shell install       add the Omarchy bar widget (--force to replace)
    maono shell uninstall     remove it again

    maono get <id>            read one raw field, e.g. 0x208e
    maono set <id> <value>    write one raw field
    maono scan [lo] [hi]      dump a field range (read-only)

Light colours, in the order the light button cycles them:
white, red, orange, lime, green, cyan, blue, purple, light blue.

Field ids are slot-based: 0x2000 transmitter 1, 0x2800 transmitter 2,
0x3000 receiver. The firmware validates nothing it is sent.
";

/// Accepts `0x208e` or plain decimal.
fn parse_id(s: &str) -> Option<u16> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u16::from_str_radix(hex, 16).ok()
    } else {
        s.parse().ok()
    }
}

struct State {
    battery: Option<u16>,
    muted: Option<bool>,
    gain: Option<u16>,
    nr_on: Option<bool>,
    nr_level: Option<u16>,
    light_on: Option<bool>,
    light_mode: Option<u16>,
}

impl State {
    fn read(m: &mut Mic) -> std::io::Result<Self> {
        Ok(Self {
            battery: m.get(BATTERY)?,
            muted: m.get(MUTE)?.map(|v| v != 0),
            gain: m.get(GAIN)?,
            nr_on: m.get(NR)?.map(|v| v != 0),
            nr_level: m.get(NR_LEVEL)?,
            light_on: m.get(LIGHT)?.map(|v| v != 0),
            light_mode: m.get(LIGHT_MODE)?,
        })
    }

    fn nr_text(&self) -> String {
        match (self.nr_on, self.nr_level) {
            (Some(true), Some(l)) if (l as usize) < NR_NAMES.len() => {
                format!("on, {}", NR_NAMES[l as usize])
            }
            (Some(true), _) => "on".into(),
            (Some(false), _) => "off".into(),
            _ => "?".into(),
        }
    }

    fn print(&self) {
        let show = |v: Option<u16>| v.map_or("?".to_string(), |x| x.to_string());
        println!("  battery         : {}%", show(self.battery));
        println!(
            "  mute            : {}",
            self.muted.map_or("?", |m| if m { "MUTED" } else { "live" })
        );
        println!("  gain            : {} / {GAIN_MAX}", show(self.gain));
        println!("  noise reduction : {}", self.nr_text());
    }

    /// Waybar-style JSON, plus the extra fields the bar panel needs.
    fn print_json(&self) {
        let muted = self.muted.unwrap_or(false);
        // Nerd Font Material Design range, matching omarchy's own bar widgets.
        let icon = if muted { "\u{f036d}" } else { "\u{f036c}" };
        let num = |v: Option<u16>| v.map_or("null".to_string(), |x| x.to_string());
        println!(
            "{{\"text\":\"{icon}\",\"class\":\"{class}\",\"tooltip\":\"{tip}\",\
\"muted\":{muted},\"battery\":{batt},\"gain\":{gain},\"gain_max\":{gm},\
\"nr\":\"{nr}\",\"nr_on\":{nr_on},\"nr_level\":{nr_lvl},\
\"light_on\":{light},\"light_mode\":{mode},\"light_mode_name\":\"{mode_name}\"}}",
            class = if muted { "muted" } else { "live" },
            tip = format!(
                "Mic {} · battery {}% · gain {}/{GAIN_MAX} · NR {}",
                if muted { "muted" } else { "live" },
                self.battery.map_or("?".into(), |b| b.to_string()),
                self.gain.map_or("?".into(), |g| g.to_string()),
                self.nr_text()
            ),
            batt = num(self.battery),
            gain = num(self.gain),
            gm = GAIN_MAX,
            nr = self.nr_text(),
            // Split out as well as the text, so the bar widget does not have
            // to parse "on, mid" back apart.
            nr_on = self.nr_on.unwrap_or(false),
            nr_lvl = num(self.nr_level),
            light = self.light_on.unwrap_or(false),
            mode = num(self.light_mode),
            mode_name = self.light_mode.map_or("?", light_mode_name),
        );
    }
}

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    let _ = writeln!(std::io::stderr(), "maono: {msg}");
    ExitCode::FAILURE
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let json = args.iter().any(|a| a == "--json");
    let positional: Vec<&str> = args
        .iter()
        .filter(|a| !a.starts_with("--"))
        .map(String::as_str)
        .collect();
    // Bare `maono` opens the TUI; everything else is an explicit subcommand.
    let cmd = positional.first().copied().unwrap_or("tui");

    if matches!(cmd, "-h" | "help") || args.iter().any(|a| a == "--help") {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }

    if cmd == "tui" {
        return match tui::run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => fail(e),
        };
    }

    // Copying files around needs no receiver, so this runs before Mic::open.
    if cmd == "shell" {
        let force = args.iter().any(|a| a == "--force");
        let result = match positional.get(1).copied() {
            Some("install") => shell::install(force),
            Some("uninstall") => shell::uninstall(),
            _ => return fail("shell takes install or uninstall"),
        };
        return match result {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => fail(e),
        };
    }

    let mut m = match Mic::open() {
        Ok(m) => m,
        Err(e) => return fail(e),
    };

    let result = (|| -> std::io::Result<ExitCode> {
        match cmd {
            "status" => {
                let s = State::read(&mut m)?;
                if json {
                    s.print_json()
                } else {
                    s.print()
                }
            }
            "mute" | "unmute" | "toggle" => {
                let want = match cmd {
                    "mute" => true,
                    "unmute" => false,
                    _ => !m.get(MUTE)?.map(|v| v != 0).unwrap_or(false),
                };
                let now = m.set_verify(MUTE, want as u16)?;
                println!(
                    "  mute : {}",
                    now.map_or("?", |v| if v != 0 { "MUTED" } else { "live" })
                );
            }
            "gain" => match positional.get(1) {
                None => println!("  gain : {:?} / {GAIN_MAX}", m.get(GAIN)?.unwrap_or(0)),
                Some(arg) => {
                    let cur = m.get(GAIN)?.unwrap_or(0) as i32;
                    let want = if arg.starts_with('+') || arg.starts_with('-') {
                        cur + arg.parse::<i32>().unwrap_or(0)
                    } else {
                        arg.parse::<i32>().unwrap_or(cur)
                    };
                    let want = want.clamp(0, GAIN_MAX as i32) as u16;
                    let now = m.set_verify(GAIN, want)?;
                    println!("  gain : {cur} -> {} / {GAIN_MAX}", now.unwrap_or(want));
                }
            },
            "nr" => match positional.get(1) {
                None => {
                    let s = State::read(&mut m)?;
                    println!("  noise reduction : {}", s.nr_text());
                }
                Some(arg) => {
                    let arg = arg.to_lowercase();
                    if matches!(arg.as_str(), "off" | "0" | "false" | "no") {
                        m.set(NR, 0)?;
                        std::thread::sleep(std::time::Duration::from_millis(300));
                        m.set(NR_LEVEL, 0)?;
                    } else {
                        let level = match arg.as_str() {
                            "low" => 0,
                            "mid" => 1,
                            "high" => 2,
                            "1" => 0,
                            "2" => 1,
                            "3" => 2,
                            "on" => m.get(NR_LEVEL)?.unwrap_or(0),
                            _ => return Ok(fail("nr takes off, low, mid, high, or 1-3")),
                        };
                        m.set(NR, 1)?;
                        std::thread::sleep(std::time::Duration::from_millis(300));
                        m.set(NR_LEVEL, level)?;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(350));
                    let s = State::read(&mut m)?;
                    println!("  noise reduction : {}", s.nr_text());
                }
            },
            "light" => match positional.get(1) {
                None => {
                    let on = m.get(LIGHT)?.unwrap_or(0) != 0;
                    let mode = m.get(LIGHT_MODE)?.unwrap_or(0);
                    println!(
                        "  light : {}  mode {mode} ({})",
                        if on { "on" } else { "off" },
                        light_mode_name(mode)
                    );
                }
                Some(arg) => {
                    let arg = arg.to_lowercase();
                    match arg.as_str() {
                        "on" => {
                            m.set(LIGHT, 1)?;
                        }
                        "off" => {
                            m.set(LIGHT, 0)?;
                        }
                        "next" => {
                            let next = (m.get(LIGHT_MODE)?.unwrap_or(0) + 1) % (LIGHT_MODE_MAX + 1);
                            m.set(LIGHT, 1)?;
                            std::thread::sleep(std::time::Duration::from_millis(150));
                            m.set(LIGHT_MODE, next)?;
                        }
                        // A number, or one of the colour names.
                        _ => match arg
                            .parse::<u16>()
                            .ok()
                            .filter(|n| *n <= LIGHT_MODE_MAX)
                            .or_else(|| light_mode_from_name(&arg))
                        {
                            Some(n) => {
                                m.set(LIGHT, 1)?;
                                std::thread::sleep(std::time::Duration::from_millis(150));
                                m.set(LIGHT_MODE, n)?;
                            }
                            None => {
                                return Ok(fail(format!(
                                    "light takes on, off, next, 0-{LIGHT_MODE_MAX}, or a colour: {}",
                                    mic::LIGHT_MODE_NAMES.join(", ")
                                )))
                            }
                        },
                    }
                    std::thread::sleep(std::time::Duration::from_millis(350));
                    let on = m.get(LIGHT)?.unwrap_or(0) != 0;
                    let mode = m.get(LIGHT_MODE)?.unwrap_or(0);
                    println!(
                        "  light : {}  mode {mode} ({})",
                        if on { "on" } else { "off" },
                        light_mode_name(mode)
                    );
                }
            },
            "get" => {
                let Some(id) = positional.get(1).and_then(|a| parse_id(a)) else {
                    return Ok(fail("usage: maono get <id>"));
                };
                match m.get(id)? {
                    Some(v) => println!("  0x{id:04x} = {v}  (0x{v:04x})"),
                    None => println!("  0x{id:04x} = no answer"),
                }
            }
            "set" => {
                let (id, val) = match (
                    positional.get(1).and_then(|a| parse_id(a)),
                    positional.get(2).and_then(|a| parse_id(a)),
                ) {
                    (Some(i), Some(v)) => (i, v),
                    _ => return Ok(fail("usage: maono set <id> <value>")),
                };
                m.set(id, val)?;
                std::thread::sleep(std::time::Duration::from_millis(350));
                println!("  0x{id:04x} = {:?}", m.get(id)?);
            }
            "scan" => {
                let lo = positional.get(1).and_then(|a| parse_id(a)).unwrap_or(0x2000);
                let hi = positional.get(2).and_then(|a| parse_id(a)).unwrap_or(0x20FF);
                println!("scanning 0x{lo:04x}..0x{hi:04x} (read-only)");
                for id in lo..=hi {
                    if let Some(v) = m.get_quick(id)? {
                        println!("  0x{id:04x} = {v}");
                    }
                }
            }
            other => {
                print!("{USAGE}");
                return Ok(fail(format!("unknown command '{other}'")));
            }
        }
        Ok(ExitCode::SUCCESS)
    })();

    match result {
        Ok(code) => code,
        Err(e) => fail(e),
    }
}
