//! Building an installer from a project file: the game's folder, its icon
//! and its Steam art are packed into the payload and appended to the
//! installer program with the project (the manifest) and a checksum.

use crate::payload;
use crate::project::Project;
use std::fs;
use std::path::{Path, PathBuf};

/// The files of `project` (loaded from `project_dir`) as payload entries:
/// the game's folder at the root, Walnut's own files under `.walnut/`.
pub fn payload_files(project: &Project, project_dir: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let folder = project_dir.join(&project.game.folder);
    if !folder.is_dir() {
        return Err(format!("the game's folder {} isn't there", folder.display()));
    }
    let mut files = Vec::new();
    walk(&folder, &folder, &mut files)?;
    files.sort();
    if !files.iter().any(|(name, _)| name == &project.game.executable) {
        return Err(format!("{} isn't in the game's folder", project.game.executable));
    }
    // M2: the welcome art goes first, so the installer finds it at once.
    if !project.art.welcome.is_empty() {
        files.insert(0, (payload::WELCOME_ART.into(), project_dir.join(&project.art.welcome)));
    }
    if !project.game.icon.is_empty() {
        files.push((".walnut/icon.png".into(), project_dir.join(&project.game.icon)));
    }
    let steam = &project.steam;
    for (slot, path) in [("portrait", &steam.portrait), ("wide", &steam.wide), ("hero", &steam.hero), ("logo", &steam.logo), ("icon", &steam.icon)] {
        if !path.is_empty() {
            files.push((format!(".walnut/steam/{slot}.png"), project_dir.join(path)));
        }
    }
    for (_, path) in &files {
        if !path.is_file() {
            return Err(format!("{} isn't there", path.display()));
        }
    }
    Ok(files)
}

/// Builds the installer for the project file at `project_file` into `out`,
/// with `program` (the walnut-installer binary) at its front.
pub fn build(project_file: &Path, program: &Path, out: &Path) -> Result<Project, String> {
    let text = fs::read_to_string(project_file).map_err(|e| format!("{}: {e}", project_file.display()))?;
    let project = Project::from_toml(&text)?;
    build_from(&project, project_file.parent().unwrap_or(Path::new(".")), program, out)
}

/// M3: builds `project` (its paths relative to `dir`) into `out`. `walnut
/// build` and the builder app both come here, so they make the same file.
pub fn build_from(project: &Project, dir: &Path, program: &Path, out: &Path) -> Result<Project, String> {
    project.validate()?;
    let mut project = project.clone();
    let files = payload_files(&project, dir)?;
    // M2: what the install will take, for the free-space check.
    project.build.install_bytes = files.iter().map(|(_, path)| fs::metadata(path).map(|m| m.len()).unwrap_or(0)).sum();
    let packed = payload::pack(&files).map_err(|e| e.to_string())?;
    let program = fs::read(program).map_err(|e| format!("the installer program {}: {e}", program.display()))?;
    payload::write_installer(out, &program, &packed, &project.to_toml()).map_err(|e| e.to_string())?;
    Ok(project)
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            walk(root, &path, out)?;
        } else {
            let rel = path.strip_prefix(root).map_err(|e| e.to_string())?;
            out.push((rel.to_string_lossy().replace('\\', "/"), path));
        }
    }
    Ok(())
}
