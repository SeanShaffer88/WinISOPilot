# Architecture

## Project brief

| | |
|---|---|
| Purpose | Customize, debloat and slim Windows 10/11 installation media, guiding users of any experience level |
| Users | Home users who want a cleaner install; enthusiasts; technicians preparing media |
| Core workflow | ISO → edition → customize → review → build ISO |
| Stack | Rust, egui/eframe (glow + AccessKit), DISM API, Windows tools |
| Target | Windows 10/11 x64 hosts; Windows 10/11 client images |
| Data | Settings, mount journal (`%LOCALAPPDATA%\WinISOPilot`); workspaces in a user-chosen folder; recipes where the user saves them |
| Destructive operations | Mounting/servicing a private copy of the image; never the source ISO |
| Networking | Only on explicit user action: Microsoft catalog and ESD downloads (Microsoft hosts only, hash-verified) |
| License | MIT |
| Non-goals (0.4) | NTFS/UEFI:NTFS USB layout, driver/update integration, answer files, Windows 7/8.1 images (see roadmap) |

## Crates

```
winisopilot (egui app) ──► wip-engine (Windows work) ──► wip-core (pure logic)
         └────────────────────────────────────────────────┘
```

- **wip-core**: the catalog (`catalog/*.toml`, embedded at build time and validated), `Inventory` (what an image contains), `plan` (availability, validation, compile to `Op`s), `raw` (uncatalogued items), `recipe`, `settings`, `journal`, `fsutil`. No platform code; all tested on any OS.
- **wip-engine**: `inspect_iso`, `prepare`, `build`, `discard`, leftover handling. Windows code lives in `src/win/`; elsewhere every entry point returns `WIP-E000` so the UI can be developed on any OS.
- **winisopilot**: app state (`app.rs`), the Customize layouts (`customize.rs`), other wizard pages (`pages.rs`), dialogs (`dialogs.rs`), background jobs (`jobs.rs`), design tokens (`theme.rs`).

## Flow

1. **Inspect**: attach ISO read-only (PowerShell `Mount-DiskImage`), find `sources\install.wim|esd`, `DismGetImageInfo`, detach.
2. **Prepare**: create `WinISOPilot-<time>\{media,work,mount,mount-boot,scratch}`; copy the media except the install image; `dism /Export-Image` the chosen edition (or all) into `work\install.wim`; write the journal; `DismMountImage` read-write; inventory = `DismGetFeatures` + `DismGetCapabilities` + `Get-AppxProvisionedPackage` + service keys from the offline SYSTEM hive.
3. **Customize/Review**: purely in memory. `plan::validate` reports errors (block) and warnings (skips, confirmations).
4. **Build**: `plan::compile` → ordered `Op`s. Apply (DISM API for capabilities/features, `dism.exe` for apps and cleanup, `reg.exe` for hives) → `DismUnmountImage` commit → optional boot.wim index 2 changes → `dism /Export-Image` every index into `media\sources\install.wim` (LZX) or `.esd` (LZMS) → `oscdimg -m -o -u2 -udfver102 -bootdata:2#p0,e,b…#pEF,e,b…`.

## Safety model

- Source immutability: only a copy is ever mounted read-write.
- Journal: `journal.json` gets a record before each mount and loses it after unmount. Records from other processes appear on the Welcome page.
- Cancellation: checked between operations and forwarded to DISM via its cancel event. Commit/unmount ignore cancel.
- Failures release DISM sessions and unload every loaded hive before reporting.
- Atomic JSON writes; unreadable files are renamed `.corrupt`, never deleted.

## Error codes

| Code | Meaning |
|---|---|
| WIP-E000 | Not running on Windows |
| WIP-E001 | Cancelled by the user |
| WIP-E002 | Not elevated |
| WIP-E010–E013 | DISM missing or failed to load/initialize |
| WIP-E011 | A DISM API call failed (message includes HRESULT and DISM's text) |
| WIP-E020 | A tool couldn't be started |
| WIP-E030–E035 | Offline registry load/write/unload |
| WIP-E040–E048 | ISO attach, missing/split install image, edition lookup |
| WIP-E050–E054 | Copy, export, free space, workspace, journal |
| WIP-E060–E063 | App removal, file deletion, component cleanup, driver file missing |
| WIP-E070–E072 | Output exists, missing boot files, oscdimg failure |
| WIP-E073 | dism.exe or oscdimg.exe not validly signed by Microsoft Corporation (build refused) |
| WIP-E080–E084 | Drive list, drive gone/changed, no drive letter, drive not allowed |
| WIP-E085–E093 | USB source not Windows media, read/plan/split failures, diskpart, bootsect, verification mismatch |
| WIP-E094 | Copying a file to the USB drive failed (write-through copy) |
| WIP-E100–E107 | Boot test: Hyper-V missing/not running, no oscdimg, ISO missing, test folder, answer disc, VM creation, unreadable results |
| WIP-E110–E120 | Download: curl missing, host refused, download failed, catalog folder/unpack/parse, unsafe name, no hash, hash mismatch, export, unexpected ESD layout |
| WIP-E121 | ADK installer not validly signed by Microsoft Corporation (deleted, never run) |
| WIP-E122 | ADK installer (adksetup.exe) returned an error code |
| WIP-E900/E901 | Background task crashed / no result |
