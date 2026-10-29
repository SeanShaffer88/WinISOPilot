# WinISOPilot staged implementation roadmap

This is the requested product backlog, not an implementation or compatibility claim. Each stable ID below names a future delivery gate. The current stage adds ten native WinForms workspaces inspired by Windows navigation patterns, a slate/crimson palette, measured job telemetry, reference filtering and a nonexecuting recipe simulation. It uses no WinUI/Mica runtime and introduces no package dependency.

Status vocabulary: **Available** means implemented with bounded local verification; **Partial** identifies exactly what works; **Not implemented** means no execution path; **Research needed** means build-specific behavior or dependency evidence remains unresolved. Every future operation must retain source immutability, explicit scope, safe cancellation and owned-resource cleanup. Tests cannot establish boot or installation success without a VM/firmware run.

| Workspace | Current scope |
|---|---|
| Overview | Available: shared ISO/edition/choice state, observed job phase, byte counters and elapsed time; no invented ETA or savings |
| Source | Available: ISO inspection and separate ISO/WIM/ESD query-only container inventory; SWM recognized and blocked |
| Editions | Available: installation-edition selection; generic inventory separately reports boot indexes; no index removal |
| Components | Reference preview: filterable researched families; actual installed components unknown |
| Apps & Features | Reference preview: app, feature and language families; removal unavailable |
| Drivers & Updates | Reference preview: driver/update families; integration unavailable |
| Settings | Preview: six broad recipe goals shared with Undo; no registry/service changes |
| Unattended Setup | Simulation: current recipe, optional region wish and prerequisite checks; no answer file or commands |
| Build & Export | Available: nonexecuting plan export, separate ISO copy/mastering, optional explicit Retail Home/Pro preference |
| Recovery & Logs | Available: last operation evidence, safety latch and honest readiness; no mount recovery commands |

## Phase 1 — foundations (in progress)

| ID | Capability | Status and gate |
|---|---|---|
| P1-01 | Native ISO ingestion | Available: read-only native attachment with exact optical association and source hash checks |
| P1-02 | WIM/ESD ingestion | Partial: query-only WIMGAPI container metadata, no unpack/apply/mount; actual ESD codec behavior still needs representative media testing |
| P1-03 | Split SWM sets | Not implemented: recognized and refused before opening; future contiguous-set validation and per-part leases/hashes; split sets cannot be modified directly |
| P1-04 | Install/boot/WinRE index inventory | Partial: ISO install and boot containers plus standalone WIM/ESD. WinRE embedded in install images is not discovered; explicitly supplied WinRE WIM can be queried |
| P1-05 | Native mount abstraction | Not implemented: typed readiness model always denies execution. Requires documented tool/build profile, elevated token, owned copy and known expansion capacity |
| P1-06 | Owned mount journal and recovery | Not implemented: intent before native calls, observed mount tuple and per-session ownership; unknown/foreign mounts must be retained |
| P1-07 | Lock detection and safe discard | Not implemented: no forced process termination, global cleanup, arbitrary force-unmount, or host ownership changes |
| P1-08 | Scratch relocation and storage preflight | Partial: no-effect local/new/NTFS path assessment and checked estimate of three source copies + known expanded size + 1 GiB reserve. Unknown expansion blocks servicing. No universal 20–30 GB guarantee |

## Phase 2 — setup options and provisioned apps

| ID | Capability | Status and gate |
|---|---|---|
| P2-01 | Offline Appx enumeration and removal | Not implemented: inventory actual provisioned package identities before selecting targets; app names are not removal authority |
| P2-02 | Appx dependencies and preservation | Not implemented: preserve Terminal, Notepad, Store and required VC++/framework runtimes; validate dependencies before each removal |
| P2-03 | App association/protocol cleanup | Research needed: Windows package registration, shell handlers and user defaults are distinct; no blind registry purge |
| P2-04 | WinPE hardware-requirement options | Research needed: requested TPM, Secure Boot, RAM, CPU, Storage and Disk LabConfig overrides are unverified per build; DLL patching is not implemented or assumed supported |
| P2-05 | OOBE network/local-account options | Research needed: NRO/MSA behavior changes with Windows builds; never promise a registry value will force a working local-account flow |
| P2-06 | Automatic device-encryption preference | Not implemented: explicit security tradeoff and build/edition check, never preselected by a recipe |

Requested app targets: Xbox Gaming Overlay, Phone Link, Copilot, Cortana, Clipchamp, News, Weather, Finance, Solitaire, Quick Assist, Feedback Hub and Mixed Reality. These are research targets; absent packages are not removable items. OneDrive setup stubs are a separate Win32/filesystem operation, not an Appx synonym.

## Phase 3 — features, component optimization and slimming

| ID | Capability | Status and gate |
|---|---|---|
| P3-01 | Windows optional features/FOD/capabilities | Not implemented: inventory actual feature names, states, dependencies and matching payloads |
| P3-02 | Component-store cleanup | Not implemented: conservative StartComponentCleanup first; ResetBase separately warns that existing update rollback is lost |
| P3-03 | Hardlinks and payload compression | Research needed: validate link integrity and servicing health; do not invent compression formats or expected savings |
| P3-04 | Diagnostic/telemetry components | Research needed: DiagTrack, UTC/Connected User Experiences, CEIP and WerSvc have different policy/service/task boundaries |
| P3-05 | Recovery/redundancy components | Research needed: WinRE, System Restore and hibernation have distinct recovery and runtime effects; no silent removal |
| P3-06 | Language/accessibility pruning | Not implemented: TTS, speech recognition, handwriting and locale data require dependency and retained-language checks |
| P3-07 | OneDrive offline installers | Research needed: architecture/build-specific System32/SysWOW64 payload ownership, updates and shell associations |
| P3-08 | Defender/SmartScreen/Security Center profiles | Research needed: never preselected; disabling a setting is not equivalent to supported component removal; no current execution path |
| P3-09 | Component dependency matrix | Not implemented: explicit warnings/hard blocks backed by build-specific evidence, including printing, networking, crypto, anti-cheat and Windows Update dependencies |

Requested feature targets: legacy Windows Media Player, WordPad, IE/IE Mode, Steps Recorder, Math Recognizer, OpenSSH Client, PowerShell 2.0, XPS and Fax/Scan. Some targets are absent from newer builds; the engine must report observed absence rather than advertise a successful removal.

## Phase 4 — offline policies, registry, services and tasks

| ID | Capability | Status and gate |
|---|---|---|
| P4-01 | Offline SOFTWARE/SYSTEM/DEFAULT hives | Not implemented: owned-copy hives only, explicit load/unload ownership and verified cleanup; no host policy edits |
| P4-02 | Privacy/search preferences | Not implemented: Advertising ID, Tailored Experiences, Activity History, Input Personalization, Cloud Search, Bing integration and Search Highlights; edition/build applicability required |
| P4-03 | Diagnostic-data policy | Research needed: AllowTelemetry=0 does not guarantee zero diagnostic data on every edition; use documented applicable policy semantics |
| P4-04 | Windows Update/Delivery Optimization | Not implemented: AUOptions=2 means notify before download, not fully disabled or security-only updates. Security-only servicing promises require separate evidence |
| P4-05 | Explorer/taskbar/theme defaults | Not implemented: classic context menu, hidden/system/extension visibility, This PC start view, Widgets/Copilot/Task View preferences, app/system light/dark defaults |
| P4-06 | Background and consumer content | Not implemented: GameDVR recording, consumer app offers, lock-screen tips and SilentInstalledAppsEnabled; defaults may be reapplied by servicing |
| P4-07 | Service startup policy | Not implemented: inventory and allowlist Remote Registry, Maps Manager, Geolocation, WerSvc, Distributed Link Tracking and DiagTrack; cannot assume the same names/state across builds |
| P4-08 | Scheduled tasks | Research needed: task XML, task registration and policy relationships must agree; no raw file deletion as task management |

Requested task research targets: Application Experience Compatibility Appraiser/ProgramDataUpdater; CEIP Consolidator/UsbCeip; Autochk Proxy. Service, scheduled task, package and filesystem operations remain separate typed operations.

## Phase 5 — slipstreaming, assets and unattended automation

| ID | Capability | Status and gate |
|---|---|---|
| P5-01 | CAB/MSU SSU/LCU/.NET integration | Not implemented: actual applicability, servicing prerequisites and cumulative-package ordering before execution |
| P5-02 | INF driver integration | Not implemented: signed package validation, architecture/build matching and explicit boot/setup/install index scope |
| P5-03 | Cross-index synchronization | Not implemented: separately preview drivers/changes for boot.wim index 2, selected install indexes and WinRE; never assume scopes equivalent |
| P5-04 | OEM assets and themes | Not implemented: support URL/logo, wallpaper and theme paths with bounded formats and undo snapshots |
| P5-05 | Custom root CA trust | Not implemented: explicit trust expansion, certificate validation and user approval; never silently imported by a preset |
| P5-06 | Answer-file builder | Simulation only: no autounattend.xml emitted. Future windowsPE/specialize/oobeSystem settings must have schema and pass validation |
| P5-07 | Disk/layout automation | Not implemented: explicit target disk, GPT EFI/MSR/Windows/Recovery layout and formatting review. Never default disk-wipe behavior |
| P5-08 | Identity and setup passes | Not implemented: names, time zones, keys, workgroup/domain, local users/password policy and autologon; no secrets in presets or logs |
| P5-09 | Privacy consent/EULA choices | Not implemented: separate explicit choices, not automatic consent hidden inside optimization presets |
| P5-10 | Post-setup payload queue | Not implemented: FirstLogonCommands/SetupComplete, silent installers, portable utilities and scripts require explicit approved payloads and execution order |

Requested driver categories: NVMe, Intel RST/VMD, USB 3.x, wired NIC and Wi-Fi. A syntactically valid INF is not proof of a trusted signature, hardware match or successful driver installation.

## Phase 6 — export, mastering, portability and CLI

| ID | Capability | Status and gate |
|---|---|---|
| P6-01 | ISO mastering | Available: Microsoft ADK Oscdimg dual BIOS/UEFI media copy using UDF 1.02, full copied-file readback and source identity checks. Firmware boot remains untested |
| P6-02 | UDF 2.01 option | Not implemented: requested new export choice requires mastering/readback and firmware tests; current output is not advertised as 2.01 |
| P6-03 | Selective index export/removal | Not implemented: export retained indexes into a new WIM; no in-place source mutation |
| P6-04 | WIM/LZX and ESD/LZMS export | Not implemented: native supported compression choices, runtime capability and output integrity checks; no unsupported thread-count promise |
| P6-05 | FAT32 split export | Not implemented: new SWM set with size/part verification; cannot assume every boot loader or firmware is compatible |
| P6-06 | Declarative recipes | Partial: existing nonexecuting JSON broad-goal export. Full versioned recipe import/validation, migrations and TOML remain unimplemented |
| P6-07 | Headless commands | Partial: inspect/inventory/plan/build. CLI termination is not guaranteed safe cancellation; no servicing automation yet |
| P6-08 | Structured timings and error translation | Partial: observed progress, elapsed time and per-run native/process evidence. Human error maps and footprint accounting remain future work |
| P6-09 | Final footprint/reporting | Not implemented: measured before/after output size, packages/services changed and validation evidence. No estimated saving is substituted for a measurement |

## Validation and source references

Every implementation stage requires local tests, independent audit and real compatible-image checks. Privileged mount/servicing tests require an elevated test environment and a documented ADK/image combination. Current unknown profiles stay blocked. Completed outputs never become cancellation-cleanup targets; Undo restores selections, not completed ISO bytes.

- [Microsoft image-management commands](https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/dism-image-management-command-line-options-s14?view=windows-11)
- [Microsoft split-image limits](https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/split-a-windows-image--wim--file-to-span-across-multiple-dvds?view=windows-11)
- [Microsoft DISM platforms](https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/dism-supported-platforms?view=windows-11)
- [Microsoft component-store cleanup](https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/clean-up-the-winsxs-folder?view=windows-11)
- [Microsoft Update policy semantics](https://learn.microsoft.com/en-us/windows/deployment/update/waas-wu-settings)
- [Microsoft Oscdimg options](https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/oscdimg-command-line-options?view=windows-11)
- [Microsoft navigation design](https://learn.microsoft.com/en-us/windows/apps/design/controls/navigationview)

No third-party tool code, paid runtime, external unpacker or new package was added for this stage. Research is not feature parity or a claim of support for arbitrary deep stripping.
