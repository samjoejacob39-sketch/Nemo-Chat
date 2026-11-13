# Nemo Design System v1.0.0

Nemo is a mobile-first messaging app with a **strict monochrome Liquid Glass**
interface. This document is the contract for every screen, reference and
native component. The native source of truth is
`apps/mobile/ui/theme.slint` plus `apps/mobile/ui/widgets.slint`. The HTML
source of truth is `design/nemo.tokens.css` plus the synchronized files in
`design/references/`.

---

## 1. Principles

1. **One material.** Every raised surface is the same translucent glass recipe:
   layered translucency, a 1px specular edge, an inset bottom reflection and a
   soft depth shadow.
2. **Monochrome only.** No hue is used for meaning. Online, danger, success and
   unread states are expressed through luminance, weight, fills and labels.
3. **Light first.** Light is the default theme. One shared appearance setting
   switches the whole application.
4. **Touch first.** Targets are at least 44px, primary controls 52–56px.
   Feedback is pressed-state, ripple and scale — never hover or cursor-follow.
5. **One rhythm.** All spacing is a multiple of 4px. Screen gutters are 20–24px.
6. **One type ramp.** No screen invents a size outside the ramp.
7. **One navigation model.** Chats / Find / Profile, and nothing else, in the
   floating root tab bar.

---

## 2. Colour tokens

| Token | Light | Dark | Use |
| --- | --- | --- | --- |
| `--nemo-bg` | `#f4f4f3` | `#0e0e0e` | Page base |
| `--nemo-bg-2` | `#ebebe9` | `#171717` | Page gradient partner |
| `--nemo-surface` | `rgba(255,255,255,.70)` | `rgba(40,40,40,.66)` | Standard glass card |
| `--nemo-surface-strong` | `#ffffff` | `#242424` | Sheets, focused fields |
| `--nemo-surface-soft` | `rgba(255,255,255,.48)` | `rgba(44,44,44,.46)` | Icon tiles, chips |
| `--nemo-glass-top` | `rgba(255,255,255,.80)` | `rgba(255,255,255,.09)` | Glass gradient start |
| `--nemo-glass-bottom` | `rgba(255,255,255,.46)` | `rgba(255,255,255,.035)` | Glass gradient end |
| `--nemo-glass-edge` | `rgba(255,255,255,.98)` | `rgba(255,255,255,.20)` | Specular edge |
| `--nemo-line` | `rgba(20,20,20,.09)` | `rgba(255,255,255,.09)` | Hairlines |
| `--nemo-line-strong` | `rgba(20,20,20,.15)` | `rgba(255,255,255,.15)` | Emphasized borders |
| `--nemo-text` | `#171717` | `#f2f2f0` | Primary text |
| `--nemo-text-2` | `#6f6f6c` | `#a8a8a4` | Secondary text |
| `--nemo-text-3` | `#9b9b97` | `#777774` | Tertiary / placeholder |
| `--nemo-primary` | `#171717` | `#f1f1ef` | Primary button / active tab |
| `--nemo-on-primary` | `#ffffff` | `#171717` | Text on primary |
| `--nemo-nav` | `rgba(250,250,249,.80)` | `rgba(26,26,26,.84)` | Bottom tab glass |
| `--nemo-track` | `rgba(0,0,0,.10)` | `rgba(255,255,255,.12)` | Toggle track |
| `--nemo-scrim` | `rgba(0,0,0,.30)` | `rgba(0,0,0,.58)` | Sheet / dialog scrim |

**Rule:** if a colour cannot be expressed with these tokens, it is not part of
the system. Never introduce blue, purple, lavender, green, pink, orange, red or
neon accents. Semantic states use `--nemo-primary` / `--nemo-text-2` and text
labels instead of hue.

Native equivalents live in `Theme.n-*` (see `theme.slint`). Existing screens
may still alias them through `ink`, `muted`, `line`, `glass`, `nav-bg`, etc.;
those aliases must resolve to `Theme.n-*`, not to local hex literals.

---

## 3. Typography

One family: the platform UI sans (Slint uses the system font stack).

| Role | Token | Size | Weight | Notes |
| --- | --- | --- | --- | --- |
| Display | `--nemo-display` | 32px | 760 | Rare hero numbers |
| Page title | `--nemo-title` / 31px native | 28–32px | 740–760 | One per root screen |
| Section | `--nemo-section` | 20–22px | 700–730 | Section headers |
| Card heading | `--nemo-card` | 16–18px | 700–740 | Names, titles |
| Body | `--nemo-body` | 14–16px | 500–600 | Paragraphs, fields |
| Secondary | `--nemo-secondary` | 12–14px | 500–650 | Supporting copy |
| Label | `--nemo-label` | 12–13px | 650–700 | Field and eyebrow labels |
| Caption | `--nemo-caption` | 10–12px | 600–700 | Metadata |
| Micro | `--nemo-micro` | 9–10px | 650–750 | Nav labels, footer |

Rules:

- Page titles are 31px on phone screens (28px minimum on compact onboarding).
- Section headings are 20px. Never use 15px for a section heading.
- Letter-spacing is negative for display sizes (`-0.5px` to `-1.7px`) and
  positive only for uppercase eyebrows (`0.6px` to `1.8px`).
- Use `overflow: elide` for single-line names and `wrap: word-wrap` for copy.

---

## 4. Spacing, radius and layout

- **4px scale:** 4, 8, 12, 16, 20, 24, 32, 40, 48.
- **Screen gutter:** 22px standard; 20–24px allowed. Never below 20px.
- **Card internal padding:** 14–16px.
- **Control height:** 52–56px (`--nemo-control-h: 54px`).
- **Touch target:** 44px minimum.
- **Radii:** cards 24px, rows 18px, controls/inputs 16px, sheets 28px,
  pills 999px, avatars circular.
- **Bottom nav:** 84px reserved; floating bar 64–66px high, 18px side inset.
- **Header:** 62px standard secondary header with a 42px back button.
- **Content bottom padding:** 32px on secondary screens, 120px on root tabs to
  clear the floating nav.

---

## 5. Glass material

Slint 1.18 has no backdrop blur. The native material is emulated:

```slint
background: @linear-gradient(160deg, Theme.n-glass-top 0%, Theme.n-glass-bottom 100%);
border-width: 1px;
border-color: Theme.is-dark ? Theme.n-line-strong : Theme.n-glass-edge;
drop-shadow-blur: 16px;
drop-shadow-color: rgba(0,0,0,0.055);
drop-shadow-offset-y: 6px;
```

Each card also draws:

1. a 1px specular top line inset by 1px;
2. a 1px inner bottom reflection at `height - 2px`;
3. a `clip: true` boundary so the highlight never leaks.

On the web references, the equivalent is `backdrop-filter: blur(24px)
saturate(115%)` plus the `.nemo-glass::after` sheen.

---

## 6. Components

Canonical Slint components (`widgets.slint`) — legacy names inherit from them:

| Component | Purpose |
| --- | --- |
| `NemoGlassCard` (`GlassCard`) | Standard glass surface |
| `NemoPrimaryButton` (`PrimaryButton`) | 54px filled primary action |
| `NemoGlassButton` (`GhostButton`) | Secondary glass action |
| `NemoTextButton` (`TextButton`) | Inline textual action |
| `NemoIconButton` (`IconButton`) | 44px icon target |
| `NemoBackButton` | 42px secondary-header back |
| `NemoInput` (`NookField`) | 54px labelled input with password reveal |
| `NemoSearchField` (`SearchField`) | 50px search input with clear |
| `NemoAvatar` (`Avatar`) | Monochrome gradient initials / image avatar |
| `NemoToggle` (`Toggle`) | 50×30 monochrome switch |
| `NemoBadge` (`CountBadge`) | Unread count |
| `NemoPillTag` (`PillTag`) | Small status tag |
| `NemoSectionLabel` (`SectionLabel`) | Uppercase section eyebrow |
| `NemoTopBar` (`TopBar`) | Secondary screen header |
| `NemoBottomNav` (`NavBar`) | Floating Chats / Find / Profile bar |
| `NemoBottomSheet` | Scrim + draggable glass sheet |
| `NemoDialog` | Centred confirmation dialog |
| `NemoStateView` (`EmptyState`) | loading / empty / error / offline / success |
| `NemoTypingBubble` (`TypingBubble`) | Three-dot typing indicator |

Rules:

- Use `NemoBackButton` for every secondary header. No custom `←` text buttons.
- Use `NavBar` only on Chats, Find and Profile. Onboarding and secondary
  screens never show authenticated navigation.
- Sheets contain a 38×4px grabber, a 17px title, 12px copy and 48px actions.
- Avatars are monochrome gradients in light and dark.

---

## 7. Navigation

- Root tabs: **Chats (0)**, **Find (1)**, **Profile (2)**.
- The nav is a single floating glass bar at the bottom of the root screens.
- Active tab: filled pill, inverse text/icon, 9px label.
- Badges appear on Chats (unread) and Find (requests).
- Secondary screens (`Notifications`, `Settings`, `Group Info`, `App Updates`,
  `Chat`, `Create Group`, onboarding policy screens) use a back header and no
  bottom nav.
- Android back must map to the same `back` callback as the visual back button.

---

## 8. States

Every data surface must handle these states with `NemoStateView`:

| State | Glyph | Copy pattern |
| --- | --- | --- |
| loading | `...` | "Loading…" + optional retry |
| empty | `◇` | What is empty + how to start |
| error | `!` | What failed + "Try again" action |
| offline | `x` | "You're offline" + "Retry" |
| success | `◇` | Confirmation, no action |

Disabled controls use `enabled: false` and 42% opacity. Destructive actions use
the monochrome `danger` token and a confirmation sheet — never red.

Applied in v1: Chats empty state, Notifications success state, Find
requests/friends/search empty states. Updates already renders checking,
download progress, ready, error and retry.

---

## 9. Motion

| Token | Duration | Use |
| --- | --- | --- |
| `--nemo-tap` | 140ms | Press / ripple |
| `--nemo-focus` | 180ms | Toggle, focus ring |
| `--nemo-tab` | 220ms | Tab selection |
| `--nemo-sheet` | 300ms | Sheet and dialog |
| `--nemo-screen` | 320ms | Screen transitions |

Easing: `cubic-bezier(0.22, 1, 0.36, 1)` for entrances and
`cubic-bezier(0.65, 0, 0.35, 1)` for symmetric moves. Respect
`prefers-reduced-motion` on the web and `Theme.reduced-motion` natively.

All interactive surfaces provide a pressed state: 6–12% ink wash plus an
80–120ms scale/ripple. No hover-only affordances.

---

## 10. Screen inventory (14)

Onboarding: Welcome, Sign Up, Recovery Codes, Login, Terms.
Root: Chats, Find, Profile.
Secondary: Chat Conversation, Notifications, Settings, Create Group,
Group Info, App Updates.

All 14 screens use the same tokens, glass recipe, radii, type ramp and motion.
The canonical order of a root screen is:

1. safe-area spacer (42px);
2. title block (eyebrow 9–10px + 31px title + optional subtitle);
3. actions row (42px glass targets);
4. search / filters;
5. scrollable content;
6. floating `NavBar`.

A secondary screen:

1. safe-area spacer;
2. 62px header: `NemoBackButton` + 15–16px centred title + optional actions;
3. scrollable content with 22px gutters and 32px bottom padding;
4. no bottom navigation.

---

## 11. HTML reference synchronization

The 15 reference files were audited on 2026-11-13:

- all are monochrome (no saturated hex values);
- all keep their own HTML, CSS, JS and light/dark toggle;
- every file includes the canonical token block from `nemo.tokens.css`;
- local palettes are compatibility aliases of the canonical tokens.

Workflow:

```powershell
# 1. Edit the canonical tokens
notepad design/nemo.tokens.css

# 2. Push the block into every self-contained reference
powershell -File design/sync-references.ps1

# 3. Re-run the audit
powershell -File design/audit-references.ps1
```

`sync-references.ps1` inserts or replaces the block between
`/* NEMO-TOKENS:START */` and `/* NEMO-TOKENS:END */` in every
`design/references/*.html`. It never touches a file's markup, scripts or
toggle. `audit-references.ps1` verifies the block exists, checks for saturated
colours and confirms each file contains a theme toggle.

---

## 12. Native ↔ reference parity checklist

- [x] Shared `Theme.n-*` tokens and 4px scale in `theme.slint`.
- [x] `Nemo*` component library with legacy aliases in `widgets.slint`.
- [x] All screen-local palettes resolve to `Theme.n-*`.
- [x] Chats / Find / Profile share one floating bottom nav.
- [x] Secondary screens use `NemoBackButton` and no bottom nav.
- [x] Onboarding has no authenticated bottom nav.
- [x] Colourful emoji replaced with monochrome vector icons.
- [x] Title sizes normalized to 31px (28px recovery).
- [x] Section headings normalized to 20px.
- [x] Empty / success states use `NemoStateView`.
- [x] HTML references carry the canonical token block.
- [ ] Device screenshot sign-off on the Android APK (release pipeline).
