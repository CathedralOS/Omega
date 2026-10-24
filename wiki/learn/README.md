# Learn Omega — a field guide for the agent era

A single-page mini-book that teaches how the Omega language works and the
reasoning behind it — organized around *reading* Omega, since the reader's real
job is supervising the AI agent that writes it.

## Run

```bash
bun build.ts && bun serve.ts
# → http://localhost:3020
```

## Structure

- `src/chapters.ts` — all content. Every code block is real `.omg`; the boundary
  trait is quoted from `source/library/std` in `C:\SoftwareDevelopmentKits\Omega`.
  All examples are receiver-free — Omega has no `self`; receivers are
  explicit parameters.
- `tools/verify.ts` (`bun run verify`) — compile-checks every code figure
  against `omega --check` in persistent fixture dirs under `scratch/verify/`
  (each figure's `omega.lock` survives, keeping the std snapshot warm),
  completes the `omega update` package-review ceremony when the compiler asks
  for entry-binding acceptance, and pins every marginalia key to a real code
  line. The build-machine figure verifies as a real `build.omg` + `main.omg`
  pair.
- `build.ts` — renders a single `index.html` (all 11 chapters on one continuous
  page; rail, TOC, and stone footers are in-page anchors) and stages a clean
  deploy-ready static bundle in `dist/` (~1 MB, 25 files: only `index.html`,
  `styles.css`, `book.js`, and the referenced `fonts/` and `assets/plates/`
  files, excluding the 4.2 MB source PNG sheets). Plate `width`/`height` come
  from the WebP headers, not constants.
- `book.js` — marginalia alignment (notes sit level with their code line),
  note↔line hover pairing, rail scrollspy. Without JS the book still reads:
  circled keys bind notes to lines statically.
- `styles.css` — field-guide grammar: left chapter rail, annotated staff
  (artifact left · keyed guide notes right), stone prev/next.
- `assets/characters/om/` — canonical Om turnaround + cast lineup (Codex
  image_gen). `assets/plates/` — watercolor vignettes + paper ground.
  Provenance in `assets/plates/PROVENANCE.md`.
- `fonts/` — self-hosted Source Serif 4 + Hanken Grotesk woff2.

## Design lineage

Comp C ("annotated spread") from `work/omega-learn/` in cinematic-director —
dealt by `impeccable concept-seed`, approved by operator delegation, comp and
prompts recorded under `.impeccable/mocks/` + `.impeccable/build/`.

## Chapters

I. A Program Is a Machine · II. States, Not Ifs · III. Fields Are the Memory ·
IV. Choice Is a Value Too (case types + guard-list dispatch) · V. Writing for
the Prover (incl. a verbatim refusal) · VI. The Boundary Wall · VII. The Build
Machine (build.omg) · VIII. Text Is a Block · IX. --check Is the Floor (incl.
the exit-code convention) · X. Working With the Agent
(brief patterns + review checklist)
