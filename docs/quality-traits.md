# Quality traits

How WinISOPilot meets each target trait, and what's still open. Review this table when adding features.

| Trait | How it's met | Still open |
|---|---|---|
| Intuitive | Six-step wizard; one primary button per page; Back always available | Real-user testing |
| User-friendly | Plain-language wording; sensible defaults (single edition, MBR, verify on) | — |
| Easy to navigate | Step rail with check marks; any finished step can be revisited; tool switch at the top | — |
| Discoverable | Help button and F1 on every page; tooltips show shortcuts; Presets and Level menus beside the list | — |
| Guided | Guided layout by default; Next button names the next section; Review summarizes before Build | — |
| Self-explanatory | Every option has summary, details, risk, what could stop working, how to undo | — |
| Well-documented | In-app help, [user guide](user-guide.md), README, architecture with error codes | Screenshots in the guide (after real runs) |
| Context-aware | One-line tip per page; explainer/inspector follows the selected option; disabled buttons say why | — |
| Beginner-friendly | Beginner level hides moderate/high risk; first-run layout choice explains both | — |
| Reliable | Journal + leftover cleanup; failures release hives and sessions; hash-verified downloads; stable error codes | Real-hardware runs (AUDIT.md) |
| Responsive | All work on a background job with stage, progress and elapsed time; cancel between steps; free-space check cached | Measured performance budgets |
| Consistent | Shared footer (Back / primary), shared cards, badges and colors from `theme.rs`; "changes" for selected options, "choices" for saved files | — |
| Accessible | Keyboard shortcuts, accessible names on custom controls, text 75–200%, stronger contrast, reduce motion, non-color risk labels | Screen-reader walkthrough on Windows; system high-contrast detection |
| Forgiving | Undo/redo (200 steps), Clear section, discard and start again, nothing changes before Build, typed confirmation before erasing | — |
| Focused | One task per page; every page fits the window without scrollbars (two columns, paged lists); advanced detail behind Advanced layout and collapsible sections | — |
| Transparent | "What happens when you select Build", stage names, Details log, measured footprint, boot-test results | — |
| Customizable | Layout, level, text size, contrast, motion, tips, DISM and working folders | — |
| Maintainable | Pure logic in `wip-core` with tests; Windows work isolated in `wip-engine/src/win`; catalog in TOML; CI on every push | UI tests |
| Polished | Consistent wording reviewed; demo mode for visual checks | Ongoing |

When two traits conflict, the core task (a working, safe installer) wins, then Forgiving and Transparent.
