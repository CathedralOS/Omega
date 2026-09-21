# OPTIMIZATION-WORKLOAD-CORPUS — verify + record (z142)

Marked row (TASKS.md:11309; second unmarked stub at :11477) — already
scope-verified at `7a5a87d5a1` and re-verified at `f72122f71e4`:
authorization gate, folds into WORKLOAD-CORPUS-AND-MULTIVERSIONING.

## Re-verified at `832c55e69b` (linux x86-64)

The gate stands:

- `wiki/drafts/learned_optimization_policy.md` — unchanged since
  `f3be428d4ae` (last commit on the file); "authorize[s] no
  implementation; any future investigation belongs to the Omega-written
  product compiler and needs its own concrete justification."
- `wiki/spec/build/optimizations.md:207` — "Do not build a trainer,
  training corpus pipeline, model evaluator, or inference path" in the
  Rust reference compiler (premature trainer removed `55ba7f6ab3`).
- Both gate dependencies still open: OMEGA-PRODUCT-COMPILER-SOURCE
  (TASKS.md:6955, live claim z138-opcs exp 15:55Z) and
  WORKLOAD-CORPUS-AND-MULTIVERSIONING's sibling leg (WORKLOAD-
  MULTIVERSIONING live claim Zergling-126 exp 11:11Z).
- Today's versioned workload surface remains BENCHMARKS'
  `tools/benchmark` records; the `predicted_cost_delta` comparison
  protocol stays scoped in `wiki/drafts/graph_cost_model_study.md`.

## Verdict

**Resolved / folds — record-only.** The corpus is a far-future
extension gated on the product compiler plus a concrete justification;
no independent slice exists under this name. Same verdict as siblings
WORKLOAD-CORPUS, GRAPH-COST-EVIDENCE-CORPUS,
LEARNED-OPTIMIZATION-COST-MODEL, WORKLOAD-MULTIVERSIONING. Coordinator:
fold this stub (and the unmarked :11477 duplicate) into
WORKLOAD-CORPUS-AND-MULTIVERSIONING.

> Field note (163618c89557..b53c7ea26032 review): re-verification record of a row already folded into WORKLOAD-CORPUS-AND-MULTIVERSIONING; fold and delete rather than re-stamping.
