//! Installing from an installer file, and uninstalling from its receipt.
//!
//! Everything the install makes is written into a receipt
//! (`<install dir>/.walnut/receipt.toml`), and the uninstaller removes
//! exactly that: the game's files, the desktop entry and icons, the desktop
//! shortcut, and only our Steam entry and art. Nothing else is touched.

use crate::payload;
use crate::project::Project;
use crate::steam::{self, Added, SteamProcess};
use crate::{desktop, vdf};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Where the player's files live. Tests give a temporary HOME, so they
/// never touch a real one.
pub struct Env<'a> {
    pub home: PathBuf,
    pub data_home: PathBuf,
    pub config_home: PathBuf,
    pub steam: &'a dyn SteamProcess,
    /// Steam is running: may it be closed? (Never closed without a yes.)
    pub may_close_steam: &'a dyn Fn() -> bool,
    /// Mark the desktop shortcut trusted (GNOME); off in tests.
    pub mark_trusted: bool,
    /// How long to wait for Steam to close.
    pub steam_wait: Duration,
}

impl<'a> Env<'a> {
    /// The real player's environment, from HOME and the XDG variables.
    pub fn from_system(steam: &'a dyn SteamProcess, may_close_steam: &'a dyn Fn() -> bool) -> Env<'a> {
        let home = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()));
        let data_home = std::env::var("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|_| home.join(".local/share"));
        let config_home = std::env::var("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|_| home.join(".config"));
        Env { home, data_home, config_home, steam, may_close_steam, mark_trusted: true, steam_wait: Duration::from_secs(30) }
    }
}

/// What the player chose.
#[derive(Debug, Clone)]
pub struct Choices {
    pub dir: PathBuf,
    pub desktop_entry: bool,
    pub desktop_shortcut: bool,
    pub add_to_steam: bool,
}

/// Everything an install made, for the uninstaller.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Receipt {
    pub name: String,
    pub id: String,
    pub version: String,
    pub dir: PathBuf,
    pub files: Vec<PathBuf>,
    pub desktop_entry: Option<PathBuf>,
    pub icons: Vec<PathBuf>,
    pub shortcut: Option<PathBuf>,
    pub steam: Option<Added>,
    /// Why Steam was left out, when it was offered but not added.
    #[serde(default)]
    pub steam_skipped: Option<String>,
}

pub const RECEIPT: &str = ".walnut/receipt.toml";
pub const UNINSTALLER: &str = ".walnut/uninstall";

/// The project an installer file carries.
pub fn project_of(installer: &Path) -> Result<Project, String> {
    let layout = payload::layout(installer).map_err(|e| e.to_string())?;
    payload::verify(installer, &layout).map_err(|e| e.to_string())?;
    Project::from_toml(&payload::read_manifest(installer, &layout).map_err(|e| e.to_string())?)
}

/// Whether "Add to Steam" can be offered here (Steam with a signed-in account).
pub fn steam_available(env: &Env) -> bool {
    steam::find_root(&env.home)
        .and_then(|root| fs::read_to_string(root.join("config/loginusers.vdf")).ok())
        .and_then(|text| steam::most_recent_account(&text))
        .is_some()
}

/// Installs the game in `installer` as `choices` say.
pub fn install(
    installer: &Path,
    choices: &Choices,
    env: &Env,
    progress: &mut dyn FnMut(u64, u64),
    keep_going: &dyn Fn() -> bool,
) -> Result<Receipt, String> {
    let layout = payload::layout(installer).map_err(|e| e.to_string())?;
    payload::verify(installer, &layout).map_err(|e| e.to_string())?;
    let project = Project::from_toml(&payload::read_manifest(installer, &layout).map_err(|e| e.to_string())?)?;
    let dir = &choices.dir;
    let mut files = payload::unpack(installer, &layout, dir, progress, keep_going).map_err(|e| e.to_string())?;
    let undo = |files: &[PathBuf]| payload::rollback(files);
    // The uninstaller: this installer's program, without the game.
    let program = payload::program_bytes(installer, &layout).map_err(|e| e.to_string())?;
    let uninstaller = dir.join(UNINSTALLER);
    if let Err(e) = write_exec(&uninstaller, &program) {
        undo(&files);
        return Err(e.to_string());
    }
    files.push(uninstaller);
    let game = &project.game;
    let exe = dir.join(&game.executable);
    let mut receipt = Receipt {
        name: game.name.clone(),
        id: game.id.clone(),
        version: game.version.clone(),
        dir: dir.clone(),
        files,
        desktop_entry: None,
        icons: Vec::new(),
        shortcut: None,
        steam: None,
        steam_skipped: None,
    };
    let text = desktop::entry_text(&game.name, &game.id, &exe, dir, &project.desktop.comment, &project.desktop.categories, &game.version);
    let icon = dir.join(".walnut/icon.png");
    if let Ok(bytes) = fs::read(&icon) {
        if let Some((size, _)) = desktop::png_size(&bytes) {
            let path = desktop::icon_path(&env.data_home, &game.id, size);
            if fs::create_dir_all(path.parent().unwrap_or(dir)).is_ok() && fs::write(&path, &bytes).is_ok() {
                receipt.icons.push(path);
            }
        }
    }
    if choices.desktop_entry {
        let path = env.data_home.join("applications").join(format!("{}.desktop", game.id));
        desktop::write_launcher(&path, &text, false).map_err(|e| e.to_string())?;
        receipt.desktop_entry = Some(path);
    }
    if choices.desktop_shortcut {
        let path = desktop::desktop_dir(&env.home, &env.config_home).join(format!("{}.desktop", game.id));
        desktop::write_launcher(&path, &text, env.mark_trusted).map_err(|e| e.to_string())?;
        receipt.shortcut = Some(path);
    }
    if choices.add_to_steam && project.steam.offer {
        match add_to_steam(&project, &exe, dir, env) {
            Ok(added) => receipt.steam = Some(added),
            Err(why) => receipt.steam_skipped = Some(why),
        }
    }
    save_receipt(&receipt)?;
    Ok(receipt)
}

fn add_to_steam(project: &Project, exe: &Path, dir: &Path, env: &Env) -> Result<Added, String> {
    let root = steam::find_root(&env.home).ok_or("Steam isn't installed here")?;
    let logins = fs::read_to_string(root.join("config/loginusers.vdf")).map_err(|_| "no Steam account signed in")?;
    let account = steam::most_recent_account(&logins).ok_or("no Steam account signed in")?;
    steam_closed(env)?;
    let shortcut = vdf::Shortcut {
        name: project.game.name.clone(),
        exe: steam::quoted(exe),
        start_dir: steam::quoted(dir),
        icon: dir.join(".walnut/icon.png").display().to_string(),
        launch_options: project.steam.launch_options.clone(),
        tags: project.steam.tags.clone(),
    };
    let art: Vec<(String, PathBuf)> = steam::ART
        .iter()
        .map(|(slot, _)| (slot.to_string(), dir.join(".walnut/steam").join(format!("{slot}.png"))))
        .filter(|(_, path)| path.is_file())
        .collect();
    steam::add(&root, &account, &shortcut, &art).map_err(|e| format!("Steam's library couldn't be written: {e}"))
}

/// Steam must be closed while its library file is written: asks first,
/// closes it only on a yes, and waits for it to go.
fn steam_closed(env: &Env) -> Result<(), String> {
    if !env.steam.running() {
        return Ok(());
    }
    if !(env.may_close_steam)() {
        return Err("Steam was running; skipped this time".into());
    }
    env.steam.shutdown();
    let step = Duration::from_millis(250);
    let mut waited = Duration::ZERO;
    while env.steam.running() {
        if waited >= env.steam_wait {
            return Err("Steam didn't close; skipped this time".into());
        }
        std::thread::sleep(step);
        waited += step;
    }
    Ok(())
}

/// Removes everything the install at `dir` made, and nothing else.
pub fn uninstall(dir: &Path, env: &Env) -> Result<Receipt, String> {
    let receipt = load_receipt(dir)?;
    if let Some(added) = &receipt.steam {
        steam_closed(env)?;
        steam::remove(added).map_err(|e| e.to_string())?;
    }
    for path in receipt.desktop_entry.iter().chain(receipt.shortcut.iter()).chain(receipt.icons.iter()) {
        let _ = fs::remove_file(path);
    }
    let _ = fs::remove_file(dir.join(RECEIPT));
    payload::rollback(&receipt.files);
    let _ = fs::remove_dir(dir.join(".walnut"));
    let _ = fs::remove_dir(dir);
    Ok(receipt)
}

pub fn load_receipt(dir: &Path) -> Result<Receipt, String> {
    let text = fs::read_to_string(dir.join(RECEIPT)).map_err(|_| format!("no Walnut install in {}", dir.display()))?;
    toml::from_str(&text).map_err(|e| format!("receipt: {e}"))
}

fn save_receipt(receipt: &Receipt) -> Result<(), String> {
    let path = receipt.dir.join(RECEIPT);
    fs::create_dir_all(path.parent().unwrap_or(&receipt.dir)).map_err(|e| e.to_string())?;
    fs::write(&path, toml::to_string_pretty(receipt).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

fn write_exec(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, data)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}
