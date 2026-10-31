# Roadmap

Planned, not implemented. Order may change.

## USB writer (shipped in 1.0)
- Done: USB drive list with safety filters; MBR + FAT32, GPT + FAT32 and GPT + NTFS layouts (windows2usb's set, Microsoft boot files only); automatic `install.wim` split on FAT32; BIOS boot code; write-through copy; uncached read-back verify; optional exFAT DATA partition for space beyond the 32 GB FAT32 cap; backup-drive guard; typed confirmation.
- Decided against: UEFI:NTFS boot partition (decision 24).
- Remaining: real-hardware boot tests.

## Evidence (shipped in 1.0)
- Done in code: measured footprint, Hyper-V boot test, recipe porting. Remaining: real-hardware verification (docs/verification-checklist.md).

## Get Windows (shipped in 1.0)
- Done in code: Media Creation Tool catalog, hash-verified ESD download, ESD→ISO. Remaining: real-run verification; optional older releases.

Items marked ★ come from the October 2026 review of ~130 similar tools (ideas only; values are taken from Microsoft documentation).

## 1.1 — Integrate
- Done in code: driver folders (signed INFs for the edition's architecture) into install.wim, storage drivers also into Windows Setup; one failing package is skipped, not fatal ★; dism.exe/oscdimg Microsoft-signature check before building ★; `.sha256` next to the ISO ★; Recall-off and Paint-AI policies ★.
- ★ Secure Boot readiness — done in code: boot manager signer shown after building; optional 2023-signed boot manager from the ISO's own files. Remaining: signer shown for any ISO on the USB page; DBX revocation check. Never revoke 2011 certificates, set anti-rollback (SVN) or copy SkuSiPolicy.
- ★ Updates, language packs and FoD into install.wim in Microsoft's order — done in code. Remaining: WinRE (SSU + Safe OS DU) and boot.wim (SSU + LCU) servicing, Setup Dynamic Update onto the media, language packs for Setup/WinRE and lang.ini; a language pack queuing the updates it needs.
- WinRE driver sync.

## 1.2 — Unattended setup
- `autounattend.xml` builder — done in code: language/region, keyboard, time zone, EULA, ★ local account via LocalAccounts + HideOnlineAccountScreens (never BypassNRO), ★ Panther copy removed at first logon, interactive partitioning always.
- Remaining: ★ guarded automatic disk wipe (only when exactly one disk matches the user's criteria, otherwise Setup stops); optional `notautounattend.xml` file name; first-logon commands; boot test that works alongside a user answer file.
- ★ Edition consistency: ei.cfg and pinned image index; never insert product keys.
- ★ Answer-file driver paths (`PnpCustomizationsWinPE`) as a second route.

## 1.3 — Risk model v2
- ★ Serviceability level (Native / Default / Default+) linked to the options; deeper levels shown disabled with the reason. "Native operations only" filter.
- ★ Reach card per option: measured size, linked services/tasks/drivers, exact registry change.
- ★ Protect toggles (anti-cheat, recommended); Windows Update, Defender and recovery are always protected.
- ★ Tier definitions that name what breaks; bulk "Safe" / "Safe + Low" selection — bulk selection done (Advanced).
- ★ Paired revert — done in code: `undo.ps1` from values recorded before each change. Remaining: read-back check after writing (Applied / Already set / Failed).
- ★ Lint for tweaks known to break Store, Update or Defender.

## 1.4 — Workflow
- ★ Resume a build from its last checkpoint; mount-recovery wizard (remount or clean up leftover mounts).
- ★ Per-stage speed and time remaining, taskbar progress, completion notification.
- ★ One-click local diagnostics bundle (logs, plan, journal; user paths removed; nothing uploaded).
- ★ USB: name processes locking the drive; exclusion list; restore a stick to a normal drive; optional fake-capacity test.
- ★ Download: resume, find an already-downloaded file by hash, specific failure messages.
- ★ Recipe import shows a diff and risk summary; inventory export.

## Later
- Reusable projects (from [JLIVECD](https://github.com/neurobin/JLIVECD), ideas only): keep the extracted, unmodified ISO media in a cache keyed by the ISO's hash so "Start another" and later builds of the same ISO skip the copy step; optional named project folder that can be rebuilt with the same choices.
- wimlib engine option (faster export, solid compression).
- Native ISO writer (no ADK needed).
- WinRE servicing and boot/install/WinRE driver sync.
- Split (`.swm`) source media.
- Localization (★ external translation files).
- ★ Size budget (DVD / FAT32 / custom) tracked against the measured footprint.
- ★ Headless command line for automation.
- ★ Catalog items as separate data files, updatable without a new app version.
