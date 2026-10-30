---
name: Omega Compiler Observatory
description: Visible history, fixed-cohort and growth charts in charcoal cards.
colors:
  paper: "#101010"
  card: "#1b1b19"
  ink: "#eeeee9"
  muted: "#aaa9a2"
  line: "#343431"
  cohort-baseline: "#898b87"
  cohort-current: "#63a5ed"
  cohort-all: "#e2ae78"
  fact-pass: "#8dd9b5"
  fact-pass-soft: "#20372d"
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
    backgroundColor: "{colors.fact-pass-soft}"
    textColor: "{colors.fact-pass}"
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

The workspace caps at 1280px with 32px desktop gutters. Plain tabs show one view
at a time. Overview leads with full-width history, then the fixed-cohort and
growth charts side by side. All three remain visible without a chart selector.
Facts places a full-width horizontal result chart above the full-width grid.
Performance leads with historical normalized compile observations, then a
LOC/time scatter and directly labeled runtime bars. Absolute times and resource
measurements remain in a visible table. Host selection filters recorded settings,
not unrelated machines merged into one trend. Dates and missing producer settings
are visible above the charts; measurement details retain raw samples and provenance.
Work uses a two-column card list, with active and unresolved lease counts above.
Cards stack on narrow screens; the fact names stay fixed during horizontal
grid scrolling. Evidence
details opens a native dialog for methods, receipts and provenance. At 900px
the comparisons stack; at 700px the page uses 14px gutters. The nine-place
table scrolls locally.

## Elevation & Depth

Charcoal cards have thin neutral borders and no shadows or gradients. History
uses the host colors and direct endpoint labels from the reference. Charts
appear immediately without entrance animation; short captions replace prose.

## Shapes

Cards use 16px corners, controls 8px, status cells 5px. The Omega mark is plain.
Avoid extra decorative badges or artwork around the data.

## Components

- Fixed cohort uses three bars per host on a shared 0–100% axis: gray baseline,
  blue same set now, warm full corpus now. Growth separates recovered old IDs
  from new running IDs with direct counts; green is recovered, gray is new.
- Tabs support arrow keys and maintain one selected panel. Evidence details
  uses native dialog focus handling and Escape dismissal.
- The shared chart defaults to all available hosts on common zero-based count
  and date axes. Desktop end labels separate vertically and use leader lines
  when values are close. Mobile labels move below the plot without shrinking.
- Each host has one keyboard entry point at its latest observation. Left/right
  arrows traverse its recorded points; selection reveals date, count and source.
- Native receipts expand to expose exit oracles, row revisions and host freshness.
- The fact grid keeps source coverage separate from measured acceptance; its
  inspector is revealed and focused, with focus restored to the cell on close.
  A compact Source column shows positive/reject coverage out of nine. Readable
  names and original identifiers both work in search.
- Work cards directly expose owner, full note, scope and expiry. Only exact
  identifiers live under Details. Expired leases remain unresolved, never complete.
- Performance uses logarithmic axes for orders-of-magnitude differences. Project
  KLOC is distinct from the full compiler input closure. Unpaired runtime data
  displays process milliseconds and ranges, never a manufactured speedup.

## Do's and Don'ts

- Preserve actual dates, per-host cohorts, unknown states and evidence provenance.
- Keep host colors and direct labels consistent across viewport sizes.
- Do not copy the reference's old numbers, smooth away regressions or turn
  growing corpus counts into a fixed-cohort success rate.
