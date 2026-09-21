# LEARNED-OPTIMIZATION-COST-MODEL — verify + record (z142)

Freeform item (the marked row is the sibling alias LEARNED-COST-MODEL,
TASKS.md:10351, which names this item as the live row). Authorization
gate, verified.

## Verified at `72fc66d6c3` (linux x86-64)

Nothing to implement by design:

- `wiki/spec/build/optimizations.md:207` — "Do not build a trainer,
  training corpus pipeline, model evaluator, or inference path" in the
  Rust reference compiler; the premature trainer was removed at
  `55ba7f6ab3`.
- `wiki/drafts/learned_optimization_policy.md` — "These notes authorize
  no implementation; any future investigation belongs to the
  Omega-written product compiler and needs its own concrete
  justification."
- Tree census (`trainer|training_corpus|cost_model|predicted_cost`
  under `omega-rust/`): hits are the deterministic candidate interface
  (optimization-core decisions/external_schema, optimization-unit
  evidence/candidate model) — selection records and the declared
  catalog-driven cost model named by optimizations.md:197/240 — not a
  learned model. No trainer machinery exists.

## Residual (gated)

The only named residual — a versioned workload corpus plus a measured
comparison against `predicted_cost_delta` (scoped in
`wiki/drafts/graph_cost_model_study.md`) — waits on
WORKLOAD-CORPUS-AND-MULTIVERSIONING and OMEGA-PRODUCT-COMPILER-SOURCE,
both still open. Today's versioned workload surface is BENCHMARKS'
`tools/benchmark` records.

## Verdict

**Resolved / record-only** — authorization gate; the spec forbids the
machinery this item names and the policy doc authorizes none. Same
verdict as siblings LEARNED-COST-MODEL (`05416dd1a0`),
GRAPH-COST-EVIDENCE-CORPUS, OPTIMIZATION-WORKLOAD-CORPUS,
WORKLOAD-MULTIVERSIONING.

> Field note (f5eca6b2b0bd..5d9afb85f822 review): ledger-only re-verification of an authorization-gated row; fold the verdict into LEARNED-COST-MODEL when its fence opens and delete this draft rather than re-stamping.
