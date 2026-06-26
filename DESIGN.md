# Meetily Design System

> Extracted from the existing Meetily codebase. This is a descriptive record of
> current conventions, not a prescription to redesign. All Week 2 UI work must
> conform to the tokens, components, and patterns documented here.

---

## 1. Color Palette

### 1.1 CSS Custom Properties (globals.css)

Colors are defined as HSL triplets (without `hsl()` wrapper) in
`frontend/src/app/globals.css` and consumed via Tailwind's `hsl(var(--token))`
pattern.

| Token | Light | Dark | Usage |
|---|---|---|---|
| `--background` | `0 0% 100%` | `0 0% 3.9%` | App background |
| `--foreground` | `0 0% 3.9%` | `0 0% 98%` | Default text |
| `--card` | `0 0% 100%` | `0 0% 3.9%` | Card surfaces |
| `--card-foreground` | `0 0% 3.9%` | `0 0% 98%` | Card text |
| `--popover` | `0 0% 100%` | `0 0% 3.9%` | Popover surfaces |
| `--popover-foreground` | `0 0% 3.9%` | `0 0% 98%` | Popover text |
| `--primary` | `0 0% 9%` | `0 0% 98%` | Primary buttons, key UI |
| `--primary-foreground` | `0 0% 98%` | `0 0% 9%` | Text on primary |
| `--secondary` | `0 0% 96.1%` | `0 0% 14.9%` | Secondary surfaces |
| `--secondary-foreground` | `0 0% 9%` | `0 0% 98%` | Text on secondary |
| `--muted` | `0 0% 96.1%` | `0 0% 14.9%` | Muted backgrounds (tabs list) |
| `--muted-foreground` | `0 0% 45.1%` | `0 0% 63.9%` | Muted text |
| `--accent` | `0 0% 96.1%` | `0 0% 14.9%` | Hover backgrounds |
| `--accent-foreground` | `0 0% 9%` | `0 0% 98%` | Text on accent |
| `--destructive` | `0 84.2% 60.2%` | `0 62.8% 30.6%` | Error/destructive actions |
| `--destructive-foreground` | `0 0% 98%` | `0 0% 98%` | Text on destructive |
| `--border` | `0 0% 89.8%` | `0 0% 14.9%` | Default borders |
| `--input` | `0 0% 89.8%` | `0 0% 14.9%` | Input borders |
| `--ring` | `0 0% 3.9%` | `0 0% 83.1%` | Focus ring |
| `--radius` | `0.5rem` | `0.5rem` | Base border radius |

### 1.2 Tailwind Config Extended Colors (tailwind.config.ts)

These override the CSS-variable-based tokens for specific semantic uses:

| Token | Value | Tailwind Equivalent |
|---|---|---|
| `primary` | `hsl(221, 83%, 53%)` | blue-600 |
| `secondary` | `hsl(210, 40%, 96%)` | gray-50 |
| `accent` | `hsl(221, 83%, 53%)` | blue-600 |
| `destructive` | `hsl(0, 84%, 60%)` | red-500 |

### 1.3 Direct Color Usage in Components

The codebase also uses Tailwind's default palette directly in components. These
are the recurring values:

| Color | Usage |
|---|---|
| `bg-gray-50` | Main page background |
| `bg-white` | Cards, sidebar, recording controls, overlays |
| `bg-red-500` / `hover:bg-red-600` | Record button, stop button |
| `bg-red-300` | Disabled recording state |
| `bg-gray-400` | Disabled primary actions |
| `bg-gray-100` / `hover:bg-gray-200` | Secondary buttons, settings button |
| `bg-gray-200` / `hover:bg-gray-300` | Settings button |
| `bg-blue-500` / `hover:bg-blue-600` | Blue variant buttons, progress bars |
| `bg-blue-100` / `hover:bg-blue-200` | Import button |
| `bg-green-100 text-green-700` | Success status banner |
| `bg-red-100 text-red-700` | Error status banner |
| `bg-blue-100 text-blue-700` | Info/processing status banner |
| `bg-red-50 border-red-300` | Destructive alert background |
| `text-gray-300` | Empty-state icons |
| `text-gray-500` | Empty-state descriptions |
| `text-gray-600` | Processing text, muted labels |
| `text-gray-700` | Secondary text, nav items |
| `text-gray-900` | Empty-state titles, spinner borders |
| `text-amber-600` | Warning hints (no model selected) |
| `bg-orange-500` | Paused recording indicator |
| `border-gray-200` | Dividers, borders |
| `border-gray-300` | Input borders, button borders |
| `bg-black/80` | Dialog overlay |

### 1.4 Chart Colors

| Token | Light | Dark |
|---|---|---|
| `--chart-1` | `12 76% 61%` | `220 70% 50%` |
| `--chart-2` | `173 58% 39%` | `160 60% 45%` |
| `--chart-3` | `197 37% 24%` | `30 80% 55%` |
| `--chart-4` | `43 74% 66%` | `280 65% 60%` |
| `--chart-5` | `27 87% 67%` | `340 75% 55%` |

---

## 2. Typography

### 2.1 Font Size Scale (tailwind.config.ts)

| Token | Size | Line Height | Weight | Usage |
|---|---|---|---|---|
| `display` | 32px | 1.2 | 700 | Page-level display headings |
| `h1` | 24px | 1.3 | 600 | Section headings |
| `h2` | 18px | 1.4 | 500 | Subsection headings |
| `body` | 16px | 1.6 | 400 | Default body text |
| `small` | 14px | 1.5 | 400 | Secondary text, labels |
| `caption` | 12px | 1.4 | 400 | Captions, version numbers |

### 2.2 Font Family

No custom font family is configured in Tailwind. The app uses the browser
default sans-serif stack. BlockNote editor uses its own internal typography.

### 2.3 Usage in Components

- Dialog titles: `text-lg font-semibold` (18px, 600)
- Nav items: `text-lg font-semibold` (18px, 600)
- Card headings: `font-medium` (inherit size, 500)
- Body text in cards: `text-sm` (14px)
- Status messages: `text-sm font-medium` (14px, 500)
- Version/footer: `text-xs` (12px)
- Recording status labels: `text-xs text-gray-600 font-medium` (12px, 500)

---

## 3. Spacing

### 3.1 System

No custom spacing scale is defined. The project uses Tailwind's default 4px
base scale:

| Token | Value | Common Usage |
|---|---|---|
| `1` | 4px | Minimal gaps, icon spacing |
| `2` | 8px | Button padding (py-2), small gaps |
| `3` | 12px | Nav item padding, input padding |
| `4` | 16px | Card padding, section spacing |
| `6` | 24px | Content padding, larger gaps |
| `8` | 32px | Empty-state padding, large gaps |
| `12` | 48px | Bottom positioning of controls |

### 3.2 Layout Dimensions

| Element | Dimension |
|---|---|
| Sidebar collapsed | `w-16` (4rem / 64px) |
| Sidebar expanded | `w-64` (16rem / 256px) |
| Content max width | `max-w-[750px]` |
| Content width ratio | `w-2/3` |
| Recording controls | `w-12 h-12` (start), `w-10 h-10` (pause/stop) |
| Spinner (small) | `h-4 w-4` or `h-5 w-5` |
| Spinner (large) | `h-12 w-12` |
| Audio bars | `w-1` width |
| Sidebar transition | `duration-300` |

---

## 4. Border Radius

| Token | Value | Usage |
|---|---|---|
| `--radius` | 0.5rem (8px) | Base radius (CSS variable) |
| `rounded-md` | 6px | Buttons, inputs, tab triggers |
| `rounded-lg` | 8px | Cards, alerts, dialogs, nav items, status banners |
| `rounded-full` | 9999px | Recording controls, pills, spinners, audio bars |
| `rounded-sm` | 2px | Dialog close button |
| `rounded` | 4px | Scrollbar thumb |

---

## 5. Shadows

| Token | Usage |
|---|---|
| `shadow-sm` | Buttons (destructive, outline, secondary), sidebar, footer buttons |
| `shadow-lg` | Recording controls container, dialogs, status overlays |
| `shadow` | Active tab trigger |
| (none) | Cards in summary panel (`shadow-sm` used for inner cards) |

---

## 6. Component Patterns

### 6.1 UI Primitive Pattern (shadcn/ui)

All 22 primitives in `frontend/src/components/ui/` follow this pattern:

```
Radix Primitive → forwardRef wrapper → CVA variants → cn() merge → Tailwind classes
```

- **Utility**: `cn()` from `@/lib/utils` (clsx + tailwind-merge)
- **Variants**: `class-variance-authority` (CVA)
- **Icons**: Lucide React (`lucide-react`)
- **Ref forwarding**: All interactive components use `React.forwardRef`
- **Slot pattern**: Button supports `asChild` via `@radix-ui/react-slot`

### 6.2 Available Primitives

`form`, `input`, `textarea`, `select`, `dropdown-menu`, `button`, `separator`,
`dialog`, `command`, `switch`, `alert`, `tooltip`, `accordion`, `label`,
`scroll-area`, `sheet`, `visually-hidden`, `progress`, `popover`, `input-group`,
`button-group`, `tabs`

### 6.3 Button Variants

| Variant | Classes | Usage |
|---|---|---|
| `default` | `bg-primary text-primary-foreground shadow hover:bg-primary/90` | Primary actions |
| `destructive` | `bg-destructive text-destructive-foreground shadow-sm hover:bg-destructive/90` | Delete, destructive |
| `outline` | `border border-input bg-background shadow-sm hover:bg-accent` | Secondary actions |
| `secondary` | `bg-secondary text-secondary-foreground shadow-sm hover:bg-secondary/80` | Tertiary actions |
| `ghost` | `hover:bg-accent hover:text-accent-foreground` | No background |
| `link` | `text-primary underline-offset-4 hover:underline` | Inline links |
| `green` | `bg-green-600 text-white hover:bg-green-600` | Success actions |
| `blue` | `bg-blue-500 text-white hover:bg-blue-600` | Blue accent actions |
| `red` | `bg-red-500 text-white hover:bg-red-600` | Recording actions |
| `gray` | `border bg-gray-100 border-input shadow-sm hover:bg-gray-200` | Neutral actions |

Button sizes: `default` (h-9 px-4 py-2), `sm` (h-8 px-3 text-xs), `lg` (h-10 px-8), `icon` (h-9 w-9)

### 6.4 Layout Pattern

```
┌─────────────────────────────────────────────┐
│ Sidebar (w-16/w-64, bg-white, border-r)     │
│                                             │
│  ┌───────────────────────────────────────┐  │
│  │ Main Content (bg-gray-50, flex-1)     │  │
│  │                                       │  │
│  │  Content area (w-2/3, max-w-[750px])  │  │
│  │                                       │  │
│  │  Recording controls (fixed bottom)    │  │
│  │  (bg-white rounded-full shadow-lg)    │  │
│  └───────────────────────────────────────┘  │
└─────────────────────────────────────────────┘
```

- Sidebar: `h-screen bg-white border-r shadow-sm flex flex-col transition-all duration-300`
- Main: `flex flex-col h-screen bg-gray-50`
- Content: `w-2/3 max-w-[750px] flex justify-center`
- Controls: `fixed bottom-12 left-0 right-0 z-10`
- Sidebar margin offset: `marginLeft: sidebarCollapsed ? '4rem' : '16rem'`

### 6.5 Form Pattern

- `react-hook-form` + `zod` + `@hookform/resolvers` for validation
- `Input`, `Textarea`, `Select`, `Label`, `Switch` primitives from `ui/`
- Form fields use `border border-gray-300 rounded-md focus:ring-2 focus:ring-blue-500`

### 6.6 Toast Pattern

- Library: `sonner`
- Methods: `toast.success()`, `toast.error()`, `toast.warning()`, `toast.info()`
- Supports `description`, `action` (label + onClick), `duration` props
- Used for recovery, save, language, and error notifications

---

## 7. Motion

### 7.1 Framer Motion

Used for page-level and section-level transitions:

```tsx
<motion.div
  initial={{ opacity: 0, y: 20 }}
  animate={{ opacity: 1, y: 0 }}
  transition={{ duration: 0.3, ease: 'easeOut' }}
>
```

- Page entry: `opacity: 0 → 1`, `y: 20 → 0`, `duration: 0.3`, `ease: 'easeOut'`
- Empty state entry: `opacity: 0 → 1`, `scale: 0.95 → 1`, `duration: 0.3`, `ease: 'easeOut'`

### 7.2 CSS Animations (globals.css)

| Class | Keyframe | Duration | Usage |
|---|---|---|---|
| `animate-spin` | (Tailwind built-in) | — | Loading spinners |
| `animate-vibrate` | `vibrate` | 0.3s linear | Error shake |
| `animate-fade-in` | `fade-in` (translateY -4px → 0) | 0.4s ease-out | Element entrance |
| `animate-fade-in-up` | `fade-in-up` (translateY 10px → 0) | 0.5s ease-out forwards | Content entrance |
| `animate-pulse` | (Tailwind built-in) | — | Search loading |

Animation delay utilities: `.delay-75` (75ms), `.delay-100` (100ms), `.delay-150` (150ms)

### 7.3 Radix Transitions

`tailwindcss-animate` plugin provides `animate-in`/`animate-out` with
`fade-in-0`, `fade-out-0`, `zoom-in-95`, `zoom-out-95`, `slide-in-from-*`,
`slide-out-to-*` for Radix dialog/popover/sheet open/close transitions.

### 7.4 Transition Properties

- `transition-colors` — Button hover/active states
- `transition-all duration-300` — Sidebar collapse/expand
- `transition-all duration-200` — Audio bar height changes
- `transition-[margin] duration-300` — Content margin offset for sidebar
- `transition-opacity` — Dialog close button

---

## 8. Icons

- **Library**: `lucide-react` (^0.469.0)
- **Pattern**: Import named icons, render as `<IconName size={16|18|20} className="..." />`
- **Common sizes**: 14 (chevrons), 16 (button icons), 18 (language picker), 20 (start recording), 4 (close button)
- **No emoji icons** — SVG icons only
- **Icon + text pattern**: `<Icon className="w-4 h-4 mr-2" /> <span>Label</span>`

Common icons used: `Mic`, `Square`, `Play`, `Pause`, `Settings`, `Home`, `Search`,
`Trash2`, `Plus`, `X`, `AlertCircle`, `FileQuestion`, `Sparkles`, `Languages`,
`ChevronDown`, `ChevronRight`, `ChevronLeftCircle`, `ChevronRightCircle`,
`Calendar`, `StickyNote`, `NotebookPen`, `Upload`, `Pencil`

---

## 9. State Patterns

### 9.1 Loading State

**Visual**: Animated spinner + text message

| Context | Spinner | Text |
|---|---|---|
| Button (inline) | `animate-spin h-5 w-5 border-b-2 border-white` | — |
| Processing (inline) | `animate-spin h-5 w-5 border-b-2 border-gray-900` | `text-sm text-gray-600` |
| Summary generation | `animate-spin h-12 w-12 border-t-2 border-b-2 border-blue-500` | `text-gray-600` "Generating AI Summary..." |
| Status overlay | `animate-spin h-4 w-4 border-b-2 border-gray-900` | `text-sm text-gray-700` |
| Model validation | `animate-spin h-5 w-5 border-b-2 border-white` | `text-xs text-gray-600` "Validating speech recognition..." |

**Pattern**: Spinner is always `rounded-full` with `border-b-2`. Size scales with
context. Text is always `text-gray-600` or `text-gray-700`.

### 9.2 Error State

**Visual**: Alert component with destructive variant + red-toned palette

```tsx
<Alert variant="destructive" className="border-red-300 bg-red-50">
  <AlertCircle className="h-5 w-5 text-red-600" />
  <AlertTitle className="text-red-800 font-semibold">
    {title}
  </AlertTitle>
  <AlertDescription className="text-red-700">
    {message}
  </AlertDescription>
</Alert>
```

| Element | Classes |
|---|---|
| Alert container | `border-red-300 bg-red-50` (overlays CVA destructive) |
| Icon | `text-red-600` |
| Title | `text-red-800 font-semibold` |
| Description | `text-red-700` |
| Close button | `text-red-600 hover:text-red-800` |
| Status banner | `bg-red-100 text-red-700` |

**Toast**: `toast.error(message, { description: errorDetails })`

### 9.3 Empty State

**Visual**: Centered column with large muted icon, title, description, CTA button

```tsx
<motion.div
  initial={{ opacity: 0, scale: 0.95 }}
  animate={{ opacity: 1, scale: 1 }}
  transition={{ duration: 0.3, ease: 'easeOut' }}
  className="flex flex-col items-center justify-center h-full p-8 text-center"
>
  <FileQuestion className="w-16 h-16 text-gray-300 mb-4" />
  <h3 className="text-lg font-semibold text-gray-900 mb-2">Title</h3>
  <p className="text-sm text-gray-500 mb-6 max-w-md">Description</p>
  <Button className="gap-2">
    <Sparkles className="w-4 h-4" />
    CTA Label
  </Button>
</motion.div>
```

| Element | Classes |
|---|---|
| Container | `flex flex-col items-center justify-center h-full p-8 text-center` |
| Icon | `w-16 h-16 text-gray-300 mb-4` |
| Title | `text-lg font-semibold text-gray-900 mb-2` |
| Description | `text-sm text-gray-500 mb-6 max-w-md` |
| CTA | `Button` with icon + `gap-2` |
| Warning hint | `text-xs text-amber-600 mt-3` |

### 9.4 Success State

**Visual**: Green status banner or success toast

| Element | Classes |
|---|---|
| Status banner | `bg-green-100 text-green-700` + `p-4 rounded-lg` |
| Toast | `toast.success(message, { description, action, duration })` |

### 9.5 Processing / Info State

**Visual**: Blue status banner or info toast

| Element | Classes |
|---|---|
| Status banner | `bg-blue-100 text-blue-700` + `p-4 rounded-lg` |
| Toast | `toast.info(message, { description })` |

---

## 10. Knowledge Graph Component State Guidance

Week 2 introduces Knowledge Graph (KG) UI components. These must follow the
existing state patterns documented above. The following table maps KG-specific
states to the established visual language:

### 10.1 KG Loading

- **Graph building**: Large spinner (`animate-spin h-12 w-12 border-t-2 border-b-2 border-blue-500`) centered in graph viewport with `text-gray-600` message: "Building knowledge graph..."
- **Node expansion**: Small inline spinner (`animate-spin h-5 w-5 border-b-2 border-gray-900`) with `text-sm text-gray-600` status text
- **Edge resolution**: Progress bar (`Progress` component from `ui/progress.tsx`) with `text-sm text-gray-700` percentage label
- **Pattern**: Match SummaryPanel loading — spinner + text, centered, `flex items-center justify-center`

### 10.2 KG Error

- **Graph build failure**: `Alert variant="destructive"` with `AlertCircle` icon, `border-red-300 bg-red-50`, title in `text-red-800 font-semibold`, description in `text-red-700`
- **Node/edge fetch error**: `toast.error(message, { description })` + inline `bg-red-100 text-red-700` banner in graph panel
- **Pattern**: Match RecordingControls device error alert and SummaryPanel error banner

### 10.3 KG Empty

- **No graph generated**: Centered `motion.div` with `FileQuestion` or graph-specific icon at `w-16 h-16 text-gray-300`, title at `text-lg font-semibold text-gray-900`, description at `text-sm text-gray-500 max-w-md`, CTA `Button` with icon
- **No meetings to build from**: Same empty-state pattern with different copy
- **Pattern**: Match `EmptyStateSummary` exactly — same motion props, same class hierarchy

### 10.4 KG Success

- **Graph ready**: `bg-green-100 text-green-700` banner with `text-sm font-medium` status message, auto-dismiss after 3s
- **Export complete**: `toast.success(message, { description, action })` with action button to view/export
- **Pattern**: Match SummaryPanel success banner and recovery toast pattern

### 10.5 KG Interactive States

| State | Visual | Classes |
|---|---|---|
| Node hover | Highlighted border | `border-blue-500 ring-2 ring-blue-500/30` |
| Node selected | Filled background | `bg-blue-500 text-white` |
| Node disabled | Reduced opacity | `opacity-50 pointer-events-none` |
| Edge hover | Thicker stroke | `stroke-2` → `stroke-4` |
| Drag preview | Semi-transparent | `opacity-70` |
| Search match | Pulsing highlight | `animate-pulse ring-2 ring-blue-500` |

---

## 11. Accessibility

### 11.1 Current Practices

- **Radix UI primitives**: All interactive components (Dialog, Tooltip, Tabs,
  Popover, Select, etc.) inherit Radix's built-in ARIA attributes, keyboard
  navigation, and focus management
- **Screen reader text**: `sr-only` class used for icon-only button labels
  (e.g., Dialog close button: `<span className="sr-only">Close</span>`)
- **aria-label**: Applied to icon-only buttons (e.g., `aria-label="Close alert"`,
  `aria-label="Set summary language"`)
- **role="alert"**: Alert component uses `role="alert"` for screen reader
  announcement
- **Focus management**: `focus-visible:outline-none focus-visible:ring-2
  focus-visible:ring-ring focus-visible:ring-offset-2` on interactive primitives
- **Keyboard navigation**: Radix primitives handle Escape to close, Tab
  traversal, arrow key navigation in lists
- **Title attribute**: Used on buttons for tooltip text (e.g.,
  `title="Summary language: ..."`)

### 11.2 Requirements for Week 2 KG Components

- All icon-only buttons must have `aria-label` or `sr-only` text
- KG graph nodes must be keyboard-focusable with `tabIndex={0}` and
  `onKeyDown` handlers for Enter/Space activation
- KG error states must use `role="alert"` (via Alert component)
- KG loading states should use `aria-live="polite"` on the status message
  container so screen readers announce loading completion
- KG empty states must have a clear CTA with descriptive text
- Color contrast: maintain WCAG AA (4.5:1 text, 3:1 large text) — the existing
  `text-gray-500` on `bg-white` and `text-gray-700` on `bg-white` pass; verify
  any new color combinations
- Do not rely on color alone for state indication — always include icon or text

---

## 12. React Dev Tooling Assessment

| Tool | Status | Notes |
|---|---|---|
| `react-scan` | Not installed | Not in package.json or source |
| `react-grab` | Not installed | Not in package.json or source |
| `react-doctor` | Not installed | Not in package.json or source |

**Assessment**: React dev tooling is deferred. No installation was performed per
task constraints. If performance profiling becomes necessary during Week 2 KG
development, consider adding `react-scan` as a dev-only dependency gated behind
`NODE_ENV === 'development'`.

---

## 13. Key Dependencies

| Dependency | Version | Purpose |
|---|---|---|
| `@radix-ui/*` | various | Accessible UI primitives (12 packages) |
| `class-variance-authority` | ^0.7.1 | Component variant management |
| `clsx` | ^2.1.1 | Conditional class names |
| `tailwind-merge` | ^3.3.1 | Tailwind class deduplication |
| `tailwindcss` | ^3.4.1 | Utility-first CSS framework |
| `tailwindcss-animate` | ^1.0.7 | Radix animation utilities |
| `framer-motion` | ^11.15.0 | Page/section transitions |
| `lucide-react` | ^0.469.0 | SVG icon library |
| `sonner` | ^2.0.7 | Toast notifications |
| `react-hook-form` | ^7.59.0 | Form state management |
| `zod` | ^3.25.71 | Schema validation |
| `cmdk` | ^1.1.1 | Command palette |
| `@tanstack/react-virtual` | ^3.13.13 | List virtualization |
| `@blocknote/*` | ^0.36.0 | Rich text editor |
| `@tailwindcss/typography` | ^0.5.15 | Prose styling plugin |

---

## 14. File Reference

| File | Role |
|---|---|
| `frontend/src/app/globals.css` | CSS variables, keyframes, scrollbar, BlockNote overrides |
| `frontend/tailwind.config.ts` | Extended colors, font sizes, typography plugin |
| `frontend/src/components/ui/*.tsx` | 22 shadcn/ui primitives |
| `frontend/src/lib/utils.ts` | `cn()` utility (clsx + tailwind-merge) |
| `frontend/src/app/page.tsx` | Main page layout, Framer Motion entry |
| `frontend/src/components/RecordingControls.tsx` | Recording button patterns, spinner, alert |
| `frontend/src/components/EmptyStateSummary.tsx` | Empty state reference pattern |
| `frontend/src/components/MeetingDetails/SummaryPanel.tsx` | Loading/error/success banner patterns |
| `frontend/src/app/_components/StatusOverlays.tsx` | Fixed-bottom status overlay pattern |
| `frontend/src/components/Sidebar/index.tsx` | Sidebar layout, nav items, footer buttons |
| `frontend/src/components/SettingTabs.tsx` | Tab navigation pattern |
| `frontend/package.json` | Dependency manifest |