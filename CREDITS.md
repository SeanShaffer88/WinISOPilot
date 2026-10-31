# Credits

WinISOPilot is written by Sean Shaffer and released under the MIT License.

## Libraries

| Crate | Use | License |
|---|---|---|
| [egui / eframe](https://github.com/emilk/egui) | User interface | MIT OR Apache-2.0 |
| [AccessKit](https://github.com/AccessKit/accesskit) (via eframe) | Screen-reader support | MIT OR Apache-2.0 |
| [rfd](https://github.com/PolyMeilex/rfd) | Native file dialogs | MIT |
| [serde](https://serde.rs), [serde_json](https://github.com/serde-rs/json) | Settings, recipes, journal | MIT OR Apache-2.0 |
| [toml](https://github.com/toml-rs/toml) | Option catalog format | MIT OR Apache-2.0 |
| [windows-sys](https://github.com/microsoft/windows-rs) | Win32 bindings | MIT OR Apache-2.0 |
| [winresource](https://github.com/BenjaminRi/winresource) | Icon, version info and manifest | MIT |

Full transitive dependency licenses can be listed with `cargo license` or `cargo about`.

## Windows tools used at run time (not bundled)

- DISM and the DISM API (`dismapi.dll`, `dism.exe`), part of Windows and the Windows ADK.
- `oscdimg.exe` from the Windows ADK Deployment Tools.
- `reg.exe` and Windows PowerShell's DISM module, part of Windows.

## References

- Microsoft Learn: [DISM API](https://learn.microsoft.com/windows-hardware/manufacture/desktop/dism/dism-api-functions), [DISM image management](https://learn.microsoft.com/windows-hardware/manufacture/desktop/dism-image-management-command-line-options-s14), [Oscdimg options](https://learn.microsoft.com/windows-hardware/manufacture/desktop/oscdimg-command-line-options), [WinSxS cleanup](https://learn.microsoft.com/windows-hardware/manufacture/desktop/clean-up-the-winsxs-folder), [Windows policy CSP documentation](https://learn.microsoft.com/windows/client-management/mdm/policy-configuration-service-provider).
- mingw-w64 `dismapi.h` (public domain), used to check DISM structure layouts.

## Assets

The icon and logo were made for this project and are covered by the project license. The app uses egui's built-in fonts.

## Ideas from other projects (no code used)

- [microsoft/secureboot_objects](https://github.com/microsoft/secureboot_objects) `Make2023BootableMedia.ps1`: the list of files to copy for the 2023-signed boot manager (reimplemented in Rust, no code copied).

- [windows2usb](https://github.com/ValdikSS/windows2usb) (Apache-2.0): USB layout modes and write-through progress.
- [JLIVECD](https://github.com/neurobin/JLIVECD) (GPL-2.0): project workflow and caching extracted media.
- October 2026 review of ~130 similar tools; ideas only, registry values taken from Microsoft Learn. Main sources: [NTLite](https://ntlite.com) (removal levels, reach cards, protect toggles), [tiny11builder](https://github.com/ntdevlabs/tiny11builder) (signed-tool check, `.sha256` sidecars, resume), [WinUtil](https://github.com/ChrisTitusTech/winutil) (driver failure tolerance), [W10UI](https://github.com/abbodi1406/BatUtil) (update order, ResetBase caution), [O&O ShutUp10++](https://www.oo-software.com/en/shutup10) (risk tiers), [Bloatynosy](https://github.com/builtbybel/Bloatynosy) (read-back checks), [Sophia Script](https://github.com/farag2/Sophia-Script-for-Windows) and [privacy.sexy](https://github.com/undergroundwires/privacy.sexy) (paired reverts), [Schneegans unattend generator](https://github.com/cschneegans/unattend-generator) (disk guard, answer-file accounts), [Rufus](https://github.com/pbatard/rufus) (Secure Boot readiness, locked-drive report), Macrium Reflect and AOMEI Backupper (progress and diagnostics).
