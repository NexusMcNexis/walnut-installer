//! M2: the project's colours as egui's look (walnut and brass by default).

use eframe::egui::{self, Color32, CornerRadius, Stroke};
use walnut_core::project::Theme;

pub struct Palette {
    pub background: Color32,
    pub bar: Color32,
    pub accent: Color32,
    pub title: Color32,
    pub body: Color32,
    pub button_text: Color32,
    /// For problems (a folder that won't do): the accent's warm red cousin.
    pub warn: Color32,
}

/// `#RRGGBB` as a colour; `fallback` if it isn't one.
pub fn hex(text: &str, fallback: Color32) -> Color32 {
    let digits = text.trim().trim_start_matches('#');
    if digits.len() != 6 {
        return fallback;
    }
    match u32::from_str_radix(digits, 16) {
        Ok(v) => Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8),
        Err(_) => fallback,
    }
}

impl Palette {
    pub fn of(theme: &Theme) -> Palette {
        Palette {
            background: hex(&theme.background, Color32::from_rgb(0x34, 0x24, 0x18)),
            bar: hex(&theme.bar, Color32::from_rgb(0x28, 0x1b, 0x13)),
            accent: hex(&theme.accent, Color32::from_rgb(0xb0, 0x7c, 0x46)),
            title: hex(&theme.title, Color32::from_rgb(0xf0, 0xe2, 0xc4)),
            body: hex(&theme.body, Color32::from_rgb(0xe1, 0xd2, 0xb4)),
            button_text: hex(&theme.button_text, Color32::from_rgb(0xff, 0xf8, 0xe6)),
            warn: Color32::from_rgb(0xe0, 0x8a, 0x6a),
        }
    }

    /// egui's look in these colours: dark walnut panels, brass buttons with
    /// light text, a clear focus ring for keyboard use.
    pub fn apply(&self, ctx: &egui::Context) {
        let visuals = self.visuals();
        // Always the walnut look, whatever the desktop's light or dark.
        ctx.set_theme(egui::Theme::Dark);
        ctx.set_visuals_of(egui::Theme::Dark, visuals);
        ctx.style_mut_of(egui::Theme::Dark, spacing);
    }

    /// M3: just one `Ui` in these colours (the builder's preview), the rest
    /// of the window left as it is.
    pub fn style_ui(&self, ui: &mut egui::Ui) {
        ui.style_mut().visuals = self.visuals();
        spacing(ui.style_mut());
    }

    pub fn visuals(&self) -> egui::Visuals {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = self.background;
        visuals.window_fill = self.bar;
        visuals.extreme_bg_color = self.bar;
        visuals.override_text_color = Some(self.body);
        visuals.selection.bg_fill = self.accent;
        visuals.selection.stroke = Stroke::new(1.5, self.title);
        let round = CornerRadius::same(4);
        for (widget, fill, text) in [
            (&mut visuals.widgets.inactive, self.accent, self.button_text),
            (&mut visuals.widgets.hovered, brighten(self.accent), self.button_text),
            (&mut visuals.widgets.active, darken(self.accent), self.button_text),
        ] {
            widget.bg_fill = fill;
            widget.weak_bg_fill = fill;
            widget.fg_stroke = Stroke::new(1.5, text);
            widget.corner_radius = round;
        }
        visuals.widgets.hovered.bg_stroke = Stroke::new(2.0, self.title);
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, self.body);
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, darken(self.accent));
        visuals
    }
}

fn spacing(style: &mut egui::Style) {
    style.spacing.button_padding = egui::vec2(14.0, 7.0);
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.interact_size.y = 30.0;
}

fn brighten(c: Color32) -> Color32 {
    Color32::from_rgb(c.r().saturating_add(18), c.g().saturating_add(14), c.b().saturating_add(10))
}

fn darken(c: Color32) -> Color32 {
    Color32::from_rgb(c.r().saturating_sub(30), c.g().saturating_sub(24), c.b().saturating_sub(18))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_come_from_the_project_and_bad_ones_fall_back() {
        assert_eq!(hex("#B07C46", Color32::BLACK), Color32::from_rgb(0xb0, 0x7c, 0x46));
        assert_eq!(hex("B07C46", Color32::BLACK), Color32::from_rgb(0xb0, 0x7c, 0x46));
        assert_eq!(hex("#nope00", Color32::RED), Color32::RED);
        assert_eq!(hex("#123", Color32::RED), Color32::RED);
        let walnut = Palette::of(&Theme::default());
        assert_eq!(walnut.background, Color32::from_rgb(0x34, 0x24, 0x18));
    }
}
