//! Walnut Installer's builder (M3): fill in a form, see every installer
//! screen as you edit, and build.
//!
//!   walnut-builder [project.toml]                    the window
//!   walnut-builder --build project.toml -o OUT [--program P]
//!                                                    headless, as `walnut build`
//!
//! Build makes the very same file `walnut build` does (both call
//! walnut_core::build::build_from), so a project made here builds in CI.

mod app;

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    if let Some(project) = value("--build") {
        let out = value("-o").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("installer"));
        let program = value("--program").map(PathBuf::from).unwrap_or_else(app::installer_program);
        return match walnut_builder::doc::Document::open(&PathBuf::from(project)).and_then(|doc| doc.build(&program, &out)) {
            Ok(project) => {
                println!("WALNUT BUILT {} ({} {})", out.display(), project.game.name, project.game.version);
                ExitCode::SUCCESS
            }
            Err(why) => {
                eprintln!("{why}");
                ExitCode::FAILURE
            }
        };
    }
    let project = args.iter().find(|a| !a.starts_with("--")).map(PathBuf::from);
    match app::run(project) {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("{why}");
            ExitCode::FAILURE
        }
    }
}
