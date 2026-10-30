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
| 15 | 2026-10-30 | Three differentiators: measured footprint, Hyper-V boot test, recipe porting | Owner chose all three; they turn claims into evidence and keep recipes useful across Windows releases | More code paths to verify on real hardware |
| 16 | 2026-10-30 | Follow the existing codebase (crates layout, `EngineError` codes, synchronous jobs on a worker thread) rather than a `src/features/` + `thiserror` + async spec | Owner's choice; avoids a refactor with no user-visible benefit | None |
| 17 | 2026-10-30 | Boot test answer disc built with `oscdimg`; Setup only searches removable/optical media for `autounattend.xml` | Keeps the user's ISO untouched | Needs the ADK |
| 18 | 2026-10-30 | Boot test uses Microsoft's published KMS client setup keys to pick the edition | Published for setup use; they don't activate | Home editions have no published key |
| 19 | 2026-10-30 | Boot test polls VM state every 15 s on the job thread | Hyper-V has no completion event reachable without WMI eventing; 15 s is negligible load | Up to 15 s latency |
| 20 | 2026-10-30 | In-app download from the Media Creation Tool catalog (products.cab) instead of scraping Microsoft's website or bundling Fido | Catalog is Microsoft's own distribution path with per-file hashes; no website scraping or GPL script | Only the current release is offered |
| 21 | 2026-10-30 | Use Windows' built-in `curl.exe`, `expand.exe` and `Get-FileHash` | No new Rust dependencies or TLS stack; resumable downloads | Needs Windows 10 1803+ |
| 22 | 2026-10-30 | Releases published by a workflow on `v*` tags or `release/v*` branches | This environment can't create tags or releases directly | Release appears after CI finishes |
