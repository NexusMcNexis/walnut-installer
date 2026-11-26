//! Walnut Installer's core: everything an installer and its builder share,
//! with no user interface. All of it is tested against temporary folders and
//! a fake Steam; nothing here needs root.

pub mod build;
pub mod desktop;
pub mod install;
pub mod payload;
pub mod project;
pub mod steam;
pub mod vdf;
