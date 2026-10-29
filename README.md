# WinISOPilot

A guided Windows 11 installation-image customization project. The current prototype is a **read-only inspector and planning tool**. It does not service Windows images or create installation media.

## Current scope

- A five-step Windows desktop walkthrough: Source ISO → Edition and checks → Your goals → Explore options → Review and export.
- A searchable catalog of 42 researched option families with purpose, prerequisites, consequences, evidence and separate implementation status. It is exploratory, not exhaustive, and is not a live image-component inventory.
- Explicit separation between observed metadata, user choices, planned operations, and verification that has not run.
- No paid-tool runtime dependencies. NTLite, MSMG ToolKit, DISMTools, WimWizard, and Dism++ inform research rather than supply copied application code.
- No host-OS modification, component removal, writable WIM mounting, ISO creation, USB formatting, or setup execution. Read-only ISO attachments may be created or reused to query the media.

## Use the walkthrough

1. Choose a Windows installation file (`.iso`) on a local drive, then select **Inspect ISO**. Choose a parent folder for a new report; it can contain private local paths. The app reads versions inside the file without extracting or changing them.
2. Choose an edition actually observed in the source. Review its self-reported metadata, source identity, local tools and compatibility result. Unknown compatibility does not mean success.
3. Record any of the six broad future goals: updates, drivers, languages, apps, setup and recovery-image planning. All requested operations remain `NotImplemented` and nonexecuting.
4. Explore reference families. Catalog rows explain possibilities; they are not extra selectable changes and do not imply installed components.
5. Review and export a future plan into a new output folder. Export rechecks source identity and cannot build or modify installation media.

Select **?** or press **F1** for step instructions and a plain-language glossary. Hover hints are off by default. Help offers optional short hints for buttons and small choices, with a delay; large text fields, labels, panels and window backgrounds never show hover help. Accessible descriptions and visible instructions remain available. Undo restores version/goal selections only, up to 100 snapshots. It does not undo source activity, reports, exported files or Windows changes.

The unified desktop shell uses an 11-point system font, a fixed 920×660 logical client size and startup DPI/working-area clamping. Long paths, reports and explanations remain selectable or vertically scrollable; **Full details** preserves long status/error text. Light/dark switching changes app-owned surfaces only. Windows high-contrast colors take priority, and native file/folder pickers keep Windows styling. Geometry and palette tests do not substitute for visual testing at every DPI or display size.

The GUI supports inspection pause/resume and safe cancellation at checkpoints. Active native reads finish before requests take effect; final source verification and native cleanup cannot pause. Closing the GUI during active work requests cancellation and waits rather than forcing process termination. These cooperative controls are GUI-only: CLI Ctrl+C or OS process termination is not a guaranteed safe cancellation path. Plan export is a separate noncancelable operation.

After an eligible canceled inspection, keeping generated diagnostic files is the default. Optional deletion is limited to that run's validated generated-file allowlist; unknown identity, unrelated entries, locks or cleanup problems retain files. It never deletes the source ISO, package inputs, exported plans or mounts. Deletion does not undo Windows changes or repair an image.

## Read-only metadata boundary

The app uses native Windows ISO APIs and read-only edition queries. When a DISM metadata query returns elevation-required error 740, a trusted inbox WIMGAPI fallback opens the image container with query-only access (`0`) to read XML metadata. It does not elevate, extract an image, apply a WIM or invoke PowerShell. Native buffers, WIM handles and module references have explicit cleanup observations; unconfirmed cleanup is not reported as success.

The app releases its own nonpermanent ISO handle after native queries, then performs final source-identity verification before the job reaches a terminal state. Existing or other-owner attachments may remain; a handle release is not a claim that every global mount disappeared. Source metadata is container-self-reported. Matching SHA-256 before/after means the bytes are unchanged, not that Microsoft authenticated the media, that it is malware-free, or that Windows will install.

## Build and check

Requires Windows x64 and the .NET 10 SDK to build. A framework-dependent published app requires the .NET 10 Windows Desktop Runtime. No third-party NuGet packages are required. The included NuGet configuration clears package sources.

```powershell
dotnet build src/WinISOPilot/ImageGuide.csproj --configuration Release
dotnet run --project src/WinISOPilot/ImageGuide.csproj --configuration Release --no-build -- --self-test
dotnet run --project src/WinISOPilot/ImageGuide.csproj --configuration Release --no-build
```

Run `--help` for supported CLI commands. No arguments opens the GUI. Self-tests exercise the prototype; they do not establish that an installation ISO boots or that Windows installs successfully.

## Direction

Detailed customization and build stages remain future work. A servicing engine must use compatible Microsoft DISM/ADK tooling, typed operations, explicit input validation and owned-resource recovery. Deep component removal remains unvalidated. The current compatibility profiles are conservative; preview or unrecognized builds remain Unknown. The catalog cannot grant execution permission.

See [architecture](docs/architecture.md) for boundaries and validation gates. The MIT [license](LICENSE) applies to this repository; generated icon provenance is recorded alongside the source assets. Windows media and Microsoft deployment tools are supplied separately under their own terms.
