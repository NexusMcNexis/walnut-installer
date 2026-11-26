//! A Walnut project (`project.toml`): what to install and how it looks. The
//! builder reads it, packs the game and its art into the payload, and writes
//! the parts the installer needs (the manifest) beside the payload.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Project {
    pub game: Game,
    #[serde(default)]
    pub desktop: Desktop,
    #[serde(default)]
    pub steam: Steam,
    #[serde(default)]
    pub theme: Theme,
    #[serde(default)]
    pub text: Text,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Game {
    /// Shown everywhere a player sees the game (never with a version).
    pub name: String,
    /// A short id: the install folder's and the desktop entry's name
    /// (letters, digits, `-` and `_`).
    pub id: String,
    /// Shown small in the installer; kept in the desktop entry as
    /// `X-Walnut-Version`; never part of a name.
    pub version: String,
    #[serde(default)]
    pub publisher: String,
    /// The game's folder (builder side), relative to the project file.
    #[serde(default)]
    pub folder: String,
    /// The program to start, relative to the game's folder.
    pub executable: String,
    /// A PNG icon (builder side), relative to the project file.
    #[serde(default)]
    pub icon: String,
    /// Where it installs by default: `~` is the player's home.
    #[serde(default = "default_install_dir")]
    pub default_install_dir: String,
}

fn default_install_dir() -> String {
    "~/.local/share/{id}".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Desktop {
    #[serde(default = "yes")]
    pub entry: bool,
    #[serde(default)]
    pub shortcut: bool,
    #[serde(default = "default_categories")]
    pub categories: Vec<String>,
    #[serde(default)]
    pub comment: String,
}

impl Default for Desktop {
    fn default() -> Self {
        Desktop { entry: true, shortcut: false, categories: default_categories(), comment: String::new() }
    }
}

fn default_categories() -> Vec<String> {
    vec!["Game".into()]
}

fn yes() -> bool {
    true
}

/// "Add to Steam library": ticked by default, shown only when Steam is here.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Steam {
    #[serde(default = "yes")]
    pub offer: bool,
    #[serde(default = "yes")]
    pub ticked: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub launch_options: String,
    /// Artwork (builder side, relative to the project file): 600x900
    /// portrait, 920x430 wide, 1920x620 hero, a transparent logo, a square icon.
    #[serde(default)]
    pub portrait: String,
    #[serde(default)]
    pub wide: String,
    #[serde(default)]
    pub hero: String,
    #[serde(default)]
    pub logo: String,
    #[serde(default)]
    pub icon: String,
}

impl Default for Steam {
    fn default() -> Self {
        Steam {
            offer: true,
            ticked: true,
            tags: Vec::new(),
            launch_options: String::new(),
            portrait: String::new(),
            wide: String::new(),
            hero: String::new(),
            logo: String::new(),
            icon: String::new(),
        }
    }
}

/// The installer's colours (RGB hex). Walnut by default.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Theme {
    pub background: String,
    pub bar: String,
    pub accent: String,
    pub title: String,
    pub body: String,
    pub button_text: String,
    /// The footer credit ("Installer built with Walnut Installer by Luke
    /// Woods"), on by default; a project may turn it off.
    pub show_credit: bool,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            background: "#342418".into(),
            bar: "#281B13".into(),
            accent: "#B07C46".into(),
            title: "#F0E2C4".into(),
            body: "#E1D2B4".into(),
            button_text: "#FFF8E6".into(),
            show_credit: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Text {
    #[serde(default)]
    pub welcome: String,
    #[serde(default)]
    pub done: String,
}

pub const CREDIT: &str = "Installer built with Walnut Installer by Luke Woods";

impl Project {
    pub fn from_toml(text: &str) -> Result<Project, String> {
        let project: Project = toml::from_str(text).map_err(|e| format!("project.toml: {e}"))?;
        project.check()?;
        Ok(project)
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).unwrap_or_default()
    }

    /// The default install folder for `home`.
    pub fn install_dir(&self, home: &std::path::Path) -> std::path::PathBuf {
        let path = self.game.default_install_dir.replace("{id}", &self.game.id);
        match path.strip_prefix("~/") {
            Some(rest) => home.join(rest),
            None => std::path::PathBuf::from(path),
        }
    }

    fn check(&self) -> Result<(), String> {
        let id = &self.game.id;
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Err("game.id must be letters, digits, '-' or '_'".into());
        }
        if self.game.name.trim().is_empty() {
            return Err("game.name is empty".into());
        }
        if self.game.name.contains(&self.game.version) && !self.game.version.is_empty() {
            return Err("game.name must not contain the version (it is shown separately)".into());
        }
        if self.game.executable.is_empty() || self.game.executable.contains("..") || self.game.executable.starts_with('/') {
            return Err("game.executable must be a path inside the game's folder".into());
        }
        Ok(())
    }
}
