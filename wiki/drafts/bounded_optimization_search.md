# BOUNDED-OPTIMIZATION-SEARCH — re-verification ledger

Re-verified on Linux x86-64 at `6f9a1f637ea7` (board tip at claim time) by
Zergling-126 under claim ticket `91ccea82`. The item (bounded candidate
search + revalidation at scale; merges BOUNDED-CANDIDATE-SEARCH and
CANDIDATE-REVALIDATION-AT-SEARCH-SCALE) is authorization-gated.

## Authorization gate (unchanged)

- `wiki/drafts/learned_optimization_policy.md:7` — "any future investigation
  belongs to the Omega-written product compiler and needs its own concrete
  justification": a bounded model-free search over the validated candidate
  interface and any search-scale revalidation-cost study are far-future
  extensions gated on OMEGA-PRODUCT-COMPILER-SOURCE plus that justification.
- `wiki/spec/build/optimizations.md:207` forbids trainer-side machinery in
  the Rust reference compiler.
- The versioned workload corpus + measurement protocol the evaluation needs
  are gated under WORKLOAD-CORPUS-AND-MULTIVERSIONING; comparison protocol
  scoped in `wiki/drafts/graph_cost_model_study.md`.

## The seam already exists (re-verified on this revision)

- `abstract-operations-to-abstract-operations/src/analyses/manager.rs:146` —
  `commit_revision(validate_retained)` cold-recomputes supposedly retained
  analyses and fails `UndeclaredInvalidation` on drift.
- `OptimizationWorkBudget` threaded through
  `validation/prephysical_manifest/{projection,validation}.rs` — all five
  axes bounded per pass.
- `pass_manager/` hosts `baseline.rs`, `external_policy/`, `accounting.rs`,
  `execution.rs` — deterministic `choose_baseline` over independently
  validated `ValidatedCandidateSummary` rows with duplicate-candidate
  rejection and strictly decreasing convergence measure; external_policy
  replays supplied decisions on exact context + row equality.
- The row's seam re-verification stands: `cargo nextest run -p
  abstract-operations-to-abstract-operations --lib` green at 18cebfa1062b
  (485/485; the earlier 7 `loop_invariant_scalar_motion` failures were
  repaired by GENERAL-LICM's leg).

## Verdict

Authorization-gated: revalidation at scale is the existing behavior's
measured cost, not a missing mechanism, and the search itself needs
OMEGA-PRODUCT-COMPILER-SOURCE plus a concrete justification. No
implementable slice exists.
