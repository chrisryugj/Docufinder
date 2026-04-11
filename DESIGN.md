# DESIGN.md — Anything Design System

> AI-native design specification for consistent, on-brand UI generation.
> Every AI coding agent should reference this file when creating or modifying UI components.

---

## 1. Visual Theme & Atmosphere

**Mood**: Government Elegance — trustworthy, precise, yet state-of-the-art.
Warm stone palette with green accent. Clean, editorial layout with generous whitespace.
Inspired by Linear (precision) + Notion (warmth) + Apple (breathing room).

**Density**: Medium — information-rich search results balanced by minimal chrome.
**Feel**: Quiet confidence, not flashy. The UI disappears; content leads.

---

## 2. Color Palette & Roles

### Light Mode

| Role | Hex | Usage |
|------|-----|-------|
| **Background Primary** | `#FAFAF7` | Page background — warm ivory, not clinical white |
| **Background Secondary** | `#FFFFFF` | Cards, elevated surfaces |
| **Background Tertiary** | `#F0EFEB` | Hover states, inactive surfaces |
| **Background Subtle** | `#E7E5E4` | Pressed states, dividers |
| **Text Primary** | `#1C1917` | Main readable text — warm stone black |
| **Text Secondary** | `#44403C` | Supportive text |
| **Text Muted** | `#78716C` | Helper text, timestamps, captions |
| **Accent (Green)** | `#01AF7A` | CTAs, active states, links — matches app icon |
| **Accent Hover** | `#019468` | Darker accent for hover |
| **Accent AI (Indigo)** | `#6366F1` | AI/semantic features |
| **Accent Warm (Amber)** | `#D97706` | Warnings, notifications |
| **Border** | `#E7E5E4` | Default borders |
| **Border Hover** | `#D6D3D1` | Hover borders |
| **Success** | `#059669` | Positive feedback |
| **Error** | `#DC2626` | Error states |
| **Info** | `#0284C7` | Informational |

### Dark Mode

| Role | Hex | Notes |
|------|-----|-------|
| **Background Primary** | `#111113` | Warm dark, NOT blue-tinted |
| **Background Secondary** | `#1A1A1F` | Cards |
| **Text Primary** | `#FAFAF9` | Inverted |
| **Accent** | `#10C48E` | Brighter green for dark contrast |
| **Accent AI** | `#818CF8` | Brighter indigo for dark |

### File Type Colors

| Type | Light | Dark |
|------|-------|------|
| HWPX | `#7C3AED` (violet) | `#A78BFA` |
| DOCX | `#2563EB` (blue) | `#60A5FA` |
| PPTX | `#D97706` (amber) | `#FBBF24` |
| XLSX | `#16A34A` (green) | `#34D399` |
| PDF | `#DC2626` (red) | `#F87171` |
| TXT | `#57534E` (stone) | `#A8A29E` |

---

## 3. Typography Rules

### Font Families
- **Headings / Display**: `Outfit Variable` → Pretendard fallback
- **Body**: `Pretendard Variable` → `Malgun Gothic` → system-ui
- **Code**: `Consolas` → `D2Coding` → `Fira Code` → monospace

### Type Scale (15px base, 1.25 ratio)

| Token | Size | Weight | Usage |
|-------|------|--------|-------|
| `--text-2xs` | 11px | 400 | Version labels, minimal meta |
| `--text-xs` | 12px | 400-600 | Badges, timestamps |
| `--text-sm` | 13px | 400 | Auxiliary text, file paths |
| `--text-base` | 15px | 400 | Body default |
| `--text-md` | 16px | 400-500 | Search input |
| `--text-lg` | 18px | 600 | Subheadings |
| `--text-xl` | 25px | 700 | Section titles |
| `--text-2xl` | 31px | 700 | Page title |
| `--text-hero` | 44px | 800 | Hero headlines |

### Letter Spacing
- **Hero/Headings** (`ts-hero`, `ts-2xl`, `ts-xl`): `-0.04em` (tight)
- **Subheadings** (`ts-lg`): `-0.02em`
- **Body**: `+0.01em` (subtle open)

### Font Rendering
- `-webkit-font-smoothing: antialiased`
- `-moz-osx-font-smoothing: grayscale`

---

## 4. Component Stylings

### Buttons

| Variant | Background | Text | Border | Hover |
|---------|-----------|------|--------|-------|
| Primary | `--color-accent` | white | none | `--color-accent-hover` + shadow |
| Secondary | transparent | `--color-text-secondary` | `--color-border` | `--color-bg-tertiary` |
| Ghost | transparent | `--color-text-muted` | none | `--color-bg-tertiary` |
| Danger | `--color-error` | white | none | `--color-error-hover` |

- Border radius: `8px` (`--radius-lg`)
- Active: `scale(0.98)` press effect
- Disabled: `opacity: 0.4`, no pointer events
- Icon buttons: color transition (not opacity)

### Cards / Result Items

- Background: transparent (borderless, no left border)
- Border radius: `--radius-lg` (8px)
- Hover: `--shadow-md` elevation + subtle file-type background tint
- Selected: `--color-accent-light` background + 1.5px accent outline
- Spacing: `space-y-1.5` (6px) between items in normal density

### Inputs

- Background: `--color-bg-primary`
- Border: 1px `--color-border` + `--shadow-inner`
- Focus: border `--color-accent` + 3px accent glow ring
- Font size: `--text-md` (16px) — prevents zoom on mobile

### Modals

- Enter: `scale(0.97→1) + translateY(8px→0)`, 200ms ease-out-expo
- Exit: reverse, 130ms (65% of entry)
- Backdrop: `rgba(0,0,0,0.4)` light / `rgba(0,0,0,0.6)` dark
- Focus trap: Tab cycles within modal, ESC closes

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
- **Max width**: `820px` (`--content-max-width`)
- Centered with auto margins
- Search results and main content constrained
- WelcomeHero: centered within column, max-w-520px for search prompt

### Sidebar
- Expanded: `200px`
- Collapsed: `48px`
- Transition: 200ms ease-out

### Header Height
- `44px` fixed

### Border Radius Scale
- `--radius-sm`: 4px
- `--radius-md`: 6px
- `--radius-lg`: 8px
- `--radius-xl`: 12px
- `--radius-2xl`: 16px

---

## 6. Depth & Elevation

### Shadow Scale (warm-tinted, stone-based)

| Level | Shadow | Usage |
|-------|--------|-------|
| sm | `0 1px 2px rgba(28,25,23,0.04)` | Subtle depth |
| md | `0 1px 3px ... + 0 1px 2px ...` | Result hover |
| lg | `0 4px 6px ... + 0 2px 4px ...` | Dropdowns |
| xl | `0 10px 15px ... + 0 4px 6px ...` | Modals |
| card | `0 1px 3px ... + 0 0 0 1px ...` | Static cards |
| card-hover | `0 4px 12px ... + accent tint` | Interactive cards |

### Dark Mode Shadows
- Stronger opacity (0.3-0.5) with pure black base
- Card hover includes accent glow border

---

## 7. Do's and Don'ts

### DO
- Use colors from the defined palette only
- Use semantic colors (success/warning/error/info) for feedback
- Use file-type colors for file-specific UI (badges, left borders)
- Use `--color-accent-ai` (indigo) for AI/semantic features
- Keep search results in `content-column` (max 820px)
- Apply staggered fade-in animation to result lists (max 10 items, 30ms delay)
- Keep breathing room between result cards (space-y-1.5 normal, space-y-0.5 compact)
- Use skeleton-shimmer for loading states
- Use warm stone shadows, not neutral gray

### DON'T
- Don't invent new colors outside the palette
- Don't use pure black (`#000`) or pure white (`#FFF`) for backgrounds
- Don't use drop shadows on static cards — use subtle 1px borders
- Don't use more than two typefaces on a single screen
- Don't center-align body text
- Don't use animations longer than 400ms (except ambient breathe/float)
- Don't use gradient backgrounds on buttons (except Anything AI banner)
- Don't use opacity for hover states on buttons — use color transitions
- Don't use left-border accents on result cards — they feel cluttered
- Don't use virtual scrolling (react-window etc.) — use "show more" pagination

---

## 8. Responsive Behavior

This is a **desktop-first Tauri app** (Windows). No mobile breakpoints required.

### Adaptive Rules
- Sidebar: collapse to 48px icon-only when user toggles
- Search input: full width within content column
- Result items: full width within content column (max 820px)
- Modals: max-width `36rem` (xl), max-height `80vh`

### Font Scaling
- Use `clamp()` for hero text: `clamp(2.5rem, 5vw, 3.5rem)`
- Body text stays consistent at 15px

---

## 9. Agent Prompt Guide

When creating or modifying UI components for Anything:

```
Use the design system defined in DESIGN.md:
- Colors: Use CSS variables (--color-*) only, never hardcoded hex
- Typography: Use type scale classes (ts-base, ts-sm, ts-xs) or CSS vars (--text-*)
- Display font: Use text-display class for headings with Outfit
- Spacing: Use 8px-based scale via Tailwind utilities
- Shadows: Use shadow vars (--shadow-sm/md/lg/xl/card)
- Borders: Use --color-border vars, --radius-* for corners
- Animations: Use existing animation classes (animate-fade-in, stagger-item, skeleton-shimmer)
- Result items: Always include file-type left border (result-stripe-*)
- Content: Wrap in content-column class for max-width constraint
- AI features: Use --color-accent-ai (indigo) for semantic/AI indicators
```

### Quick Reference
- **Primary accent**: `#01AF7A` (green)
- **AI accent**: `#6366F1` (indigo)
- **Warm accent**: `#D97706` (amber)
- **Typography base**: Pretendard, 15px, weight 400
- **Display font**: Outfit Variable
- **Content max-width**: 820px
- **Sidebar**: 200px / 48px
- **Easing**: `cubic-bezier(0.16, 1, 0.3, 1)` (ease-out-expo)

---

*Version: 1.0 | Updated: 2026-04-11 | Based on awesome-design-md 9-section framework*
