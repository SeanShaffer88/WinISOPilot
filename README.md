# WinISOPilot

## Single-instance desktop and ISO downloads

Only one participating GUI instance runs for the current Windows user/session, across the WPF and legacy executables and copies in different folders. A second launch signals the existing window and exits; a minimized window is restored when Windows permits foreground activation. CLI commands and self-tests remain independent. Older builds do not participate in this protocol. Cross-elevation activation still needs a live elevated/unelevated check.

The Source page offers **Download Windows 11**. The automatic option uses the bundled, hash-pinned [Fido 1.71 provider](third_party/Fido/NOTICE.md) from Rufus to obtain a Microsoft x64 Home/Pro/Edu link. This first picker offers English and English International for the latest release known to that pin (26H2/build 26300.9457); it is not a complete release/language catalog. It runs only without administrator privileges through Windows PowerShell, never changes execution policy, never downloads script updates, and never runs the ISO. Existing Windows policy can block the provider, as it did in development. The manual alternative opens Microsoft's official page and accepts its generated direct ISO link.

WinISOPilot itself streams the ISO over HTTPS from exact approved Microsoft hosts and revalidates redirects. A declared 4–32 GiB binary response, enough local NTFS space, exclusive partial-file creation, recognized ISO/UDF descriptor, actual transfer length, SHA-256 and file readback are checked before a no-overwrite rename. An optional matching checksum from Microsoft can be supplied; a computed hash alone does not establish publisher authenticity. Expiring query tokens are not written to reports. No source selection or inspection occurs until **Use downloaded ISO** and then **Inspect ISO** are chosen explicitly.

Pause stops reads at chunk boundaries; long pauses may cause server timeout. Cross-session/range resume is not implemented: failed transfers restart. Cancel closes the HTTP request, retains unpublished partial data and offers Keep by default. Optional deletion requires this session's creation identity, unchanged bytes/timestamp/hash and parent-directory checks. Foreign, restarted or changed partials are retained. Fido link requests wait for the link-only process to finish safely before cancellation is acknowledged.

Deterministic transfer/provider fixtures and native single-instance process tests are included in CI. The actual provider was refused by Windows PowerShell's existing Restricted script policy; it was not bypassed. A complete real multi-gigabyte ISO transfer remains **NotRun**. Microsoft connectivity probes do not establish full-transfer success. Downloading build 26300 does not change its separate servicing compatibility block.

Fido is an explicitly approved third-party dependency and ships with its unmodified source, GPL license and provenance notice. The project-specific dependency permission is recorded in [AGENTS.md](AGENTS.md). A guided USB writer and the newly requested premium visual pass are subsequent stages; no raw USB erase/write operation is introduced here.

The default guided view follows the supported journey: explicitly inspect an ISO, confirm an observed edition, optionally request a Retail edition preference, review and create a separate ISO, then read its verified result. Select **Advanced workspaces** to explore the ten optional workspaces. Guide buttons navigate and focus controls; they never start inspection or building automatically. Unimplemented preview wishes are optional and are never prerequisites for creating the supported media copy.

**Your changes** separates actual media choices from preview wishes. Remove one choice to restore Keep/Unchanged; global Undo restores the preceding selection. Edition is context rather than a deletion target. These actions never remove source media, reports or completed ISOs. Choices lock during jobs, while the changes view remains readable. Capability labels derive from current observations and safety state.

A guided Windows 11 installation-image customization project. The current preview inspects media, saves future plans, and **creates a separate ISO copy**, optionally adding a Retail Home/Pro setup edition preference. It does not service or slim Windows images.

## Current scope

- Ten shared-state Windows desktop workspaces: Image & Indexes, Hardware & OOBE, Apps & Packages, System Components, Registry Settings, Services & Tasks, Drivers & Updates, Unattended Setup, Presets & Compare, and Build & Export. The default native WPF desktop uses layered dark gradients, rounded cards, cyan/blue accents, a light/dark toggle and Windows high-contrast colors.
- A searchable catalog of 42 researched option families with purpose, prerequisites, consequences, evidence and separate implementation status. It is exploratory, not exhaustive, and is not a live image-component inventory.
- Explicit separation between observed metadata, user choices, planned operations, and verification that has not run.
- No paid-tool runtime dependencies. NTLite, MSMG ToolKit, DISMTools, WimWizard, and Dism++ inform research rather than supply copied application code.
- No host-OS modification, USB formatting or setup execution. Offline app removal and supported component cleanup are implemented behind the strict first-slice gates below; live successful servicing has not yet been validated. Read-only ISO attachments may be created or reused for inspection and media verification. ISO creation uses installed Microsoft ADK Oscdimg; no paid tool is required.

## First offline servicing slice — experimental, live success not yet validated

The WPF **Apps** workspace can copy `install.wim`, mount the private copy read-only, and query actual provisioned apps. The narrow removal list accepts observed full identities for Clipchamp, Solitaire, News, Weather, Get Help and Get Started with expected publishers and no resource packages. Other apps are visible but blocked; this is not a general dependency resolver. Provisioning removal affects new users of the selected offline edition, not apps on your running PC.

**Components** offers DISM `StartComponentCleanup` after an app inventory. No `ResetBase`, arbitrary component/package stripping, feature-payload removal, WinRE removal or manual system-file deletion is implemented. Cleanup may save no space. Only the selected index is serviced; all editions remain in the ISO. A separate new ISO is published only after post-change app verification, component-health checking, commit/unmount verification, full file readback and original-hash verification.

Required: an explicitly confirmed x64 Windows 11 build **26100** edition, Microsoft ADK DISM **26100**, administrator, a local fixed NTFS run destination with at least four times ISO size plus 60 GB free and enough for the observed expanded image (unknown expansion blocks mounting), and exactly `install.wim`. ESD/SWM conversion is deferred. This conservative profile follows Microsoft's documented Windows 11 offline operations; it is **not live-validation evidence**. Build **26300** with installed ADK 26100 stays blocked. The present development session is unelevated, so real mounted-servicing success cannot be claimed. Boot/install, retained-app behavior, updates, rollback and recovery remain untested.

Read apps again after changing source/edition/tool. Executable selections are separate from preview JSON and exist only in the current app session. Undo restores choices before a build; after a build, create a new copy from the unchanged original to change its contents. Pause/cancel requests wait for DISM to actually finish, then stop at a safe checkpoint; DISM is never forcibly killed. Native errors outrank pending cancellation. A cancellation offers Keep by default. DISM-created mount/scratch trees and modified WIMs are retained because deletion ownership is unproven; existing guarded cleanup can delete only independently proven eligible generated files.

Before mounting, the engine writes a pending recovery record under `%LOCALAPPDATA%/WinISOPilot/ServicingRecovery`. It records exact copied-WIM/mount/index/tool identities. An unfinished record blocks further servicing and tells the user to keep evidence for manual recovery; it never authorizes adoption or automatic cleanup of a historical or foreign mount. No global cleanup-mount command exists. A failed/uncertain unmount latches restart/recovery and prevents publication and cleanup. Pending component actions and unproven health block commit. These checks do not prove every possible Windows dependency or rollback condition.

The legacy CLI media build remains unchanged by default. `--servicing-preflight <inspection.json> <index>` reports installed privilege/profile gates without copying, mounting or modifying an image (exit 18 when blocked). `--servicing-self-test` runs scripted process-boundary fault fixtures in temporary folders; it does not execute DISM. The Windows CI suite includes those fixtures and WPF state/layout checks. A successful elevated compatible-image servicing run remains a separate outstanding acceptance gate.

## Reference-inspired interface

The native WPF presentation follows the supplied six visual references using cyan/blue gradients, rounded panels, a glowing stepper, split source/index view, hardware illustration, catalog trees, honest dependency-unavailable view, live nonexecuting JSON preview and measured progress ring. No violet accents or third-party UI libraries are used. These are native gradient/shadow effects, not a claim of Windows Mica or Acrylic. Hardware, registry, service and integration cards remain **preview wishes only**, default to unchanged, and are never passed to the build engine. Apps and Components now have separate typed executable choices described below; preview recipe imports cannot populate them. Two-column hardware/component cards adapt to content width; long content scrolls inside the fixed 1280×800-DIP window, which clamps to the current work area. No sample mount states, safe-removal badges, fictional payloads or reclaimed-space estimates are displayed.

**Presets & Compare** imports/exports strictly validated, nonexecuting JSON preview recipes (64 KiB limit). The schema accepts only known option IDs, service startup wishes and fixed region choices. It excludes arbitrary commands, credentials and source paths. Imported recipes replace choices atomically, remain undoable, and never start a job. Comparison uses unchanged defaults initially or the choices present before the last import. Existing files cannot be overwritten. TOML is not implemented.

Source inspection, edition selection and readiness are together in **Image & Indexes**. Actual ISO creation, an edition-only nonexecuting plan export, and measured activity are in **Build & Export**. The WPF edition-plan export does not include preview wishes; use **Presets & Compare** to save the full preview recipe. The retained legacy guide also exposes its six broad planning goals.

## Use the workspaces

1. Choose a Windows installation file (`.iso`) on a local drive, then select **Inspect ISO**. Choose a parent folder for a new report; it can contain private local paths. The app reads versions inside the file without extracting or changing them.
2. Choose an edition actually observed in the source. Review its self-reported metadata, source identity, local tools and compatibility result. Unknown compatibility does not mean success.
3. Optionally explore advanced preview wishes. These remain nonexecuting and are never required to create a supported media copy. The six broad planning goals remain in the retained legacy guide.
4. Explore research references or use Ctrl+K to find them. Catalog rows explain possibilities; they are not extra selectable changes and do not imply installed components.
5. Review and export an edition-only nonexecuting plan, save preview wishes separately, or choose **Create new ISO**. Creation copies all media into a new run folder and uses Microsoft ADK Oscdimg to create a separate ISO. Optionally ask Setup to use the selected observed Home or Pro edition through `sources/ei.cfg` (Retail channel, VL=0). This is a setup preference, not a license or activation, and all editions remain inside. Existing edition/product-ID/answer-file configuration blocks the preference; no configuration is silently replaced. Your future goals are not applied.

Select **?** or press **F1** for instructions. The WPF desktop uses inline explanations and optional Details expanders; hover popups are not used. Accessible descriptions remain available. Undo restores edition, edition preference, preview option, service and region selections, up to 100 snapshots. The retained legacy guide also supports its broad planning-goal history. It does not undo source activity, reports, exported files or Windows changes.

The new WPF desktop uses body text of at least 15 DIPs, a fixed 1280×800-DIP size and work-area clamping. Guided mode emphasizes the supported journey; Advanced workspaces shows grouped direct navigation. Long paths, reports and explanations remain selectable or vertically scrollable; **Your job** preserves current operation evidence. The retained legacy WinForms guide keeps its original compact geometry. Light/dark switching changes app-owned surfaces only. Windows high-contrast colors take priority, and native file/folder pickers keep Windows styling. Geometry and palette tests do not substitute for visual testing at every DPI or display size.

The GUI supports inspection and ISO creation pause/resume and safe cancellation at checkpoints. Copying pauses between chunks; active Oscdimg/native reads finish before requests take effect. The original ISO is held read-only with writes/deletion denied for the entire build, rehashed against inspection, and checked again before publication. A partial ISO is renamed without overwrite only after readback verification and the final cancellation checkpoint. Unknown Windows servicing compatibility does not block this media-only operation, because no image servicing occurs. Active native reads finish before requests take effect; final source verification and native cleanup cannot pause. Closing the GUI during active work requests cancellation and waits rather than forcing process termination. These cooperative controls are GUI-only: CLI Ctrl+C or OS process termination is not a guaranteed safe cancellation path. Plan export is a separate noncancelable operation.

After an eligible canceled inspection or build, keeping generated diagnostic files is the default. Optional deletion is limited to that run's validated generated-file allowlist; build files also require identity recorded on their creation handle. Build folders, unexpected entries, external-tool partial ISO files and native diagnostics without creation ownership evidence are retained. No recursive deletion is used; unknown identity, unrelated entries, locks or cleanup problems retain files. It never deletes the source ISO, package inputs, exported plans or mounts. Deletion does not undo Windows changes or repair an image.

## Read-only metadata boundary

The app uses native Windows ISO APIs and read-only edition queries. When a DISM metadata query returns elevation-required error 740, a trusted inbox WIMGAPI fallback opens the image container with query-only access (`0`) to read XML metadata. It does not elevate, extract an image, apply a WIM or invoke PowerShell. Native buffers, WIM handles and module references have explicit cleanup observations; unconfirmed cleanup is not reported as success.

The app releases its own nonpermanent ISO handle after native queries, then performs final source-identity verification before the job reaches a terminal state. Existing or other-owner attachments may remain; a handle release is not a claim that every global mount disappeared. Source metadata is container-self-reported. Matching SHA-256 before/after means the bytes are unchanged, not that Microsoft authenticated the media, that it is malware-free, or that Windows will install.

## Start the new desktop

Run **WinISOPilot.Desktop.exe** from the published folder. The original **WinISOPilot.exe** is retained for CLI compatibility and legacy WinForms tests; it is not the new default interface. Both assemblies must remain beside the desktop executable. A .NET 10 Windows Desktop Runtime is required. Source selection and drop accept one local ISO and never start work automatically. Ctrl+K searches local navigation targets; F1 opens help.

## Build and check

Requires Windows x64 and the .NET 10 SDK to build. A framework-dependent published app requires the .NET 10 Windows Desktop Runtime. No third-party NuGet packages are required. The included NuGet configuration clears package sources.

```powershell
dotnet build src/WinISOPilot.Desktop/WinISOPilot.Desktop.csproj --configuration Release
dotnet run --project src/WinISOPilot/ImageGuide.csproj --configuration Release --no-build -- --self-test
dotnet run --project src/WinISOPilot.Desktop/WinISOPilot.Desktop.csproj --configuration Release --no-build -- --self-test
dotnet run --project src/WinISOPilot.Desktop/WinISOPilot.Desktop.csproj --configuration Release --no-build
```

Run `--help` for supported CLI commands. No arguments on WinISOPilot.Desktop.exe opens the new WPF GUI. Self-tests exercise the prototype; they do not establish that an installation ISO boots or that Windows installs successfully.

## Direction

The first gated Windows-image servicing slice is implemented as described above, with live success still unverified. The earlier edition-preference and separate ISO creation remain available without servicing. Its verifier reads back every media file and compares hashes and edition inventory, but does not prove bootability, installation or Setup honoring the preference. Real firmware/VM boot and installation gates remain NotRun. A servicing engine must use compatible Microsoft DISM/ADK tooling, typed operations, explicit input validation and owned-resource recovery. Deep component removal remains unvalidated. The current compatibility profiles are conservative; preview or unrecognized builds remain Unknown. The catalog cannot grant execution permission.

See [architecture](docs/architecture.md) for boundaries and validation gates. The MIT [license](LICENSE) applies to this repository; generated icon provenance is recorded alongside the source assets. Windows media and Microsoft deployment tools are supplied separately under their own terms.

## Create a separate ISO from the command line

```powershell
WinISOPilot.exe build --inspection C:\reports\inspection.json --index 6 --prefer-edition Retail --output C:\builds\new-run
```

Omit `--prefer-edition Retail` to preserve the original setup configuration. Index must be observed in the source; Home/Core and Pro/Professional are supported for preference writing. Requires local x64 Windows 11 media, standard BIOS/UEFI boot files, one `install.wim` or `install.esd`, installed ADK Oscdimg, a new output folder without `#`, and free space of three times the copied media size plus 1 GB. No input scripts or commands run. CLI Ctrl+C still does not offer guaranteed safe cancellation; use GUI Pause/Cancel for cooperative job controls. Completed outputs are never eligible for canceled-run cleanup. Existing outputs are never overwritten. A valid Windows license is supplied separately.

## Container inventory and staged roadmap

Source also offers **Inspect image containers**. It queries installation and boot WIM metadata inside an ISO, or a standalone WIM/ESD, using trusted query-only WIMGAPI. The generic parser accepts WinPE metadata without weakening the separate Windows 11 client validation used by actual ISO builds. Source files are held read-only and hashed before and after. Split SWM is recognized but blocked; no Windows image mount or extraction runs. Embedded WinRE discovery is not implemented. Reports may include private local paths. Canceling this inventory retains reports and explains that automatic cleanup is unavailable for this scope.

Overview displays observed phase, bytes and elapsed time from real jobs, including paused time. It does not fabricate ETA, size savings, throughput or boot success. Sidebar navigation remains available while work is active; source/edition/goals/build changes stay locked. Unattended Setup is a **nonexecuting recipe simulation**: it previews current choices and missing implementation/compatibility checks. Its region wish is preview-only and is not an exported or executed setting.

`WinISOPilot inventory --source <local.iso|wim|esd> --output <NEW directory>` exposes the same query-only inventory in the CLI. ESD support uses the installed WIMGAPI codec and still needs representative real ESD validation. This generic inventory remains query-only. The separate Apps workflow implements private mounting and narrow writable servicing under strict gates; historical mount recovery remains manual. Scratch capacity assessment never approves an unknown expansion size or an unimplemented worker.

See [the full six-phase roadmap](docs/roadmap.md) for the requested feature families, stable IDs, statuses, dependencies and evidence gates. The first foundations increment is partial; phases 2–6 are successive work, not completed capabilities. A matching [interactive design preview](docs/fluent-workspaces-mockup.html) is included.
