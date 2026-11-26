//! The program at the front of every Walnut installer. With a display it
//! opens the walnut-themed screens (M2); without one, or with --text or
//! --yes, it is the text-mode installer (M1):
//!
//!   ./game-installer                 the screens, or (text mode) asks with sensible defaults
//!   ./game-installer --yes [--dir D] [--no-desktop-entry] [--desktop-shortcut]
//!                    [--no-steam] [--no-launch]
//!   <install dir>/.walnut/uninstall  removes it again (or --uninstall --dir D)
//!
//! Never needs root.

use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use walnut_core::install::{self, Choices, Env};
use walnut_core::project::CREDIT;
use walnut_core::steam::{RealSteam, SteamProcess};

#[cfg(feature = "gui")]
mod gui;
#[cfg(feature = "gui")]
mod worker;

/// For tests and demos only (documented in CONTRIBUTING.md):
/// `WALNUT_FAKE_STEAM_RUNNING=0|1` stands in for the real Steam, so a test
/// never sees or closes the Steam of the person running it.
struct FakeSteam(std::cell::Cell<bool>);

impl SteamProcess for FakeSteam {
    fn running(&self) -> bool {
        self.0.get()
    }
    fn shutdown(&self) {
        self.0.set(false);
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |flag: &str| args.iter().any(|a| a == flag);
    let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let interactive = !has("--yes") && io::stdin().is_terminal();
    let ask_close = move || {
        if !interactive {
            return false;
        }
        ask_yes("Steam is running. It must be closed while your library is changed. Close Steam now?", false)
    };
    let fake = std::env::var("WALNUT_FAKE_STEAM_RUNNING").ok().map(|v| FakeSteam(std::cell::Cell::new(v == "1")));
    let real = RealSteam;
    let steam: &dyn SteamProcess = match &fake {
        Some(fake) => fake,
        None => &real,
    };
    let env = Env::from_system(steam, &ask_close);
    let me = std::env::current_exe().unwrap_or_default();
    let uninstalling = has("--uninstall") || me.parent().and_then(|p| p.file_name()).map(|n| n == ".walnut").unwrap_or(false);
    let result = if uninstalling {
        let dir = value("--dir").map(PathBuf::from).unwrap_or_else(|| me.parent().and_then(Path::parent).map(Path::to_path_buf).unwrap_or_default());
        uninstall(&dir, &env, interactive)
    } else if let Some(shown) = try_gui(&me, &env, &has) {
        shown
    } else {
        install_flow(&me, &env, interactive, &has, &value)
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("\n{message}");
            ExitCode::FAILURE
        }
    }
}

/// M2: the graphical installer when there is a display and no --text or
/// --yes; None to use text mode (no display, or the window couldn't open).
#[cfg(feature = "gui")]
fn try_gui(me: &Path, env: &Env, has: &dyn Fn(&str) -> bool) -> Option<Result<(), String>> {
    let screenshots = std::env::var("WALNUT_SCREENSHOTS").ok().map(PathBuf::from);
    let display = std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some();
    if has("--text") || has("--yes") || !display {
        return None;
    }
    let project = match install::project_of(me) {
        Ok(project) => project,
        Err(why) => return Some(Err(why)),
    };
    let layout = walnut_core::payload::layout(me).ok()?;
    let welcome_art = walnut_core::payload::peek(me, &layout, walnut_core::payload::WELCOME_ART).ok().flatten();
    let setup = gui::Setup {
        installer: me.to_path_buf(),
        default_dir: project.install_dir(&env.home),
        steam_offered: project.steam.offer && install::steam_available(env),
        project,
        welcome_art,
        screenshots,
    };
    match gui::run(setup) {
        Ok(()) => Some(Ok(())),
        Err(why) => {
            eprintln!("  (The installer's window couldn't open: {why}. Using text mode.)");
            None
        }
    }
}

#[cfg(not(feature = "gui"))]
fn try_gui(_me: &Path, _env: &Env, _has: &dyn Fn(&str) -> bool) -> Option<Result<(), String>> {
    None
}

fn install_flow(me: &Path, env: &Env, interactive: bool, has: &dyn Fn(&str) -> bool, value: &dyn Fn(&str) -> Option<String>) -> Result<(), String> {
    let project = install::project_of(me)?;
    let game = &project.game;
    println!("\n  {}", game.name);
    if !project.text.welcome.is_empty() {
        println!("  {}", project.text.welcome);
    }
    println!("  version {}{}\n", game.version, if game.publisher.is_empty() { String::new() } else { format!(" · {}", game.publisher) });
    let default_dir = project.install_dir(&env.home);
    let dir = match value("--dir") {
        Some(dir) => PathBuf::from(dir),
        None if interactive => ask_text("Install to", &default_dir.display().to_string()).into(),
        None => default_dir,
    };
    let steam_here = project.steam.offer && install::steam_available(env) && !has("--no-steam");
    let choices = Choices {
        desktop_entry: !has("--no-desktop-entry") && (!interactive || ask_yes("Add it to your applications menu?", project.desktop.entry)),
        desktop_shortcut: if has("--desktop-shortcut") { true } else if has("--no-desktop-shortcut") { false } else { interactive && ask_yes("Put a shortcut on your desktop?", project.desktop.shortcut) || (!interactive && project.desktop.shortcut) },
        // Only offered when Steam is here; ticked by default.
        add_to_steam: steam_here && (!interactive || ask_yes("Add it to your Steam library (with its artwork)?", project.steam.ticked)),
        dir: dir.clone(),
    };
    let mut last = 0u64;
    let mut progress = |done: u64, total: u64| {
        let percent = done * 100 / total.max(1);
        if percent != last || done == total {
            last = percent;
            let bar = "#".repeat((percent / 4) as usize);
            print!("\r  Installing [{bar:<25}] {percent:>3}%");
            let _ = io::stdout().flush();
        }
    };
    let receipt = install::install(me, &choices, env, &mut progress, &|| true)?;
    println!("\n\n  Installed to {}", receipt.dir.display());
    if receipt.steam.is_some() {
        println!("  Added to your Steam library.");
    } else if let Some(why) = &receipt.steam_skipped {
        println!("  Not added to Steam: {why}.");
    }
    if !project.text.done.is_empty() {
        println!("  {}", project.text.done);
    }
    println!("  To remove it: {}", receipt.dir.join(install::UNINSTALLER).display());
    if project.theme.show_credit {
        println!("\n  {CREDIT}");
    }
    let launch = !has("--no-launch") && interactive && ask_yes("Launch it now?", true);
    if launch {
        let exe = receipt.dir.join(&game.executable);
        std::process::Command::new(&exe).current_dir(&receipt.dir).spawn().map_err(|e| format!("couldn't start {}: {e}", exe.display()))?;
    }
    Ok(())
}

fn uninstall(dir: &Path, env: &Env, interactive: bool) -> Result<(), String> {
    let receipt = install::load_receipt(dir)?;
    if interactive && !ask_yes(&format!("Remove {} from {}?", receipt.name, dir.display()), true) {
        return Ok(());
    }
    install::uninstall(dir, env)?;
    println!("  {} was removed. Saved games elsewhere were left alone.", receipt.name);
    Ok(())
}

fn ask_yes(question: &str, default: bool) -> bool {
    print!("  {question} [{}] ", if default { "Y/n" } else { "y/N" });
    let _ = io::stdout().flush();
    let mut line = String::new();
    if io::stdin().lock().read_line(&mut line).is_err() {
        return default;
    }
    match line.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "y" | "yes" => true,
        _ => false,
    }
}

fn ask_text(question: &str, default: &str) -> String {
    print!("  {question} [{default}]: ");
    let _ = io::stdout().flush();
    let mut line = String::new();
    let _ = io::stdin().lock().read_line(&mut line);
    let answer = line.trim();
    if answer.is_empty() { default.to_string() } else { answer.to_string() }
}
