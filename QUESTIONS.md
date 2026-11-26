# Questions for Luke

Newest at top. Each has a sensible default in place so work carries on.

## 1. M2: try the installer on a real GNOME, KDE and Steam Deck desktop (26 Nov 2026)
- **Done here:**
  - Every screen was checked on Wayland (a headless compositor) at 100%, 150% and 200%.
  - A full install went through the window, then an uninstall, with a fake Steam.
  - The desktop-shortcut rules (`gio` on GNOME, the executable bit on KDE/SteamOS/Bazzite) are unit-tested.
- **What a person needs to check,** on GNOME, KDE (CachyOS) and the Steam Deck in desktop mode:
  - the window opens;
  - the shortcut opens on double-click;
  - Orca reads the controls;
  - Steam asks to close, and comes back with the game in the library.
- **Try it:** `cargo build --release && ./target/release/walnut build examples/hello/project.toml -o /tmp/hello && /tmp/hello`
