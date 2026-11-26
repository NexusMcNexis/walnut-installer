#!/usr/bin/env bash
# End to end: builds the sample installer, installs it into a temp HOME with a
# fake Steam holding two other games (never the real one), runs the game,
# uninstalls, and checks that Steam's file is back byte for byte and that a
# damaged installer is refused.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${WALNUT_BIN:-$ROOT/target/release}"
T="$(mktemp -d)"
trap 'rm -rf "$T"' EXIT
"$BIN/walnut" build "$ROOT/examples/hello/project.toml" --program "$BIN/walnut-installer" -o "$T/installer"
# M3: the builder's headless build makes the very same file.
if [ -x "$BIN/walnut-builder" ]; then
	"$BIN/walnut-builder" --build "$ROOT/examples/hello/project.toml" --program "$BIN/walnut-installer" -o "$T/from-builder" > /dev/null
	cmp "$T/installer" "$T/from-builder"
fi
H="$T/home"; S="$H/.local/share/Steam"; C="$S/userdata/39734314/config"
mkdir -p "$S/config" "$C/grid"
cp "$ROOT/tests/fixtures/loginusers.vdf" "$S/config/"
cp "$ROOT/tests/fixtures/shortcuts_two_games.vdf" "$C/shortcuts.vdf"
run() { env -i HOME="$H" PATH=/usr/bin:/bin WALNUT_FAKE_STEAM_RUNNING=0 "$@"; }
run "$T/installer" --yes --no-launch > "$T/install.log"
grep -q "Added to your Steam library." "$T/install.log"
[ "$(ls "$C/grid" | wc -l)" -eq 5 ]
grep -q '^Name=Hello Walnut$' "$H/.local/share/applications/hello-walnut.desktop"
"$H/.local/share/hello-walnut/hello.sh" | grep -q "Hello from a game"
run "$H/.local/share/hello-walnut/.walnut/uninstall" --yes > /dev/null
cmp "$C/shortcuts.vdf" "$ROOT/tests/fixtures/shortcuts_two_games.vdf"
[ "$(ls "$C/grid" | wc -l)" -eq 0 ]
[ ! -e "$H/.local/share/hello-walnut" ]
cp "$T/installer" "$T/bad"
python3 -c 'import sys;p=sys.argv[1];b=bytearray(open(p,"rb").read());b[len(b)-100]^=1;open(p,"wb").write(b)' "$T/bad"
if run "$T/bad" --yes --no-launch > /dev/null 2>&1; then echo "E2E a damaged installer was accepted"; exit 1; fi
echo "E2E OK"
