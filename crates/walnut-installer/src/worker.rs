//! M2: the install, run on its own thread so the window stays responsive.
//! Progress and the result come back through shared state; Cancel sets a
//! flag the unpacker checks between files (walnut-core then rolls back
//! everything it made).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use walnut_core::install::{self, Choices, Env, Receipt};
use walnut_core::steam::SteamProcess;

#[derive(Default)]
pub struct Shared {
    pub done: AtomicU64,
    pub total: AtomicU64,
    pub cancel: AtomicBool,
    pub result: Mutex<Option<Result<Receipt, String>>>,
}

/// Starts installing `installer` with `choices`. `may_close_steam` is the
/// player's answer on the Close Steam screen (never closed without a yes).
pub fn start(installer: PathBuf, choices: Choices, may_close_steam: bool, steam: Box<dyn SteamProcess + Send>, repaint: Box<dyn Fn() + Send>) -> Arc<Shared> {
    let shared = Arc::new(Shared::default());
    let mine = shared.clone();
    std::thread::spawn(move || {
        let ask = move || may_close_steam;
        let env = Env::from_system(steam.as_ref(), &ask);
        let progress_to = mine.clone();
        let mut progress = |done: u64, total: u64| {
            progress_to.done.store(done, Ordering::Relaxed);
            progress_to.total.store(total, Ordering::Relaxed);
            repaint();
        };
        let cancel = mine.clone();
        let keep_going = move || !cancel.cancel.load(Ordering::Relaxed);
        let result = install::install(&installer, &choices, &env, &mut progress, &keep_going);
        *mine.result.lock().unwrap() = Some(result);
    });
    shared
}

/// The real Steam, or the stand-in the tests and screenshots use
/// (`WALNUT_FAKE_STEAM_RUNNING=0|1`).
pub struct AnySteam {
    fake: Option<AtomicBool>,
}

impl AnySteam {
    pub fn from_env() -> AnySteam {
        AnySteam { fake: std::env::var("WALNUT_FAKE_STEAM_RUNNING").ok().map(|v| AtomicBool::new(v == "1")) }
    }
}

impl SteamProcess for AnySteam {
    fn running(&self) -> bool {
        match &self.fake {
            Some(flag) => flag.load(Ordering::Relaxed),
            None => walnut_core::steam::RealSteam.running(),
        }
    }
    fn shutdown(&self) {
        match &self.fake {
            Some(flag) => flag.store(false, Ordering::Relaxed),
            None => walnut_core::steam::RealSteam.shutdown(),
        }
    }
}
