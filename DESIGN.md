---
name: KeelDesk
description: Windows 11 native MCP runtime spine and monochrome developer workbench design system
colors:
  keel-indigo: "#7c66ff"
  keel-indigo-hover: "#8e7bff"
  primary-solid: "#634ce8"
  primary-solid-hover: "#705af0"
  primary-ink: "#a99bff"
  surface-sunken: "#0b0b0f"
  surface-canvas: "#111116"
  surface-subtle: "#14141b"
  surface-panel: "#16161d"
  surface-overlay: "#1c1c25"
  surface-hover: "#20202b"
  surface-active: "#262633"
  ink-primary: "#f3f3f7"
  ink-secondary: "#b8b8c7"
  ink-muted: "#8c8ca1"
  border-subtle: "#ffffff0f"
  border-default: "#ffffff17"
  border-strong: "#ffffff29"
  status-ok: "#2ec47d"
  status-warn: "#f5a524"
  status-danger: "#f2555a"
  status-info: "#38bdf8"
typography:
  display:
    fontFamily: "Segoe UI Variable Text, Segoe UI Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "1.125rem"
    fontWeight: 600
    lineHeight: 1.333
    letterSpacing: "-0.015em"
  headline:
    fontFamily: "Segoe UI Variable Text, Segoe UI Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "0.9375rem"
    fontWeight: 600
    lineHeight: 1.467
    letterSpacing: "-0.01em"
  body:
    fontFamily: "Segoe UI Variable Text, Segoe UI Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "0.8125rem"
    fontWeight: 400
    lineHeight: 1.538
    letterSpacing: "0em"
  label:
    fontFamily: "Segoe UI Variable Text, Segoe UI Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "0.75rem"
    fontWeight: 500
    lineHeight: 1.333
    letterSpacing: "0em"
  micro:
    fontFamily: "Cascadia Code, Cascadia Mono, Consolas, ui-monospace, monospace"
    fontSize: "0.6875rem"
    fontWeight: 500
    lineHeight: 1.455
    letterSpacing: "0.01em"
  code:
    fontFamily: "Cascadia Code, Cascadia Mono, Consolas, ui-monospace, monospace"
    fontSize: "0.75rem"
    fontWeight: 400
    lineHeight: 1.667
    letterSpacing: "0em"
rounded:
  none: "0px"
  xs: "4px"
  sm: "6px"
  md: "8px"
  lg: "10px"
  xl: "12px"
  full: "9999px"
spacing:
  2xs: "4px"
  xs: "6px"
  sm: "8px"
  md: "12px"
  lg: "16px"
  xl: "20px"
  2xl: "24px"
components:
  button-primary:
    backgroundColor: "{colors.primary-solid}"
    textColor: "{colors.ink-primary}"
    typography: "{typography.body}"
    rounded: "{rounded.sm}"
    height: "32px"
    padding: "0px 12px"
  button-primary-hover:
    backgroundColor: "{colors.primary-solid-hover}"
  button-outline:
    backgroundColor: "{colors.surface-panel}"
    textColor: "{colors.ink-primary}"
    typography: "{typography.body}"
    rounded: "{rounded.sm}"
    height: "32px"
    padding: "0px 12px"
  input-default:
    backgroundColor: "{colors.surface-sunken}"
    textColor: "{colors.ink-primary}"
    typography: "{typography.body}"
    rounded: "{rounded.sm}"
    height: "32px"
    padding: "0px 10px"
  card-panel:
    backgroundColor: "{colors.surface-panel}"
    textColor: "{colors.ink-primary}"
    rounded: "{rounded.lg}"
    padding: "12px"
  code-well:
    backgroundColor: "{colors.surface-sunken}"
    textColor: "{colors.ink-primary}"
    typography: "{typography.code}"
    rounded: "{rounded.md}"
    padding: "10px 12px"
  badge-technical:
    backgroundColor: "{colors.surface-hover}"
    textColor: "{colors.ink-secondary}"
    typography: "{typography.micro}"
    rounded: "{rounded.xs}"
    padding: "2px 6px"
---

# Design System: KeelDesk

## 1. Overview

**Creative North Star: "The Graphite Spine"**

KeelDesk is a Windows 11-native MCP runtime daemon and developer environment workbench engineered with the austere, high-density clarity of Cursor and Vercel. Every screen operates inside a viewport-locked `100dvh` desktop frame (`overflow: clip`) fused seamlessly to the native Win32 DWM dark caption bar. Rather than competing for attention with decorative gradients, heavy drop shadows, or spacious marketing cards, the interface recedes into crisp graphite-black surfaces, `1px` translucent hairlines, and aligned monospace telemetry—surfacing visual urgency only when a `ConfirmRequired` safety gate demands human judgment.

Structural consistency across **typography, spacing, and concentric corner radii** is the backbone of the system. Every text node maps to one of six fixed desktop type roles; every margin, gap, and cell gutter locks onto a `4px` base grid aligned to a unified `12px` panel plumb line; and every nested container steps down concentrically in `2px` radius increments (`12px -> 10px -> 8px -> 6px -> 4px`). The system explicitly rejects gradient text, decorative glassmorphism, colored side-stripe borders, bubbly pill badges, page-level scrollbars, and light-mode token drift.

**Key Characteristics:**
- **Dark-Only Win11 DWM Fusion:** Single-track graphite dark palette locked pixel-for-pixel to Win32 `DWMWA_CAPTION_COLOR` (`#111116`), paired with Windows High Contrast (`forced-colors`) fallbacks.
- **Unified Six-Role Desktop Type Scale:** Fixed `rem` steps (`18 / 15 / 13 / 12 / 11px` + `12px` mono code) tuned for `100%–200%` Windows 11 desktop scaling with `tabular-nums` discipline.
- **12px Vertical Plumb-Line Rhythm:** `4px` base grid with unified `12px` (`spacing.md`) panel chrome padding so headers, filter bars, table columns, and footers share one unbroken vertical axis.
- **Concentric 2px Corner Geometry:** Strict five-tier corner hierarchy from `12px` modals down to `4px` technical chips.
- **State-Only Chromatic Signal:** 95% monochrome graphite and Crisp White/Slate ink; Keel Indigo (`≤5%`) marks primary focus, while semantic status hues appear exclusively on live telemetry states.

## 2. Colors

An ultra-restrained graphite-black monochrome ramp anchored by a single electric **Keel Indigo** primary accent and four high-legibility semantic telemetry signals.

### Primary
- **Keel Indigo** (`{colors.keel-indigo}`, `oklch(0.635 0.195 286)`): Reserved strictly for primary confirmation actions, active line-tab indicators, text selection tints, and `:focus-visible` keyboard rings.
- **Keel Indigo Lifted** (`{colors.keel-indigo-hover}`): Hover state for primary buttons only.

### Secondary
- **Signal Emerald** (`{colors.status-ok}`, `oklch(0.73 0.165 154)`): Running MCP listener dot, verified `keel_ping` round-trip metrics, and installed toolchain indicators.
- **Amber Gate** (`{colors.status-warn}`, `oklch(0.79 0.155 76)`): Server starting transitions, session-allowlist bypass notices, and approval countdown warnings.
- **Coral Intercept** (`{colors.status-danger}`, `oklch(0.66 0.2 23)`): `ConfirmRequired` high-risk tool badges, approval denial actions, bind errors, and urgent countdowns (`≤15s`).
- **Loopback Cyan** (`{colors.status-info}`, `oklch(0.75 0.135 232)`): Informational dry-run drill banners and protocol inspection hints.

### Neutral
- **Abyss Sunken** (`{colors.surface-sunken}`): Lowest luminance layer used for CLI snippet wells, JSON argument inspectors, text inputs, and `Kbd` keycaps.
- **DWM Canvas** (`{colors.surface-canvas}`): Root window background matching Win32 `0x00161111` so the native titlebar and WebView2 canvas read as a single slab.
- **Graphite Subtle** (`{colors.surface-subtle}`): Sticky table headers, secondary toolbar strips, and `CardFooter` status bars.
- **Graphite Panel** (`{colors.surface-panel}`): Standard resting surface for primary workbench cards.
- **Elevated Overlay** (`{colors.surface-overlay}`): Floating modals (`AlertDialog`, `Dialog`), side drawers (`Sheet`), dropdown menus, and tooltips.
- **Interactive Hover & Active** (`{colors.surface-hover}` / `{colors.surface-active}`): Row hover highlights, secondary buttons, and pressed control states.
- **Crisp Ink, Slate Secondary & Muted Steel** (`{colors.ink-primary}` / `{colors.ink-secondary}` / `{colors.ink-muted}`): Three-tier text hierarchy hitting `16.6:1`, `9.2:1`, and `5.4:1` contrast against panel surfaces—surpassing WCAG AA `4.5:1` even on `11px`–`12px` labels and placeholders.
- **Hairline Borders** (`{colors.border-subtle}` / `{colors.border-default}` / `{colors.border-strong}`): White-alpha `1px` strokes (`6%`, `9%`, `16%`) that adapt cleanly across dark graphite layers.

### Named Rules
**The DWM Pixel-Lock Rule.** `surface-canvas` must remain `#111116` at all times to match `DWMWA_CAPTION_COLOR` (`0x00161111`) in `apps/env-workbench/src-tauri/src/win11.rs`. Changing one without the other breaks the native window illusion and is forbidden.

**The Monochrome 95% Rule.** At rest, at least 95% of the viewport must remain monochrome graphite and neutral ink. Keel Indigo is capped at ≤5% of screen area; never tint entire card backgrounds or decorative headers with brand color.

## 3. Typography

**Display & UI Font:** Segoe UI Variable Text (with Segoe UI Variable, Segoe UI, system-ui, sans-serif fallback)
**Label & Mono Font:** Cascadia Code (with Cascadia Mono, Consolas, ui-monospace, monospace fallback)

**Character:** Authentic Windows 11 system clarity paired with crisp IDE-grade monospace precision. One humanist-geometric variable sans carries all natural-language UI at high desktop density, while Cascadia Code anchors every technical identifier, metric, port, and protocol payload.

### Hierarchy
- **Display / Window Brand (`type-title-lg`)** (`600`, `1.125rem` / `18px`, `line-height: 24px`, `-0.015em`): Top-bar application identity and primary modal titles (`AlertDialogTitle`).
- **Headline / Panel Title (`type-title`)** (`600`, `0.9375rem` / `15px`, `line-height: 22px`, `-0.01em`): Workbench card titles (`CardTitle`) and drawer section headers.
- **Body / Desktop Base (`type-body`)** (`400` / `500`, `0.8125rem` / `13px`, `line-height: 20px`, `0em`): Canonical desktop UI text size for buttons, inputs, primary table cells, alert copy, and descriptions (prose capped at `68ch`).
- **Label / Caption (`type-caption`)** (`400` / `500`, `0.75rem` / `12px`, `line-height: 16px`, `0em`): Card subtitles (`CardDescription`), sticky table column headers (`TableHead`), form labels, tooltips, and footer status bars.
- **Micro / Technical Tag (`type-micro`)** (`500`, `0.6875rem` / `11px`, `line-height: 16px`, `0.01em`): Monospace badges, protocol/source tags, `Kbd` shortcut caps, and call sequence `#id` counters.
- **Code / Payload (`type-code`)** (`400`, `0.75rem` / `12px`, `line-height: 20px`, `0em`): Multi-line CLI configuration blocks and pretty-printed JSON-RPC / tool argument payloads inside `CodeWell`.

### Named Rules
**The Six-Step Type Floor Rule.** Every text element must use one of the six semantic tokens (`text-title-lg`, `text-title`, `text-body`, `text-caption`, `text-micro`, `text-code`). Arbitrary bracketed pixel utilities (`text-[10px]`, `text-[11px]`, `text-[0.8rem]`) and fluid web `clamp()` scales are strictly prohibited; `11px` (`0.6875rem`) is the absolute legibility floor.

**The Tabular Telemetry Rule.** Every port (`:19527`), PID, latency (`2 ms`), counter, timestamp (`14:08:32`), and approval countdown (`58s`) must render in `--font-mono` with `font-variant-numeric: tabular-nums` so live updates never cause horizontal layout jitter.

## 4. Elevation

KeelDesk uses **flat tonal layering + `1px` crisp hairlines** at rest, reserving ambient shadows strictly for transient floating overlays that detach from the document plane. Depth moves from sunken `#0b0b0f` code wells, up to `#111116` DWM canvas, up to `#16161d` workbench panels bordered by `1px solid rgba(255,255,255,0.09)`, and finally to `#1c1c25` floating overlays.

### Shadow Vocabulary
- **Resting Panel** (`box-shadow: none`, `1px` hairline `{colors.border-default}`): All cards, tables, sunken code wells, and toolbars on the main workbench plane.
- **Floating Popover** (`box-shadow: 0 8px 24px -4px rgba(0, 0, 0, 0.55), 0 0 0 1px rgba(255, 255, 255, 0.12)`): Dropdown menus, select popovers, and keyboard tooltips (`--z-dropdown: 30` / `--z-tooltip: 70`).
- **Modal & Drawer Gate** (`box-shadow: 0 24px 64px -12px rgba(0, 0, 0, 0.75), 0 0 0 1px rgba(255, 255, 255, 0.15)`): `ToolApprovalDialog`, `Dialog`, and `Sheet` drawers over a `rgba(0, 0, 0, 0.68)` dim backdrop (`--z-drawer: 40` / `--z-modal: 50`).

### Named Rules
**The Hairline-Over-Shadow Rule.** Resting workbench containers never cast drop shadows and never nest cards inside cards. Inner grouping inside a panel uses sunken `#0b0b0f` wells (`CodeWell`) or `1px` `{colors.border-subtle}` dividers.

**The Semantic Z-Index Rule.** Stacking contexts use exclusively the semantic scale (`sticky: 10`, `dropdown: 30`, `drawer: 40`, `modal: 50`, `toast: 60`, `tooltip: 70`). Arbitrary values like `z-[999]` are forbidden.

## 5. Components

Components share a unified control-height ladder (`xs: 24px`, `sm: 28px`, `md: 32px`, `lg: 36px`), crisp `150ms` `ease-out-quart` state transitions, and concentric corner radii (`12px -> 10px -> 8px -> 6px -> 4px`).

### Buttons
- **Shape & Sizing:** Gently crisp corners (`6px` / `rounded.sm`) across four strict heights: `xs` (`24px` inline table actions), `sm` (`28px` toolbar actions), `default` (`32px` primary actions), and `lg` (`36px` modal confirmation gates). Typography uses `type-body` (`13px`, weight `500`), stepping to `type-caption` (`12px`) on `xs`.
- **Primary:** Solid `{colors.keel-indigo}` fill with `{colors.ink-primary}` text; lifts to `{colors.keel-indigo-hover}` on hover.
- **Outline & Secondary:** `{colors.surface-panel}` or `{colors.surface-hover}` with `1px` `{colors.border-default}` stroke, shifting to `{colors.surface-active}` and `{colors.border-strong}` on hover.
- **Destructive:** Tinted coral surface (`rgba(242, 85, 90, 0.14)`) with `{colors.status-danger}` text and `rgba(242, 85, 90, 0.32)` border for explicit rejection/termination actions.
- **Focus:** `2px` crisp ring in `{colors.keel-indigo}` at `50%` opacity with zero layout shift.

### Chips
- **Style:** Structured technical badges (`4px` / `rounded.xs`, never bubbly `9999px` text pills) set in `type-micro` (`11px` `Cascadia Code`, `tabular-nums`, `2px 6px` padding) with a `1px` subtle border.
- **State:** Neutral `MetricPill` (`{colors.surface-hover}` + `{colors.ink-secondary}`), semantic `StatusBadge` (leading `6px` circular dot + subtle hue tint), and `SafetyTierBadge` (`Safe` neutral outline vs `ConfirmRequired` coral tint with keyboard-focusable tooltip).

### Cards / Containers
- **Corner Style:** `10px` (`rounded.lg`) on workbench panels; `12px` (`rounded.xl`) on floating modals; `8px` (`rounded.md`) on nested `CodeWell` and `Alert` blocks.
- **Background & Border:** `{colors.surface-panel}` (`#16161d`) with `1px` `{colors.border-default}` hairline and zero resting shadow; `CardFooter` steps to `{colors.surface-subtle}` (`#14141b`) separated by `1px` `{colors.border-subtle}`.
- **Internal Padding & Plumb Line:** Standardized `12px` (`spacing.md`) horizontal gutter across `CardHeader`, filter toolbars, `Table` outer columns, and `CardFooter`, stepping to `20px` (`spacing.xl`) inside `Dialog` and `AlertDialog`.

### Inputs / Fields
- **Style:** Sunken `{colors.surface-sunken}` (`#0b0b0f`) fill with `1px` `{colors.border-default}` stroke, `6px` (`rounded.sm`) corners, `32px` (`md`) or `28px` (`sm`) height, and `{colors.ink-muted}` placeholders (≥5.4:1 contrast).
- **Focus & Error:** Border shifts to `{colors.keel-indigo}` with a `2px` focus ring; `aria-invalid="true"` shifts stroke and ring to `{colors.status-danger}`.

### Navigation
- **Line Tabs:** Horizontal `TabsList` (`variant="line"`) set in `type-caption` (`12px`, weight `500`), separated by a bottom `1px` `{colors.border-subtle}` divider. Active tab uses `{colors.ink-primary}` and a `2px solid` `{colors.keel-indigo}` `border-block-end` indicator that survives Windows `forced-colors` mode.

### Signature Components (`CodeWell` & `ToolApprovalDialog`)
- **`CodeWell`:** Sunken `{colors.surface-sunken}` (`#0b0b0f`) container (`8px` radius, `1px` `{colors.border-subtle}` border, `type-code` `12px/20px` `Cascadia Code`) housing bounded `<ScrollArea>` CLI configs or JSON payloads with an integrated `aria-live="polite"` `CopyButton`.
- **`ToolApprovalDialog`:** Non-dismissible human-in-the-loop gate (`12px` radius, `{colors.surface-overlay}`) displaying a live `tabular-nums` countdown chip, FIFO queue counter, caller/session metadata grid, `CodeWell` JSON inspector, and explicit **Deny / Allow Session / Approve Once** controls.

## 6. Do's and Don'ts

### Do:
- **Do** lock `html, body, #root` and the root workbench shell to `100dvh` with `overflow: clip`, placing `min-h-0 min-w-0` on every nested Flex/Grid/Tabs track and wrapping scrollable regions in `<ScrollArea>`.
- **Do** align `CardHeader`, toolbar strips, outer `TableHead`/`TableCell` padding, and `CardFooter` on the unified `12px` (`px-3`) horizontal plumb line.
- **Do** follow the concentric `2px` radius step-down (`12px` modal -> `10px` card -> `8px` well/alert -> `6px` button/input -> `4px` badge/kbd).
- **Do** use exclusively the six semantic typography utilities (`text-title-lg`, `text-title`, `text-body`, `text-caption`, `text-micro`, `text-code`) and apply `font-mono tabular-nums` to all ports, PIDs, durations, and counters.
- **Do** wrap truncated paths, versions, and descriptions in `<TruncatedText>` (keyboard-focusable Radix `<Tooltip>` with `aria-hidden="true"` duplicate suppression) rather than native `title` attributes.
- **Do** synchronize `apps/env-workbench/index.html`'s inline `#keel-critical-boot` skeleton colors and radii whenever canvas or panel tokens change.

### Don't:
- **Don't** use gradient text (`background-clip: text`), neon AI glows, or decorative glassmorphism across workbench panels.
- **Don't** use colored side-stripe borders (`border-left` or `border-right` greater than `1px`) on alerts, rows, or cards.
- **Don't** use spacious SaaS marketing hero cards, bubbly `rounded-full` text badges, or bounce/elastic motion curves.
- **Don't** introduce browser-style page scrollbars, custom HTML Min/Max/Close window buttons, or uncalibrated light-mode tokens that break the `#111116` Win32 DWM titlebar fusion.
- **Don't** write arbitrary font sizes (`text-[10px]`, `text-[11px]`, `clamp()`), arbitrary `rounded-[min(...)]` corner formulas, or `space-x-*` / `space-y-*` margin utilities.
- **Don't** nest `<Card>` inside `<Card>`; use sunken `#0b0b0f` wells (`<CodeWell>`) or `1px` hairlines for inner structure.
