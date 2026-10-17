---
name: PalaClips
description: Instant replay and per-app audio capture for Windows, read like an end-of-round scoreboard.
colors:
  stadium-night: "#0f1420"
  rail-night: "#0a0e17"
  surface: "#171d2b"
  surface-raised: "#1e2536"
  surface-hover: "#232b3e"
  line: "#262e40"
  line-strong: "#343d52"
  ink: "#e8ecf4"
  ink-muted: "#a7b0c3"
  ink-faint: "#7b869c"
  rec-red: "#ff4d5e"
  rec-ink: "#ffffff"
  warn-amber: "#f2b84b"
  focus-blue: "#8fb8ff"
  meter-ground: "#0b0f18"
  track-mix: "#e8ecf4"
  track-mic: "#f2b84b"
  track-game: "#7fd18b"
  track-1: "#4fc1b9"
  track-2: "#8f86f5"
  track-3: "#5aa9e6"
  track-4: "#e58fb8"
  track-5: "#c9b458"
  track-6: "#6fd6e8"
  track-7: "#b98cf2"
  track-8: "#e6a16a"
  track-9: "#9cc2ff"
  track-10: "#8fd1a6"
  track-11: "#d6a0e0"
typography:
  display:
    fontFamily: "Barlow Condensed, Barlow, Segoe UI, sans-serif"
    fontSize: "40px"
    fontWeight: 600
    lineHeight: 1
    letterSpacing: "0.01em"
    fontFeature: "\"tnum\", \"lnum\""
  score:
    fontFamily: "Barlow Condensed, Barlow, Segoe UI, sans-serif"
    fontSize: "17px"
    fontWeight: 500
    lineHeight: 1
    letterSpacing: "0.01em"
    fontFeature: "\"tnum\", \"lnum\""
  headline:
    fontFamily: "Barlow, Segoe UI Variable Text, Segoe UI, sans-serif"
    fontSize: "22px"
    fontWeight: 600
  title:
    fontFamily: "Barlow, Segoe UI Variable Text, Segoe UI, sans-serif"
    fontSize: "17px"
    fontWeight: 600
  title-cell:
    fontFamily: "Barlow, Segoe UI Variable Text, Segoe UI, sans-serif"
    fontSize: "15px"
    fontWeight: 600
  body:
    fontFamily: "Barlow, Segoe UI Variable Text, Segoe UI, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.45
    fontFeature: "\"tnum\""
  caption:
    fontFamily: "Barlow, Segoe UI Variable Text, Segoe UI, sans-serif"
    fontSize: "13px"
    fontWeight: 400
  label:
    fontFamily: "Barlow Condensed, Barlow, Segoe UI, sans-serif"
    fontSize: "12px"
    fontWeight: 500
    letterSpacing: "0.08em"
rounded:
  hairline: "2px"
  chip: "3px"
  plate: "4px"
  md: "6px"
  pill: "9999px"
spacing:
  xs: "4px"
  sm: "8px"
  md: "12px"
  lg: "16px"
  xl: "20px"
components:
  button-primary:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.stadium-night}"
    rounded: "{rounded.md}"
    padding: "0 14px"
    height: "36px"
  button-primary-disabled:
    backgroundColor: "{colors.surface-raised}"
    textColor: "{colors.ink-faint}"
    rounded: "{rounded.md}"
  button-secondary:
    textColor: "{colors.ink}"
    rounded: "{rounded.md}"
    padding: "0 14px"
    height: "36px"
  button-secondary-hover:
    backgroundColor: "{colors.surface-hover}"
  rail-item:
    textColor: "{colors.ink-faint}"
    rounded: "{rounded.md}"
    width: "68px"
    padding: "10px 0"
  rail-item-current:
    backgroundColor: "{colors.surface-raised}"
    textColor: "{colors.ink}"
  jersey-plate:
    textColor: "{colors.stadium-night}"
    typography: "{typography.score}"
    rounded: "{rounded.plate}"
    width: "44px"
    height: "32px"
  clip-card:
    backgroundColor: "{colors.surface}"
    rounded: "{rounded.md}"
  meter:
    backgroundColor: "{colors.meter-ground}"
    rounded: "{rounded.hairline}"
    height: "10px"
  status-banner:
    backgroundColor: "{colors.surface}"
    padding: "16px 20px"
---

# Design System: PalaClips

## Overview

**Creative North Star: "The After-Match Scoreboard"**

Every audio track is a player on the team, lined up in a roster like the scoreboard a competitive shooter shows at the end of a round. The window is read at a glance, then closed: it states whether the replay is armed, how many seconds are held, what that costs, and which tracks will land in the clip. Density is high but calm; figures are large, condensed and tabular, prose is short and in the second person.

The world is stadium night: a blue-black ground with flat tonal surfaces, a single ink, hairline rules that cut the window into whole cells, and one fixed jersey colour per track that never changes meaning. It deliberately refuses the category's default of dark cards with one neon accent; colour carries identity (which track) or one state (it records), never decoration. Native Windows 11 in spirit: restrained, gently rounded, quiet motion, offline fonts.

**Key Characteristics:**
- Recomposition by whole cells on a strict grid, separated by 1 px rules, no floating cards on the home screen.
- Scoreboard figures in Barlow Condensed with tabular lining numerals; text in Barlow.
- One fixed colour per audio track, shown as a jersey number plate and repeated as dots wherever that track appears.
- Red is a single word: "it records".
- Dark by default with a full light theme derived from the same roles.

## Colors

A stadium-night neutral ramp with one ink, one alarm, one caution, and a fixed jersey palette that belongs to audio tracks rather than to the interface.

### Primary
- **Stadium Ink** (ink): the only interface "accent". Text, the primary button fill, the rail mark, and the Mix track. Its rarity as a fill is what makes the primary action findable.

### Secondary
- **Recording Red** (rec-red): means only "it records". An outlined ring is the discreet replay-armed witness; a solid red fill is reserved for manual recording in progress, with **Recording White** (rec-ink) on it.
- **Caution Amber** (warn-amber): warnings and degraded states (silent track, unavailable device, window fallback, replay error ring, action errors).
- **Focus Blue** (focus-blue): keyboard focus outline and text selection tint only.

### Tertiary: jersey palette
- **Mix** (track-mix, same as ink), **Micro** (track-mic, amber), **Jeu** (track-game, green), then **Track 1 to 11** (track-1..track-11) for apps in configuration order. A title not in the configuration is hashed onto the 11 app slots. Mid-saturation, chosen to read on stadium-night without glowing.

### Neutral
- **Stadium Night** (stadium-night): window ground; also the text colour on ink and jersey fills.
- **Rail Night** (rail-night): the navigation rail, one step darker than the ground.
- **Surface** (surface): status banner and clip cards.
- **Surface Raised** (surface-raised): alert rows, current rail item, disabled primary button.
- **Surface Hover** (surface-hover): hover fill for secondary and icon buttons.
- **Line / Line Strong** (line, line-strong): cell rules and borders / button outlines, dashed empty states, scrollbar thumb.
- **Ink Muted / Ink Faint** (ink-muted, ink-faint): secondary copy / metadata, inactive rail items, column labels, idle figures.
- **Meter Ground** (meter-ground): meter trough and missing-thumbnail backdrop.

### Light theme
Under `prefers-color-scheme: light` every role is remapped, never re-chosen: ground #eef1f6, rail #e2e7ef, surface #ffffff, surface-raised #f5f7fb, surface-hover #edf1f7, line #d9dfe9, line-strong #c3cbd9, ink #121826, ink-muted #445066, ink-faint #536076, rec #d92639, warn #9a6200, focus #2f6fd8, meter-ground #e4e9f1; jersey colours darken to keep contrast (mix #2b3345, mic #8a5f00, game #23703a, track-1..11 #127a73, #5b4fd6, #1f6fb3, #b23f78, #7d6a00, #0e7f93, #7d3fc7, #a85314, #2f5fbf, #2d7a4c, #9a3fa8).

### Named Rules
**The One Meaning Red Rule.** Red says "it records" and nothing else. Outlined ring = replay armed; solid red = manual recording in progress. Brand marks, errors and emphasis never borrow it; the rail mark is drawn in ink.

**The Fixed Jersey Rule.** A track's colour is assigned once (Mix ink, Micro amber, Jeu green, apps track-1..11 in config order, unknown titles hashed) and never reassigned or reused for anything else. The same colour follows the track to every screen: roster, clip cards, audio sources, player, toast.

**The Ink Is The Accent Rule.** No interface accent colour exists. The primary action is an ink fill; everything else is tonal. One exception: a switch in the on state is an ink fill, because a tonal fill would fall under the 3:1 non-text contrast a control state needs. Selected segments stay tonal (surface-raised, ink text, semibold).

## Typography

**Display Font:** Barlow Condensed 500/600 (with Barlow, Segoe UI)
**Body Font:** Barlow 400/500/600 (with Segoe UI Variable Text, Segoe UI)

Both self-hosted through @fontsource (OFL), never loaded from the network.

**Character:** Barlow's plain grotesque reads like Windows signage; its condensed cut gives the scoreboard its wide, stacked figures without shouting.

### Hierarchy
- **Display** (Condensed 600, 40px, line-height 1): the buffered replay clock in the banner. One per screen.
- **Score** (Condensed 500–600, 17–28px, line-height 1): every figure the user scans: recording timer (28px), disk free and buffer total (20px), peak dB (19px), jersey number (18px), RAM and resolution (17px), clip duration (14px), shortcut keys (13px).
- **Headline** (600, 22px): page titles.
- **Title** (600, 17px): section titles ("Pistes audio", "Derniers clips"); 15px for banner cell headings.
- **Body** (400, 14px, line-height 1.45, tabular numerals on by default): all running text; alert text capped at 90ch, placeholder pages at 60ch.
- **Caption** (400, 12–13px): metadata, track source, track state, relative time.
- **Label** (Condensed 500, 12px, 0.08em, uppercase, ink-faint): table column headers only.

### Named Rules
**The Figures Are Condensed Rule.** Any number the user reads at a glance (time, size, dB, track number, shortcut) is set in Barlow Condensed with tabular lining numerals. Body copy never uses the condensed face.

**The State Is The Heading Rule.** A banner cell is introduced by its current state ("Replay armé", "Enregistrement arrêté", the source name, the free space), never by an uppercase category label. Uppercase labels exist only as table column headers.

## Layout

A two-column shell: a fixed 84 px navigation rail and a fluid main area that scrolls on its own. The home screen stacks a full-width status banner (cells side by side, the replay cell 2.2 times wider, separated by 1 px vertical rules), an alert strip, then a grid of roster (fluid) and recent clips (260–320 px). Below 1000 px the banner cells wrap and the clips column moves under the roster, switching its cards to an auto-fill grid of 220 px minimum. Window minimum 900 x 600, target 1180 x 760, Windows scaling 100–200%.

Spacing rhythm on a 4 px base: 20 px horizontal inset for every cell, row and section; 16 px vertical inside banner cells; 12 px for rows, alerts and gaps between cards; 8 px between stacked items inside a cell. The roster is a five-column grid (52 px number, 1.3fr name, min 140 px / 2fr meter, 76 px peak, 1fr state) with 16 px column gaps; header and rows share the same template.

**The Whole Cell Rule.** Content recomposes by whole cells on the grid, divided by 1 px line rules. Cells do not float, overlap or carry their own shadows.

## Elevation & Depth

Flat. No shadows anywhere. Depth is tonal: rail-night under ground under surface under surface-raised, with 1 px line rules marking every boundary. State is shown by fill change (hover, current) or by the track colour, never by lift.

**The Flat Scoreboard Rule.** If a surface needs to stand apart, step its tone or rule it off; do not add a shadow.

## Shapes

Gentle Windows-like corners on interactive and container elements (6px), tighter corners on small inset pieces (jersey plate 4px, duration chip 3px, meter 2px), and pills for clip track dots. Rules are always 1 px; dashed line-strong borders mark empty states. Icons come from one stroke family (lucide), 16–20 px, stroke 1.8 at rest and 2.2 when current or on the primary button.

## Components

### Buttons
Compact, tactile, quiet.
- **Shape:** 36px high, 6px corners, 14px horizontal padding, semibold label with optional 16px icon.
- **Primary:** ink fill, stadium-night text, shortcut shown inline in score type at 70% opacity. Hover ink at 90%. Disabled: surface-raised fill, ink-faint text, not-allowed cursor.
- **Secondary:** 1 px line-strong outline, ink text; hover surface-hover fill; busy state shows a wait cursor.
- **Icon button:** 4px padding, ink-faint, hover surface-hover and ink.
- **Press:** every pressable scales to 0.97 on :active over 140 ms with the out curve; colours transition 140 ms.
- **Focus:** 2px focus-blue outline, 2px offset, 4px radius, on every focusable element.

### Navigation (rail)
84 px column on rail-night with the PalaClips mark in ink at the top, then stacked items (icon above a 12px medium label, 68 px wide, 6px corners). Current: surface-raised fill, ink, heavier icon stroke, `aria-current="page"`. Rest: ink-faint; hover surface fill and ink.

### Status banner
Surface band of cells divided by 1 px rules. Each cell leads with its state as a 15px semibold heading, then its figures in score type, then its actions. The replay witness is a 12 px ring: rec-red when armed, warn-amber on error, ink-faint otherwise.

### Roster (signature)
One row per audio track, in file order. Each row: jersey plate, track icon in its colour with name and source, live meter, peak in dB, state in caption type (ink-muted capturing, ink-faint waiting, amber silent or unavailable).
- **Jersey plate:** 44 x 32 px, 4px corners, filled with the track colour, two-digit number in 18px condensed semibold, stadium-night text.
- **Meter:** 10 px trough on meter-ground, 2px corners, fill and 2 px peak-hold marker in the track colour, tick marks at −48/−36/−24/−12/−6 dB on a −60 dB floor. Levels arrive at ~20 Hz and are interpolated in one shared rAF loop: instant attack, 26 dB/s release, 1.2 s peak hold then 14 dB/s fall; the dB text refreshes every 120 ms; the loop sleeps when idle.
- **Save moment:** every row washes to 16% of its track colour in 80 ms, then fades out over 600 ms on the out curve. Meters never freeze.

**The Jersey Plate Rule.** A track's colour is carried by its numbered plate (and its icon and meter), never by a coloured stripe on the row edge.

### Clip cards
Surface, 1 px line border, 6px corners. 16:9 thumbnail on meter-ground (film icon when missing), duration chip bottom-right in score type on ground at 85%. Body: game name semibold, relative time caption, then one 20 x 6 px pill per track in its jersey colour. A freshly saved clip enters with @starting-style from 12 px above at 98% scale and 0 opacity over 240 ms. Empty state: dashed line-strong box naming the save shortcut.

### Alerts
Full-width rows on surface-raised under the banner, 1 px rules between them, 18 px severity icon (info ink-muted; warning and error amber, told apart by their icon, never red), message capped at 90ch, dismiss icon button. Rows enter from 4 px above over 200 ms.

### Motion
One curve, `cubic-bezier(0.23, 1, 0.32, 1)`, for every eased transition; durations 80–600 ms as above. Under `prefers-reduced-motion` every transition becomes 0 ms, press scaling is removed, and meters jump straight to their target.

## Do's and Don'ts

### Do:
- **Do** set every glanceable figure in Barlow Condensed with tabular lining numerals.
- **Do** give each audio track its fixed jersey colour and repeat it wherever that track appears.
- **Do** lead banner cells with their current state as the heading.
- **Do** divide the window into whole cells with 1 px line rules on a 20 px inset.
- **Do** keep the primary action an ink fill and everything else tonal.
- **Do** drop every transition to 0 ms under `prefers-reduced-motion`.

### Don't:
- **Don't** use red for anything except recording: not for errors, brand marks or emphasis. Armed replay is an outlined ring; solid red is recording in progress only.
- **Don't** reassign or reuse a track colour for another track or for interface chrome.
- **Don't** mark tracks with a coloured stripe on the row edge; use the jersey plate.
- **Don't** put uppercase eyebrow labels above banner cells or sections; uppercase labels are for table column headers only.
- **Don't** add shadows or neon accents; depth is tonal and flat.
- **Don't** load fonts or icons from the network, or mix icon families.
