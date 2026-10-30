# Roadmap

Planned, not implemented. Order may change.

## 0.2 — USB writer (in progress)
- Done: USB drive list with safety filters, MBR/GPT FAT32, automatic `install.wim` split, BIOS boot code, read-back verify, typed confirmation.
- Remaining: NTFS layout with a UEFI:NTFS boot partition; FAT32 beyond 32 GB without diskpart's limit; real-hardware boot tests.
- Planned (from [windows2usb](https://github.com/ValdikSS/windows2usb), ideas only): offer its layout set — MBR+FAT32 (default, BIOS+UEFI+Secure Boot, split wim), GPT+FAT32, GPT+NTFS (UEFI only, no split) — and keep Microsoft's own boot files so Secure Boot stays intact; write with write-through/unbuffered I/O so progress reflects data actually on the stick, not in the cache.

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
- Reusable projects (from [JLIVECD](https://github.com/neurobin/JLIVECD), ideas only): keep the extracted, unmodified ISO media in a cache keyed by the ISO's hash so "Start another" and later builds of the same ISO skip the copy step; optional named project folder that can be rebuilt with the same choices.
- wimlib engine option (faster export, solid compression).
- Native ISO writer (no ADK needed).
- WinRE servicing and boot/install/WinRE driver sync.
- Split (`.swm`) source media.
- Before/after footprint report from measured sizes.
- Localization.
