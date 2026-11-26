# Contributing to Walnut Installer

Thank you for helping. Issues and pull requests are welcome.

## Building and testing
```sh
cargo test                      # everything; no network, no root
cargo build --release
./scripts/e2e.sh                # builds the sample installer and runs it in a temp HOME
```

## Rules
- **Tests never touch a real home or a real Steam.** Use `tempfile` for HOME and a fake Steam folder (see `crates/walnut-core/tests/m1.rs`). The installer reads `WALNUT_FAKE_STEAM_RUNNING=0|1` (tests and demos only) in place of looking for the real Steam.
- **Other games' Steam entries are sacred.** Any change to the VDF code must keep the round-trip and byte-for-byte tests passing.
- **No root, ever.** Installs go into the player's home.
- **Sample art must be original or properly licensed.** Never include assets you don't have the right to redistribute.
- **Keep `walnut-core` free of UI code,** so it stays testable.
- **Licensing of your work:** contributions are under Apache-2.0 (see LICENSE). By sending a pull request, you agree to license your work that way.
