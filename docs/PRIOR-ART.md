# Prior art

What exists, what we copy, what we avoid. From general knowledge of these projects; links are to their own pages.

## Installers and bundles
- **[makeself](https://makeself.io/):** a shell script with an archive appended, which unpacks itself.
  - **Copy:** one file that carries its own payload, checked before unpacking.
  - **Avoid:** a shell script as the installer: no UI, fragile quoting, and nothing to tick.
- **[AppImage](https://appimage.org/):** one portable file per app, run in place.
  - **Copy:** "download one file and run it".
  - **Avoid:** it isn't an install. No folder choice, no Steam, often no menu entry without extra tools.
- **Flatpak bundles** (`.flatpak`): sandboxed, need Flatpak and a runtime; good for apps, awkward for games handed out directly. Avoided for that reason.
- **[MojoSetup](https://icculus.org/mojosetup/)** (GOG's Linux installers): a small installer with a GUI and a text fallback, the payload appended.
  - **Copy:** the closest ancestor. A real installer feel, a text mode when there is no display, and the payload appended to a native binary.
  - **Avoid:** Lua-scripted configs (a TOML project file is enough) and its dated UI.
- **NSIS and Inno Setup** (Windows): the builder UX people know.
  - **Copy:** the page flow (Welcome, Folder, Progress, Finish with "Launch"), a project file that builds headless, and an uninstaller that removes what was installed.
  - **Avoid:** script languages for configuration.

## Steam's non-Steam shortcuts
- **Steam has no command-line way to register a non-Steam game.** `steam://addnonsteamgame` only opens Steam's own dialog. Registering means editing `userdata/<id>/config/shortcuts.vdf` (binary VDF) and putting art in `config/grid/`, with Steam closed so it doesn't overwrite the file.
- **[Valve: Steam Library Shortcuts](https://developer.valvesoftware.com/wiki/Steam_Library_Shortcuts):** the file's format.
- **[SteamTinkerLaunch](https://github.com/sonic2kk/steamtinkerlaunch), [BoilR](https://github.com/PhilipK/BoilR), Steam ROM Manager:** they write shortcuts.vdf and the grid art.
  - **Copy:** the app id is CRC-32 of the quoted executable followed by the name, with the top bit set. It is written into the entry's `appid` field, and the same number names the grid files (`<id>p.png` portrait, `<id>.png` wide, `<id>_hero.png`, `<id>_logo.png`, `<id>_icon.png`).
  - **Copy:** the account is the one `loginusers.vdf` marks `MostRecent`; tools that pick the first account write to the wrong one.
  - **Copy:** close Steam first.
  - **Avoid:** rewriting other entries. Walnut keeps every other entry's bytes exactly, backs the file up first, writes it whole (temp file, then rename), and asks before closing Steam.

## The GUI toolkit (for M2 and M3)

The installer and builder need a toolkit that runs on X11 and Wayland, handles DPI and keyboard, gives screen readers names, and can be shipped in one file.

| | egui / eframe | iced | Slint |
|---|---|---|---|
| License | MIT / Apache-2.0 | MIT | GPLv3, royalty-free (with its own terms), or commercial |
| Rendering | OpenGL (glow) or wgpu | wgpu first | software renderer available, or GL |
| Screen readers | AccessKit | in progress | AccessKit |
| Live preview in a builder | natural (immediate mode: the builder draws the same screens) | possible | strong (.slint files), but a second language |

- **Choice: egui/eframe.** Its license fits an Apache-2.0 project with no extra terms. The builder's live preview is just the installer's own screen functions drawn inside it. AccessKit gives screen readers names, and DPI scaling is built in.
- **The static-binary caveat, honestly:**
  - No Rust GUI toolkit can be a fully static musl binary on desktop Linux, because the display (X11, Wayland, EGL/GL) is loaded from the system at run time.
  - So the graphical installer is one file linked only against an old glibc baseline (every desktop, SteamOS and Bazzite have it), and it loads the display libraries the desktop already has.
  - The text-mode installer and the `walnut` CLI are fully static (musl in CI). When there is no display, or it can't be opened, the installer falls back to text mode.
