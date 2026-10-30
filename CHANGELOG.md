# Changelog

All notable changes are listed here. Versions follow [Semantic Versioning](https://semver.org/).

## [0.1.0] - Unreleased

Complete rewrite in Rust. The earlier C#/WPF prototype is preserved in git history (`main` before this release).

### Added
- Six-step wizard: Welcome, Choose ISO, Edition, Customize, Review, Build.
- First-launch choice between **Guided** (stepper rail, option list, explainer pane) and **Advanced** (icon rail, category list, dense table, inspector) layouts; switchable in Settings.
- Option catalog with 117 entries across Apps, Windows extras, Optional features, Services, Privacy, Desktop & system, Setup and Slimming. Each has a summary, details, risk level, what could stop working, and how to undo it.
- Options are matched against the mounted image's real inventory (provisioned apps, capabilities, features, services) so only applicable items are offered.
- Advanced mode lists uncatalogued ("unrated") apps, capabilities and features, excluding framework packages and Windows Security.
- Experience levels (Beginner / Intermediate / Expert) filter Guided mode by risk.
- Presets: Light cleanup, Privacy, Gaming. Undo (Ctrl+Z) for every selection change.
- Review page: grouped change list with per-item removal, conflict errors, skipped-item warnings, explicit confirmation for high-risk and unrated changes.
- Save and load choices as a versioned JSON recipe (IDs and output settings only).
- Engine: DISM API loaded at runtime (ADK copy preferred), single- or all-edition export, read-write mount, provisioned app removal, capability removal, feature enable/disable (.NET 3.5 from `sources\sxs`), offline registry edits on SOFTWARE, SYSTEM, Default user and boot.wim, component cleanup, WinRE removal, rebuild to WIM (LZX) or ESD (LZMS), ISO creation with `oscdimg` (BIOS + UEFI).
- Safety: original ISO attached read-only; mount journal written before every mount; leftover sessions offered for cleanup on next launch; cancel stops between steps and never interrupts DISM commit/unmount; never overwrites an existing output file; registry hives always unloaded on failure.
- Settings: layout, level, text size (75–200%), ADK DISM folder, default working folder.
- `--demo` (labelled sample data) and `--self-test` command-line modes.
- Icon, logo, README, AUDIT, CREDITS, architecture, decisions, limitations, roadmap and design-token docs.
- GitHub Actions workflow: format, lint, test and release build on Windows.
