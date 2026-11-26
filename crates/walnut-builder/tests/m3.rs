//! Milestone 3: the builder. Its art checks, its project file round trip,
//! and that its Build makes the very same installer as `walnut build`.

use std::fs;
use walnut_builder::art::{self, Slot};
use walnut_builder::doc::Document;

#[test]
fn each_art_slot_checks_its_size() {
    assert!(art::check(Slot::Portrait, (600, 900), false).is_ok());
    assert!(art::check(Slot::Portrait, (920, 430), false).unwrap_err().contains("600x900"));
    assert!(art::check(Slot::Wide, (920, 430), false).is_ok());
    assert!(art::check(Slot::Hero, (1920, 620), false).is_ok());
    assert!(art::check(Slot::Hero, (1920, 1080), false).is_err());
    assert!(art::check(Slot::Logo, (800, 300), true).is_ok());
    assert!(art::check(Slot::Logo, (800, 300), false).unwrap_err().contains("transparen"));
    assert!(art::check(Slot::SteamIcon, (256, 256), false).is_ok());
    assert!(art::check(Slot::SteamIcon, (256, 200), false).unwrap_err().contains("square"));
    assert!(art::check(Slot::GameIcon, (32, 32), false).unwrap_err().contains("at least"));
    assert!(art::check(Slot::Welcome, (920, 430), false).is_ok(), "the welcome art may be any size");
    assert_eq!(art::Slot::all().len(), 7, "every art the project has");
}

#[test]
fn a_png_is_measured_and_its_alpha_seen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.png");
    image::RgbaImage::from_pixel(10, 20, image::Rgba([1, 2, 3, 0])).save(&path).unwrap();
    assert_eq!(art::measure(&path).unwrap(), ((10, 20), true));
    image::RgbImage::from_pixel(4, 4, image::Rgb([1, 2, 3])).save(&path).unwrap();
    assert_eq!(art::measure(&path).unwrap(), ((4, 4), false));
    assert!(art::measure(&dir.path().join("none.png")).is_err());
}

#[test]
fn a_project_opens_edits_and_saves_as_the_same_toml() {
    let root = env!("CARGO_MANIFEST_DIR");
    let sample = format!("{root}/../../examples/hello/project.toml");
    let mut doc = Document::open(std::path::Path::new(&sample)).unwrap();
    assert_eq!(doc.project.game.name, "Hello Walnut");
    let dir = tempfile::tempdir().unwrap();
    doc.project.text.welcome = "Edited in the builder.".into();
    doc.path = dir.path().join("project.toml");
    doc.save().unwrap();
    let again = Document::open(&doc.path).unwrap();
    assert_eq!(again.project, doc.project, "what was saved opens the same");
    assert_eq!(again.project.text.welcome, "Edited in the builder.");
}

#[test]
fn build_makes_the_same_installer_as_walnut_build() {
    let root = env!("CARGO_MANIFEST_DIR");
    let sample = std::path::PathBuf::from(format!("{root}/../../examples/hello/project.toml"));
    let dir = tempfile::tempdir().unwrap();
    let program = dir.path().join("program");
    fs::write(&program, b"#!/bin/sh\nexit 0\n").unwrap();
    let headless = dir.path().join("headless");
    walnut_core::build::build(&sample, &program, &headless).unwrap();
    let doc = Document::open(&sample).unwrap();
    let from_builder = dir.path().join("from-builder");
    doc.build(&program, &from_builder).unwrap();
    assert_eq!(fs::read(&headless).unwrap(), fs::read(&from_builder).unwrap(), "byte for byte");
}

#[test]
fn problems_are_listed_before_building() {
    let root = env!("CARGO_MANIFEST_DIR");
    let sample = std::path::PathBuf::from(format!("{root}/../../examples/hello/project.toml"));
    let mut doc = Document::open(&sample).unwrap();
    assert!(doc.problems().is_empty(), "the sample is fine: {:?}", doc.problems());
    doc.project.game.name = format!("Hello {}", doc.project.game.version);
    doc.project.steam.hero = "art/steam_portrait.png".into();
    let problems = doc.problems();
    assert!(problems.iter().any(|p| p.contains("version")), "{problems:?}");
    assert!(problems.iter().any(|p| p.contains("1920x620")), "{problems:?}");
}
