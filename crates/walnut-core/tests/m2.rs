//! Milestone 2, the core's part: what the graphical installer needs before
//! anything is unpacked (the welcome art, read straight from the payload;
//! the size the install will take, for the free-space check), the free
//! space itself, and the desktop quirks for the shortcut.

use std::fs;
use std::path::Path;
use walnut_core::desktop::{self, Trust};
use walnut_core::{build, payload, space};

fn sample(dir: &Path, with_art: bool) -> std::path::PathBuf {
    fs::create_dir_all(dir.join("game/data")).unwrap();
    fs::write(dir.join("game/run.sh"), "#!/bin/sh\necho hi\n").unwrap();
    fs::write(dir.join("game/data/big.bin"), vec![7u8; 300_000]).unwrap();
    // A tiny valid PNG header is enough for the payload; the GUI decodes it.
    fs::write(dir.join("welcome.png"), b"\x89PNG\r\n\x1a\nwelcome-art").unwrap();
    let art = if with_art { "[art]\nwelcome = \"welcome.png\"\n" } else { "" };
    let text = format!("[game]\nname = \"Sample\"\nid = \"sample\"\nversion = \"1.0\"\nfolder = \"game\"\nexecutable = \"run.sh\"\n{art}");
    fs::write(dir.join("project.toml"), text).unwrap();
    fs::write(dir.join("program"), b"#!/bin/sh\nexit 0\n").unwrap();
    let out = dir.join("installer");
    build::build(&dir.join("project.toml"), &dir.join("program"), &out).unwrap();
    out
}

#[test]
fn the_welcome_art_is_read_from_the_payload_without_unpacking() {
    let dir = tempfile::tempdir().unwrap();
    let installer = sample(dir.path(), true);
    let layout = payload::layout(&installer).unwrap();
    let art = payload::peek(&installer, &layout, payload::WELCOME_ART).unwrap();
    assert_eq!(art.as_deref(), Some(&b"\x89PNG\r\n\x1a\nwelcome-art"[..]), "the art comes back as it was packed");
    let none = tempfile::tempdir().unwrap();
    let plain = sample(none.path(), false);
    let layout = payload::layout(&plain).unwrap();
    assert_eq!(payload::peek(&plain, &layout, payload::WELCOME_ART).unwrap(), None, "no art: none, quickly");
}

#[test]
fn the_manifest_says_how_much_the_install_takes() {
    let dir = tempfile::tempdir().unwrap();
    let installer = sample(dir.path(), true);
    let project = walnut_core::install::project_of(&installer).unwrap();
    assert!(project.build.install_bytes >= 300_000, "at least the game's files ({} bytes)", project.build.install_bytes);
    assert!(project.build.install_bytes < 400_000, "and not wildly more");
}

#[test]
fn free_space_is_found_for_a_folder_that_does_not_exist_yet() {
    let dir = tempfile::tempdir().unwrap();
    let here = space::free_bytes(dir.path()).expect("free space of a real folder");
    assert!(here > 0);
    let later = space::free_bytes(&dir.path().join("not/made/yet")).expect("asked of the nearest folder that exists");
    assert!(later > 0);
    assert!(space::enough(1, &dir.path().join("x")), "one byte fits");
    assert!(!space::enough(u64::MAX / 2, &dir.path().join("x")), "half of all the bytes there are doesn't");
}

#[test]
fn the_shortcut_is_trusted_the_way_each_desktop_wants() {
    assert_eq!(desktop::trust_for("GNOME", "fedora"), Trust::GioMetadata);
    assert_eq!(desktop::trust_for("ubuntu:GNOME", "ubuntu"), Trust::GioMetadata);
    assert_eq!(desktop::trust_for("X-Cinnamon", "linuxmint"), Trust::GioMetadata);
    assert_eq!(desktop::trust_for("KDE", "bazzite"), Trust::ExecutableBit);
    assert_eq!(desktop::trust_for("KDE", "steamos"), Trust::ExecutableBit);
    assert_eq!(desktop::trust_for("", "steamos"), Trust::ExecutableBit, "SteamOS's desktop mode is KDE");
    assert_eq!(desktop::trust_for("sway", "arch"), Trust::ExecutableBit, "anything else: the executable bit only");
    assert_eq!(desktop::distro_id("NAME=\"Bazzite\"\nID=bazzite\nID_LIKE=\"fedora\"\n"), "bazzite");
}
