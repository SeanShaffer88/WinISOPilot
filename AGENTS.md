# WinISOPilot project instructions

The user explicitly approved third-party dependencies for WinISOPilot. This project-specific exception supersedes the earlier no-dependency requirement; it does not apply to other projects. Use justified pinned dependencies with reviewed interfaces, bundled licenses and clear user-visible roles. Do not auto-update or execute newly fetched code. Existing source-protection, cleanup, approval and release rules remain in force.

Fido is bundled only as the reviewed official link-generation provider. Its GPL source/license must accompany distribution; native WinISOPilot code handles ISO transfer, verification and cleanup. Do not add execution-policy bypasses or run Fido elevated.
