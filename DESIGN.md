# DESIGN.md: Anything Design System

> AI-native design specification for consistent, on-brand UI generation.
> Every AI coding agent should reference this file when creating or modifying UI components.
> Source of truth for values: `src/styles/variables.css` (`@theme` = type scale, `:root`/`.dark` = colors).

---

## 1. Visual Theme & Atmosphere

**Mood**: Government Elegance. Trustworthy, precise, yet state-of-the-art.
Warm stone palette with green accent. Clean, editorial layout with generous whitespace.
Inspired by Linear (precision) + Notion (warmth) + Apple (breathing room).

**Density**: Medium. Information-rich search results balanced by minimal chrome.
**Feel**: Quiet confidence, not flashy. The UI disappears; content leads.

**Accessibility floor**: text and icons meet WCAG AA (4.5:1 text, 3:1 UI graphics) in both themes.
Public-sector users read small meta text for hours; contrast is not optional.

---

## 2. Color Palette & Roles

### Light Mode

| Role | Token | Hex | Usage |
|------|-------|-----|-------|
| Background Primary | `--color-bg-primary` | `#FAFAF7` | Page background, warm ivory |
| Background Secondary | `--color-bg-secondary` | `#FFFFFF` | Cards, elevated surfaces |
| Background Tertiary | `--color-bg-tertiary` | `#F0EFEB` | Hover states, inactive surfaces |
| Background Subtle | `--color-bg-subtle` | `#E7E5E4` | Pressed states, dividers |
| Text Primary | `--color-text-primary` | `#1C1917` | Main readable text |
| Text Secondary | `--color-text-secondary` | `#44403C` | Supportive text |
| Text Muted | `--color-text-muted` | `#736C66` | Helper text, captions (4.9:1) |
| Text Tertiary | `--color-text-tertiary` | `#78716C` | Least-emphasis meta (4.6:1) |
| **Accent (Green)** | `--color-accent` | `#017F58` | Links, active states, filled CTAs (text 4.8:1, white on fill 5.0:1) |
| Accent Hover / Active | `--color-accent-hover` / `-active` | `#016B4A` / `#015A3E` | Hover, pressed |
| On Accent | `--color-on-accent` | `#FFFFFF` | Text/icons on accent fills |
| Accent AI (Indigo) | `--color-accent-ai` | `#4F46E5` | AI features (text 6.0:1) |
| On Accent AI | `--color-on-accent-ai` | `#FFFFFF` | Text on AI fills |
| Accent Warm (Amber) | `--color-accent-warm` | `#D97706` | Highlights, relevance dots |
| Success | `--color-success` | `#047857` | Positive feedback |
| Warning | `--color-warning` | `#B45309` | Warning text/icons |
| Error | `--color-error` | `#DC2626` | Error states |
| On Error | `--color-on-error` | `#FFFFFF` | Text on error fills (danger button) |
| Info | `--color-info` | `#0369A1` | Informational |
| Border / Hover | `--color-border` / `-hover` | `#E7E5E4` / `#D6D3D1` | Borders |

**Brand green `#01AF7A`** (app icon) stays in the `@theme` palette and in translucent tints
(`--color-accent-subtle`, `-light`, `-border`, `-glow`). It is too light for text (2.7:1) or for
white labels (2.8:1), so solid accent usage goes through `--color-accent`.

### Dark Mode

| Role | Hex | Notes |
|------|-----|-------|
| Background Primary / Secondary / Tertiary | `#111113` / `#1A1A1F` / `#242429` | Warm dark, NOT blue-tinted |
| Text Primary / Muted / Tertiary | `#FAFAF9` / `#A8A29E` / `#8A837D` | Tertiary 5.0:1 |
| Accent | `#10C48E` | Bright green (8.4:1 as text) |
| On Accent | `#06281C` | Dark label on bright green fill (white would be 2.3:1) |
| Accent AI / On Accent AI | `#818CF8` / `#111113` | |
| Error / On Error | `#F87171` / `#111113` | |

### File Type Colors

| Type | Light | Dark |
|------|-------|------|
| HWPX / HWP / HML | `#7C3AED` (violet) | `#A78BFA` |
| DOCX | `#2563EB` (blue) | `#60A5FA` |
| PPTX | `#D97706` (amber) | `#FBBF24` |
| XLSX | `#16A34A` (green) | `#34D399` |
| PDF | `#DC2626` (red) | `#F87171` |
| TXT | `#57534E` (stone) | `#A8A29E` |

---

## 3. Typography Rules

### Font Families
- **Headings / Display**: `Outfit Variable` → Pretendard fallback (`text-display` class)
- **Body**: `Pretendard Variable` → `Malgun Gothic` → system-ui
- **Code**: `Consolas` → `D2Coding` → `Fira Code` → monospace

### Type Scale (~1.2 ratio, 15px base)

Defined once in `@theme` (`variables.css`). Tailwind `text-*` utilities and the `ts-*` classes read the same tokens.

| Token / Utility | Size | Usage |
|-------|------|-------|
| `text-2xs` / `--text-2xs` | 11px | Minimum size: badges, timestamps, meta |
| `text-xs` | 12px | Badges, secondary labels |
| `text-sm` | 13px | Auxiliary text, file paths |
| `text-base` | 15px | Body default |
| `text-md` | 16px | Search input |
| `text-lg` | 17px | Emphasized body, result file names |
| `text-xl` | 21px | Subsection headings |
| `text-2xl` | 26px | Page titles |
| `text-hero` | 44px | Hero headline (`clamp()` on the home screen) |

- **No text below 11px.** Do not use arbitrary sizes (`text-[10px]`); pick a scale step.
- Korean labels get no `uppercase` and no wide letter-spacing (uppercase does nothing to Hangul; tracking scatters it). English eyebrow labels (e.g. "Step 1 / 5") may keep them.

### Letter Spacing
- **Hero/Headings** (`ts-hero`, `ts-2xl`, `ts-xl`): `-0.04em` (tight)
- **Subheadings** (`ts-lg`): `-0.02em`
- **Body**: `+0.01em` (subtle open)

---

## 4. Component Stylings

### Buttons

| Variant | Background | Text | Hover |
|---------|-----------|------|-------|
| Primary | `--color-accent` | `--color-on-accent` | `--color-accent-hover` + shadow |
| Secondary | `--color-bg-secondary` | `--color-text-secondary` | `--color-bg-tertiary` |
| Ghost | transparent | `--color-text-muted` | `--color-bg-tertiary` |
| Danger | `--color-error` | `--color-on-error` | `--color-error-hover` |

- Never hardcode `white`/`#fff` on a colored fill; use the matching `--color-on-*` token (dark mode flips it).
- Active: `scale(0.98)` press effect. Disabled: `opacity: 0.4`.
- Icon-only buttons need `aria-label` (a `title` alone is not enough) and flex/grid centering.
- Focus: global `:focus-visible` outline in accent. `ring-offset-*` picks up the panel color automatically (no white halo in dark mode).

### Result Items

- Borderless rows (`.result-card`, radius `--radius-md`), divided by 1px lines; hover = subtle background tint.
- Selected: `--color-accent-light` background + 1.5px accent outline, same `--radius-md` radius (no corner jump).
- List and group views both expose `role="option"` + `aria-selected`; the listbox tracks the active option with `aria-activedescendant`.
- Relevance: 3-dot signal (`RelevanceDots`), only for semantic/hybrid matches, in both views. No raw %.
- File names: `HighlightedFilename middle`: long names truncate in the **middle** so the suffix (`…_최종_수정본.hwpx`) stays visible.
- Section headers (파일명 매치 / 내용 매치): tinted background, no colored left border.

### Empty & Loading States

- Every empty state says **why** and offers the next action: 문서 읽는 중 (progress N / M), 필터에 가려짐 (필터 풀기), 폴더 없음 (폴더 추가), 결과 없음 (다른 검색어·파일명 검색).
- Lazy panels show `PanelLoading` (spinner + label), never a blank area.
- Preview errors show a plain reason plus [다시 시도] and [원본 열기].

### Inputs

- Background `--color-bg-primary`, 1px `--color-border` + `--shadow-inner`.
- Focus: container ring in accent.
- Search inputs show a clear (×) button when non-empty.

### Modals & Confirmations

- Enter: `scale(0.97→1) + translateY(8px→0)`, 200ms ease-out-expo; exit 130ms.
- Backdrop: `rgba(0,0,0,0.4)` light / `rgba(0,0,0,0.6)` dark. Focus trap, ESC closes.
- Destructive or heavy actions (폴더 제거, 다시 읽기, 전체 드라이브 인덱싱) confirm first with `ask()` and say what is kept (원본 파일은 그대로). Failures always surface a toast.

---

## 5. Layout Principles

### Spacing Scale (8px base)

| Token | Value | Usage |
|-------|-------|-------|
| xs | 4px | Very tight |
| sm | 8px | Tight |
| md | 16px | Normal |
| lg | 24px | Generous |
| xl | 32px | Large |
| 2xl | 48px | Section gaps |

### Content Column
- **Max width**: `--content-max-width: clamp(820px, 62vw, 1400px)`: readable on narrow windows, uses space on wide ones
- WelcomeHero: centered, max-w-520px for prompts

### Sidebar & Header
- Sidebar 200px expanded / 48px collapsed, 200ms ease-out
- Header 44px. The collapsed search bar uses the same horizontal padding as the header (logo does not shift)

### Preview Panel
- One `PreviewContainer`: push beside results on wide windows, overlay on narrow ones. Switching modes never remounts the panel.
- Splitter drag updates once per animation frame.

### Border Radius Scale
- `--radius-sm` 4px · `--radius-md` 6px · `--radius-lg` 8px · `--radius-xl` 12px (`--radius-card`) · `--radius-2xl` 16px

---

## 6. Depth & Elevation

| Level | Usage |
|-------|-------|
| `--shadow-sm` / `md` / `lg` / `xl` | Subtle depth → dropdowns → modals |
| `--shadow-card` / `card-hover` | Static / interactive cards |
| `--shadow-premium` | Interactive surface lift (hero search, main search bar) |
| `--shadow-premium-accent` | Primary CTA / focused hero (green-tinted lift) |

- `--gradient-accent`: primary CTA + AI send only (one per screen). Its stops stay dark enough for `--color-on-accent`.
- Dark mode shadows use stronger black opacity; card hover adds an accent glow border.

---

## 7. Do's and Don'ts

### DO
- Use tokens (`--color-*`, `--text-*`, `--radius-*`, `--shadow-*`); Tailwind palette classes (`bg-gray-200`, `text-red-500`) break dark mode
- Use `--color-accent-ai` (indigo) for AI/semantic features
- Keep keyboard parity: clickable rows are real `<button>`s; hover-revealed actions also appear on `group-focus-within` / `focus-visible`
- Keep selection visible and keyboard order equal to visual order (group view moves file by file)
- Apply staggered fade-in to result lists (max 10 items, 30ms delay)
- Use Lucide icons (not emoji) for UI glyphs

### DON'T
- Don't put text below 11px or use arbitrary font sizes
- Don't hardcode `white` on colored fills; don't use the brand `#01AF7A` as a text color
- Don't use drop shadows on static cards; don't use colored left-border stripes
- Don't use gradients on ordinary buttons
- Don't use opacity to build text hierarchy or hover states; use color tokens
- Don't use virtual scrolling; use "N개 더 보기" pagination
- Don't animate longer than 400ms (except ambient breathe/float)

### Copy (Korean UI)
- No em-dash (U+2014) in any user-facing string; use `:` or `·`
- No trailing "..." in loading labels or placeholders ("저장 중", "키워드로 문서 검색"); "…" only for truncation
- Platform words come from `utils/platform`: `FILE_MANAGER_NAME` (탐색기/Finder), `MOD_KEY` (Ctrl/⌘), `REVEAL_LABEL`
- Errors go through `getErrorMessage()` (`types/error.ts`, one message per backend `ApiError` code); never render `String(e)`
- Plain words over internals: no "FTS5", "청크", "DB", raw token counts or ms where a user reads them

---

## 8. Responsive Behavior

Desktop-first Tauri app (Windows + macOS). No mobile breakpoints.

- Sidebar collapses to 48px icons
- Preview switches push → overlay when the results area would drop below 480px
- Modals: max-width `36rem`, max-height `80vh`
- Hero text uses `clamp(2.5rem, 5vw, 3.5rem)`

---

## 9. Agent Prompt Guide

```
Use the design system defined in DESIGN.md:
- Colors: CSS variables (--color-*) only; text on fills uses --color-on-accent / --color-on-accent-ai / --color-on-error
- Typography: Tailwind text-2xs … text-2xl (tokens in @theme) or ts-* classes; 11px minimum
- Display font: text-display class for headings with Outfit
- Spacing: 8px-based Tailwind utilities; radius via --radius-*; shadows via --shadow-*
- Result items: borderless rows, selected = accent-light + outline (--radius-md), RelevanceDots, middle-truncated file names
- Accessibility: WCAG AA contrast, real buttons, aria-label on icon buttons, focus-visible for hover actions
- Copy: Korean, no em-dash, no "..." except truncation, platform words from utils/platform
```

### Quick Reference
- **Accent**: `#017F58` light / `#10C48E` dark (brand icon `#01AF7A` for tints only)
- **AI accent**: `#4F46E5` light / `#818CF8` dark
- **Typography base**: Pretendard 15px, minimum 11px
- **Content max-width**: `clamp(820px, 62vw, 1400px)`
- **Sidebar**: 200px / 48px
- **Easing**: `cubic-bezier(0.16, 1, 0.3, 1)` (ease-out-expo)

---

*Version: 1.1 | Updated: 2026-09-23 (v3.8.9 contrast tokens, type scale in @theme, keyboard parity, copy rules)*
