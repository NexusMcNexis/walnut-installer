//! Walnut Installer's screens: the flow between them (no UI, unit-tested),
//! the project's colours, and the drawing of each screen. The installer
//! draws them for real; the builder (M3) draws the same functions as its
//! live preview, so what you see while editing is what players get.

pub mod flow;
pub mod screens;
pub mod theme;

pub use eframe;
pub use eframe::egui;
