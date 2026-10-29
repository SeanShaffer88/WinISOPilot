# Crimson And Charcoal Theme

The WPF desktop app starts in dark mode. Its five actual guide stages now form a vertical left rail in the crimson sidebar. Advanced mode uses the same sidebar for its ten workspaces. Stage labels, navigation guards and supported operations remain unchanged; the approved mockup supplies the layout and color treatment.

| Surface | Dark | Light |
| --- | --- | --- |
| Page | `#1E1E22` | `#F1F4F8` |
| Card | `#25252B` | `#F8FAFC` |
| Field | `#2D2D33` | `#FBFCFE` |
| Sidebar And Guide | `#650F23` | `#F2E2E7` |
| Header And Footer | `#18181C` | `#E9E5E9` |
| Primary Action | `#A52240` | `#A52240` |
| Focus And Accent | `#EF7894` | `#92203B` |
| Text | `#F2F0F2` | `#17212E` |
| Secondary Text | `#B9B3BC` | `#42546B` |
| Control Boundary | `#85818B` | `#64748B` |
| Divider | `#3B3B43` | `#CDD5DF` |

Primary text is `#F2F0F2` in both themes. Legacy resource keys `BlueBrush` and `AccentGradient` now resolve to solid crimson. The lighter dark-mode accent provides a visible focus boundary against charcoal and the deep crimson sidebar. Text contrast is at least 4.5:1 and focus contrast at least 3:1 against the adjacent non-primary surfaces, verified by the desktop self-tests.

System high contrast replaces the complete palette with Windows system brushes. The Light / Dark action remains available. Native title bars use Windows DWM dark and caption colors without replacing their standard controls; unsupported attributes retain Windows defaults. High contrast restores system caption colors. Text fields and scrollbars use the shared theme. Native file pickers retain Windows styling. No dependency, network operation, ISO servicing behavior or source-protection rule changes.