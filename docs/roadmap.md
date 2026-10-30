# Roadmap

Planned, not implemented. Order may change.

## 0.2 — USB writer (in progress)
- Done: USB drive list with safety filters, MBR/GPT FAT32, automatic `install.wim` split, BIOS boot code, read-back verify, typed confirmation.
- Remaining: NTFS layout with a UEFI:NTFS boot partition; FAT32 beyond 32 GB without diskpart's limit; real-hardware boot tests.

## 0.3 — Evidence (in progress)
- Done in code: measured footprint, Hyper-V boot test, recipe porting. Remaining: real-hardware verification (docs/verification-checklist.md).

## 0.4 — Get Windows (in progress)
- Done in code: Media Creation Tool catalog, hash-verified ESD download, ESD→ISO. Remaining: real-run verification; optional older releases.

## 0.5 — Integrate
- Drivers (INF folders) into install, boot and WinRE images.
- Updates (SSU/LCU/.NET CAB/MSU) with order and applicability checks.
- Windows language packs and Features on Demand.

## 0.6 — Unattended setup
- `autounattend.xml` builder: region, keyboard, local account, privacy pages, disk layout (never a default wipe), first-logon commands.

## Later
- wimlib engine option (faster export, solid compression).
- Native ISO writer (no ADK needed).
- WinRE servicing and boot/install/WinRE driver sync.
- Split (`.swm`) source media.
- Before/after footprint report from measured sizes.
- Localization.
