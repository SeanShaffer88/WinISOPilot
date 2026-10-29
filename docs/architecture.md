# Architecture and verification boundaries

## Present prototype

The Windows GUI and CLI expose inspection, plan review and a bounded media-copy/build workflow. Ten workspace pages share one source, edition, broad-goal and job state. Overview/recovery views expose observations; Source/Editions/Build retain existing engine guards. Reference catalogs and recipe simulations never authorize operations. Native installation-media querying remains independent of any future privileged servicing worker. Reports distinguish observations from planned actions and unknowns. Selected media is not executed.

Pause and cancellation requests are cooperative. Hashing and operation boundaries provide checkpoints; active native queries and their stream drains complete before native lifetimes are released. Final integrity verification and cleanup are not interrupted. Closing the GUI while active requests cancellation and waits. Diagnostic-file deletion is a separate user choice after an eligible terminal canceled state, defaults to keeping files, and is restricted by the core to a per-run generated-file allowlist.

Source identity changes invalidate GUI inspection results and selection history. Plan export independently rehashes the source. GUI Undo stores at most 100 edition/goal snapshots and has no filesystem or OS undo semantics. Catalog families have no checkboxes and are not silently translated into export choices.

Inspection outputs can contain private local paths and image identifiers. Keep real reports, logs, media, and machine-specific choices outside version control. CI runs deterministic fixtures and uploads no inspection artifacts.

## Future implementation

1. Unelevated UI collects source and choices into a typed manifest.
2. Read-only preflight verifies image identity, build, edition, architecture, payloads, tools, workspace, and mount ownership.
3. A capability planner declares applicable operations, dependencies, conflicts, affected image scopes, and verification requirements.
4. A minimal privileged worker executes an allowlist of operations with structured arguments and fixed tool identities.
5. A journal tracks app-owned resources, observed exit status, verification, and recovery. Cancellation does not imply successful cleanup.

Original media is preserved. No ISO workflow may silently substitute host `/Online` servicing or clean unrelated mounts. Boot, install, and recovery images require separate scope decisions. Secrets do not belong in reusable presets or reports.

## Gates

- Source/build/architecture and payload compatibility checks before servicing.
- Inventory verification after each material operation and explicit process/log evidence.
- Media-layout and boot-entry checks after export.
- Real VM boot, installation/OOBE, update, network, recovery, and retained-app tests before claims of usable installation media.

Native DISM package/feature/app provisioning operations are not equivalent to arbitrary NTLite or MSMG component removal. Such parity remains a research gap; working software must not promise it without build-specific implementation and test evidence.

This repository implements media copying, optional edition preference, ISO mastering and file readback verification. It does not implement the privileged servicing stages above.

## Low-level components and future boundaries

The present C# implementation already uses low-level Windows facilities where they serve the read-only task: `NativeIso.AttachOrReuse` and `VerifyAssociation` manage and recheck ISO access; `WimMetadata.Read` performs query-only native metadata calls; `JobControl` coordinates cooperative checkpoints. The engine bounds captured output, verifies source hashes and latches unresolved cleanup failures. Generated-file cleanup uses validated ownership and identity instead of treating a folder name as deletion authority. These safeguards now support the bounded media-copy/build workflow below; privileged image servicing remains future work.

Future servicing should introduce three independent contracts, **none implemented yet**: an `IImageWorker.ExecuteAsync(ApprovedImageOperation, JobContext)` boundary for a separate privileged worker that independently validates an allowlist of typed operations and accepts no raw commands; an `IResourceJournal.Record(ResourceEvent)` / `Reconcile(JobId)` boundary for observed app-owned resources without assumed cleanup authority; and an `IImageVerifier.VerifyAsync(ExpectedImageState, ObservedArtifact)` boundary for independent checks reported as passed, failed or not run. A successful tool exit is never proof that media boots or Windows installs.

Prefer Microsoft APIs and the standard library. There is no plan to invent ISO/WIM parsers or cryptography or to rewrite the application in Rust. A future isolated native component would require a demonstrated need, a reviewed interface and its own evidence and tests before changing that decision.

## Media build boundary

`Engine.BuildIso(Inspection, MediaBuildChoices, output, JobControl, progress)` rediscovers installed ADK Oscdimg and supports x64 client media only. A read lease denies source writes/deletion through source hashing, read-only attachment, deterministic bounded media copy, mastering and final verification. Caller inspection metadata is rechecked using trusted query-only WIMGAPI; metadata alone never authorizes commands. No raw command or `/Online` mode exists.

The optional EI.cfg action is restricted to observed Core/Professional IDs and explicit Retail/VL0 configuration. Existing EI.cfg, PID.txt or root/sources answer files reject this action before copying. All editions and all payload bytes remain. This is no claim that Windows Setup honors the preference on every build.

Copied media and app-managed configuration/log files are opened CreateNew and record identity from their creation handles. The exclusive run creator also records the ownership marker identity. Root and nested directory identities bind the session-owned journal. External-tool files without creation ownership are retained during cleanup. Canceled cleanup freezes final hashes, lengths, timestamps and identities, rechecks a matching preview, and deletes only guarded unchanged regular single-link files. Directory identities are rechecked before each deletion. No recursive delete is used; folders and unrelated entries remain. Cleanup after app exit is not authorized by a disk marker alone.

Oscdimg runs through an absolute discovered tool path and structured ArgumentList. stdout/stderr are bounded but fully drained, actual process exit is awaited, and time-budget/error failures outrank pending cancellation. Pause/cancel does not kill a child. Native WIM cleanup uncertainty latches restart and disables cleanup. Publication is a no-overwrite rename from a partial path only after complete file-set/hash readback, preserved edition inventory, original hash verification, observed native handle release and final cancellation check. Boot, installation, activation, updates and recovery are separate NotRun gates.

## Generic container inventory and workspace state

`WimMetadata.ReadRaw` owns the same query-only native buffer/handle lifecycle; existing `Read` still uses strict client-only `Parse`. `ContainerInventory.Parse` is a separate bounded generic model for optional metadata such as WindowsPE. It is never adapted silently into an executable installation edition. Native cleanup uncertainty is latched before report-writing can fail. ISO sources are held read-only; standalone WIM/ESD gets the same source lease and before/after hash. Exclusive output folders and directory leases contain CreateNew reports. No split-part query or embedded WinRE discovery is implied.

The inventory scope has no deletion journal. Cancellation retains diagnostics, offers Keep with Delete disabled and states that automatic cleanup is unavailable. Existing guarded ISO inspection/build cleanup remains separate. MountReadiness always denies execution; ScratchPreflight only assesses a new local NTFS destination and treats unknown expanded size as blocking. No privileged worker, global mount cleanup or ownership escalation is introduced.

Read-only workspace navigation can continue during jobs so Overview telemetry is visible, while action and selection controls stay locked. Telemetry is derived from real JobProgress byte counters and a Stopwatch; elapsed time includes pauses. Simulations are pure strings built from the current selection snapshot, explicitly nonexecuting and never accepted by BuildIso. The region preview has no engine effect.
