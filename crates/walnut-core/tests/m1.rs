//! Milestone 1: the payload, Steam's shortcuts.vdf, the desktop entry, and
//! install/uninstall, all in a temporary HOME with a fake Steam holding two
//! other games. Never a real home, never a real Steam.

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use walnut_core::install::{self, Choices, Env};
use walnut_core::steam::{self, SteamProcess};
use walnut_core::{build, payload, vdf};

const FIXTURE: &[u8] = include_bytes!("../../../tests/fixtures/shortcuts_two_games.vdf");
const LOGINS: &str = include_str!("../../../tests/fixtures/loginusers.vdf");
const ACCOUNT: &str = "39734314";

/// A stand-in Steam: running or not, and whether it was asked to close.
struct FakeSteam {
    running: Cell<bool>,
    asked_to_close: Cell<bool>,
}

impl SteamProcess for FakeSteam {
    fn running(&self) -> bool {
        self.running.get()
    }
    fn shutdown(&self) {
        self.asked_to_close.set(true);
        self.running.set(false);
    }
}

fn fake_steam(running: bool) -> FakeSteam {
    FakeSteam { running: Cell::new(running), asked_to_close: Cell::new(false) }
}

fn env<'a>(home: &Path, steam: &'a FakeSteam, may_close: &'a dyn Fn() -> bool) -> Env<'a> {
    Env {
        home: home.to_path_buf(),
        data_home: home.join(".local/share"),
        config_home: home.join(".config"),
        steam,
        may_close_steam: may_close,
        mark_trusted: false,
        steam_wait: Duration::from_millis(500),
    }
}

/// A temp HOME with a fake Steam holding the two-game fixture.
fn home_with_steam() -> (tempfile::TempDir, PathBuf) {
    let home = tempfile::tempdir().unwrap();
    let root = home.path().join(".local/share/Steam");
    fs::create_dir_all(root.join("config")).unwrap();
    fs::create_dir_all(root.join("userdata").join(ACCOUNT).join("config/grid")).unwrap();
    fs::write(root.join("config/loginusers.vdf"), LOGINS).unwrap();
    fs::write(root.join("userdata").join(ACCOUNT).join("config/shortcuts.vdf"), FIXTURE).unwrap();
    // Someone else's art, which must survive.
    fs::write(root.join("userdata").join(ACCOUNT).join("config/grid/12345p.png"), b"theirs").unwrap();
    let path = home.path().to_path_buf();
    (home, path)
}

/// A tiny PNG header of the given size (enough for the icon's size).
fn png(width: u32, height: u32) -> Vec<u8> {
    let mut out = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    out.extend_from_slice(&width.to_be_bytes());
    out.extend_from_slice(&height.to_be_bytes());
    out.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    out
}

/// Builds the sample game's installer into `dir`.
fn sample_installer(dir: &Path) -> PathBuf {
    let project = dir.join("project");
    fs::create_dir_all(project.join("game/data")).unwrap();
    fs::write(project.join("game/hello.sh"), "#!/bin/sh\necho Hello from Walnut\n").unwrap();
    fs::write(project.join("game/data/words.txt"), "hello\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(project.join("game/hello.sh"), fs::Permissions::from_mode(0o755)).unwrap();
    }
    fs::write(project.join("icon.png"), png(256, 256)).unwrap();
    for slot in ["portrait", "wide", "hero", "logo", "icon"] {
        fs::write(project.join(format!("steam_{slot}.png")), png(16, 16)).unwrap();
    }
    fs::write(
        project.join("project.toml"),
        r#"[game]
name = "Hello Walnut"
id = "hello"
version = "1.2.3"
publisher = "Walnut samples"
folder = "game"
executable = "hello.sh"
icon = "icon.png"

[desktop]
shortcut = true
comment = "A tiny sample game"
categories = ["Game", "Utility"]

[steam]
tags = ["Walnut"]
portrait = "steam_portrait.png"
wide = "steam_wide.png"
hero = "steam_hero.png"
logo = "steam_logo.png"
icon = "steam_icon.png"
"#,
    )
    .unwrap();
    let program = dir.join("program");
    fs::write(&program, b"#!/bin/sh\necho the walnut-installer program\n").unwrap();
    let out = dir.join("hello-installer");
    build::build(&project.join("project.toml"), &program, &out).unwrap();
    out
}

fn choices(home: &Path, steam: bool) -> Choices {
    Choices { dir: home.join(".local/share/hello"), desktop_entry: true, desktop_shortcut: true, add_to_steam: steam }
}

#[test]
fn a_shortcuts_file_with_two_other_games_reads_and_writes_back_byte_for_byte() {
    let entries = vdf::parse(FIXTURE).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].text("AppName"), Some("Other Game"));
    assert_eq!(entries[1].text("AppName"), Some("RetroArch — Ünïcode"));
    assert_eq!(vdf::build(&entries), FIXTURE);
    assert!(vdf::parse(b"junk").is_err());
    assert!(vdf::parse(&[]).unwrap().is_empty(), "no file yet: no entries");
}

#[test]
fn the_app_id_is_steams_and_is_the_one_written_into_the_entry() {
    // Python's zlib.crc32 (tests/fixtures/make_fixtures.py).
    assert_eq!(vdf::app_id("\"/home/player/Games/Other/other.x86_64\"", "Other Game"), 3669374557);
    assert_eq!(vdf::app_id("\"/tmp/home/.local/share/hello/hello.sh\"", "Hello Walnut"), 2616304259);
    assert_eq!(vdf::app_id("\"/opt/x/y\"", "Ünïcode Game"), 2403570138);
    let entries = vdf::parse(FIXTURE).unwrap();
    let stored = entries[0].int("appid").unwrap() as u32;
    assert_eq!(stored, vdf::app_id(entries[0].text("Exe").unwrap(), "Other Game"), "the fixture's own id agrees");
}

#[test]
fn adding_never_duplicates_and_removing_restores_the_file_exactly() {
    let ours = vdf::Shortcut {
        name: "Hello Walnut".into(),
        exe: "\"/tmp/home/.local/share/hello/hello.sh\"".into(),
        start_dir: "\"/tmp/home/.local/share/hello\"".into(),
        icon: String::new(),
        launch_options: String::new(),
        tags: vec!["Walnut".into()],
    };
    let added = vdf::with_ours(FIXTURE, &ours).unwrap();
    let entries = vdf::parse(&added).unwrap();
    assert_eq!(entries.len(), 3);
    let original = vdf::parse(FIXTURE).unwrap();
    assert_eq!(entries[0].raw, original[0].raw, "the others byte for byte");
    assert_eq!(entries[1].raw, original[1].raw);
    assert_eq!(entries[2].int("appid").unwrap() as u32, 2616304259, "our id in the appid field");
    let again = vdf::with_ours(&added, &ours).unwrap();
    assert_eq!(vdf::parse(&again).unwrap().len(), 3, "adding again refreshes ours");
    assert_eq!(vdf::without_ours(&added, &ours.exe).unwrap(), FIXTURE, "removing ours leaves the file exactly as it was");
}

#[test]
fn the_most_recent_steam_account_is_chosen() {
    assert_eq!(steam::most_recent_account(LOGINS).as_deref(), Some(ACCOUNT));
    assert_eq!(steam::most_recent_account(""), None);
}

#[test]
fn a_damaged_payload_is_refused_and_nothing_is_unpacked() {
    let work = tempfile::tempdir().unwrap();
    let installer = sample_installer(work.path());
    let layout = payload::layout(&installer).unwrap();
    let mut bytes = fs::read(&installer).unwrap();
    let middle = (layout.payload_at + layout.footer.payload_len / 2) as usize;
    bytes[middle] ^= 0xff;
    fs::write(&installer, &bytes).unwrap();
    let (_home, home) = home_with_steam();
    let steam = fake_steam(false);
    let never = || false;
    let error = install::install(&installer, &choices(&home, true), &env(&home, &steam, &never), &mut |_, _| {}, &|| true).unwrap_err();
    assert!(error.contains("checksum"), "{error}");
    assert!(!home.join(".local/share/hello").exists(), "nothing was unpacked");
    assert_eq!(fs::read(home.join(".local/share/Steam/userdata").join(ACCOUNT).join("config/shortcuts.vdf")).unwrap(), FIXTURE);
}

#[test]
fn a_changed_manifest_is_refused_too() {
    let work = tempfile::tempdir().unwrap();
    let installer = sample_installer(work.path());
    let layout = payload::layout(&installer).unwrap();
    let mut bytes = fs::read(&installer).unwrap();
    // Point the game at another program, keeping the length.
    let at = layout.manifest_at as usize;
    let text = String::from_utf8(bytes[at..at + layout.footer.manifest_len as usize].to_vec()).unwrap();
    let changed = text.replace("hello.sh", "evil.sh");
    bytes[at..at + changed.len()].copy_from_slice(changed.as_bytes());
    fs::write(&installer, &bytes).unwrap();
    let (_home, home) = home_with_steam();
    let steam = fake_steam(false);
    let never = || false;
    let error = install::install(&installer, &choices(&home, true), &env(&home, &steam, &never), &mut |_, _| {}, &|| true).unwrap_err();
    assert!(error.contains("checksum"), "{error}");
    assert!(install::project_of(&installer).is_err(), "nothing is read from it");
    assert!(!home.join(".local/share/hello").exists());
}

#[test]
fn a_cancelled_install_rolls_back_cleanly() {
    let work = tempfile::tempdir().unwrap();
    let installer = sample_installer(work.path());
    let (_home, home) = home_with_steam();
    let steam = fake_steam(false);
    let never = || false;
    let error = install::install(&installer, &choices(&home, true), &env(&home, &steam, &never), &mut |_, _| {}, &|| false).unwrap_err();
    assert!(error.contains("cancelled"), "{error}");
    assert!(!home.join(".local/share/hello").exists(), "the folder it made is gone again");
}

#[test]
fn install_then_uninstall_in_a_temp_home_with_a_fake_steam() {
    let work = tempfile::tempdir().unwrap();
    let installer = sample_installer(work.path());
    let (_home, home) = home_with_steam();
    let steam = fake_steam(false);
    let never = || false;
    let env = env(&home, &steam, &never);
    let unrelated = home.join(".local/share/applications/someone-else.desktop");
    fs::create_dir_all(unrelated.parent().unwrap()).unwrap();
    fs::write(&unrelated, "theirs").unwrap();
    let mut seen = Vec::new();
    let receipt = install::install(&installer, &choices(&home, true), &env, &mut |done, total| seen.push((done, total)), &|| true).unwrap();
    let dir = home.join(".local/share/hello");
    assert!(seen.last().map(|(d, t)| d == t).unwrap_or(false), "progress ends at 100%");
    assert_eq!(fs::read_to_string(dir.join("hello.sh")).unwrap(), "#!/bin/sh\necho Hello from Walnut\n");
    assert!(dir.join("data/words.txt").is_file());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert!(fs::metadata(dir.join("hello.sh")).unwrap().permissions().mode() & 0o111 != 0, "the game stays executable");
    }
    // The desktop entry: the name without its version, the version apart.
    let entry = fs::read_to_string(home.join(".local/share/applications/hello.desktop")).unwrap();
    assert!(entry.contains("Name=Hello Walnut\n") && entry.contains("X-Walnut-Version=1.2.3\n") && entry.contains("Icon=hello\n"));
    assert!(!entry.lines().any(|l| l.starts_with("Name=") && l.contains("1.2.3")), "no version in the name");
    assert!(entry.contains(&format!("Exec=\"{}\"", dir.join("hello.sh").display())));
    assert!(home.join(".local/share/icons/hicolor/256x256/apps/hello.png").is_file(), "the icon in its size");
    assert!(home.join("Desktop/hello.desktop").is_file(), "the desktop shortcut");
    // Steam: ours added with Steam's id, art named by it, the others untouched.
    let config = home.join(".local/share/Steam/userdata").join(ACCOUNT).join("config");
    let added = receipt.steam.clone().expect("added to Steam");
    let entries = vdf::parse(&fs::read(config.join("shortcuts.vdf")).unwrap()).unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[2].text("AppName"), Some("Hello Walnut"));
    assert_eq!(entries[2].int("appid").unwrap() as u32, added.app_id);
    for suffix in ["p.png", ".png", "_hero.png", "_logo.png", "_icon.png"] {
        assert!(config.join("grid").join(format!("{}{suffix}", added.app_id)).is_file(), "art {suffix}");
    }
    assert_eq!(fs::read(config.join("shortcuts.vdf.walnut-backup")).unwrap(), FIXTURE, "backed up first");
    // Uninstall: everything it made goes, nothing else.
    assert!(dir.join(install::UNINSTALLER).is_file(), "an uninstaller is shipped");
    install::uninstall(&dir, &env).unwrap();
    assert_eq!(fs::read(config.join("shortcuts.vdf")).unwrap(), FIXTURE, "Steam's file back byte for byte");
    assert!(!config.join("grid").join(format!("{}p.png", added.app_id)).exists(), "our art gone");
    assert!(config.join("grid/12345p.png").is_file(), "someone else's art stays");
    assert!(!dir.exists(), "the install folder is gone");
    assert!(!home.join(".local/share/applications/hello.desktop").exists());
    assert!(!home.join(".local/share/icons/hicolor/256x256/apps/hello.png").exists());
    assert!(!home.join("Desktop/hello.desktop").exists());
    assert!(unrelated.is_file(), "someone else's desktop entry stays");
}

#[test]
fn steam_is_closed_only_when_the_player_says_so() {
    let work = tempfile::tempdir().unwrap();
    let installer = sample_installer(work.path());
    let (_home, home) = home_with_steam();
    let steam = fake_steam(true);
    let no = || false;
    let receipt = install::install(&installer, &choices(&home, true), &env(&home, &steam, &no), &mut |_, _| {}, &|| true).unwrap();
    assert!(receipt.steam.is_none() && receipt.steam_skipped.is_some(), "skipped");
    assert!(!steam.asked_to_close.get(), "Steam left alone");
    let config = home.join(".local/share/Steam/userdata").join(ACCOUNT).join("config");
    assert_eq!(fs::read(config.join("shortcuts.vdf")).unwrap(), FIXTURE, "and its file untouched");
    install::uninstall(&home.join(".local/share/hello"), &env(&home, &steam, &no)).unwrap();
    let yes = || true;
    let steam = fake_steam(true);
    let receipt = install::install(&installer, &choices(&home, true), &env(&home, &steam, &yes), &mut |_, _| {}, &|| true).unwrap();
    assert!(steam.asked_to_close.get(), "closed when the player said yes");
    assert!(receipt.steam.is_some(), "and added");
}

#[test]
fn no_steam_means_no_steam_step() {
    let work = tempfile::tempdir().unwrap();
    let installer = sample_installer(work.path());
    let home = tempfile::tempdir().unwrap();
    let steam = fake_steam(false);
    let never = || false;
    let env = env(home.path(), &steam, &never);
    assert!(!install::steam_available(&env), "the option is hidden without Steam");
    let receipt = install::install(&installer, &choices(home.path(), false), &env, &mut |_, _| {}, &|| true).unwrap();
    assert!(receipt.steam.is_none());
}
