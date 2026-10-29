---
name: Omega Compiler Observatory
description: A quiet evidence workspace with chart cards and inspectable provenance.
colors:
  paper: "#f6f7f9"
  card: "#fff"
  ink: "#17202b"
  muted: "#596575"
  line: "#e4e8ed"
  orange: "#df570b"
  orange-soft: "#fff0e5"
  blue: "#1765c0"
  blue-soft: "#eaf3ff"
  green: "#176b4a"
  green-soft: "#e6f5ec"
  red: "#b23b3b"
  red-soft: "#fcecec"
typography:
  headline:
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
    fontSize: "34px"
    fontWeight: 650
    lineHeight: 1.2
    letterSpacing: "-1.1px"
  title:
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
    fontSize: "18px"
    fontWeight: 650
    letterSpacing: "-.45px"
  body:
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
    fontSize: "14px"
    lineHeight: 1.55
  label:
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
    fontSize: "12px"
  code:
    fontFamily: 'ui-monospace, SFMono-Regular, Consolas, monospace'
    fontSize: "12px"
rounded:
  card: "18px"
  button: "10px"
  field: "9px"
  segment: "7px"
  cell: "6px"
spacing:
  workspace: "24px"
  stat-grid: "16px"
  toolbar: "10px"
components:
  card:
    backgroundColor: "{colors.card}"
    textColor: "{colors.ink}"
    rounded: "{rounded.card}"
  button-primary:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.card}"
    rounded: "{rounded.button}"
    padding: "9px 14px"
  input-search:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.ink}"
    rounded: "{rounded.field}"
    padding: "9px 12px"
  cell-pass:
    backgroundColor: "{colors.green-soft}"
    textColor: "{colors.green}"
    rounded: "{rounded.cell}"
---

# Design System: Omega Compiler Observatory

## Overview

The user-selected [Mebius chart-card direction](https://www.mebius.design/freebies/charts)
appears as a light workspace, white cards, restrained type and compact data graphics.
This is an operating surface for fact coverage, native results and active claims.
The signature is evidence density with calm framing: numbers lead, qualifications
remain readable, and selecting a fact reveals its provenance.

This document records the built `index.html`, not a new identity proposal. It does
not establish compiler acceptance or hosted verification. Wider screens, browser
and assistive-technology combinations beyond the current review remain unqualified.

## Colors

Orange identifies selection, small activity markers and search carets. Blue carries
native result bars, links, focus rings and historical evidence. Green marks recorded
passes and active leases; red marks recorded failures. Pale tints support these
roles without competing with the numbers. Paper, card, ink, muted and line supply
the neutral structure.

**The evidence rule.** Status cells pair color with symbols: pass `✓`, historical
`◐`, failure `!`, unmeasured `·`, and missing source `−` with a dashed border.
Keep their accessible labels and visible legend; color alone is insufficient.

## Typography

Use the platform system stack and tabular numerals throughout; provenance uses
the monospace stack. Headlines and summary values share the large numeric scale.
Card titles are modest, while labels, qualifiers and table content stay readable
at the label size. The mobile headline and summary values reduce to (29px).
Long names, paths and notes wrap rather than disappear behind truncation.

## Layout

The centered workspace caps at (1520px), with desktop horizontal gutters of
(36px). Three summary cards precede a wide fact panel and narrower native-results
column; claims occupy the full width below. Cards use the workspace gap, with
smaller spacing inside controls and lists.

At (1100px) the workspace becomes one column and native results use two columns.
At (700px), gutters reduce to (16px), summary cards stack, navigation wraps, and
native results and claims become single columns. The nine-place table retains a
(650px) minimum width inside its own keyboard-focusable horizontal scroller.
Sticky headers and bounded table/list heights preserve local context.

## Elevation & Depth

Thin neutral borders and a faint ambient card shadow establish hierarchy. Selected
segments receive a smaller shadow. Depth remains quiet: content does not float,
tilt or lift on hover. The refresh feedback is a short background flash and is
disabled when reduced motion is requested. Exact shadow and motion values live
in the sidecar.

## Shapes

Broadly rounded cards frame the workspace. Controls use smaller rounded corners;
status cells are compact squares, badges are gently rounded, and the Omega mark
is circular. Dividers separate rows without boxing every datum.

## Components

- **Summary cards:** muted labels, small tinted icons, dark values and explicit
  denominators or evidence limits.
- **Primary charts:** a blue native-history line/area chart with a quiet dashed
  cohort line, plus pale categorical fact-outcome bars with one orange gradient
  selection. Both have zero-based count axes and keyboard-selectable marks.
  Point/bar selection updates the card's prominent number and dated readout.
  The chart pair precedes summary cards; on narrow screens the charts stack.
- **Fact grid:** nine transport columns, symbol-bearing cells, selected-row orange
  edge and a revealed provenance region. Opening details focuses and reveals the
  region; closing returns focus to the originating cell.
- **Native results:** blue proportion bars, visible valid-row denominators and
  expandable provenance. Historical results retain their age and limitations.
- **Claims:** owner and lease state lead; notes and scope expand on demand.
- **Controls:** dark refresh action, pale search fields, native target select and
  segmented filters with a raised white selected state. Interactive elements use
  a blue (3px) focus outline with (3px) offset.

## Do's and Don'ts

- **Do** preserve timestamps, revisions, targets and unknown states alongside data.
- **Do** keep qualifiers at least the existing label size and retain status symbols.
- **Do** keep horizontal table scrolling local on narrow screens.
- **Don't** convert source presence or lease ownership into completion percentages.
- **Don't** introduce marketing artwork, display fonts or decorative chart noise
  into this operating surface.
