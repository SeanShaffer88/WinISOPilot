# Audit — WinISOPilot 1.0.0

Date: 2026-10-30. This audit says what has and hasn't been checked. Labels: **Verified** (run and observed), **Partially verified**, **Unverified**.

## Verification matrix

| Area | Status | Evidence |
|---|---|---|
| Catalog loads; every entry valid (IDs, risk vs. "what breaks", hive paths, no `CurrentControlSet`, presets reference real IDs) | Verified | `wip-core` tests, `catalog_rejects_bad_entries` |
| Availability against an inventory (present / already done / missing / wrong Windows) | Verified | `availability_with_inventory`, `netfx3_needs_sxs` |
| Plan compile order (apps → capabilities → features → files → registry groups → cleanup; boot.wim separate; every hive load paired with unload) | Verified | `compile_orders_and_resolves` |
| Conflicts, unknown IDs, high-risk and unrated confirmations | Verified | `conflicts_are_errors_and_high_risk_warns`, `raw_items_exclude_catalogued_and_protected` |
| Recipe format: round trip, unknown IDs dropped, future schema rejected, size cap | Verified | `recipe_roundtrip_and_unknowns` |
| Atomic writes; corrupt JSON quarantined, not deleted | Verified | `atomic_roundtrip_and_quarantine` |
| Engine compiles for Windows x64 | Verified | Cross-compiled `x86_64-pc-windows-gnu` (release), clippy clean |
| Exe embeds icon, version info, `requireAdministrator` manifest; imports only system DLLs | Verified | `strings` / `objdump -p` on the built exe |
| Guided and Advanced layouts and first-run dialog render | Partially verified | Rendered on Linux/Mesa with `--demo` data; not yet on Windows, at 150–200% scaling, or with high contrast |
| DISM API structure layouts | Partially verified | Checked against mingw-w64 `dismapi.h` (`pack(1)`); not yet exercised against a live `dismapi.dll` |
| Inspect ISO, prepare, build, discard, leftover cleanup on real media | **Unverified** | Requires an elevated Windows run with a real ISO |
| Resulting ISO boots (BIOS and UEFI) and installs | **Unverified** | Requires a VM install test per edition/build |
| Each catalog option has the documented effect after installation | **Unverified** | Option texts are based on Microsoft documentation and well-known behavior; per-build effects (especially Copilot, Recall, BypassNRO, OneDrive) change over time |
| Keyboard-only and screen-reader walkthrough | **Unverified** | AccessKit is enabled; custom rail and table controls have accessible names; shortcuts for Next/Back/Undo/Redo/Search/Help. Needs a Narrator/NVDA pass on Windows |
| Stronger contrast, reduce motion, tips, help panel, review summary | Partially verified | Rendered on Linux/Mesa with `--demo --review` / `--help-panel`; settings back-compat unit test |
| Undo/redo history | Verified | `history` tests |
| Guided and Advanced differ on every page (see user guide table) | Partially verified | Linux/Mesa `--demo` screenshots of every page in both layouts at 1200×760 and 1024×700; not yet tested with real beginners or on Windows |
| ADK auto-install (download, Microsoft signature check, silent Deployment Tools install, re-probe) | **Unverified** | Link and arguments from learn.microsoft.com and published scripts; not run yet |
| Backup-drive guard (> 300 GB) | Verified | `eligibility_rules` test |
| Live Microsoft catalog parses (Windows 10 and 11) | Verified on Windows | Sean's run after the BOM-less UTF-16 fix listed language/architecture/package for both |
| Fixed 960×456 window; only the Customize option list scrolls; pages fill the window; Settings closes with Done/Esc | Partially verified | Linux/Mesa `--demo` screenshots of every page in both layouts plus Help, Settings and the launch question. Scale setting resizes the window with its contents (75/100/150% screenshots match pixel-for-pixel in layout); not yet checked on Windows at 125–200% display scaling |
| Windows 10 host | **Unverified** | Supported by design; not run |
| USB eligibility rules, identity check, diskpart scripts (MBR/GPT FAT32, GPT NTFS, 32 GB cap, DATA partition order), file plan (split only install.wim on FAT32, none on NTFS), labels | Verified | `wip-core::usb` tests |
| Write-through copy, uncached verify (aligned buffers), drive-letter detection for setup and DATA partitions | **Unverified** | Compiles and passes clippy for Windows; needs a real stick |
| Driver INF parsing (encodings, [Version], architecture decorations, signed = CatalogFile), folder scan, plan placement and skips | Verified | `wip-core::drivers` tests |
| Adding drivers with DismAddDriver to install.wim and boot.wim #2; one failure skipped | **Unverified** | Needs an elevated Windows run; signature from mingw-w64 dismapi.h |
| dism.exe / oscdimg Microsoft-signature check; `.sha256` sidecar | **Unverified** | Same Get-AuthenticodeSignature / Get-FileHash pattern as the verified-in-code ADK and ESD checks; needs a Windows run |
| Boot manager signer readout and 2023 boot-file swap (paths from Microsoft's Make2023BootableMedia.ps1); ISO built with efisys_ex.bin | **Unverified** | Needs a Windows run and a Secure Boot boot on 2011- and 2023-trusting firmware; CA parsing verified by `wip-core::secureboot` tests |
| Package kind/architecture guessing, Microsoft install-image order, KB parsing | Verified | `wip-core::packages` and boottest tests |
| `dism /Add-Package` of MSU/CAB into install.wim, 24H2 checkpoint lookup | **Unverified** | Needs an elevated Windows run with real catalog files |
| autounattend.xml content (only documented elements, no partitioning/keys/ProtectYourPC, escaping, validation) | Verified | `wip-core::unattend` tests |
| Windows Setup honoring the answer file on 24H2/25H2 (local account, skipped pages) | **Unverified** | Needs real installs; reports say some windowsPE settings are ignored on 24H2 |
| reg query parsing, undo script content (restore/remove, feature/capability reversal, Setup-only values excluded) | Verified | `wip-core::revert` tests |
| Prior values read from offline hives before writing; undo.ps1 run on an installed PC | **Unverified** | Needs a Windows build and install |
| Drivers page renders (Guided and Advanced) | Partially verified | Linux/Mesa `--demo --drivers` screenshots |
| USB page renders (Guided and Advanced rails) | Partially verified | Linux/Mesa with `--demo --usb` sample drives |
| Writing a real USB stick; BIOS and UEFI boot from it | **Unverified** | Needs elevated Windows run with a real stick |
| Footprint diff and report text; recipe port statuses (same / changed / missing / unverified / new / version-only) | Verified | `footprint` and `recipe_port_detects_same_changed_missing_new` tests |
| Boot test answer file, check generation, in-guest script, scoring, encoded loader size | Verified | `boottest` tests |
| Boot test end to end on Hyper-V; footprint numbers from a real build | **Unverified** | Run docs/verification-checklist.md |
| Catalog parsing, host allow-list, file-name safety, ESD image classification | Verified | `download` tests (fixture shaped after the documented `MCT/Catalogs/Catalog/PublishedMedia/Files/File` layout) |
| Live catalog download, ESD download/resume/hash check, ESD→ISO conversion, booting the result | **Unverified** | Needs a real run; the real products.xml has not been inspected in this environment. A first real run failed with WIP-E114 (products.xml not found after `expand -F:products.xml`); unpacking now takes every file and picks the product list by content. Re-test pending |

## Destructive operations review

| Operation | Safeguards |
|---|---|
| Opening the ISO | Attached read-only; detached afterwards only if WinISOPilot attached it |
| Copying media | Into a new uniquely named workspace folder; never overwrites |
| Mounting | Journal record written first (mount refused if the journal can't be written); failed reads unmount without saving |
| Applying changes | Only to the private exported copy; cancel checked between operations; registry hives unloaded on any failure |
| Commit / unmount | Never cancelled part-way; discard is always available |
| Output ISO | Refuses an existing path; partial ISO deleted if `oscdimg` fails |
| Erasing a USB drive | Only USB-bus disks; never system/boot, read-only, offline, under 7 GB, or holding the source/working folder; identity re-checked right before diskpart; typed `ERASE`; cancel honored only before erasing and between files (the stick is then reported incomplete) |
| Boot test VM | Created only in a new `WinISOPilot-BootTest-<time>` folder; VM has no network; its disk is the VM's own VHDX; removed with its folder only after a pass (unless kept) or on cancel |
| Leftover sessions | Shown on next launch: discard (unmount without saving) or forget the record; workspace files are never deleted by the app |

## Security and privacy

- No telemetry, accounts or auto-update. Network use happens only when the user selects Load list or Download: Microsoft hosts only (allow-list), HTTPS preferred, every ESD verified against Microsoft's catalog hash before use; files with no usable hash are refused.
- Runs elevated because DISM and offline hive loading require it (see [docs/decisions.md](docs/decisions.md)).
- External tools run from System32 or the chosen ADK folder by full path, without console windows. PowerShell strings use single-quoted literals with `'` doubled; package names passed to DISM come from DISM's own inventory.
- `dismapi.dll` is loaded with `LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS` (no current-directory search).
- Recipes contain option IDs and output settings only; they can't carry commands, paths or credentials. Unknown IDs are dropped and reported.
- Windows Update, Defender, firewall and recovery aren't disabled by any preset; Windows Security UI and app frameworks can't be removed.
- Setup answers: the optional local-account password is plain text on the media (stated in the UI), removed from the installed system at first logon, never saved in recipes.
- Drivers: only packages with a CatalogFile (signed) are added, never with ForceUnsigned; dism.exe and oscdimg.exe must be validly signed by Microsoft before a build runs.
- `unsafe` is confined to `wip-engine/src/win/*` (FFI). Every block has a SAFETY comment.

## Dependencies

183 crates for the Windows target; all permissive (MIT, Apache-2.0, BSD, Zlib, ISC, Unicode-3.0, BSL-1.0, CC0-1.0). No copyleft code. Release task: ship a generated third-party notices file (e.g. `cargo about`).

## Open verification items

1. Elevated end-to-end runs on Windows 11 24H2/25H2 and Windows 10 22H2 ISOs (single and all editions, WIM and ESD sources), plus cancel and power-loss drills.
2. VM install test of the output (BIOS and UEFI), with each preset.
3. Real screenshots for the README (list in [docs/screenshots.md](docs/screenshots.md)).
4. Accessibility pass (keyboard only, Narrator, 200% scaling, high contrast).
5. Measure performance budgets (cold start, idle CPU/memory, build time) and record them.
6. Third-party notices file, code signing, checksums.
