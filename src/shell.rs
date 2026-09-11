//! Installs the Omarchy shell bar widget.
//!
//! The Omarchy shell only discovers plugins in two places: its own package
//! directory, and `~/.config/omarchy/plugins/<id>/`. A distro package cannot
//! write to either at build time, so the QML ships as package data and gets
//! copied into the user's config on demand.

use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

/// Must match the `id` in `shell/manifest.json` - it names the install
/// directory the shell scans.
const PLUGIN_ID: &str = "maono";

/// Files that make up the plugin. Kept explicit so a stray file in the source
/// directory never lands in the user's config.
const FILES: [&str; 2] = ["manifest.json", "Panel.qml"];

fn missing(msg: impl Into<String>) -> io::Error {
    io::Error::new(ErrorKind::NotFound, msg.into())
}

fn config_dir() -> io::Result<PathBuf> {
    if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME").filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    let home = std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .ok_or_else(|| missing("neither XDG_CONFIG_HOME nor HOME is set"))?;
    Ok(PathBuf::from(home).join(".config"))
}

fn install_dir() -> io::Result<PathBuf> {
    Ok(config_dir()?.join("omarchy").join("plugins").join(PLUGIN_ID))
}

/// Where the packaged QML lives. Checked in order so an explicit override wins,
/// then a system install, then a binary run straight out of the source tree.
fn source_dir() -> io::Result<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Some(dir) = std::env::var_os("MAONO_SHELL_DIR").filter(|d| !d.is_empty()) {
        candidates.push(PathBuf::from(dir));
    }
    candidates.push(PathBuf::from("/usr/share/maono/shell"));
    candidates.push(PathBuf::from("/usr/local/share/maono/shell"));

    // A relocatable install: <prefix>/bin/maono -> <prefix>/share/maono/shell.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(prefix) = exe.parent().and_then(Path::parent) {
            candidates.push(prefix.join("share").join("maono").join("shell"));
        }
    }

    // Running from a cargo build in the source tree.
    candidates.push(PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/shell")));

    candidates
        .into_iter()
        .find(|dir| dir.join("manifest.json").is_file())
        .ok_or_else(|| {
            missing("could not find the bundled shell plugin; set MAONO_SHELL_DIR to its directory")
        })
}

pub fn install(force: bool) -> io::Result<()> {
    let source = source_dir()?;
    let target = install_dir()?;

    if target.exists() && !force {
        return Err(io::Error::new(
            ErrorKind::AlreadyExists,
            format!("{} already exists; pass --force to overwrite", target.display()),
        ));
    }

    std::fs::create_dir_all(&target)?;
    for name in FILES {
        let from = source.join(name);
        if !from.is_file() {
            return Err(missing(format!("missing {name} in {}", source.display())));
        }
        std::fs::copy(&from, target.join(name))?;
    }

    println!("installed {PLUGIN_ID} to {}", target.display());
    println!();
    println!("Enable it with:");
    println!("  omarchy-shell shell rescanPlugins");
    println!("  omarchy plugin enable {PLUGIN_ID}");
    Ok(())
}

pub fn uninstall() -> io::Result<()> {
    let target = install_dir()?;
    if !target.exists() {
        return Err(missing(format!("{PLUGIN_ID} is not installed")));
    }
    std::fs::remove_dir_all(&target)?;
    println!("removed {}", target.display());
    println!();
    println!("Drop it from the bar with:");
    println!("  omarchy plugin disable {PLUGIN_ID}");
    println!("  omarchy-shell shell rescanPlugins");
    Ok(())
}
