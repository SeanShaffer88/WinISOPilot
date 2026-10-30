# WinISOPilot user guide

WinISOPilot makes a new Windows 10 or 11 installer with your changes, and can put any Windows installer on a USB stick. Your original ISO is never modified.

Press **F1** on any page for help about that page.

## Before you start

- A Windows 10 or 11 PC (64-bit). WinISOPilot asks for administrator rights when it starts, because changing a Windows image needs them.
- Free disk space: the Choose ISO page shows how much is needed (roughly twice the ISO size plus about 25 GB).
- Recommended: the free [Windows ADK Deployment Tools](https://learn.microsoft.com/windows-hardware/get-started/adk-install). They provide the tool that writes ISO files. Without them you get a folder of setup files instead of an ISO.

## First launch

You choose a layout. You can switch later in **Settings**.

- **Guided**: one section at a time, with an explanation panel. Recommended for most people.
- **Advanced**: one dense table of everything in the image, with package names and technical details.

## Customize an ISO

The left panel shows the six steps. Finished steps have a check mark and can be revisited by selecting them.

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
- **Clear section** unticks everything in the current section.
- Mistakes are easy to fix: **Ctrl+Z** undoes, **Ctrl+Y** redoes.

Only options that apply to the opened edition are shown. Options the image already has are marked "Already set".

### 5. Review
Every change in one list. Remove any item with ✖. Risky and unrated changes need their **Confirm** box ticked. The **What happens when you select Build** box lists each step in plain words.

Choose compression (Standard is best for USB sticks), a disc label, and where to save the ISO. **Save choices…** keeps your list in a small file you can load later, even on a newer Windows release; WinISOPilot then shows what still matches and what changed.

### 6. Build
Each stage is shown with progress and elapsed time. **Cancel** stops after the current step and saves nothing to the image.

When it's done you see:
- **Measured result**: installed size, image size, and app/feature counts before and after.
- **Test it in a virtual machine**: installs your ISO in a temporary Hyper-V VM and checks from inside Windows that every change landed (needs Windows Pro/Enterprise/Education with Hyper-V).
- **Write it to a USB drive**.

## Write to USB

Choose **Write to USB** at the top of the left panel.

1. **What to write**: an ISO, or a folder of setup files.
2. **USB drive**: plug it in and select **Refresh**. Only USB drives of 8 GB or more are offered; internal disks and the drive holding your files never are.
3. **Options**: keep **MBR** unless you know you need GPT. Leave **Check every file** on.
4. Select **Write to USB** and type `ERASE` to confirm.

## Settings

Select **Settings** at the bottom of the left panel. Changes are saved straight away; close with **Done** or Esc.

- **Layout**: Guided or Advanced. Switching takes you back to step 1; your ISO, folders, opened edition, ticked changes and other entries are kept.
- **Options shown in Guided mode**: Beginner, Intermediate or Expert.
- **Text size** 75–200% (Ctrl + and Ctrl − also work).
- **Stronger contrast**, **Reduce motion**, **Show tips**.
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

- Errors show a code (for example `WIP-E031`) and a plain explanation. **Copy details** copies them; **Open log folder** shows the logs.
- The **Details** section under any task shows each step and command.
- Error codes are listed in [architecture.md](architecture.md#error-codes).
- If the build stops part-way, select **Discard and start again**; your choices are kept.
