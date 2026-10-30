# Design tokens

Black/red product-family look, carried over from the crimson & charcoal theme. Source of truth: `crates/winisopilot/src/theme.rs`.

| Token | Hex | Use |
|---|---|---|
| PAGE | `#1E1E22` | Page background |
| CARD | `#25252B` | Cards, explainer/inspector panes |
| FIELD | `#2D2D33` | Text fields, inactive controls |
| CHROME | `#18181C` | Footer, Advanced category list |
| RAIL | `#650F23` | Step rail |
| RAIL_SELECTED | `#8A1A35` | Selected rail item |
| RAIL_MUTED | `#E8C4CD` | Upcoming steps on the rail |
| PRIMARY | `#A52240` | Primary buttons, checked state |
| ACCENT | `#EF7894` | Focus ring, current step, links |
| ROW_SELECTED | `#3A1520` | Focused list/table row |
| TEXT | `#F2F0F2` | Primary text |
| TEXT_BODY | `#D9D4DB` | Body text |
| TEXT_DIM | `#B9B3BC` | Secondary text |
| BOUNDARY | `#85818B` | Control borders |
| DIVIDER | `#3B3B43` | Dividers, card borders |

Risk badges always carry a text label (SAFE / LOW / MODERATE / HIGH / UNRATED); colors differ in lightness as well as hue.

| Risk | Background | Text |
|---|---|---|
| Safe | `#1F3D35` | `#9EE6CB` |
| Low | `#2E2A35` | `#D8CFE6` |
| Moderate | `#4A3A16` | `#F5D38A` |
| High / Unrated | `#4D1624` | `#FF9DB0` |

Layouts: Guided = mockup 2 (232 px rail, list, 340 px explainer); Advanced = mockup 5 (64 px icon rail, 210 px categories, table, 290 px inspector). Fixed window 1200×760 (not resizable). No scroll areas anywhere: use two columns (`pages::two_columns`), page long lists (`pager.rs`), and call `theme::fill_next` before a column's last card so it stretches to the bottom (cards start with `theme::card_body`). Spacing: page margin 20/14, card padding 12, item spacing 8×6, controls 26 px high, buttons 34 px.

Deviation from the InstallScope defaults: none recorded.
