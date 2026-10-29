# Architecture and verification boundaries

## Present prototype

The Windows GUI and CLI expose inspection and plan review only. The GUI guides users through source selection, real edition checks, six broad goals, a reference-family catalog and nonexecuting export. Native installation-media querying remains independent of any future privileged servicing worker. Reports distinguish observations from planned actions and unknowns. Selected media is not executed.

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

This repository does not currently implement the future servicing stages above.

## Low-level components and future boundaries

The present C# implementation already uses low-level Windows facilities where they serve the read-only task: `NativeIso.AttachOrReuse` and `VerifyAssociation` manage and recheck ISO access; `WimMetadata.Read` performs query-only native metadata calls; `JobControl` coordinates cooperative checkpoints. The engine bounds captured output, verifies source hashes and latches unresolved cleanup failures. Generated-file cleanup uses validated ownership and identity instead of treating a folder name as deletion authority. These are implemented safeguards, not an image-customization engine.

Future servicing should introduce three independent contracts, **none implemented yet**: an `IImageWorker.ExecuteAsync(ApprovedImageOperation, JobContext)` boundary for a separate privileged worker that independently validates an allowlist of typed operations and accepts no raw commands; an `IResourceJournal.Record(ResourceEvent)` / `Reconcile(JobId)` boundary for observed app-owned resources without assumed cleanup authority; and an `IImageVerifier.VerifyAsync(ExpectedImageState, ObservedArtifact)` boundary for independent checks reported as passed, failed or not run. A successful tool exit is never proof that media boots or Windows installs.

Prefer Microsoft APIs and the standard library. There is no plan to invent ISO/WIM parsers or cryptography or to rewrite the application in Rust. A future isolated native component would require a demonstrated need, a reviewed interface and its own evidence and tests before changing that decision.
