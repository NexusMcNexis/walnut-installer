//! The game's place in the desktop: a freedesktop `.desktop` entry, its icon
//! in the hicolor theme, and an optional shortcut on the desktop itself.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The `.desktop` text for a game. `name` never carries the version; the
/// version is kept in `X-Walnut-Version`.
pub fn entry_text(name: &str, id: &str, exec: &Path, workdir: &Path, comment: &str, categories: &[String], version: &str) -> String {
    let mut cats = categories.join(";");
    if !cats.is_empty() {
        cats.push(';');
    }
    format!(
        "[Desktop Entry]\nType=Application\nName={name}\nComment={comment}\nExec={exec}\nPath={path}\nIcon={id}\nTerminal=false\nCategories={cats}\nX-Walnut-Version={version}\n",
        name = escape(name),
        comment = escape(comment),
        exec = exec_quote(exec),
        path = workdir.display(),
    )
}

/// The width of a PNG, from its header (no image library needed).
pub fn png_size(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() < 24 || &data[0..8] != b"\x89PNG\r\n\x1a\n" || &data[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(data[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(data[20..24].try_into().ok()?);
    Some((width, height))
}

/// Where an icon of `size` pixels goes in the hicolor theme under `data_home`.
pub fn icon_path(data_home: &Path, id: &str, size: u32) -> PathBuf {
    data_home.join("icons/hicolor").join(format!("{size}x{size}")).join("apps").join(format!("{id}.png"))
}

/// The user's desktop folder: `XDG_DESKTOP_DIR` from user-dirs.dirs, else `~/Desktop`.
pub fn desktop_dir(home: &Path, config_home: &Path) -> PathBuf {
    let dirs = fs::read_to_string(config_home.join("user-dirs.dirs")).unwrap_or_default();
    for line in dirs.lines() {
        if let Some(value) = line.strip_prefix("XDG_DESKTOP_DIR=") {
            let value = value.trim_matches('"').replace("$HOME", &home.display().to_string());
            return PathBuf::from(value);
        }
    }
    home.join("Desktop")
}

/// Writes a desktop file executable, and on GNOME-like desktops marks it
/// trusted so it opens on double-click (best effort; `gio` may be absent).
pub fn write_launcher(path: &Path, text: &str, mark_trusted: bool) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    // M2: trusted the way this desktop wants (the executable bit is set above
    // for all of them).
    if mark_trusted && trust_for(&current_desktop(), &distro_id(&fs::read_to_string("/etc/os-release").unwrap_or_default())) == Trust::GioMetadata {
        let _ = std::process::Command::new("gio").args(["set"]).arg(path).args(["metadata::trusted", "true"]).status();
    }
    Ok(())
}

/// M2: how a desktop shortcut is made trusted so it opens on double-click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trust {
    /// GNOME and its family read `metadata::trusted` (set with `gio`).
    GioMetadata,
    /// KDE Plasma (SteamOS's and Bazzite's desktop mode too) and the rest:
    /// the executable bit is enough (Plasma asks once on first open).
    ExecutableBit,
}

/// The quirk for `desktop` (`XDG_CURRENT_DESKTOP`, colon-separated) and
/// `distro` (os-release's ID).
pub fn trust_for(desktop: &str, distro: &str) -> Trust {
    let gnome_like = ["GNOME", "UNITY", "X-CINNAMON", "CINNAMON", "BUDGIE", "PANTHEON", "MATE", "XFCE"];
    let desktop = desktop.to_ascii_uppercase();
    if desktop.split(':').any(|part| part == "KDE") || distro == "steamos" {
        return Trust::ExecutableBit;
    }
    if desktop.split(':').any(|part| gnome_like.contains(&part)) {
        return Trust::GioMetadata;
    }
    Trust::ExecutableBit
}

/// The desktop the player is using, from the XDG variables (for quirks).
pub fn current_desktop() -> String {
    std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default()
}

/// The distribution's id from os-release (for quirks), e.g. "steamos".
pub fn distro_id(os_release: &str) -> String {
    os_release
        .lines()
        .find_map(|l| l.strip_prefix("ID="))
        .map(|v| v.trim_matches('"').to_string())
        .unwrap_or_default()
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\n', "\\n")
}

/// An Exec value: quoted, with the characters the spec reserves escaped.
fn exec_quote(path: &Path) -> String {
    let text = path.display().to_string();
    let escaped: String = text.chars().flat_map(|c| match c {
        '"' | '`' | '$' | '\\' => vec!['\\', c],
        _ => vec![c],
    }).collect();
    format!("\"{escaped}\"")
}
