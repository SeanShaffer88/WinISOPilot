# WinISOPilot project instructions

- Rust workspace: `wip-core` (pure logic, must stay platform-independent), `wip-engine` (Windows work under `src/win/`), `winisopilot` (egui app).
- Before publishing a change: `cargo fmt --all`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`, and a Windows build (CI or `--target x86_64-pc-windows-gnu`).
- New catalog options go in `crates/wip-core/catalog/*.toml` with summary, details, risk, `breaks` (required for moderate/high), `undo`, `applies`. Never add options that disable Windows Update, Defender, the firewall or recovery generically.
- Every operation touches only the workspace copy; record mounts in the journal first; never overwrite user files; keep cancel between steps and never interrupt commit/unmount.
- Use the stable error codes in `docs/architecture.md`; add new ones there.
- Third-party dependencies are allowed when justified; pin via `Cargo.lock` and record licenses in CREDITS.md.
- Don't claim boot/install success or performance numbers without a recorded run (see AUDIT.md).
