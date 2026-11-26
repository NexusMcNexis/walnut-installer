//! `walnut`: builds an installer from a project file, headless (for CI).
//!
//!   walnut build project.toml [-o OUT] [--program PATH]
//!   walnut inspect INSTALLER
//!
//! `--program` is the walnut-installer binary to put at the installer's
//! front (default: the one beside this `walnut`).

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use walnut_core::{build, install, payload};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("build") => build_cmd(&args[1..]),
        Some("inspect") => inspect_cmd(&args[1..]),
        _ => Err("usage: walnut build project.toml [-o OUT] [--program PATH] | walnut inspect INSTALLER".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("walnut: {message}");
            ExitCode::FAILURE
        }
    }
}

fn build_cmd(args: &[String]) -> Result<(), String> {
    let mut project: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut program: Option<PathBuf> = None;
    let mut at = 0;
    while at < args.len() {
        match args[at].as_str() {
            "-o" | "--out" => {
                out = args.get(at + 1).map(PathBuf::from);
                at += 1;
            }
            "--program" => {
                program = args.get(at + 1).map(PathBuf::from);
                at += 1;
            }
            other => project = Some(PathBuf::from(other)),
        }
        at += 1;
    }
    let project = project.ok_or("which project.toml?")?;
    let program = program.unwrap_or_else(|| beside_me("walnut-installer"));
    let built = {
        let text = std::fs::read_to_string(&project).map_err(|e| format!("{}: {e}", project.display()))?;
        walnut_core::project::Project::from_toml(&text)?
    };
    let out = out.unwrap_or_else(|| PathBuf::from(format!("{}-{}-installer", built.game.id, built.game.version)));
    build::build(&project, &program, &out)?;
    let layout = payload::layout(&out).map_err(|e| e.to_string())?;
    println!(
        "WALNUT BUILT {} ({} {}): payload {} bytes, sha256 {}",
        out.display(),
        built.game.name,
        built.game.version,
        layout.footer.payload_len,
        hex(&layout.footer.sha256)
    );
    Ok(())
}

fn inspect_cmd(args: &[String]) -> Result<(), String> {
    let path = Path::new(args.first().ok_or("which installer?")?);
    let layout = payload::layout(path).map_err(|e| e.to_string())?;
    let ok = payload::verify(path, &layout).is_ok();
    if !ok {
        return Err("the installer doesn't match its checksum (damaged or changed)".into());
    }
    let project = install::project_of(path)?;
    println!("game: {} ({}), version {}", project.game.name, project.game.id, project.game.version);
    println!("payload: {} bytes, sha256 {} (matches)", layout.footer.payload_len, hex(&layout.footer.sha256));
    Ok(())
}

fn beside_me(name: &str) -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join(name))).unwrap_or_else(|| PathBuf::from(name))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
