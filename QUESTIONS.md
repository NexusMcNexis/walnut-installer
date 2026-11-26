# Questions for Luke

Newest at top. Each has a sensible default in place so work carries on.

## 2. Merge M2, M3 and M4, and publish v0.3.0 (26 Nov 2026)
- **Ready:**
  - pull requests #1 (M2), #2 (M3, on #1) and #3 (M4, on #2), each with green CI; #3's release workflow built and checked x86_64 and aarch64;
  - the release notes (`docs/releases/v0.3.0.md`) and the version (0.3.0).
- **Blocked here:** merging to main was refused by this session's permission settings, so nothing reached main and no tag was pushed.
- **To finish:**
  1. Merge #1, then #2, then #3 (each into the one before, then main).
  2. Tag `v0.3.0` on main. The release workflow builds both architectures and publishes the GitHub Release with `SHA256SUMS`.

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
