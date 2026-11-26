//! The project's art slots and the sizes each needs (Steam's library sizes,
//! from Valve's own guidance; the welcome art may be any size).

use std::path::Path;
use walnut_core::project::Project;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Welcome,
    GameIcon,
    Portrait,
    Wide,
    Hero,
    Logo,
    SteamIcon,
}

impl Slot {
    pub fn all() -> [Slot; 7] {
        [Slot::Welcome, Slot::GameIcon, Slot::Portrait, Slot::Wide, Slot::Hero, Slot::Logo, Slot::SteamIcon]
    }

    pub fn label(self) -> &'static str {
        match self {
            Slot::Welcome => "Welcome art (any size; wide works best)",
            Slot::GameIcon => "Game icon (square, 256 px or more)",
            Slot::Portrait => "Steam portrait (600x900)",
            Slot::Wide => "Steam wide (920x430)",
            Slot::Hero => "Steam hero (1920x620)",
            Slot::Logo => "Steam logo (transparent PNG)",
            Slot::SteamIcon => "Steam icon (square)",
        }
    }

    /// The project field this slot fills.
    pub fn path(self, project: &mut Project) -> &mut String {
        match self {
            Slot::Welcome => &mut project.art.welcome,
            Slot::GameIcon => &mut project.game.icon,
            Slot::Portrait => &mut project.steam.portrait,
            Slot::Wide => &mut project.steam.wide,
            Slot::Hero => &mut project.steam.hero,
            Slot::Logo => &mut project.steam.logo,
            Slot::SteamIcon => &mut project.steam.icon,
        }
    }
}

/// Whether art of `size` (and with or without transparency) suits `slot`;
/// the reason in words when it doesn't.
pub fn check(slot: Slot, size: (u32, u32), alpha: bool) -> Result<(), String> {
    let exact = |want: (u32, u32), name: &str| {
        if size == want { Ok(()) } else { Err(format!("{name} must be {}x{} (this is {}x{}).", want.0, want.1, size.0, size.1)) }
    };
    match slot {
        Slot::Welcome => Ok(()),
        Slot::Portrait => exact((600, 900), "The Steam portrait"),
        Slot::Wide => exact((920, 430), "The Steam wide art"),
        Slot::Hero => exact((1920, 620), "The Steam hero"),
        Slot::Logo if !alpha => Err("The Steam logo must have transparency (a PNG with alpha), so it sits on the hero.".into()),
        Slot::Logo => Ok(()),
        Slot::SteamIcon | Slot::GameIcon if size.0 != size.1 => Err(format!("The icon must be square (this is {}x{}).", size.0, size.1)),
        Slot::GameIcon if size.0 < 64 => Err(format!("The game icon must be at least 64x64, better 256x256 (this is {}x{}).", size.0, size.1)),
        Slot::SteamIcon | Slot::GameIcon => Ok(()),
    }
}

/// A PNG's size and whether it has an alpha channel.
pub fn measure(path: &Path) -> Result<((u32, u32), bool), String> {
    let reader = image::ImageReader::open(path).map_err(|e| format!("{}: {e}", path.display()))?.with_guessed_format().map_err(|e| e.to_string())?;
    let image = reader.decode().map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(((image.width(), image.height()), image.color().has_alpha()))
}
