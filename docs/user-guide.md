# WinISOPilot user guide

WinISOPilot makes a new Windows 10 or 11 installer with your changes, and can put any Windows installer on a USB stick. Your original ISO is never modified.

Press **F1** on any page for help about that page.

## Before you start

- A Windows 10 or 11 PC (64-bit). WinISOPilot asks for administrator rights when it starts, because changing a Windows image needs them.
- Free disk space: the Choose ISO page shows how much is needed (roughly twice the ISO size plus about 25 GB).
- Recommended: the free [Windows ADK Deployment Tools](https://learn.microsoft.com/windows-hardware/get-started/adk-install). They provide the tool that writes ISO files. Without them you get a folder of setup files instead of an ISO.

## First launch

Each time WinISOPilot starts it asks which layout to use (your last choice is pre-selected). Tick **Don't ask again** to skip the question; turn it back on in **Settings** with **Ask Guided or Advanced at every launch**. Switch any time with **Guided / Advanced** at the top of the left panel, under **ISO / USB**.

- **Guided** (for everyone): plain words and as few choices as possible. WinISOPilot picks safe defaults for you: where temporary files go (your Downloads folder, or the ISO's folder), one edition per ISO, standard compression, where the new ISO is saved, MBR for USB sticks. Riskier options stay hidden until you select **Show them**.
- **Advanced** (for experts): everything is visible and editable: host diagnostics (DISM/oscdimg paths and versions), typed or pasted ISO and workspace paths, image index metadata, the full catalog at Expert level plus unrated items, every option's DISM/registry actions, the compiled **operation plan** (copyable), compression, volume label, recipes, GPT/MBR, USB volume label, boot-test timeout, and the log open by default.

| | Guided | Advanced |
|---|---|---|
| Welcome | Two big choices: make a cleaner Windows, or just make a USB stick; plain "before you start" fixes | Host diagnostics table and keyboard reference |
| Choose ISO | "Choose the ISO file…" and "Download Windows for me" (language only) | Editable paths, architecture and package, file name and hash |
| Edition | Names only, Pro marked as the safe pick | Index, EditionID, type, build, architecture, size, languages; single-index toggle |
| Customize | Quick picks, search, plain explanations; riskier options hidden | Dense table, risk filter, unrated items, technical actions in the inspector |
| Review | Changes, "what happens", where the ISO is saved | Operation plan, compression, volume label, save/load recipe |
| Results | Plain summary; optional "Test it" | Boot-test options (keep VM, timeout); log open |
| USB | Three numbered questions | Folder source, GPT/MBR, volume label |

## Customize an ISO

The slim left panel has the **ISO / USB** and **Guided / Advanced** switches and the six numbered steps (hover a number to see its name). Finished steps have a check mark and can be revisited by selecting them.

### 1. Welcome
Shows whether this PC has what's needed. Anything missing has a link that explains how to get it. If WinISOPilot was closed while working on an image, this page offers to clean it up.

### 2. Choose ISO
- **Browse** for an ISO you already have, or
- **Download Windows from Microsoft**: pick Windows 11 or 10, select **Load list**, choose language, architecture and package, then **Download and make ISO**. The download is checked against Microsoft's published hash. An interrupted download continues where it stopped.
- Choose a **working folder** on a fast local drive. The free and needed space are shown.

Select **Read ISO** to see the editions inside. Nothing is changed.

### 3. Edition
Pick the edition you'll install (Home, Pro…). Keeping only that edition makes the ISO smaller. Select **Open edition**; WinISOPilot copies the files and reads exactly what this edition contains (a few minutes).

### 4. Customize
- Tick what to change. Anything left unticked stays as Microsoft ships it.
- Long lists are split into pages: use **Previous page** / **Next page**, PageUp / PageDown, or scroll the mouse wheel over the list.
- Select an option's name to read what it does, **what could stop working**, and **how to undo it later**.
- Risk badges: **Safe**, **Low**, **Moderate**, **High**. Beginner level shows only Safe and Low; use the **Level** menu to see more.
- **Presets** add a common set (Light cleanup, Privacy, Gaming).
- **Untick all here** unticks everything on the current page.
- Advanced: **Select tier…** ticks every available Safe (or Safe + Low) option in the current category.
- Mistakes are easy to fix: **Ctrl+Z** undoes, **Ctrl+Y** redoes.

Only options that apply to the opened edition are shown. Options the image already has are marked "Already set".

**Drivers (optional)** comes after the last section (Guided) or under **Drivers** in the category list (Advanced). Most PCs don't need it. Use it when Windows Setup can't find your disk (common on laptops with Intel RST or VMD) or Wi-Fi doesn't work right after installing:
1. Download the driver from your PC maker's support page and unzip it. Installers (`.exe`) can't be added.
2. **Add driver folder…** and pick the folder. Sub-folders are searched.
3. WinISOPilot adds the signed drivers made for this edition (x64, ARM64, …) and skips the rest; hover the summary (Guided) or the STATUS column (Advanced) to see why. Unsigned drivers are never forced in.
4. **Also give Windows Setup the disk drivers** (on by default) adds storage-controller drivers to Windows Setup too.

Driver folders aren't saved in recipes, because folder paths differ between PCs.

### 5. Review
Every change in one list. Remove any item with ✖. Risky and unrated changes need their **Confirm** box ticked. The **What happens when you select Build** box lists each step in plain words.

Choose compression (Standard is best for USB sticks), a disc label, and where to save the ISO. **Use the 2023-signed boot manager** (off by default) switches the media to Microsoft's newer boot manager from the ISO's own files: tick it if the target PC has applied Microsoft's boot manager revocation (it then refuses 2011-signed media with Secure Boot on). PCs whose firmware doesn't trust the 2023 certificate need Secure Boot off to start such media. **Save choices…** keeps your list in a small file you can load later, even on a newer Windows release; WinISOPilot then shows what still matches and what changed.

### 6. Build
Each stage is shown with progress and elapsed time. **Cancel** stops after the current step and saves nothing to the image.

If one driver can't be added, the build carries on and lists it as a warning. Before building, WinISOPilot checks that DISM and the ISO tool are signed by Microsoft.

When it's done you see:
- **The ISO**, with **Copy SHA-256** (the same hash is saved next to the ISO as `name.iso.sha256`).
- **Measured result**: installed size, image size, and app/feature counts before and after.
- **Test it in a virtual machine**: installs your ISO in a temporary Hyper-V VM and checks from inside Windows that every change landed (needs Windows Pro/Enterprise/Education with Hyper-V).
- **Write it to a USB drive**.

## Missing tool: Windows ADK Deployment Tools

Making an ISO file (and downloading Windows) needs Microsoft's free Deployment Tools. Select **Install it for me**: WinISOPilot downloads Microsoft's installer, checks it's signed by Microsoft, and installs only the Deployment Tools (about 100 MB, a few minutes). If you install it yourself, select **I installed it, check again**.

## Write to USB

Choose **USB** at the top of the left panel.

1. **What to write**: an ISO, or a folder of setup files.
2. **USB drive**: plug it in and select **Refresh**. Only USB drives of 8 GB or more are offered; internal disks and the drive holding your files never are. In Guided, drives over 300 GB are treated as backup drives and not offered; Advanced shows them with a warning.
3. **Options**: Guided always uses MBR + FAT32, which boots on almost every PC. Advanced also offers GPT + FAT32 (UEFI only) and GPT + NTFS (UEFI only, no file splitting, but only PCs whose firmware reads NTFS can boot it). Leave **Check every file** on: it reads each file back from the stick itself, not from Windows' cache.
   - On sticks larger than about 33 GB with FAT32, the setup partition stops at 32 GB. Tick **Use the remaining … as a second drive** to turn the rest into an exFAT drive named DATA for your own files.
4. Select **Write to USB** and type `ERASE` to confirm.


**Restore to a normal drive…** (under the drive list) erases the chosen stick and turns it back into one ordinary exFAT drive for files. It asks for `ERASE` like writing does.
## Settings

Select **Settings** at the bottom of the left panel. Changes are saved straight away; close with **Done** or Esc.

- **Layout**: whether to ask Guided or Advanced at every launch. (Switching itself is in the left panel; it takes you back to step 1 and keeps everything you entered.)
- **Options shown in Guided mode**: Beginner, Intermediate or Expert.
- **Scale** 75–200%: the whole window and everything in it grows or shrinks (Ctrl +, Ctrl − and Ctrl 0 also work). Capped so the window still fits the screen.
- **Stronger contrast**, **Reduce motion**, **Show tips**, **Ask Guided or Advanced at every launch**.
- **Windows ADK DISM folder**: use the ADK's DISM when customizing a newer Windows than this PC runs.
- **Default working folder**.

## Keyboard

| Keys | Action |
|---|---|
| F1 | Help for this page |
| Alt+Right / Alt+Left | Next / Back |
| Ctrl+Z / Ctrl+Y | Undo / Redo |
| Ctrl+F | Search options |
| Tab / Shift+Tab | Move between controls |
| Space / Enter | Tick a box / press a button |
| PageUp / PageDown | Previous / next page of a list (pointer over the list) |
| Esc | Close Settings |

## When something goes wrong

**Save diagnostics…** (in the error box, or in Settings) writes one text file with the app version, PC facts, settings, the plan and recent logs. Your account name, PC name and profile path are replaced, and nothing is sent anywhere; read it before sharing it.

- Errors show a code (for example `WIP-E031`) and a plain explanation. **Copy details** copies them; **Open log folder** shows the logs.
- The **Details** section under any task shows each step and command.
- Error codes are listed in [architecture.md](architecture.md#error-codes).
- If the build stops part-way, select **Discard and start again**; your choices are kept.
