# Walnut Installer: plan

## 0. The repository (first requirement)
- **Where:** its own public GitHub repository, `NexusMcNexis/walnut-installer`, default branch `main`, public from day one.
- **Separate from Gemstone:** completely separate from the Gemstone game's repository, which is private and stays private.
- **Nothing copied in from Gemstone:** no code, art, purchased packs, keys or configs. Sample art is original (drawn by `examples/hello/make_art.py`).
- **Commits:** carry the author's GitHub no-reply address, never a personal email.

## 1. What it is
A free, open-source framework for shipping a Linux game as **one friendly installer file**: Welcome, Folder, Progress, and Done with Launch, plus desktop entry and shortcuts, **Add to Steam with artwork**, and a clean uninstall. No root, ever. It is built from a TOML project file, by a WYSIWYG builder app or headless in CI (`walnut build project.toml`).

License: Apache-2.0 with a NOTICE. The footer "Installer built with Walnut Installer by Luke Woods" is on by default and can be turned off.

## 2. Architecture (Cargo workspace)

| Crate | What |
|---|---|
| `walnut-core` | No UI; all unit-tested: the payload format, binary VDF, Steam, desktop entries and icons, paths, the project file, install and uninstall (receipt) |
| `walnut-installer` | The program at the front of every installer: the GUI screens (M2), the text mode (M1, the fallback), and the uninstaller (the same program, run from the install folder) |
| `walnut-builder` | The WYSIWYG builder app (M3) |
| `walnut-cli` | `walnut build project.toml`, `walnut inspect INSTALLER` (headless, CI) |

**The installer file:**

```
[program][payload: zstd tar][manifest: project TOML][footer: "WALNUT01", lengths, SHA-256 of payload+manifest]
```

- **Checked before use:** the checksum is verified before the manifest is read or anything is unpacked.
- **Rollback:** on cancel or any error, unpacking removes everything it made.
- **Receipt:** the install writes `.walnut/receipt.toml`, listing everything it made, and the uninstaller removes exactly that.

**Steam:**
- **Finding it:** `~/.local/share/Steam`, `~/.steam/steam` and the Flatpak path.
- **Which account:** the `MostRecent` account in `loginusers.vdf`.
- **Writing:** a backup first, never a duplicate (matched by executable path), other entries kept byte for byte, the file written whole.
- **Ids and art:** the current client's appid (CRC-32 of the quoted exe and name, top bit set) goes in the entry, and the same number names the five grid files.
- **Steam running:** if it is, the installer asks, and closes it only on a yes.

**Theme (defaults, all from the project file):**

| Part | Colour |
|---|---|
| Background | `#342418` |
| Bar | `#281B13` |
| Brass | `#B07C46` |
| Title | `#F0E2C4` |
| Body | `#E1D2B4` |
| Button text | `#FFF8E6` |

**The GUI toolkit:** egui/eframe (see [PRIOR-ART.md](PRIOR-ART.md)). The graphical binary is linked against an old glibc baseline; the text mode and the CLI are static musl.

## 3. Milestones

### M1: core, headless builder, text-mode installer (done in this repo's first commit)
**Acceptance:**
- `walnut build examples/hello/project.toml` writes one installer file.
- Running it (text mode, `--yes` for unattended) installs into `~/.local/share/<id>`, writes the `.desktop` entry and the icon, adds the game to Steam with art when Steam is present, and puts an uninstaller beside the game.
- A damaged or altered installer is refused before anything is unpacked. A cancelled install is rolled back.
- The uninstaller removes exactly what was installed, and Steam's `shortcuts.vdf` is restored byte for byte.
- **Tests:** a binary VDF round trip, no duplicates, uninstall restoring the file, a fake Steam with two other games, a temp HOME only, and checksum mismatches (payload and manifest) refused.

### M2: the walnut graphical installer (done)
**What shipped:**
- **The screens:** `walnut-installer`'s `gui` feature (on by default): `flow.rs` (the screens as a state machine, unit-tested), `gui.rs` (eframe/egui, glow), `theme.rs` and `worker.rs` (the install on its own thread; Cancel rolls back).
- **The welcome art:** the project's `[art] welcome` is packed first in the payload and read without unpacking (`payload::peek`).
- **The free-space check:** the builder records `build.install_bytes`; `space::free_bytes` is asked of the nearest existing folder.
- **Trusting the shortcut:** `desktop::trust_for` uses `gio` metadata on GNOME-family desktops, and the executable bit on KDE (including SteamOS and Bazzite) and the rest.
- **For tests:** `WALNUT_SCREENSHOTS=<dir>` saves every screen; `WALNUT_SCALE` sets 1.0–2.0; `WALNUT_AUTOPILOT=1` presses through a real install.
- **On real desktops:** checked on Wayland (a headless compositor) at 100/150/200%, plus a full install through the window (with a fake Steam). On real GNOME, KDE and SteamOS desktops it waits for a person; see QUESTIONS.md.

**Acceptance:**
- **Screens:** Welcome (the game's art and name, the version small), Folder (with a free-space check), Progress (with Cancel and rollback), Done (Launch ticked, desktop entry, desktop shortcut, Add to Steam; that last box hidden without Steam), and a themed dialog for closing Steam.
- **Accessibility:** DPI 100–200% without clipping; keyboard (Enter for the primary button, Esc to cancel, Tab order); AccessKit names on every control.
- **Displays:** X11 and Wayland, falling back to text mode when there is no display.
- **Desktop quirks:** a desktop shortcut marked trusted where the desktop supports it (GNOME `gio`), with quirks keyed by `XDG_CURRENT_DESKTOP` and `/etc/os-release`. Tested on GNOME, KDE and SteamOS/Bazzite (desktop mode).
- **Screenshots** in the README.

### M3: the WYSIWYG builder
**Acceptance:**
- **A form for every project field:**
  - the game: name, version (shown small, never in a name), publisher, icon, the game's archive or folder, the executable inside it, the default folder;
  - the desktop entry: categories and comment, and the shortcut options;
  - Steam: the option, plus five art slots with drag-and-drop and size checks (600x900, 920x430, 1920x620, the logo, a square icon);
  - the theme colours, and the welcome and done text.
- **Live preview:** of every installer screen as you edit.
- **Build installer:** writes the binary and the TOML project file, and the same file builds headless with `walnut build`.

### M4: release builds
**Acceptance:**
- **Builds:** CI builds x86_64, then aarch64. Text mode and the CLI are static musl; the GUI targets the glibc baseline. A release workflow attaches the binaries and checksums.
- **Gates:** the tests, an end-to-end run of the sample in a container, and a size budget.

### M5: Gemstone adopts it
- **Who:** done from the Gemstone side, by Claude CLI, not in this repo.
- **What:** Gemstone's Linux package becomes a Walnut installer with its own project file and art, replacing its shell installer.

## 4. Decisions for Luke
- **The name:** `walnut-installer`, or `walnut-games-installer`.
- **The credit rule:** keep the footer credit on by default but switchable (OSI-compatible), or require it (which would make the license non-OSI).
- **The toolkit:** egui/eframe as proposed (MIT/Apache-2.0), or Slint, whose license terms are less clean for an Apache project.
