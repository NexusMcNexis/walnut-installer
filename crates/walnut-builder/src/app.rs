//! The builder's window: the form on the left (every project field), the
//! live preview on the right (the installer's own screens, in the
//! project's colours), and Open / Save / Build along the top. Art can be
//! chosen or dropped onto its slot, and is checked for size at once.

use std::path::{Path, PathBuf};
use walnut_builder::art::{self, Slot};
use walnut_builder::doc::Document;
use walnut_core::project::Project;
use walnut_ui::egui::{self, RichText, TextureHandle};
use walnut_ui::flow::Flow;
use walnut_ui::screens::{self, View};
use walnut_ui::theme::{hex, Palette};
use walnut_ui::eframe;

/// The preview's size: the installer's own window at 100%.
const PREVIEW: egui::Vec2 = egui::vec2(760.0, 520.0);

/// The installer program a build puts at the front: `walnut-installer`
/// beside this program.
pub fn installer_program() -> PathBuf {
    std::env::current_exe().ok().and_then(|me| me.parent().map(|dir| dir.join("walnut-installer"))).unwrap_or_else(|| PathBuf::from("walnut-installer"))
}

pub fn run(project: Option<PathBuf>) -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("Walnut Builder").with_inner_size([1360.0, 860.0]).with_min_inner_size([1240.0, 760.0]).with_app_id("walnut-builder"),
        ..Default::default()
    };
    eframe::run_native("Walnut Builder", options, Box::new(move |cc| {
        Palette::of(&Default::default()).apply(&cc.egui_ctx);
        Ok(Box::new(Builder::new(project)))
    }))
    .map_err(|e| e.to_string())
}

struct Builder {
    doc: Option<Document>,
    screen: usize,
    flow: Flow,
    art: Option<(String, TextureHandle)>,
    said: String,
    problems: Vec<String>,
    shots: Option<(PathBuf, usize, u32, std::sync::Arc<std::sync::Mutex<Option<std::sync::Arc<egui::ColorImage>>>>)>,
}

impl Builder {
    fn new(project: Option<PathBuf>) -> Builder {
        let mut builder = Builder {
            doc: None,
            screen: 0,
            flow: preview_flow(&Project::from_toml(SAMPLE).unwrap()),
            art: None,
            said: String::new(),
            problems: Vec::new(),
            shots: std::env::var("WALNUT_SCREENSHOTS").ok().map(|dir| (PathBuf::from(dir), 0, 0, Default::default())),
        };
        if let Some(path) = project {
            builder.open(&path);
        }
        builder
    }

    fn open(&mut self, path: &Path) {
        match Document::open(path) {
            Ok(doc) => {
                self.flow = preview_flow(&doc.project);
                self.flow.needed_bytes = doc.size();
                self.problems = doc.problems();
                self.said = format!("Opened {}", path.display());
                self.doc = Some(doc);
                self.art = None;
            }
            Err(why) => self.said = why,
        }
    }

    fn welcome_texture(&mut self, ctx: &egui::Context) -> Option<TextureHandle> {
        let doc = self.doc.as_ref()?;
        let path = doc.project.art.welcome.clone();
        if path.is_empty() {
            return None;
        }
        if self.art.as_ref().map(|(was, _)| was != &path).unwrap_or(true) {
            let bytes = std::fs::read(doc.dir().join(&path)).ok()?;
            let image = screens::decode(&bytes)?;
            self.art = Some((path, ctx.load_texture("preview-welcome", image, egui::TextureOptions::LINEAR)));
        }
        self.art.as_ref().map(|(_, texture)| texture.clone())
    }
}

/// A flow for the preview: the project's choices, a stand-in player's path.
fn preview_flow(project: &Project) -> Flow {
    let mut flow = Flow::new(Path::new(&format!("/home/player/.local/share/{}", project.game.id)), project.build.install_bytes, project.desktop.entry, project.desktop.shortcut, true, project.steam.ticked);
    flow.progress(42, 100);
    flow
}

impl eframe::App for Builder {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
        egui::Panel::top("bar").show(ui, |ui| self.bar(ui));
        egui::Panel::left("form").resizable(true).default_size(420.0).size_range(340.0..=520.0).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.form(ui, &dropped));
        });
        let art = self.welcome_texture(&ctx);
        egui::CentralPanel::default().show(ui, |ui| self.preview(ui, art.as_ref()));
        self.screenshots(&ctx);
    }
}

impl Builder {
    fn bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Walnut Builder").strong().size(18.0));
            if ui.button("Open…").clicked() {
                if let Some(path) = rfd::FileDialog::new().add_filter("Walnut project", &["toml"]).pick_file() {
                    self.open(&path);
                }
            }
            let has = self.doc.is_some();
            if ui.add_enabled(has, egui::Button::new("Save")).clicked() {
                if let Some(doc) = &self.doc {
                    self.said = match doc.save() { Ok(()) => format!("Saved {}", doc.path.display()), Err(why) => why };
                }
            }
            if ui.add_enabled(has && self.problems.is_empty(), egui::Button::new("Build installer…")).clicked() {
                self.build();
            }
            ui.label(RichText::new(&self.said).small());
        });
    }

    fn build(&mut self) {
        let Some(doc) = &self.doc else { return };
        let name = format!("{}-installer", doc.project.game.id);
        let Some(out) = rfd::FileDialog::new().set_directory(doc.dir()).set_file_name(&name).save_file() else { return };
        let program = installer_program();
        self.said = match doc.save().and_then(|()| doc.build(&program, &out)) {
            Ok(project) => format!("Built {} ({} {}). The same file `walnut build` makes.", out.display(), project.game.name, project.game.version),
            Err(why) => why,
        };
    }

    fn form(&mut self, ui: &mut egui::Ui, dropped: &[PathBuf]) {
        let Some(doc) = &mut self.doc else {
            ui.label("Open a project.toml to start (examples/hello has one).");
            return;
        };
        let dir = doc.dir();
        let before = doc.project.clone();
        let p = &mut doc.project;
        section(ui, "The game", |ui| {
            text(ui, "Name", &mut p.game.name);
            text(ui, "Version (shown small, never in the name)", &mut p.game.version);
            text(ui, "Publisher", &mut p.game.publisher);
            text(ui, "Id (folder and menu entry name)", &mut p.game.id);
            text(ui, "The game's folder", &mut p.game.folder);
            text(ui, "The program to start, inside it", &mut p.game.executable);
            text(ui, "Default install folder (~ is home)", &mut p.game.default_install_dir);
        });
        section(ui, "Desktop", |ui| {
            ui.checkbox(&mut p.desktop.entry, "Ticked at first: add to the applications menu");
            ui.checkbox(&mut p.desktop.shortcut, "Ticked at first: a desktop shortcut");
            text(ui, "Comment", &mut p.desktop.comment);
            let mut categories = p.desktop.categories.join(";");
            text(ui, "Categories (; between)", &mut categories);
            p.desktop.categories = categories.split(';').map(str::trim).filter(|c| !c.is_empty()).map(String::from).collect();
        });
        section(ui, "Steam", |ui| {
            ui.checkbox(&mut p.steam.offer, "Offer \"Add to Steam\" (only shown when Steam is there)");
            ui.checkbox(&mut p.steam.ticked, "Ticked by default");
            text(ui, "Launch options", &mut p.steam.launch_options);
        });
        section(ui, "Art (choose, or drop a PNG on its row)", |ui| {
            for slot in Slot::all() {
                art_row(ui, slot, p, &dir, dropped);
            }
        });
        section(ui, "Colours", |ui| {
            colour(ui, "Background", &mut p.theme.background);
            colour(ui, "Bar", &mut p.theme.bar);
            colour(ui, "Accent (buttons)", &mut p.theme.accent);
            colour(ui, "Titles", &mut p.theme.title);
            colour(ui, "Text", &mut p.theme.body);
            colour(ui, "Button text", &mut p.theme.button_text);
            ui.checkbox(&mut p.theme.show_credit, "Show \"Installer built with Walnut Installer\"");
        });
        section(ui, "Words", |ui| {
            ui.label("Welcome");
            ui.add(egui::TextEdit::multiline(&mut p.text.welcome).desired_rows(2).desired_width(f32::INFINITY));
            ui.label("Done");
            ui.add(egui::TextEdit::multiline(&mut p.text.done).desired_rows(2).desired_width(f32::INFINITY));
        });
        if doc.project != before {
            self.problems = doc.problems();
            let (screen, dir_text, progress) = (self.flow.screen.clone(), self.flow.dir.clone(), self.flow.done_bytes);
            self.flow = preview_flow(&doc.project);
            self.flow.needed_bytes = doc.size();
            self.flow.screen = screen;
            self.flow.dir = dir_text;
            self.flow.done_bytes = progress;
        }
        ui.separator();
        if self.problems.is_empty() {
            ui.label(RichText::new("Ready to build.").color(egui::Color32::from_rgb(0x8f, 0xb5, 0x73)));
        }
        for problem in &self.problems {
            ui.label(RichText::new(problem).color(egui::Color32::from_rgb(0xe0, 0x8a, 0x6a)));
        }
    }

    fn preview(&mut self, ui: &mut egui::Ui, art: Option<&TextureHandle>) {
        ui.horizontal_wrapped(|ui| {
            for (index, (name, _)) in screens::every_screen().iter().enumerate() {
                if ui.selectable_label(self.screen == index, *name).clicked() {
                    self.screen = index;
                }
            }
        });
        ui.label(RichText::new("The installer as players will see it (at 100%).").small());
        let Some(doc) = &self.doc else { return };
        self.flow.screen = screens::every_screen()[self.screen].1.clone();
        let palette = Palette::of(&doc.project.theme);
        let (rect, _) = ui.allocate_exact_size(PREVIEW, egui::Sense::hover());
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Min)));
        child.set_clip_rect(rect);
        palette.style_ui(&mut child);
        let view = View { project: &doc.project, palette: &palette, art, live: false };
        screens::draw(&mut child, &view, &mut self.flow);
        ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(1.0, palette.accent), egui::StrokeKind::Outside);
    }

    /// WALNUT_SCREENSHOTS: one shot per preview tab, then quit.
    fn screenshots(&mut self, ctx: &egui::Context) {
        let Some((dir, at, frames, slot)) = &mut self.shots else { return };
        if let Some(image) = slot.lock().unwrap().take() {
            let path = dir.join(format!("builder-{}.png", at));
            let _ = std::fs::create_dir_all(&*dir);
            let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
            let _ = image::save_buffer(&path, &rgba, image.size[0] as u32, image.size[1] as u32, image::ColorType::Rgba8);
            println!("SHOT {} {}x{}", path.display(), image.size[0], image.size[1]);
            *at += 1;
            *frames = 0;
        }
        if *at >= 3 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        self.screen = [0, 1, 4][*at];
        *frames += 1;
        if *frames == 8 {
            let slot = slot.clone();
            ctx.request_screenshot(move |image| *slot.lock().unwrap() = Some(image));
        }
        ctx.request_repaint();
    }
}

fn section(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    egui::CollapsingHeader::new(RichText::new(title).strong()).default_open(true).show(ui, add);
}

fn text(ui: &mut egui::Ui, label: &str, value: &mut String) {
    let shown = ui.label(label);
    ui.add(egui::TextEdit::singleline(value).desired_width(f32::INFINITY)).labelled_by(shown.id);
}

fn colour(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.horizontal(|ui| {
        let mut rgb = hex(value, egui::Color32::BLACK).to_array();
        let mut picked = [rgb[0], rgb[1], rgb[2]];
        if ui.color_edit_button_srgb(&mut picked).changed() {
            rgb = [picked[0], picked[1], picked[2], 255];
            *value = format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]);
        }
        ui.label(format!("{label}  {value}"));
    });
}

/// One art slot: its path, Choose…, a drop onto the row, and its check.
fn art_row(ui: &mut egui::Ui, slot: Slot, project: &mut Project, dir: &Path, dropped: &[PathBuf]) {
    let row = ui.group(|ui| {
        ui.label(slot.label());
        ui.horizontal(|ui| {
            let path = slot.path(project);
            ui.add(egui::TextEdit::singleline(path).desired_width(ui.available_width() - 90.0));
            if ui.button("Choose…").clicked() {
                if let Some(picked) = rfd::FileDialog::new().add_filter("PNG", &["png"]).set_directory(dir).pick_file() {
                    *path = relative(&picked, dir);
                }
            }
        });
        let path = slot.path(project).clone();
        if !path.is_empty() {
            match art::measure(&dir.join(&path)).and_then(|(size, alpha)| art::check(slot, size, alpha).map(|()| size)) {
                Ok(size) => ui.label(RichText::new(format!("✓ {}x{}", size.0, size.1)).small()),
                Err(why) => ui.label(RichText::new(why).small().color(egui::Color32::from_rgb(0xe0, 0x8a, 0x6a))),
            };
        }
    });
    if !dropped.is_empty() && row.response.contains_pointer() {
        if let Some(file) = dropped.iter().find(|f| f.extension().map(|e| e.eq_ignore_ascii_case("png")).unwrap_or(false)) {
            *slot.path(project) = relative(file, dir);
        }
    }
}

/// `path` relative to the project's folder when it is inside it.
fn relative(path: &Path, dir: &Path) -> String {
    path.strip_prefix(dir).map(|p| p.display().to_string()).unwrap_or_else(|_| path.display().to_string())
}

/// A stand-in project for the preview before one is opened.
const SAMPLE: &str = "[game]\nname = \"Your Game\"\nid = \"your-game\"\nversion = \"1.0\"\nexecutable = \"game\"\n";
