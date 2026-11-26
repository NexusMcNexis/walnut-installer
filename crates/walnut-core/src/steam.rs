//! Adding a game to the Steam library as a non-Steam shortcut, with its
//! artwork, and taking it out again.
//!
//! Steam has no command to register a non-Steam game (`steam://addnonsteamgame`
//! only opens a dialog), so this edits the most recently signed-in account's
//! `userdata/<id>/config/shortcuts.vdf` and copies the art into
//! `config/grid/`, with Steam closed. The file is backed up first, only our
//! entry is ever changed, and the art is named by the same app id the entry
//! holds.

use crate::vdf::{self, Shortcut};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Steam's 64-bit account ids start here; `userdata/` folders use the rest.
pub const ID_BASE: u64 = 76_561_197_960_265_728;

/// The grid files Steam reads, by the art they show: (suffix after the app id).
pub const ART: [(&str, &str); 5] = [("portrait", "p.png"), ("wide", ".png"), ("hero", "_hero.png"), ("logo", "_logo.png"), ("icon", "_icon.png")];

/// Where Steam keeps its files for `home`, if it is installed there: the
/// native folders first, then the Flatpak one.
pub fn find_root(home: &Path) -> Option<PathBuf> {
    [".local/share/Steam", ".steam/steam", ".var/app/com.valvesoftware.Steam/.local/share/Steam"]
        .iter()
        .map(|rel| home.join(rel))
        .find(|root| root.join("userdata").is_dir() && root.join("config/loginusers.vdf").is_file())
        .map(|root| fs::canonicalize(&root).unwrap_or(root))
}

/// The `userdata/` folder name of the account `loginusers.vdf` marks
/// `MostRecent` (else the one with the latest `Timestamp`): the person signed
/// in now, who is the one installing.
pub fn most_recent_account(loginusers: &str) -> Option<String> {
    let mut best: Option<(u64, u64)> = None;
    let mut current: Option<u64> = None;
    let (mut recent, mut time) = (false, 0u64);
    for line in loginusers.lines() {
        let quoted: Vec<&str> = line.split('"').skip(1).step_by(2).collect();
        if quoted.len() == 1 && quoted[0].len() == 17 && quoted[0].chars().all(|c| c.is_ascii_digit()) {
            current = quoted[0].parse().ok();
            recent = false;
            time = 0;
        } else if quoted.len() == 2 && current.is_some() {
            match quoted[0].to_ascii_lowercase().as_str() {
                "mostrecent" => recent = quoted[1] == "1",
                "timestamp" => time = quoted[1].parse().unwrap_or(0),
                _ => {}
            }
        } else if line.trim() == "}" {
            if let Some(id) = current.take() {
                let score = time + if recent { 1 << 40 } else { 0 };
                if best.map(|(_, s)| score > s).unwrap_or(true) {
                    best = Some((id, score));
                }
            }
        }
    }
    best.and_then(|(id, _)| id.checked_sub(ID_BASE)).map(|id| id.to_string())
}

/// Whether Steam is running, and asking it to close. The real one looks at
/// the user's own processes; tests use a stand-in, so they never see or
/// touch a real Steam.
pub trait SteamProcess {
    fn running(&self) -> bool;
    fn shutdown(&self);
}

/// The real Steam of the user running the installer.
pub struct RealSteam;

impl SteamProcess for RealSteam {
    fn running(&self) -> bool {
        let Ok(entries) = fs::read_dir("/proc") else { return false };
        let me = uid();
        entries.flatten().any(|entry| {
            let path = entry.path();
            let comm = fs::read_to_string(path.join("comm")).unwrap_or_default();
            comm.trim() == "steam" && owner(&path) == me
        })
    }

    fn shutdown(&self) {
        let _ = std::process::Command::new("steam").arg("-shutdown").status();
    }
}

fn uid() -> Option<u32> {
    fs::read_to_string("/proc/self/status").ok().and_then(|s| status_uid(&s))
}

fn owner(proc_dir: &Path) -> Option<u32> {
    fs::read_to_string(proc_dir.join("status")).ok().and_then(|s| status_uid(&s))
}

fn status_uid(status: &str) -> Option<u32> {
    status.lines().find(|l| l.starts_with("Uid:"))?.split_whitespace().nth(1)?.parse().ok()
}

/// What adding put where, for the uninstaller.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Added {
    pub shortcuts: PathBuf,
    pub backup: PathBuf,
    pub exe: String,
    pub app_id: u32,
    pub art: Vec<PathBuf>,
    /// Whether shortcuts.vdf existed before (if not, removing ours deletes it).
    pub existed: bool,
}

/// Adds `ours` for the account `account` under `root`, with the art in
/// `art` (name -> PNG on disk, names as in [`ART`]). Backs up the file
/// first; refreshes ours rather than adding a second.
pub fn add(root: &Path, account: &str, ours: &Shortcut, art: &[(String, PathBuf)]) -> io::Result<Added> {
    let config = root.join("userdata").join(account).join("config");
    fs::create_dir_all(config.join("grid"))?;
    let file = config.join("shortcuts.vdf");
    let existed = file.is_file();
    let before = if existed { fs::read(&file)? } else { Vec::new() };
    let after = vdf::with_ours(&before, ours).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    let backup = config.join("shortcuts.vdf.walnut-backup");
    if existed {
        fs::write(&backup, &before)?;
    }
    write_whole(&file, &after)?;
    let id = vdf::app_id(&ours.exe, &ours.name);
    let mut placed = Vec::new();
    for (name, source) in art {
        if let Some((_, suffix)) = ART.iter().find(|(slot, _)| slot == name) {
            let target = config.join("grid").join(format!("{id}{suffix}"));
            fs::copy(source, &target)?;
            placed.push(target);
        }
    }
    Ok(Added { shortcuts: file, backup, exe: ours.exe.clone(), app_id: id, art: placed, existed })
}

/// Takes ours out again: only our entry and only our art.
pub fn remove(added: &Added) -> io::Result<()> {
    if added.shortcuts.is_file() {
        let before = fs::read(&added.shortcuts)?;
        let after = vdf::without_ours(&before, &added.exe).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        if !added.existed && vdf::parse(&after).map(|e| e.is_empty()).unwrap_or(false) {
            fs::remove_file(&added.shortcuts)?;
        } else {
            write_whole(&added.shortcuts, &after)?;
        }
    }
    for art in &added.art {
        let _ = fs::remove_file(art);
    }
    Ok(())
}

/// Writes `data` to a temporary file beside `path`, then renames it over
/// `path`, so a crash never leaves Steam a half-written library.
fn write_whole(path: &Path, data: &[u8]) -> io::Result<()> {
    let temp = path.with_extension("vdf.walnut-tmp");
    fs::write(&temp, data)?;
    fs::rename(&temp, path)
}

/// `path` quoted as Steam keeps an executable or folder.
pub fn quoted(path: &Path) -> String {
    format!("\"{}\"", path.display())
}
