//! M2: the graphical installer: Welcome, Folder, (Close Steam?), Installing,
//! Done, in the project's colours. Everything it decides is in flow.rs; this
//! only draws the screen the flow is on and passes clicks and keys back.
//!
//! Keyboard: Enter is the main button, Esc the other (Back / Cancel), Tab
//! moves between controls. Every control has a name screen readers read
//! (AccessKit). It scales with the display (100-200%) without clipping.

use crate::flow::{human, Flow, Machine, Screen};
use crate::theme::Palette;
use crate::worker::{self, AnySteam, Shared};
use eframe::egui::{self, RichText, TextureHandle};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use walnut_core::install::Choices;
use walnut_core::project::{Project, CREDIT};
use walnut_core::steam::SteamProcess;

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
        let art = setup.welcome_art.as_deref().and_then(decode).map(|image| ctx.load_texture("welcome-art", image, egui::TextureOptions::LINEAR));
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
        let mut pressed_primary = false;
        let mut pressed_secondary = false;
        let p = &self.palette;
        // The buttons and the credit, along the bottom.
        egui::Panel::bottom("buttons").frame(egui::Frame::new().fill(p.bar).inner_margin(egui::Margin::symmetric(18, 12))).show(ui, |ui| {
            ui.horizontal(|ui| {
                if self.setup.project.theme.show_credit {
                    ui.label(RichText::new(CREDIT).small().color(p.body.gamma_multiply(0.7)));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(label) = self.flow.primary_label() {
                        let button = ui.add(egui::Button::new(RichText::new(label).strong()));
                        // Keyboard users start on the main button (the folder
                        // field takes it on the Folder screen).
                        if self.flow.screen != Screen::Folder && ui.memory(|m| m.focused().is_none()) {
                            button.request_focus();
                        }
                        pressed_primary |= button.clicked();
                    }
                    if let Some(label) = self.flow.secondary_label() {
                        let button = ui.add(egui::Button::new(label).fill(p.bar).stroke(egui::Stroke::new(1.5, p.accent)));
                        pressed_secondary |= button.clicked();
                    }
                });
            });
        });
        egui::CentralPanel::default().frame(egui::Frame::new().fill(p.background).inner_margin(egui::Margin::symmetric(28, 22))).show(ui, |ui| {
            match self.flow.screen.clone() {
                Screen::Welcome => self.welcome(ui),
                Screen::Folder => self.folder(ui),
                Screen::CloseSteam => self.close_steam(ui),
                Screen::Installing => self.installing(ui),
                Screen::Done => self.done(ui),
                Screen::Cancelled => self.ended(ui, "The install was cancelled.", "Nothing was left behind."),
                Screen::Failed(why) => self.ended(ui, "The install didn't finish.", &format!("{why}. Nothing was left behind.")),
            }
            if let Some(shots) = &mut self.shots {
                shots.note_fit(ui);
            }
        });
        if enter || pressed_primary {
            self.primary(&ctx);
        } else if escape || pressed_secondary {
            self.secondary();
        }
    }
}

impl App {
    fn heading(&self, ui: &mut egui::Ui, text: &str) {
        ui.label(RichText::new(text).size(26.0).color(self.palette.title).strong());
    }

    fn welcome(&mut self, ui: &mut egui::Ui) {
        let game = &self.setup.project.game;
        if let Some(art) = &self.art {
            let width = ui.available_width();
            let size = art.size_vec2();
            let height = (width * size.y / size.x.max(1.0)).min(ui.available_height() * 0.55);
            let shown = ui.add(egui::Image::new(art).fit_to_exact_size(egui::vec2(height * size.x / size.y.max(1.0), height)));
            let name = format!("{} artwork", game.name);
            shown.widget_info(move || egui::WidgetInfo::labeled(egui::Role::Image, true, name.clone()));
            ui.add_space(6.0);
        }
        self.heading(ui, &game.name);
        let mut small = format!("Version {}", game.version);
        if !game.publisher.is_empty() {
            small.push_str(&format!("  ·  {}", game.publisher));
        }
        ui.label(RichText::new(small).small());
        ui.add_space(8.0);
        let welcome = self.setup.project.text.welcome.clone();
        ui.label(if welcome.is_empty() { format!("This will install {} on this computer.", game.name) } else { welcome });
    }

    fn folder(&mut self, ui: &mut egui::Ui) {
        self.heading(ui, "Where should it go?");
        let label = ui.label("Install to");
        ui.horizontal(|ui| {
            let field = ui.add(egui::TextEdit::singleline(&mut self.flow.dir).desired_width(ui.available_width() - 110.0)).labelled_by(label.id);
            if ui.memory(|m| m.focused().is_none()) {
                field.request_focus();
            }
            if ui.button("Browse…").clicked() {
                if let Some(picked) = rfd::FileDialog::new().set_directory(&self.flow.dir).pick_folder() {
                    self.flow.dir = picked.display().to_string();
                }
            }
        });
        let free = walnut_core::space::free_bytes(&self.flow.dir_path());
        ui.label(RichText::new(format!("Needs {}{}", human(self.flow.needed_bytes), free.map(|f| format!(" · {} free there", human(f))).unwrap_or_default())).small());
        ui.add_space(6.0);
        ui.checkbox(&mut self.flow.desktop_entry, "Add it to the applications menu");
        ui.checkbox(&mut self.flow.desktop_shortcut, "Put a shortcut on the desktop");
        if self.flow.steam_offered {
            ui.checkbox(&mut self.flow.add_to_steam, "Add it to my Steam library (with its artwork)");
        }
        if !self.flow.folder_problem.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new(&self.flow.folder_problem).color(self.palette.warn));
        }
    }

    fn close_steam(&mut self, ui: &mut egui::Ui) {
        self.heading(ui, "Close Steam?");
        let name = &self.setup.project.game.name;
        ui.label(format!("Steam is running. To add {name} to your library, Steam has to be closed while the installer changes it."));
        ui.label("Your Steam games and settings aren't touched. You can open Steam again as soon as the install is done.");
    }

    fn installing(&mut self, ui: &mut egui::Ui) {
        self.heading(ui, &format!("Installing {}…", self.setup.project.game.name));
        ui.label(RichText::new(format!("Into {}", self.flow.dir)).small());
        ui.add_space(12.0);
        let fraction = self.flow.fraction();
        let bar = ui.add(egui::ProgressBar::new(fraction).show_percentage().fill(self.palette.accent));
        bar.widget_info(move || egui::WidgetInfo::labeled(egui::Role::ProgressIndicator, true, format!("Installing, {:.0} percent", fraction * 100.0)));
        if self.flow.cancel_asked {
            ui.label("Cancelling, and removing what was copied…");
        }
    }

    fn done(&mut self, ui: &mut egui::Ui) {
        let name = self.setup.project.game.name.clone();
        self.heading(ui, &format!("{name} is installed"));
        let done = self.setup.project.text.done.clone();
        if !done.is_empty() {
            ui.label(done);
        }
        ui.label(RichText::new(format!("To remove it later, run {}", self.flow.dir_path().join(walnut_core::install::UNINSTALLER).display())).small());
        ui.add_space(8.0);
        ui.checkbox(&mut self.flow.launch, format!("Launch {name}"));
    }

    fn ended(&mut self, ui: &mut egui::Ui, title: &str, body: &str) {
        self.heading(ui, title);
        ui.label(body);
    }
}

fn decode(png: &[u8]) -> Option<egui::ColorImage> {
    let image = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw()))
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
        let order = vec![
            ("1-welcome", Screen::Welcome),
            ("2-folder", Screen::Folder),
            ("3-close-steam", Screen::CloseSteam),
            ("4-installing", Screen::Installing),
            ("5-done", Screen::Done),
            ("6-cancelled", Screen::Cancelled),
            ("7-failed", Screen::Failed("The disk is full".into())),
        ];
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
