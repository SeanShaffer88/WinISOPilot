# Changelog

All notable changes are listed here. Versions follow [Semantic Versioning](https://semver.org/).

## [0.3.0] - Unreleased

### Added
- **Measured footprint**: every build reads the image before and after (DISM installed size, image file size, apps, capabilities, features) and lists exactly what changed. Shown on the result screen and saved as `footprint.md` / `footprint.json` in the workspace.
- **Boot test**: one click installs the built ISO unattended in a temporary Hyper-V VM (Gen 2, Secure Boot, vTPM, no network), signs in, and checks from inside Windows that every removed app, capability, feature, service, registry value and file change actually landed. VM removed after a pass; kept for inspection after a failure, timeout or when asked.
- **Recipe porting**: saved choices (format 2) record what each option resolved to and which catalog options existed. Loading them on another Windows build reports, per option, same / changed items / doesn't apply, and offers options added since. Version-only package changes don't count as changes.
- `--done` demo flag (with `--demo`) for the result screen.
- docs/verification-checklist.md.

### Fixed
- Page content could extend past the window's right edge when the Guided rail was shown.
- Long status lines now wrap.

## [0.2.0] - Unreleased

### Added
- **Write to USB** tool (switch in the rail): bootable USB from an ISO or media folder.
  - Lists disks with bus type and drive letters; offers only USB drives of 7 GB or more that aren't the system/boot disk, read-only, offline, or holding the source or working folder.
  - Re-checks the drive's identity (number, size, name, serial, unique ID) immediately before erasing.
  - Typed `ERASE` confirmation.
  - diskpart: clean, MBR or GPT, FAT32 (32 GB partition on larger drives), active (MBR).
  - Copies the media; splits `install.wim` over 4 GB into `install.swm` parts with DISM.
  - `bootsect /nt60 /mbr` for BIOS boot on MBR drives; warns when no UEFI boot loader is present.
  - Optional byte-for-byte read-back verification.
- "Write it to a USB drive" button after a successful build.
- `--usb` start-up flag (with `--demo`, shows sample drives).

### Changed
- Step rail is more compact to fit the tool switch; demo label moved to the header.

## [0.1.0] - 2026-10-30

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
