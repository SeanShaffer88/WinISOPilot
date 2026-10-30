# Roadmap

Planned, not implemented. Order may change.

## 0.2 — USB writer (in progress)
- Done: USB drive list with safety filters; MBR + FAT32, GPT + FAT32 and GPT + NTFS layouts (windows2usb's set, Microsoft boot files only); automatic `install.wim` split on FAT32; BIOS boot code; write-through copy; uncached read-back verify; optional exFAT DATA partition for space beyond the 32 GB FAT32 cap; backup-drive guard; typed confirmation.
- Decided against: UEFI:NTFS boot partition (decision 24).
- Remaining: real-hardware boot tests.

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
