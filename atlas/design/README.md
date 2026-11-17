# Atlas design kit

Any model can make an atlas page with this kit, whether that is GPT, Codex or
Claude, without special tools. A page is one static HTML file in
`atlas/pages/` that links `../design/tokens.css`, draws its figures as
inline SVG, and opens in any browser. This file holds the rules; `tokens.css`,
`tokens.json` and `fonts/` hold the values; `page-template.html` is a page to
copy.

The rules come from the Omega design system (owner's design-system artifact)
and from the diagrams already published in the atlas. The two existing pages,
`pages/pipeline-map.html` and `pages/foundation-backend-plan.html`, are worked
examples of every rule here (they are generated, and carry the same token
values inline rather than linking `tokens.css`).

## The look in one paragraph

Omega looks like masonry. Build every page from flat, square-edged blocks on
`--ground`, separated by space and hairlines, never by shadows. Words are set
in Omega Serif, code in IBM Plex Mono, nothing else. Color carries meaning,
never decoration: green (`--checked`) means proven or passing, red (`--trap`)
means a rejection, a trap or a reach-around, and everything else is ink on
stone.

## Page contract

- One `.html` file per page in `atlas/pages/`, named for its subject
  (`psi-stages.html`, not `page3.html`). It must open from disk with no build
  step and no network except Google Fonts for IBM Plex Mono.
- Start the file with `<!doctype html>`, `<meta charset="utf-8">`,
  `<meta name="viewport" content="width=device-width, initial-scale=1">`, a
  `<title>` that names the page in two to four words, and
  `<link rel="stylesheet" href="../design/tokens.css">`.
- Use only the tokens for color, type, space and shape. Never write a hex
  value in a page; if a color is missing, it does not belong on the page.
- Both themes must read. `tokens.css` switches them; give `body` an explicit
  `background: var(--ground)`, and give every SVG mark a token color.
- No JavaScript is required. A page may add a small inline script for a
  toggle (see the reach-around switch in `pipeline-map.html`), and must read
  correctly without it.
- Running text stays under 68 characters a line. Wide tables and figures
  scroll inside their own frame (`overflow-x: auto`); the page itself never
  scrolls sideways. Check it at 400px wide.

## Voice

- Lead with what the reader can now do or decide and why it matters; put
  evidence after.
- Use real words and define each term once: `expression`, not `expr`.
- Give measured results with denominators: "9 of 9 places pass", not "all
  green". Mark anything not checked *unverified* and every classification
  choice *judgment*.
- Sentence case for headings and labels. No emoji, no exclamation marks, no
  hype words.

## Color

| Token | Use it for |
| --- | --- |
| `--ground` | The page background |
| `--surface` | Panels, code, figures, tables |
| `--ink` | Text and solid marks; realized (Omega) things |
| `--ink-muted` | Secondary text, labels, control and outline edges |
| `--rule` | Decorative hairlines between sections; never the only edge of anything |
| `--checked` | Proven, passing, closed; the checker's verdict; the focus ring |
| `--trap` | Rejections, traps and reach-arounds; keep it small |

Never show status by color alone: pair `--checked` with a word or check mark,
`--trap` with a word or cross. No gradients, no opacity tints, no shadows.

## Type

| Role | Token | Use |
| --- | --- | --- |
| display | `--type-display` | The page title, once |
| heading | `--type-heading` | Section headings |
| subheading | `--type-subheading` | Subsections, tile titles |
| body | `--type-body` | All running text |
| label | `--type-label` (+ `letter-spacing: .02em`) | Status words, counts, captions, eyebrows |
| code | `--type-code` | Code, commands, file paths, on `--surface` |

Use `font-variant-numeric: tabular-nums` wherever digits line up.

## Space and shape

- Space doubles: `--space-2` inside controls, `--space-3` tile padding,
  `--space-4` between groups, `--space-5` between sections. Lay out siblings
  with `gap`, not margins.
- Corners are square (`--radius-stone`). Only buttons and inputs get
  `--radius-chamfer`; `--radius-arch` rounds top corners only. No pills.
- Hairlines (`--border-hairline` in `--rule`) between sections; control and
  outline edges are `--border-control` (2px).

## Diagram grammar

Atlas figures show mechanisms: what flows, which boundary it crosses, what
changes between before and after. A figure that only names boxes is a list;
write the list instead.

| Mark | Meaning |
| --- | --- |
| Outlined box (`--surface` fill, 2px `--ink-muted` edge) | A target-neutral thing: source, a Psi representation |
| Solid box (`--ink` fill, `--ground` text) | A realized thing: a native form, an executable |
| Heavy outline (2.5px `--ink`) | A hub: Terminal Psi, where Psi and Omega meet |
| Dashed outline | A proposed package or form that does not exist yet |
| Thin box on `--ground`, 1.5px `--rule` edge | Something around the pipeline (CLI, Build, packages) |
| Solid arrow in `--ink`, labeled | One numbered stage, or a named hand-off; label it with the stage number and verb (`03 select`) |
| Loop arrow on a box | A stage that rewrites its own representation |
| Green arrow or text (`--checked`) | The checker and its verdict |
| Dashed red arrow or box (`--trap`) | Code that reaches around the pipeline today |
| Dotted line across the figure | A firewall (Psi never depends on Omega) |

Drawing rules:

- Inline `<svg viewBox="0 0 W H" role="img" aria-label="...">` inside
  `<figure>` with a `<figcaption>` that states the figure's claim. The
  `aria-label` says the same claim.
- Flows read left to right; when a row runs out, continue right to left on
  the next row (the pipeline map does this for the Omega half).
- Arrowheads are `<marker>` paths filled with the arrow's own token color.
- Text 11.5–14px at drawn scale. Give arrow labels a halo so lines never cut
  them: `paint-order: stroke; stroke: var(--surface); stroke-width: 4px`.
- Align boxes to a grid with equal gaps; give every box the same height in a
  row.
- Before publishing, look at the page once in light and once in dark, at full
  width and at 400px. Fix overlaps; do not ship a label crossing a line.

## Evidence on a page

- Every number comes from a script in `atlas/evidence/scripts/` or a line
  count anyone can rerun; say which in the caption or a "How far to trust it"
  note.
- Quotes carry `file:line`.
- A page's source revision is stated near the top.

## Checklist before a page is done

- [ ] Opens from disk; no console errors; fonts load from `../design/fonts/`.
- [ ] Reads in light and dark; no hard-coded colors.
- [ ] No horizontal page scroll at 400px.
- [ ] Every figure has a caption that states its claim and an `aria-label`.
- [ ] Every number and quote has its evidence; *unverified* and *judgment*
      are marked.
- [ ] The page is linked from the table in `atlas/README.md`.
