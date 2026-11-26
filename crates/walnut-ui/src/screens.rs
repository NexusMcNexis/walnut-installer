//! Drawing the installer's screens into any `Ui`: the installer's own
//! window, or the builder's preview pane. What a screen shows comes from the
//! project and the flow; the caller decides what the buttons do.

use crate::flow::{human, Flow, Screen};
use crate::theme::Palette;
use eframe::egui::{self, RichText, TextureHandle};
use walnut_core::project::{Project, CREDIT};

/// What one frame of a screen needs.
pub struct View<'a> {
    pub project: &'a Project,
    pub palette: &'a Palette,
    pub art: Option<&'a TextureHandle>,
    /// The installer itself (keyboard focus, the real free space), or the
    /// builder's preview (drawn exactly the same; the builder ignores its
    /// buttons).
    pub live: bool,
}

/// What was pressed this frame.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct Pressed {
    pub primary: bool,
    pub secondary: bool,
    pub browse: bool,
}

/// Draws the screen the flow is on, with the button bar and the credit.
pub fn draw(ui: &mut egui::Ui, view: &View, flow: &mut Flow) -> Pressed {
    let mut pressed = Pressed::default();
    let p = view.palette;
    egui::Panel::bottom("walnut-buttons").frame(egui::Frame::new().fill(p.bar).inner_margin(egui::Margin::symmetric(18, 12))).show(ui, |ui| {
        ui.scope(|ui| {
            ui.horizontal(|ui| {
                if view.project.theme.show_credit {
                    ui.label(RichText::new(CREDIT).small().color(p.body.gamma_multiply(0.7)));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(label) = flow.primary_label() {
                        let button = ui.add(egui::Button::new(RichText::new(label).strong()));
                        // Keyboard users start on the main button (the folder
                        // field takes it on the Folder screen).
                        if view.live && flow.screen != Screen::Folder && ui.memory(|m| m.focused().is_none()) {
                            button.request_focus();
                        }
                        pressed.primary |= button.clicked();
                    }
                    if let Some(label) = flow.secondary_label() {
                        let button = ui.add(egui::Button::new(label).fill(p.bar).stroke(egui::Stroke::new(1.5, p.accent)));
                        pressed.secondary |= button.clicked();
                    }
                });
            });
        });
    });
    egui::CentralPanel::default().frame(egui::Frame::new().fill(p.background).inner_margin(egui::Margin::symmetric(28, 22))).show(ui, |ui| {
        ui.scope(|ui| match flow.screen.clone() {
            Screen::Welcome => welcome(ui, view),
            Screen::Folder => pressed.browse = folder(ui, view, flow),
            Screen::CloseSteam => close_steam(ui, view),
            Screen::Installing => installing(ui, view, flow),
            Screen::Done => done(ui, view, flow),
            Screen::Cancelled => ended(ui, view, "The install was cancelled.", "Nothing was left behind."),
            Screen::Failed(why) => ended(ui, view, "The install didn't finish.", &format!("{why}. Nothing was left behind.")),
        });
    });
    pressed
}

fn heading(ui: &mut egui::Ui, view: &View, text: &str) {
    ui.label(RichText::new(text).size(26.0).color(view.palette.title).strong());
}

fn welcome(ui: &mut egui::Ui, view: &View) {
    let game = &view.project.game;
    if let Some(art) = view.art {
        let width = ui.available_width();
        let size = art.size_vec2();
        let height = (width * size.y / size.x.max(1.0)).min(ui.available_height() * 0.55);
        let shown = ui.add(egui::Image::new(art).fit_to_exact_size(egui::vec2(height * size.x / size.y.max(1.0), height)));
        let name = format!("{} artwork", game.name);
        shown.widget_info(move || egui::WidgetInfo::labeled(egui::Role::Image, true, name.clone()));
        ui.add_space(6.0);
    }
    heading(ui, view, &game.name);
    let mut small = format!("Version {}", game.version);
    if !game.publisher.is_empty() {
        small.push_str(&format!("  ·  {}", game.publisher));
    }
    ui.label(RichText::new(small).small());
    ui.add_space(8.0);
    let welcome = &view.project.text.welcome;
    ui.label(if welcome.is_empty() { format!("This will install {} on this computer.", game.name) } else { welcome.clone() });
}

/// True when Browse was pressed.
fn folder(ui: &mut egui::Ui, view: &View, flow: &mut Flow) -> bool {
    heading(ui, view, "Where should it go?");
    let label = ui.label("Install to");
    let mut browse = false;
    ui.horizontal(|ui| {
        let field = ui.add(egui::TextEdit::singleline(&mut flow.dir).desired_width(ui.available_width() - 110.0)).labelled_by(label.id);
        if view.live && ui.memory(|m| m.focused().is_none()) {
            field.request_focus();
        }
        browse = ui.button("Browse…").clicked();
    });
    let free = if view.live { walnut_core::space::free_bytes(&flow.dir_path()) } else { None };
    ui.label(RichText::new(format!("Needs {}{}", human(flow.needed_bytes), free.map(|f| format!(" · {} free there", human(f))).unwrap_or_default())).small());
    ui.add_space(6.0);
    ui.checkbox(&mut flow.desktop_entry, "Add it to the applications menu");
    ui.checkbox(&mut flow.desktop_shortcut, "Put a shortcut on the desktop");
    if flow.steam_offered {
        ui.checkbox(&mut flow.add_to_steam, "Add it to my Steam library (with its artwork)");
    }
    if !flow.folder_problem.is_empty() {
        ui.add_space(4.0);
        ui.label(RichText::new(&flow.folder_problem).color(view.palette.warn));
    }
    browse
}

fn close_steam(ui: &mut egui::Ui, view: &View) {
    heading(ui, view, "Close Steam?");
    let name = &view.project.game.name;
    ui.label(format!("Steam is running. To add {name} to your library, Steam has to be closed while the installer changes it."));
    ui.label("Your Steam games and settings aren't touched. You can open Steam again as soon as the install is done.");
}

fn installing(ui: &mut egui::Ui, view: &View, flow: &Flow) {
    heading(ui, view, &format!("Installing {}…", view.project.game.name));
    ui.label(RichText::new(format!("Into {}", flow.dir)).small());
    ui.add_space(12.0);
    let fraction = flow.fraction();
    let bar = ui.add(egui::ProgressBar::new(fraction).show_percentage().fill(view.palette.accent));
    bar.widget_info(move || egui::WidgetInfo::labeled(egui::Role::ProgressIndicator, true, format!("Installing, {:.0} percent", fraction * 100.0)));
    if flow.cancel_asked {
        ui.label("Cancelling, and removing what was copied…");
    }
}

fn done(ui: &mut egui::Ui, view: &View, flow: &mut Flow) {
    let name = view.project.game.name.clone();
    heading(ui, view, &format!("{name} is installed"));
    if !view.project.text.done.is_empty() {
        ui.label(view.project.text.done.clone());
    }
    ui.label(RichText::new(format!("To remove it later, run {}", flow.dir_path().join(walnut_core::install::UNINSTALLER).display())).small());
    ui.add_space(8.0);
    ui.checkbox(&mut flow.launch, format!("Launch {name}"));
}

fn ended(ui: &mut egui::Ui, view: &View, title: &str, body: &str) {
    heading(ui, view, title);
    ui.label(body);
}

/// A PNG as an image egui can show.
pub fn decode(png: &[u8]) -> Option<egui::ColorImage> {
    let image = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw()))
}

/// All the screens a project shows, in order, for previews and screenshots.
pub fn every_screen() -> Vec<(&'static str, Screen)> {
    vec![
        ("Welcome", Screen::Welcome),
        ("Folder", Screen::Folder),
        ("Close Steam?", Screen::CloseSteam),
        ("Installing", Screen::Installing),
        ("Done", Screen::Done),
        ("Cancelled", Screen::Cancelled),
        ("Failed", Screen::Failed("The disk is full".into())),
    ]
}
