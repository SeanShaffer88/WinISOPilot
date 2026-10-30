# Verification checklist — footprint, boot test, recipe porting

Run on Windows 11 (elevated) unless noted. Record each result with the date, Windows build, ISO name and log path. "Expected" is what must happen; anything else is a bug.

## 1. Measured footprint

| # | Scenario | How | Expected |
|---|---|---|---|
| F1 | Normal build | Light cleanup preset, Standard compression | Result screen shows installed size, image file, apps, capabilities and features before/after; `footprint.md` and `footprint.json` in the workspace; apps list matches the preset's apps found in the image |
| F2 | Only settings changed | Select only Privacy options | Sizes nearly equal; "N registry and service settings changed" > 0; change lists empty |
| F3 | ESD output | Smallest (install.esd) compression | Image-file line notes "includes ESD recompression" |
| F4 | All editions kept | Untick "Keep only this edition" | Sizes refer to the customized edition (expanded) and the whole file (file size); no crash |
| F5 | After-inventory fails | Lock `Users\Default\NTUSER.DAT` in the mount (open in Registry Editor via Load Hive) just before build stage 1 ends | Build still completes; "footprint couldn't be measured" note; log has the reason |
| F6 | Cancel during apply | Cancel in stage 1 | No footprint files written; image still open; Discard works |
| F7 | Unit tests | `cargo test -p wip-core footprint` | Pass |

## 2. Boot test (Hyper-V)

| # | Scenario | How | Expected |
|---|---|---|---|
| B1 | Happy path, Pro | Build Windows 11 Pro with Light cleanup + Privacy; Run boot test | Within 15–40 min: "Installed and booted… Every change checked out"; VM and folder removed |
| B2 | Keep VM | Tick "Keep the VM" | Passes; VM remains in Hyper-V Manager; folder kept |
| B3 | A check fails | After build, before testing, edit nothing — instead pick an option known to be re-applied by Windows (e.g. a ContentDeliveryManager value) or temporarily add `Microsoft.WindowsCalculator` to the plan's checks via a debug build | "Not as expected" lists the failing check with its detail; VM kept |
| B4 | Hyper-V off | Disable Hyper-V feature, reboot, run | Error WIP-E100 with instructions; nothing created |
| B5 | Hyper-V installed, hypervisor not running | Disable virtualization in firmware | WIP-E101; nothing created |
| B6 | No oscdimg | Rename ADK `oscdimg.exe` (or set an empty DISM folder) and restart the app | Card shows "Needs an ISO file" (no ISO) or WIP-E102 |
| B7 | Home edition | Build Windows 11 Home | Warning about the product key page shown before running; if it times out, VM kept with WIP outcome "didn't finish within N min" |
| B8 | Timeout | Set "Give up after" to 20 min on a slow disk | VM turned off; outcome TimedOut; VM kept |
| B9 | Cancel | Cancel during stage 3 | VM turned off and removed with its folder; "Stopped" shown |
| B10 | Not enough space | Working folder on a drive with < 70 GB free | VM creation fails with WIP-E106 and the Hyper-V message; partial VM removed |
| B11 | Name clash | Start two tests quickly (second after the first finishes stage 2) | Second is refused while the first runs (one job at a time) |
| B12 | Results unreadable | During stage 3, delete `C:\WinISOPilot` inside the VM before shutdown | Outcome "VM stopped before Windows finished setting up"; VM kept |
| B13 | Unit tests | `cargo test -p wip-core boottest` | Pass (keys, base64, answer file, checks, scoring) |

## 3. Recipe porting

| # | Scenario | How | Expected |
|---|---|---|---|
| P1 | Same image | Save choices after a build; open the same ISO and edition; Load choices | "Recipe checked: every option does the same thing" toast; no dialog |
| P2 | Newer build | Save on 24H2, load on 25H2 (or vice versa) | Dialog lists options that changed (− / + items), options that don't apply, and new options; counts add up to the recipe size |
| P3 | Version-only change | Capability versions differ between builds | Reported as Same, not Changed |
| P4 | Remove non-applying | Press "Remove options that don't apply" | Those options leave the selection; Ctrl+Z restores |
| P5 | Add new options | Tick two new options, "Add ticked new options" | Both added; Ctrl+Z restores |
| P6 | Old recipe (schema 1) | Load a 0.1.0 recipe | Applicable options shown as "can't be compared in detail"; missing ones listed |
| P7 | Recipe loaded before an image is open | Load on Review with no edition open, then open an edition | Check runs when the edition opens |
| P8 | Future/corrupt recipe | Edit `schema` to 99; truncate the file | Clear error toast; selection unchanged |
| P9 | Unknown IDs | Add `"future.option"` to `options` | Dialog notes ignored entries |
| P10 | Unit tests | `cargo test -p wip-core recipe_port` | Pass |
