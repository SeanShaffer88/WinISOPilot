# Decision log

| # | Date | Decision | Why | Trade-off |
|---|---|---|---|---|
| 1 | 2026-10-30 | Rewrite the C# WPF prototype in Rust | Owner's choice; matches the project family's Rust default | Earlier C# work (Fido download, inspection) to be re-implemented |
| 2 | 2026-10-30 | egui/eframe UI | Quick to build a rich wizard; single exe; AccessKit screen-reader support | Not a native Fluent look; needs OpenGL 2+ (glow) |
| 3 | 2026-10-30 | DISM API loaded at runtime, ADK copy preferred | Progress callbacks and cancel events; works with the DISM that matches newer images | Struct layouts must match DismApi.h (checked against mingw-w64) |
| 4 | 2026-10-30 | `dism.exe` for app removal, export and cleanup; PowerShell for app listing | The DISM API's appx structures aren't in public headers we could verify; PowerShell JSON is locale-independent; the API has no export or cleanup calls | Extra processes; output parsed for progress |
| 5 | 2026-10-30 | `reg.exe` for offline hives | Handles the backup/restore privileges hive loading needs; transparent commands in the log | One process per value |
| 6 | 2026-10-30 | wimlib deferred | DISM export covers WIM↔ESD; avoids shipping an LGPL DLL in 0.1 | No solid-compression tuning or faster export yet |
| 7 | 2026-10-30 | Scope: Windows 10/11 images only | Owner direction; Windows 7/8.1 lack capabilities/appx and need different tooling | Older images rejected with a clear message |
| 8 | 2026-10-30 | Layout: Guided (mockup 2) by default, Advanced (mockup 5) optional; chosen on first launch, switchable in Settings | Owner choice from six mockups | Two Customize layouts to maintain |
| 9 | 2026-10-30 | `requireAdministrator` manifest | Nearly every step (DISM, mount, hive load) needs elevation | Whole app runs elevated |
| 10 | 2026-10-30 | Customize only after the edition is mounted | Offer only what the image contains; resolve exact package names | A few minutes of preparation before choosing |
| 11 | 2026-10-30 | Single-edition output by default | Smaller ISO, Setup doesn't ask; matches most users' intent | Users wanting all editions untick one box |
| 12 | 2026-10-30 | No ISO without `oscdimg` (folder instead) | Writing a correct dual-boot UDF ISO natively is a project on its own | ADK needed for ISO output in 0.1 |
| 13 | 2026-10-30 | TPM/Secure Boot and RAM bypass rated Moderate, CPU bypass High | Setup-check bypasses don't damage anything but leave PCs unsupported; very old CPUs can still fail | Shown from Intermediate level |
| 14 | 2026-10-30 | Never offer disabling Windows Update, Defender or firewall | Owner's dev charter | Users who want this need other tools |
