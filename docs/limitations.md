# Known limitations (0.1.0)

- No real end-to-end run yet; see [AUDIT.md](../AUDIT.md).
- Windows 10/11 client images only. Server, WinPE, Windows 7/8.1 images are rejected.
- Split media (`install.swm`) isn't supported.
- ISO output needs `oscdimg.exe` from the Windows ADK; otherwise the result is a media folder.
- Servicing a newer Windows build with an older host DISM can fail; install the matching ADK and set its DISM folder in Settings.
- Only the chosen edition is customized; other kept editions are unchanged.
- If a build fails after the edition is saved (export or ISO stage), **Discard and start again** may report that nothing is mounted. Start over from the Edition step.
- Option effects depend on the Windows build. Microsoft changes Copilot, Recall, BypassNRO and OneDrive behavior between releases.
- Removing a provisioned app affects only accounts created after setup.
- The `apps.media_player` option targets the `Microsoft.ZuneMusic` package (Media Player on Windows 11, Groove Music on Windows 10).
- egui's glow renderer needs OpenGL 2+. Some virtual machines without GPU drivers can't open the window.
- No system high-contrast theme detection yet; text size is adjustable (75–200%).
- English only.
- Leftover-session detection ignores records made by the current process ID.
