//! The project being edited: where it lives, what it says, what is wrong
//! with it, saving it, and building it.

use crate::art::{self, Slot};
use std::path::{Path, PathBuf};
use walnut_core::project::Project;

pub struct Document {
    pub path: PathBuf,
    pub project: Project,
}

impl Document {
    pub fn open(path: &Path) -> Result<Document, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Document { path: path.to_path_buf(), project: Project::from_toml(&text)? })
    }

    /// The folder the project's paths are relative to.
    pub fn dir(&self) -> PathBuf {
        self.path.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."))
    }

    pub fn save(&self) -> Result<(), String> {
        let mut saved = self.project.clone();
        // What the builder measured is written at build time, not kept.
        saved.build = Default::default();
        std::fs::write(&self.path, saved.to_toml()).map_err(|e| format!("{}: {e}", self.path.display()))
    }

    /// Everything that would stop a build or make bad art, in words.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Err(why) = self.project.validate() {
            out.push(why);
        }
        if let Err(why) = walnut_core::build::payload_files(&self.project, &self.dir()) {
            out.push(why);
        }
        let mut project = self.project.clone();
        for slot in Slot::all() {
            let path = slot.path(&mut project).clone();
            if path.is_empty() {
                continue;
            }
            match art::measure(&self.dir().join(&path)) {
                Ok((size, alpha)) => {
                    if let Err(why) = art::check(slot, size, alpha) {
                        out.push(why);
                    }
                }
                Err(why) => out.push(why),
            }
        }
        out
    }

    /// What the install would take (the files a build packs), for the
    /// preview's Folder screen; 0 when the game's folder isn't there yet.
    pub fn size(&self) -> u64 {
        walnut_core::build::payload_files(&self.project, &self.dir())
            .map(|files| files.iter().map(|(_, path)| std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)).sum())
            .unwrap_or(0)
    }

    /// Builds the installer, the same way `walnut build` does.
    pub fn build(&self, program: &Path, out: &Path) -> Result<Project, String> {
        walnut_core::build::build_from(&self.project, &self.dir(), program, out)
    }
}
