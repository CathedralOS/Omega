---
name: Omega Compiler Observatory
description: A dark evidence workspace centered on a shared host comparison.
colors:
  paper: "#101318"
  card: "#191d24"
  ink: "#edf0f5"
  muted: "#a6afbd"
  line: "#2c333e"
  orange: "#f2a16b"
  orange-soft: "#35271e"
  blue: "#71adff"
  blue-soft: "#1c2c42"
  green: "#75d8b0"
  green-soft: "#1b322b"
  red: "#ef929a"
  red-soft: "#3b252d"
  windows: "#42cba5"
  linux: "#62a5ff"
  macos: "#f18a62"
typography:
  headline:
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
    fontSize: "28px"
    fontWeight: 600
    lineHeight: 1.2
    letterSpacing: "-.85px"
  body:
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
    fontSize: "14px"
    lineHeight: 1.55
  label:
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
    fontSize: "12px"
rounded:
  card: "16px"
  control: "8px"
  cell: "5px"
spacing:
  workspace: "22px"
  stat-grid: "16px"
components:
  card:
    backgroundColor: "{colors.card}"
    textColor: "{colors.ink}"
    rounded: "{rounded.card}"
  cell-pass:
    backgroundColor: "{colors.green-soft}"
    textColor: "{colors.green}"
    rounded: "{rounded.cell}"
---

# Design System: Omega Compiler Observatory

## Overview

The user's Claude dashboard screenshot (2026-10-29) is the primary composition
reference: a large shared plot with host labels directly on its lines. Mebius
Charts supplies compact control grouping, restrained chart furniture and one
accent on the selected outcome bar. The workspace is charcoal, with dark cards
and readable neutral text. This records the implementation, not user acceptance.

## Colors

Keep host colors stable: Windows teal, Linux blue, macOS orange. Evidence states
use tinted green/blue/red surfaces paired with symbols and accessible labels;
missing source retains a dashed border. Status colors do not imply current
acceptance when the underlying observation is historical or qualified.

## Typography

Use the platform system stack and tabular numerals. The page heading is modest;
the comparison chart and its directly labeled series are the focus. Qualifiers
remain at least 12px, and provenance uses monospace. Names and notes wrap.

## Layout

The workspace caps at 1440px with 32px desktop gutters. A short introduction and
revision strip precede the full-width host comparison. Three compact metrics,
expandable native receipts, then a fact grid with a secondary outcome chart and
full-width claims follow. At 1100px the evidence workspace stacks; at 700px the
whole page uses one column and 16px gutters. The nine-place table scrolls locally.

## Elevation & Depth

Thin neutral borders and a faint 0 6px 24px shadow separate panels. The chart's
charcoal gradient is subtle. Controls brighten on hover; panels do not move.
Refresh uses a short tonal flash; reduced-motion mode disables transitions.

## Shapes

Cards use 16px corners, controls 8px, status cells 5px. The Omega mark is circular.
Avoid extra decorative badges or artwork around the data.

## Components

- The shared chart defaults to all available hosts on common zero-based count
  and date axes. Desktop end labels separate vertically and use leader lines
  when values are close. Mobile labels move below the plot without shrinking.
- Each host has one keyboard entry point at its latest observation. Left/right
  arrows traverse its recorded points; selection reveals date, count and source.
- Native receipts expand to expose exit oracles, row revisions and host freshness.
- The fact grid keeps source coverage separate from measured acceptance; its
  inspector is revealed and focused, with focus restored to the cell on close.
- Claims expose owner notes and leases as reported coordination, not completion.

## Do's and Don'ts

- Preserve actual dates, per-host cohorts, unknown states and evidence provenance.
- Keep host colors and direct labels consistent across viewport sizes.
- Do not copy the reference's old numbers, smooth away regressions or turn
  growing corpus counts into a fixed-cohort success rate.
