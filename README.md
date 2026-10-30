<p align="center"><img src="assets/logo.png" alt="WinISOPilot" width="560"></p>

# WinISOPilot

A guided Windows 10/11 installation-media customizer. Pick an official ISO, choose an edition, then remove built-in apps, turn features on or off, change privacy and desktop defaults, and adjust Windows Setup. Each option explains what it does, what could stop working, and how to undo it. WinISOPilot then builds a new ISO. The original ISO is never changed.

> **Status: 0.3.0 pre-release.** The core logic is unit-tested and the Windows build compiles, but no real ISO has been serviced end to end yet. See [AUDIT.md](AUDIT.md). Test any result in a virtual machine before installing it on a PC you rely on.

## Two tools

- **Customize ISO**: the six-step wizard described below.
- **Write to USB**: makes a bootable USB stick from any Windows 10/11 ISO or media folder (including the one you just built). FAT32 with MBR (BIOS + UEFI, recommended) or GPT (UEFI only); `install.wim` over 4 GB is split automatically; optional read-back check of every file. Only USB drives are offered, never the disk Windows runs from or the drive holding your ISO or working folder, and erasing needs you to type `ERASE`.

## What sets it apart

- **Proof, not promises.** After building, WinISOPilot can install your ISO in a throwaway Hyper-V VM and check from inside Windows that every change landed, then report each one that didn't.
- **Measured results.** The result screen shows the installed size, image size and component counts read before and after your changes. Nothing is estimated.
- **Choices that survive new Windows releases.** Saved choices remember what each option did. Load them on a newer build and WinISOPilot shows what still matches, what changed, what no longer applies, and what's new.

## Two layouts

On first launch WinISOPilot asks how you want to work. You can switch any time in **Settings**.

- **Guided** (recommended): one section at a time with a plain-language explainer pane. Options are filtered by experience level (Beginner shows only safe and low-risk changes).
- **Advanced**: a dense table of everything in the image, with package IDs, risk filter, technical inspector, and *unrated items* (anything in the image the catalog doesn't describe).

## What it can change

117 catalogued options in 8 sections. Each one is checked against what the mounted image actually contains, so you only see options that would do something.

| Section | Examples |
|---|---|
| Apps | Clipchamp, News, Weather, Solitaire, Xbox, Phone Link, Copilot, Teams, Outlook (new), Widgets, Store (expert) |
| Windows extras | Steps Recorder, WordPad, Media Player Legacy, Math Input Panel, speech/handwriting/OCR data |
| Optional features | Turn off SMB 1.0 or PowerShell 2.0; turn on .NET 3.5 (from the ISO), Sandbox, WSL, Hyper-V |
| Services | Telemetry (DiagTrack), Maps, location, Xbox, Error Reporting, SysMain, Search, Print Spooler |
| Privacy | Advertising ID, suggested apps, tips and lock-screen ads, web search in Start, activity history, Recall |
| Desktop & system | File extensions, hidden files, classic context menu, left taskbar, dark mode, Fast Startup, device encryption |
| Setup | Skip TPM/Secure Boot/RAM/CPU checks, allow offline local-account setup |
| Slimming | Component cleanup, ResetBase, remove WinRE; single-edition output; smaller ESD compression |

Presets (**Light cleanup**, **Privacy**, **Gaming**) add a set of options in one click; Ctrl+Z undoes. Choices can be saved and loaded as a small JSON file.

WinISOPilot never offers to disable Windows Update, Defender or the firewall, and never removes framework packages other apps depend on.

## Requirements

- Windows 10 or 11 (x64), run as administrator (the app requests it).
- An official Windows 10 or 11 client ISO (`install.wim` or `install.esd`). Split `install.swm` media isn't supported yet.
- Free space: roughly twice the ISO size plus the expanded edition plus 10 GB (shown before you start).
- Recommended: [Windows ADK Deployment Tools](https://learn.microsoft.com/windows-hardware/get-started/adk-install). It provides `oscdimg.exe` to write the ISO, and a DISM that matches newer Windows builds. Without `oscdimg.exe` you get a ready-to-use media folder instead of an ISO.

## Use it

1. **Welcome**: check that the PC is ready (administrator, DISM, ISO tool).
2. **Choose ISO**: browse to the ISO and a working folder on a fast local NTFS drive.
3. **Edition**: pick the edition, then **Open edition**. WinISOPilot copies the media, extracts the edition and reads what's inside (a few minutes).
4. **Customize**: tick what to change.
5. **Review**: remove anything you're unsure about, confirm risky changes, choose compression, label and output file.
6. **Build**: apply, save, rebuild the image and write the ISO. Cancel stops after the current step without saving.

If WinISOPilot is closed while an edition is open, the next launch offers to clean it up.

## Build from source

Rust 1.82+. No Visual Studio is needed.

- **GitHub Actions**: every push builds and tests on `windows-latest` and uploads `WinISOPilot.exe` as an artifact.
- **Windows with the GNU toolchain**: `rustup toolchain install stable-x86_64-pc-windows-gnu`, then `cargo +stable-x86_64-pc-windows-gnu build --release`. Without MSYS2's `windres` the icon and administrator manifest aren't embedded; run the exe with **Run as administrator**.
- **Linux cross-build**: install `mingw-w64`, `rustup target add x86_64-pc-windows-gnu`, then `cargo build --release --target x86_64-pc-windows-gnu`.

Run the tests with `cargo test --workspace`. `WinISOPilot.exe --demo` opens the interface with labelled sample data (no Windows tools needed). The debug build's `--self-test` prints catalog and host checks.

## Project layout

| Path | Contents |
|---|---|
| `crates/wip-core` | Option catalog (TOML), plan validation and compilation, recipes, settings, journal. No Windows APIs; fully unit-tested. |
| `crates/wip-engine` | DISM API (runtime-loaded `dismapi.dll`), offline registry via `reg.exe`, provisioned apps via PowerShell, export and ISO creation. |
| `crates/winisopilot` | The egui desktop app: Guided and Advanced layouts, wizard pages, dialogs. |
| `docs/` | Architecture, decisions, limitations, design tokens, roadmap, screenshot list. |

## Documentation

[CHANGELOG](CHANGELOG.md) · [AUDIT](AUDIT.md) · [CREDITS](CREDITS.md) · [Architecture](docs/architecture.md) · [Decisions](docs/decisions.md) · [Limitations](docs/limitations.md) · [Roadmap](docs/roadmap.md) · [Design tokens](docs/design-tokens.md)

## License

MIT. See [LICENSE](LICENSE).
