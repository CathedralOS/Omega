Omega looks like masonry. A wall stands because every stone is placed and carries its load, and an Omega program runs because every fact is placed and proved. Build every page from flat, square-edged blocks on `ground`. Separate them with space and hairlines, never shadows: stone does not float.

## Voice

- Lead with what the reader can now do and why it matters. Put the details and evidence after that.
- Use real words: `expression`, not `expr`; `pipeline`, not `driver`. Define a term the first time it is useful, then use that exact term every time.
- Give measured results with their denominators: "9 of 9 places pass", not "all green".
- Use sentence case for headings, buttons and labels. No emoji, no exclamation marks and no hype words.

## Color

| Token | Use it for |
| --- | --- |
| `ground` | The page background |
| `surface` | Panels, code blocks, inputs and tiles |
| `ink` | Text and solid marks |
| `ink-muted` | Secondary text, labels and control borders |
| `rule` | Decorative hairlines between sections |
| `checked` | Proven or passing results, and the focus ring |
| `trap` | Traps and rejections, kept small |

- Set pages on `ground` and put panels, code and inputs on `surface`.
- Use `ink` for text. Use `ink-muted` for anything secondary and for the edges of controls.
- `rule` is decoration only. It is too faint to be the only edge of a control.
- `checked` means proved, passing or closed. It is also the focus ring: 2px solid, offset 2px.
- `trap` means a runtime trap or a rejection. Use it for nothing else, and keep it small.
- Never rely on color alone to show status. Pair `checked` with the word and a check mark, and `trap` with the word and a cross. On their fills, use `on-checked` and `on-trap`.
- No gradients and no tints made by lowering opacity. Every color is one of these tokens.

## Type

Two typefaces, one rule: **Omega Serif for words, IBM Plex Mono for code.** Nothing else.

- **Omega Serif** (`--font-serif`) sets every word: `display` for the page title, `heading` for sections, `subheading` for subsections and tile titles, `body` for all running text, and `label` for status words, counts and captions. It is the system's own typeface: its round letters are arches, its serifs are flat stone-cut wedges, and the feet of Ω reach outward.
- **IBM Plex Mono** (`--font-mono`) sets only `code`: code, commands and file paths, on `surface`.
- Keep running text to at most 68 characters a line, and use tabular numbers where digits line up in columns.
- Omega Serif covers ASCII, the Western European accented letters, Greek, and the proof and logic notation Omega's docs use: − ≤ ≥ ≠ ≈ ≡ ∧ ∨ ∩ ∈ ⊆ ∅ ∘ ∀ ∃ ⊢ ↦ ↔ ⇒ ⟶ ⟨ ⟩ ⟦ ⟧ § µ ¹ ² ³. Operators share one stroke weight and one centre line, so `committed ≤ bound` sits level with the words around it. Anything else falls back to Georgia.
- Load Omega Serif from this system's `fonts/` (Light 300, Regular 400, Medium 500) and IBM Plex Mono 400 from Google Fonts.
- Change Omega Serif by editing `fonts/source/build.py`, not by editing the font files. Every letter is built from the settings in its `setup` function.

## Space

Each step doubles the one before, so blocks always line up, the way aligned memory does.

- Use `space-2` inside controls, `space-3` for tile padding and between related items, `space-4` between groups and `space-5` between page sections.
- Lay out siblings with `gap`, not margins.

## Shape

- Every corner is `radius-stone` (square) unless a rule below says otherwise.
- Buttons and inputs use `radius-chamfer`.
- `radius-arch` goes on top corners only, to make an arch. Use it for status pips, section markers and arch-topped tiles. Never round all four corners, and never use pills.
- **Psi in outline, Omega in solid.** Draw target-neutral things (source, checking, Psi) as outlines in `border-control`. Draw realized things (native code, executables, Omega) as solid `ink` fills. Where the two meet (Terminal Psi), mark the joint in `checked`.
- Show the nine places as a 3×3 grid, always in this order: local, field, parameter, call result, match join, const name, nested machine, another machine, another target.

## Borders and depth

- Use `border-hairline` in `rule` between sections.
- Use `border-control` in `ink-muted` around inputs and buttons.
- No shadows. Show that something is raised by putting it on `surface` instead.

## Motion

- Change state with a 120ms opacity or color transition. Nothing slides, bounces or loops.
- With `prefers-reduced-motion`, change state instantly.

## Components

- Use `Button` for actions: `solid` for the one action that runs or realizes something, `outline` for checking, `quiet` for navigation.
- Report every result with `Status`, never with a colored word alone.
- Group content in `Tile`: `outline` for target-neutral things, `solid` for realized ones.
- Show code and commands in `CodeBlock`.
- Show a fact's coverage with `PlaceGrid`. Use the `compact` size where space is tight.

## Iconography

- No emoji.
- Draw icons as strokes on a 24px grid: 2px strokes with square caps and miter joins, in `currentColor`.
- There is no icon set yet. Until there is one, draw only check, cross and arrow.

## Logo

No mark is final. Until one is, set the name "Omega" in `display`, in `ink`. The candidate marks are on the [Omega Marks](https://claude.ai/artifact/TfQaA32k7rdDT5FRZcjvyy) canvas.


---

## Consuming this system (generated — do not edit)

Every path named below is under `project/` in this design system: read `project/api/tokens.md`, not `api/tokens.md`.

`components/bundle.js` defines `window.Omega` (5 components); `components/bundle.css` is its stylesheet; `tokens.css` is every token as a CSS variable plus `@font-face` for the fonts. `components/bundle.css` reads its variables from `tokens.css`. The bundle needs react 18 (not packed in this system: bring your own copy) (`window.React`), react-dom 18 (not packed in this system: bring your own copy) (`window.ReactDOM`), loaded before it. Build any UI by mounting these components; never hand-build a control or draw an icon the system provides.

- **Standalone page:** inline `tokens.css` and `components/bundle.css` in a `<style>`, then the library files and `components/bundle.js` as classic scripts (a file containing `</style`, `</script` or `<!--` breaks an inline element: write the sequence `<\/style`, `<\/script` or `\x3C!--` in your copy, or load that file by URL).
- **Design canvas:** bring `tokens.css`, `components/bundle.css`, `components/bundle.js` and `components/index.d.ts` (for the editor’s props panel) onto the canvas in full, as the canvas type’s design-system components reference says (a server-side copy first where it offers one); in each artboard that uses this system, right after the `support.js` line, load `tokens.css` first, then the stylesheet, then the script; skip the library files (the artboard supplies React); mount with `<x-import component-from-global-scope="Omega.<Comp>" …>`.
- **Slides deck, or any surface that cannot run the bundle:** tokens and fonts — the values are on `api/tokens.md`; the deck takes `tokens.json` by file path for its colour pickers, and fonts as “Fonts” below says.

Fonts: `tokens.css` loads each font from its file, by path.

- **Design canvas:** copy `fonts/OmegaSerif-Light.woff2`, `fonts/OmegaSerif-Regular.woff2` and `fonts/OmegaSerif-Medium.woff2` to the same paths under the canvas’s `project/ds/<folder>/` (`<folder>` is the folder you install this system under; `fonts/OmegaSerif-Light.woff2` lands on `project/ds/<folder>/fonts/OmegaSerif-Light.woff2`), in the same call that installs this system; the copy of `tokens.css` that each artboard links loads them from there. If the install’s reply leaves a font out, bring that font in as for any other page.
- **Slides deck:** for each family you use in the deck (`faces` holds at most the number of typefaces Slides’ `deck-files.md` gives), install its file with the system and name the path it lands on as that family’s `src` in `project/deck.json` `faces`, under the key in the Slides column. A deck takes a font as a file only under the rule in Slides’ `fonts.md` (no space or leading underscore in its path, for two): any other font, or one kept by id, is uploaded as `fonts.md` says.
- **Any other page:** one file per family is enough (below: the upright face nearest regular weight; bold and italic synthesize). Fetch it as the fetch column says (Artifact tool `read`, that id or path as `path`), upload it as an asset (`publish`, `file_path`, `asset:true`) and declare it with an `@font-face { font-family: "<family>"; src: url(<uploaded>) }` rule.

| family | file | fetch | CSS | Slides `faces` key |
| --- | --- | --- | --- | --- |
| Omega Serif | `fonts/OmegaSerif-Regular.woff2` (weight 400) | `read` the file with `project/` in front | `var(--font-display)` | `omega-serif` |

**Read, per thing:** a component’s props, parts and examples: `api/components/<Comp>.md`; token values: `api/tokens.md`. After this README, fetch the cards and fonts you need in ONE message as parallel calls — none depends on another.

**Two rules.** Before you use a thing — a component, a token group, an icon, an asset — read its card from the index below; a value you did not read from a card is a guess. `tokens.json`, `manifest.json`, `components/index.d.ts` and `design-system.json` are sources for tools: hand them over. `components/<Comp>/README.md` is the long-form second read a card links to; `SKILL.md` and `artifact-type/` beside them are authoring guidance, not needed to consume the system.

## Index (generated — do not edit)

**Tokens**

- `api/tokens.md` — Every token: surface, text, fill, palette, type, spacing, radius, border. (10.4k)

**Components** (`api/components/<Comp>.md`, 5)

- **Actions**: `Button` — A button for one action, in three weights that follow the Psi and Omega shape rule
- **Status**: `Status` — One result, shown as an arch-topped pip with a word beside it: checked, trap or open
- **Layout**: `Tile` — A square-cornered block that holds one idea, drawn in outline for target-neutral things and solid for realized ones
- **Content**: `CodeBlock` — Code or a command on surface, under an ink rule across the top, set in code
- **Evidence**: `PlaceGrid` — One fact across the nine places, as a 3×3 grid of results in place order, with a count beneath it
