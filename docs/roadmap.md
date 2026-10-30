# Roadmap

Planned, not implemented. Order may change.

## 0.2 — USB writer (Rufus-style)
- Drive list: removable drives only by default; fixed drives behind an explicit expert switch.
- GPT/UEFI and MBR/BIOS layouts; FAT32 with automatic `install.wim` split (SWM), or NTFS with a UEFI:NTFS boot partition.
- Write the built ISO or any Windows ISO; verify after writing.
- Proportional confirmations: drive name, size and contents shown before erasing.

## 0.3 — Get Windows
- In-app download of official Windows 10/11 ISOs from Microsoft (release, edition, language, architecture), with hash verification.

## 0.4 — Integrate
- Drivers (INF folders) into install, boot and WinRE images.
- Updates (SSU/LCU/.NET CAB/MSU) with order and applicability checks.
- Windows language packs and Features on Demand.

## 0.5 — Unattended setup
- `autounattend.xml` builder: region, keyboard, local account, privacy pages, disk layout (never a default wipe), first-logon commands.

## Later
- wimlib engine option (faster export, solid compression).
- Native ISO writer (no ADK needed).
- WinRE servicing and boot/install/WinRE driver sync.
- Split (`.swm`) source media.
- Before/after footprint report from measured sizes.
- Localization.
