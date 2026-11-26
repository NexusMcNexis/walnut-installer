//! M2: the graphical installer: Welcome, Folder, (Close Steam?), Installing,
//! Done, in the project's colours. Everything it decides is in flow.rs; this
//! only draws the screen the flow is on and passes clicks and keys back.
//!
//! Keyboard: Enter is the main button, Esc the other (Back / Cancel), Tab
//! moves between controls. Every control has a name screen readers read
//! (AccessKit). It scales with the display (100-200%) without clipping.

use crate::worker::{self, AnySteam, Shared};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use walnut_core::install::Choices;
use walnut_core::project::Project;
use walnut_core::steam::SteamProcess;
use walnut_ui::egui::{self, TextureHandle};
use walnut_ui::flow::{Flow, Machine, Screen};
use walnut_ui::screens::{self, View};
use walnut_ui::theme::Palette;
use walnut_ui::eframe;

/// The window's size at 100%; it grows with the display's scale.
const BASE_SIZE: [f32; 2] = [760.0, 520.0];

pub struct Setup {
    pub installer: PathBuf,
    pub project: Project,
    pub welcome_art: Option<Vec<u8>>,
    pub default_dir: PathBuf,
    pub steam_offered: bool,
    /// WALNUT_SCREENSHOTS: save every screen here (at 100/150/200%) and quit.
    pub screenshots: Option<PathBuf>,
}

struct RealMachine;

impl Machine for RealMachine {
    fn free_bytes(&self, dir: &Path) -> Option<u64> {
        walnut_core::space::free_bytes(dir)
    }
    fn writable(&self, dir: &Path) -> bool {
        let mut here = Some(dir);
        while let Some(path) = here {
            if path.exists() {
                return path.metadata().map(|m| !m.permissions().readonly()).unwrap_or(false)
                    && std::fs::metadata(path).is_ok()
                    && tempfile_probe(path);
            }
            here = path.parent();
        }
        false
    }
    fn steam_running(&self) -> bool {
        AnySteam::from_env().running()
    }
}

/// Whether a file can really be made in `dir` (permissions bits alone lie
/// for root-owned folders).
fn tempfile_probe(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    let probe = dir.join(format!(".walnut-probe-{}", std::process::id()));
    let ok = std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

pub fn run(setup: Setup) -> Result<(), String> {
    let scale = std::env::var("WALNUT_SCALE").ok().and_then(|s| s.parse::<f32>().ok()).unwrap_or(1.0).clamp(1.0, 2.0);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(format!("Install {}", setup.project.game.name))
            .with_inner_size([BASE_SIZE[0] * scale, BASE_SIZE[1] * scale])
            .with_min_inner_size([BASE_SIZE[0] * scale, BASE_SIZE[1] * scale])
            .with_app_id(format!("walnut-installer-{}", setup.project.game.id)),
        ..Default::default()
    };
    let title = format!("Install {}", setup.project.game.name);
    eframe::run_native(&title, options, Box::new(move |cc| {
        Palette::of(&setup.project.theme).apply(&cc.egui_ctx);
        cc.egui_ctx.set_zoom_factor(scale);
        Ok(Box::new(App::new(setup, &cc.egui_ctx)))
    }))
    .map_err(|e| e.to_string())
}

struct App {
    setup: Setup,
    flow: Flow,
    palette: Palette,
    art: Option<TextureHandle>,
    job: Option<Arc<Shared>>,
    close: bool,
    shots: Option<Shots>,
    /// WALNUT_AUTOPILOT=1: presses through a real install (tests): Next,
    /// Install, Close Steam, then Finish with Launch unticked.
    autopilot: bool,
}

impl App {
    fn new(setup: Setup, ctx: &egui::Context) -> App {
        let game = &setup.project;
        let flow = Flow::new(&setup.default_dir, game.build.install_bytes, game.desktop.entry, game.desktop.shortcut, setup.steam_offered, game.steam.ticked);
        let art = setup.welcome_art.as_deref().and_then(screens::decode).map(|image| ctx.load_texture("welcome-art", image, egui::TextureOptions::LINEAR));
        let shots = setup.screenshots.clone().map(Shots::new);
        let mut flow = flow;
        if shots.is_some() {
            // Screenshots go in a public README: a stand-in player's path,
            // never the real home folder (screenshot mode never installs).
            flow.dir = format!("/home/player/.local/share/{}", game.game.id);
        }
        let autopilot = std::env::var("WALNUT_AUTOPILOT").map(|v| v == "1").unwrap_or(false);
        App { palette: Palette::of(&game.theme), flow, art, job: None, close: false, shots, autopilot, setup }
    }

    fn start_install(&mut self, ctx: &egui::Context) {
        let choices = Choices {
            dir: self.flow.dir_path(),
            desktop_entry: self.flow.desktop_entry,
            desktop_shortcut: self.flow.desktop_shortcut,
            add_to_steam: self.flow.add_to_steam,
        };
        let repaint = ctx.clone();
        self.job = Some(worker::start(self.setup.installer.clone(), choices, self.flow.may_close_steam, Box::new(AnySteam::from_env()), Box::new(move || repaint.request_repaint())));
    }

    fn follow_job(&mut self) {
        let Some(job) = &self.job else { return };
        self.flow.progress(job.done.load(Ordering::Relaxed), job.total.load(Ordering::Relaxed));
        if self.flow.cancel_asked {
            job.cancel.store(true, Ordering::Relaxed);
        }
        let finished = job.result.lock().unwrap().take();
        if let Some(result) = finished {
            self.flow.finished(result.map(|_| ()));
            self.job = None;
        }
    }

    fn primary(&mut self, ctx: &egui::Context) {
        if matches!(self.flow.screen, Screen::Done | Screen::Cancelled | Screen::Failed(_)) {
            if self.flow.launch_on_finish() {
                let dir = self.flow.dir_path();
                let exe = dir.join(&self.setup.project.game.executable);
                let _ = std::process::Command::new(&exe).current_dir(&dir).spawn();
            }
            self.close = true;
            return;
        }
        if self.flow.primary(&RealMachine) {
            self.start_install(ctx);
        }
    }

    fn secondary(&mut self) {
        if self.flow.secondary() {
            self.close = true;
        }
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.follow_job();
        if self.job.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        if let Some(shots) = &mut self.shots {
            if shots.step(ctx, &mut self.flow) {
                self.close = true;
            }
        }
        if self.autopilot && self.job.is_none() && !self.close {
            if self.flow.screen == Screen::Done {
                self.flow.launch = false;
            }
            println!("AUTOPILOT {:?}", self.flow.screen);
            self.primary(ctx);
            ctx.request_repaint();
        }
        if self.close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let (enter, escape) = ctx.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
        let view = View { project: &self.setup.project, palette: &self.palette, art: self.art.as_ref(), live: true };
        let pressed = screens::draw(ui, &view, &mut self.flow);
        if let Some(shots) = &mut self.shots {
            shots.note_fit(ui);
        }
        if pressed.browse {
            if let Some(picked) = rfd::FileDialog::new().set_directory(&self.flow.dir).pick_folder() {
                self.flow.dir = picked.display().to_string();
            }
        }
        if enter || pressed.primary {
            self.primary(&ctx);
        } else if escape || pressed.secondary {
            self.secondary();
        }
    }
}

/// WALNUT_SCREENSHOTS: walks every screen (with made-up progress, never a
/// real install), saves each as a PNG, and says if any didn't fit.
struct Shots {
    dir: PathBuf,
    order: Vec<(&'static str, Screen)>,
    at: usize,
    frames: u32,
    pending: Arc<std::sync::Mutex<Option<Arc<egui::ColorImage>>>>,
    clipped: bool,
}

impl Shots {
    fn new(dir: PathBuf) -> Shots {
        let names = ["1-welcome", "2-folder", "3-close-steam", "4-installing", "5-done", "6-cancelled", "7-failed"];
        let order: Vec<(&'static str, Screen)> = names.into_iter().zip(screens::every_screen().into_iter().map(|(_, screen)| screen)).collect();
        let _ = std::fs::create_dir_all(&dir);
        Shots { dir, order, at: 0, frames: 0, pending: Default::default(), clipped: false }
    }

    /// One frame: put the flow on the next screen, wait for it to settle,
    /// take the shot, save it. True when all are done.
    fn step(&mut self, ctx: &egui::Context, flow: &mut Flow) -> bool {
        if let Some(image) = self.pending.lock().unwrap().take() {
            let (name, _) = &self.order[self.at];
            let scale = (ctx.zoom_factor() * 100.0).round() as u32;
            let path = self.dir.join(format!("{name}-{scale}.png"));
            let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
            let _ = image::save_buffer(&path, &rgba, image.size[0] as u32, image.size[1] as u32, image::ColorType::Rgba8);
            println!("SHOT {} {}x{}{}", path.display(), image.size[0], image.size[1], if self.clipped { " CLIPPED" } else { "" });
            self.at += 1;
            self.frames = 0;
            self.clipped = false;
        }
        if self.at >= self.order.len() {
            return true;
        }
        flow.screen = self.order[self.at].1.clone();
        flow.progress(42, 100);
        flow.steam_offered = true;
        self.frames += 1;
        if self.frames == 6 {
            let slot = self.pending.clone();
            ctx.request_screenshot(move |image| *slot.lock().unwrap() = Some(image));
        }
        ctx.request_repaint();
        false
    }

    /// Whether this frame's content ran past the panel's bottom.
    fn note_fit(&mut self, ui: &egui::Ui) {
        if ui.min_rect().bottom() > ui.max_rect().bottom() + 0.5 {
            self.clipped = true;
        }
    }
}
