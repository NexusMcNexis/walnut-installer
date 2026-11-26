//! M2: the installer's screens as a plain state machine, with no UI in it,
//! so every step is unit-tested. The GUI (gui.rs) draws whatever screen
//! this says and calls the methods below on clicks and keys.
//!
//! Welcome -> Folder -> (Close Steam?) -> Installing -> Done
//!                                            |-> Cancelled (rolled back)
//!                                            |-> Failed (rolled back)

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Welcome,
    Folder,
    /// Steam is running and "Add to Steam" is ticked: close it first?
    CloseSteam,
    Installing,
    Done,
    Cancelled,
    Failed(String),
}

/// What the flow needs from the machine (tests give their own).
pub trait Machine {
    /// Bytes free where `dir` would go; None if it can't be told.
    fn free_bytes(&self, dir: &Path) -> Option<u64>;
    /// Whether a folder can be made (or written) at `dir`.
    fn writable(&self, dir: &Path) -> bool;
    fn steam_running(&self) -> bool;
}

#[derive(Debug, Clone)]
pub struct Flow {
    pub screen: Screen,
    pub dir: String,
    pub desktop_entry: bool,
    pub desktop_shortcut: bool,
    /// Only offered (and only drawn) when Steam is on this machine.
    pub steam_offered: bool,
    pub add_to_steam: bool,
    pub launch: bool,
    /// What the install takes (from the manifest).
    pub needed_bytes: u64,
    /// Why Next is refused on the Folder screen, in words; empty when fine.
    pub folder_problem: String,
    /// The player agreed Steam may be closed.
    pub may_close_steam: bool,
    pub done_bytes: u64,
    pub total_bytes: u64,
    /// Set by Cancel while installing; the worker stops and rolls back.
    pub cancel_asked: bool,
}

impl Flow {
    pub fn new(default_dir: &Path, needed_bytes: u64, entry: bool, shortcut: bool, steam_offered: bool, steam_ticked: bool) -> Flow {
        Flow {
            screen: Screen::Welcome,
            dir: default_dir.display().to_string(),
            desktop_entry: entry,
            desktop_shortcut: shortcut,
            steam_offered,
            add_to_steam: steam_offered && steam_ticked,
            launch: true,
            needed_bytes,
            folder_problem: String::new(),
            may_close_steam: false,
            done_bytes: 0,
            total_bytes: 0,
            cancel_asked: false,
        }
    }

    /// The main button's words on this screen (None: there is none).
    pub fn primary_label(&self) -> Option<&'static str> {
        match self.screen {
            Screen::Welcome => Some("Next"),
            Screen::Folder => Some("Install"),
            Screen::CloseSteam => Some("Close Steam and install"),
            Screen::Installing => None,
            Screen::Done | Screen::Cancelled | Screen::Failed(_) => Some("Finish"),
        }
    }

    /// The other button (Back, Cancel, or none).
    pub fn secondary_label(&self) -> Option<&'static str> {
        match self.screen {
            Screen::Welcome => Some("Cancel"),
            Screen::Folder => Some("Back"),
            Screen::CloseSteam => Some("Back"),
            Screen::Installing => Some("Cancel"),
            _ => None,
        }
    }

    /// The main button (Enter). True when the installer should start the
    /// install now (the caller starts the worker).
    pub fn primary(&mut self, machine: &dyn Machine) -> bool {
        match self.screen {
            Screen::Welcome => {
                self.screen = Screen::Folder;
                false
            }
            Screen::Folder => {
                self.folder_problem = self.check_folder(machine);
                if !self.folder_problem.is_empty() {
                    return false;
                }
                if self.steam_offered && self.add_to_steam && machine.steam_running() {
                    self.screen = Screen::CloseSteam;
                    return false;
                }
                self.start()
            }
            Screen::CloseSteam => {
                self.may_close_steam = true;
                self.start()
            }
            _ => false,
        }
    }

    /// The other button (Esc). True when the installer should close.
    pub fn secondary(&mut self) -> bool {
        match self.screen {
            Screen::Welcome => true,
            Screen::Folder => {
                self.screen = Screen::Welcome;
                false
            }
            Screen::CloseSteam => {
                self.screen = Screen::Folder;
                false
            }
            Screen::Installing => {
                self.cancel_asked = true;
                false
            }
            // Finish, or Esc on a last screen, closes.
            _ => true,
        }
    }

    fn start(&mut self) -> bool {
        self.screen = Screen::Installing;
        self.cancel_asked = false;
        self.done_bytes = 0;
        true
    }

    /// Why the folder won't do, in the player's words; empty when it will.
    pub fn check_folder(&self, machine: &dyn Machine) -> String {
        let dir = self.dir.trim();
        if dir.is_empty() {
            return "Choose a folder to install into.".into();
        }
        let path = Path::new(dir);
        if !path.is_absolute() {
            return "Choose a full folder path (starting with /).".into();
        }
        if !machine.writable(path) {
            return "You can't write to that folder. Choose one in your home folder.".into();
        }
        if let Some(free) = machine.free_bytes(path) {
            if free < self.needed_bytes {
                return format!("Not enough space there: it needs {}, and {} is free.", human(self.needed_bytes), human(free));
            }
        }
        String::new()
    }

    pub fn dir_path(&self) -> PathBuf {
        PathBuf::from(self.dir.trim())
    }

    /// The worker's progress.
    pub fn progress(&mut self, done: u64, total: u64) {
        self.done_bytes = done;
        self.total_bytes = total;
    }

    pub fn fraction(&self) -> f32 {
        if self.total_bytes == 0 { 0.0 } else { (self.done_bytes as f64 / self.total_bytes as f64).clamp(0.0, 1.0) as f32 }
    }

    /// The worker finished: Ok, cancelled (rolled back), or an error (rolled back).
    pub fn finished(&mut self, result: Result<(), String>) {
        self.screen = match result {
            Ok(()) => Screen::Done,
            Err(_) if self.cancel_asked => Screen::Cancelled,
            Err(why) => Screen::Failed(why),
        };
    }

    /// Launch on Finish only after a good install with the box ticked.
    pub fn launch_on_finish(&self) -> bool {
        self.screen == Screen::Done && self.launch
    }
}

/// Bytes as a person reads them.
pub fn human(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["bytes", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 { format!("{bytes} bytes") } else { format!("{value:.1} {}", UNITS[unit]) }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake {
        free: Option<u64>,
        writable: bool,
        steam: bool,
    }

    impl Machine for Fake {
        fn free_bytes(&self, _dir: &Path) -> Option<u64> {
            self.free
        }
        fn writable(&self, _dir: &Path) -> bool {
            self.writable
        }
        fn steam_running(&self) -> bool {
            self.steam
        }
    }

    fn fine() -> Fake {
        Fake { free: Some(10_000_000_000), writable: true, steam: false }
    }

    fn flow(steam_offered: bool) -> Flow {
        Flow::new(Path::new("/home/player/.local/share/game"), 2_000_000_000, true, false, steam_offered, true)
    }

    #[test]
    fn welcome_folder_install_done() {
        let mut f = flow(true);
        assert_eq!(f.screen, Screen::Welcome);
        assert_eq!(f.primary_label(), Some("Next"));
        assert!(!f.primary(&fine()));
        assert_eq!(f.screen, Screen::Folder);
        assert_eq!(f.primary_label(), Some("Install"));
        assert!(f.primary(&fine()), "Install starts the worker");
        assert_eq!(f.screen, Screen::Installing);
        assert_eq!(f.primary_label(), None, "nothing to press but Cancel while installing");
        f.progress(50, 100);
        assert!((f.fraction() - 0.5).abs() < 1e-6);
        f.finished(Ok(()));
        assert_eq!(f.screen, Screen::Done);
        assert!(f.launch_on_finish(), "Launch is ticked by default");
        f.launch = false;
        assert!(!f.launch_on_finish());
        assert_eq!(f.primary_label(), Some("Finish"));
        assert!(f.secondary(), "Esc on the last screen closes");
    }

    #[test]
    fn back_and_esc() {
        let mut f = flow(false);
        f.primary(&fine());
        assert!(!f.secondary(), "Back from Folder");
        assert_eq!(f.screen, Screen::Welcome);
        assert!(f.secondary(), "Cancel on Welcome closes the installer");
    }

    #[test]
    fn the_folder_is_checked_before_installing() {
        let mut f = flow(false);
        f.primary(&fine());
        f.dir = "relative/path".into();
        assert!(!f.primary(&fine()));
        assert!(f.folder_problem.contains("full folder path"));
        f.dir = "/root/nope".into();
        assert!(!f.primary(&Fake { writable: false, ..fine() }));
        assert!(f.folder_problem.contains("can't write"));
        f.dir = "/home/player/games".into();
        assert!(!f.primary(&Fake { free: Some(1_000_000_000), ..fine() }));
        assert!(f.folder_problem.contains("Not enough space") && f.folder_problem.contains("2.0 GB") && f.folder_problem.contains("1.0 GB"), "{}", f.folder_problem);
        assert_eq!(f.screen, Screen::Folder, "still on the folder screen");
        assert!(f.primary(&Fake { free: None, ..fine() }), "unknown space doesn't block (the install still rolls back if full)");
    }

    #[test]
    fn steam_is_closed_only_after_a_yes() {
        let mut f = flow(true);
        f.primary(&fine());
        let running = Fake { steam: true, ..fine() };
        assert!(!f.primary(&running), "Steam running and Add to Steam ticked: ask first");
        assert_eq!(f.screen, Screen::CloseSteam);
        assert!(!f.secondary(), "Back");
        assert_eq!(f.screen, Screen::Folder);
        assert!(!f.may_close_steam, "no yes, no closing");
        f.primary(&running);
        assert!(f.primary(&running), "the yes starts the install");
        assert!(f.may_close_steam);
        let mut g = flow(true);
        g.primary(&fine());
        g.add_to_steam = false;
        assert!(g.primary(&running), "not adding to Steam: never asked");
        assert!(!g.may_close_steam);
    }

    #[test]
    fn without_steam_the_box_is_not_offered() {
        let f = flow(false);
        assert!(!f.steam_offered && !f.add_to_steam);
    }

    #[test]
    fn cancel_rolls_back_and_a_failure_says_why() {
        let mut f = flow(false);
        f.primary(&fine());
        f.primary(&fine());
        assert!(!f.secondary(), "Cancel while installing asks the worker to stop");
        assert!(f.cancel_asked);
        f.finished(Err("cancelled".into()));
        assert_eq!(f.screen, Screen::Cancelled);
        assert!(!f.launch_on_finish());
        let mut g = flow(false);
        g.primary(&fine());
        g.primary(&fine());
        g.finished(Err("disk full".into()));
        assert_eq!(g.screen, Screen::Failed("disk full".into()));
    }

    #[test]
    fn sizes_read_like_a_person_would_say_them() {
        assert_eq!(human(512), "512 bytes");
        assert_eq!(human(1_500_000), "1.5 MB");
        assert_eq!(human(2_000_000_000), "2.0 GB");
    }
}
