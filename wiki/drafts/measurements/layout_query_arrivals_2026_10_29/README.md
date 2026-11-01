# Layout-query arrival qualification

Qualification of the `size_align_layout_geometry` fact row: `size_of` and `align_of` on a two-field
`u64` record establish `size == 16u64` and `align == 8u64`, discharging `size - (align * 2u64)` at all
nine places. Compiler source `8b519ca647cd` at optimization level 1; native execution on macOS ARM64.

- Baseline `9d0ba50bdc86` lost the query ranges in nested-machine arithmetic, at the private arrival of
  ordinary call results, and at a terminal helper return. Over 35 fixtures the candidate lost 0 and
  gained 18.
- Source check: 35 fixtures (16 positive, 19 negative), all as expected.
- Sample `--both`: 12 of 12 positives passed; 11 of 11 static twins matched.
- Native against Terminal: 16 fixtures agreed with exit 0; 12 ran the same artifact on all four targets.
- Runtime falsifiers: 12 programs built with corrupted size and alignment (0 / 0 and 32 / 16); native
  execution signalled 5 and Terminal crashed with `Trap`.
- Golden guard: 0 lost keys, 9 added to `corpus_outcomes.txt`.
- Limits: no x86-64 macOS native claim; this repairs layout-query fact transport, not the whole
  `LIBRARY-REPRESENTATION-FAMILIES` acceptance.

Raw data (evidence record, per-leg observations and transcripts) was removed from the tree. It is at
[this folder at commit `6e5131c510b9`](https://github.com/CathedralOS/Omega/tree/6e5131c510b9e6227b4592652c9a5a39e350a970/wiki/drafts/measurements/layout_query_arrivals_2026_10_29),
starting with its [evidence.json](https://github.com/CathedralOS/Omega/blob/6e5131c510b9e6227b4592652c9a5a39e350a970/wiki/drafts/measurements/layout_query_arrivals_2026_10_29/evidence.json).
