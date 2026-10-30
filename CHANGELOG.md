# Changelog

All notable changes are listed here. Versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- docs/references.md: curated primary sources (Windows deployment, disc formats, boot, security, testing).

### Changed
- Roadmap: USB layout modes and write-through progress (from windows2usb), reusable projects / cached source media (from JLIVECD). Credited in CREDITS.md as ideas only.

## [0.4.0] - 2026-10-30

### Added
- **Download Windows from Microsoft** on the Choose ISO page: loads Microsoft's Media Creation Tool catalog for Windows 11 or 10, lets you pick language, architecture and package, downloads the ESD with the built-in `curl.exe` (resumes after interruption), verifies it against the catalog's hash, and turns it into a bootable ISO (setup media, boot.wim with Windows PE + Setup, install.esd with every edition) using DISM and `oscdimg`.
- Downloads are only from Microsoft hosts (`dl.delivery.mp.microsoft.com`, `download.microsoft.com`, `go.microsoft.com`); HTTPS first, HTTP only as a fallback that is still hash-verified.
- Tag- or `release/v*`-branch-triggered release workflow that builds, tests and publishes the exe with checksums.
- `--source` demo flag.
- **Help everywhere**: F1 or the Help button opens help for the current page (what it does, what happens next, keyboard shortcuts); each page shows a one-line tip (can be turned off).
- **Redo** (Ctrl+Y / Ctrl+Shift+Z) alongside Undo; Undo/Redo history kept for 200 changes.
- **Keyboard**: Alt+Right / Alt+Left for Next / Back, Ctrl+F to search options, clear-search button.
- **Clear section** button in Guided mode (undoable).
- Review page explains **what happens when you select Build**, step by step, from the actual plan.
- Error messages offer **Copy details** and **Open log folder**.
- Settings: **Stronger contrast** (white text, thick yellow focus outline), **Reduce motion**, **Show tips**.
- Window title shows the current step.
- docs/user-guide.md and docs/quality-traits.md.

### Changed
- Settings files from older versions load with new options at their defaults instead of being reset.
- Free disk space is checked at most every 5 seconds instead of every frame.
- Guided toolbar wraps on narrow windows; Help and Settings share one row in the rail.
- **Every page fits the window without scrollbars** (default 1200×760, minimum now 1024×700): Welcome, Choose ISO, Review, results and Write to USB use two columns; editions, options, the Advanced table and the Review list are split into pages (Previous/Next, PageUp/PageDown or the mouse wheel over the list).
- **Guided and Advanced are now distinct on every page.** Guided is for complete beginners: plain wording, numbered questions, safe automatic defaults (temporary-files folder, single edition, standard compression, output path, MBR USB), "Download Windows for me" asks only the language, no package names or technical details. Advanced is for experts: host diagnostics table, editable ISO/workspace paths, image index metadata, compiled operation plan with Copy, all output/USB/boot-test options, recipes, and the Details log open by default.
- **Compact fixed window: 960×456**, not resizable. Only the Customize option list/table scrolls; page bodies, the explainer/inspector and Help never do. Long drop-down lists (languages) open in columns; the recipe-check dialog caps long lists with "…and N more"; the inspector lists at most 3 technical actions (all on hover).
- **Data-dense styling** (after uistyleguide.com "data-dense dashboard"): 13 px body text, 4–8 px gaps and padding, 22 px controls, 4 px corners, 24/20 px list rows; the last card in each column stretches to the bottom. Guided Welcome cards list the steps; Edition gets a second column (Guided: Home/Pro/Education explained; Advanced: full index metadata); Help uses two columns.
- **One slim left panel in both layouts** (72 px): ISO / USB and **Guided / Advanced** switches, numbered steps (names on hover), Help and Settings. The layout choice moved there from Settings. "DEMO" moved to the window title.
- **Layout question at every launch** (Guided or Advanced, last choice pre-selected; the cards themselves are the buttons, Enter picks the highlighted one) with a **Don't ask again** box; turn it back on in Settings ("Ask Guided or Advanced at every launch"). Demo flag `--ask`.
- Picking Advanced no longer raises the Guided option level to Expert, so beginners switching back still see only safe options.
- Advanced Welcome footer no longer points to a Guided-only section.
- Guided Welcome: the footer "Get started" button is gone (the page's Start button does the same).
- Checkboxes are visibly square again (control corner radius 6 → 3 px); they looked like radio buttons.
- **"Install it for me"** for the Windows ADK Deployment Tools (needed for ISO files and downloads): downloads Microsoft's official `adksetup.exe` (go.microsoft.com/fwlink/?linkid=2289980), refuses to run it unless it carries a valid Microsoft Corporation signature, installs only the Deployment Tools silently, then re-checks the PC. Also "check again" after a manual install, no restart needed. Offered on Welcome, the download card and Review.
- **Backup-drive guard**: USB disks over 300 GB are treated as external/backup drives. Guided refuses them (with the reason); Advanced allows them with a warning on the page and in the erase confirmation.
- Guided Welcome no longer shows a green tick beside a missing tool, and the footer no longer says "ready" while something is missing.
- Download packages have plain names (Home / Pro / Education vs Pro / Enterprise / Education) and the consumer package is the default instead of Enterprise.
- Switching layout (Guided ↔ Advanced) in Settings returns to step 1 of Customize ISO and keeps everything entered: ISO, working folder, opened edition, ticked changes, search text and output settings. While a task runs, the page stays where it is.
- Review: risky changes are confirmed with a **Confirm** tick on their own row; output settings are a compact form.
- Results: "What changed" opens as a pop-up list.
- The Details log shows the last 6 lines and links to the log folder instead of scrolling.
- Long file paths are shortened with "…" (full path on hover).
- Demo flags `--welcome`, `--edition`, `--customize`, `--build`, `--advanced`, `--settings`, `--first-run` for screenshots.

### Fixed
- The Settings window could be taller than the app window, pushing its title bar and close button off-screen so it couldn't be closed. Settings is now a two-column dialog with a **Done** button that also closes with Esc or a click outside.
- **Load list** failed for both Windows 10 and 11 with WIP-E114 ("The catalog had no products.xml"). The catalog cab is now fully unpacked and any XML that is a product list is used (whatever its name, UTF-8 or UTF-16); a download that isn't a cab is checked and parsed directly or reported plainly. The Details log records the download size and what the cab contained.
- Error messages now show the Details log right under them on every page.
- Catalog product lists saved as UTF-16 without a byte-order mark are now recognized; each file in the catalog is logged with its size and first characters.
- **Copy details** now copies the last 40 log lines along with the error.

## [0.3.0] - 2026-10-30

### Added
- **Measured footprint**: every build reads the image before and after (DISM installed size, image file size, apps, capabilities, features) and lists exactly what changed. Shown on the result screen and saved as `footprint.md` / `footprint.json` in the workspace.
- **Boot test**: one click installs the built ISO unattended in a temporary Hyper-V VM (Gen 2, Secure Boot, vTPM, no network), signs in, and checks from inside Windows that every removed app, capability, feature, service, registry value and file change actually landed. VM removed after a pass; kept for inspection after a failure, timeout or when asked.
- **Recipe porting**: saved choices (format 2) record what each option resolved to and which catalog options existed. Loading them on another Windows build reports, per option, same / changed items / doesn't apply, and offers options added since. Version-only package changes don't count as changes.
- `--done` demo flag (with `--demo`) for the result screen.
- docs/verification-checklist.md.

### Fixed
- Page content could extend past the window's right edge when the Guided rail was shown.
- Long status lines now wrap.

## [0.2.0] - 2026-10-30

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
