# BOUNDED-OPTIMIZATION-SEARCH — re-verification ledger

Re-verified on Linux x86-64 at `75650d2e94` (board tip at claim time) by the
w10-05 session on lane `zergling/z5` under claim ticket `db39f3e2`. The item
(bounded candidate search + revalidation at scale; merges
BOUNDED-CANDIDATE-SEARCH and CANDIDATE-REVALIDATION-AT-SEARCH-SCALE) is
authorization-gated.

## Authorization gate (unchanged)

- `wiki/drafts/learned_optimization_policy.md:7` — "any future investigation
  belongs to the Omega-written product compiler and needs its own concrete
  justification": a bounded model-free search over the validated candidate
  interface and any search-scale revalidation-cost study are far-future
  extensions gated on OMEGA-PRODUCT-COMPILER-SOURCE plus that justification.
  The source doc is unchanged since `f3be428d4ae`.
- `wiki/spec/build/optimizations.md` forbids trainer-side machinery (a
  trainer, training corpus pipeline, model evaluator, or inference path) in
  the Rust reference compiler.
- The versioned workload corpus + measurement protocol the evaluation needs
  are gated under WORKLOAD-CORPUS-AND-MULTIVERSIONING; comparison protocol
  scoped in `wiki/drafts/graph_cost_model_study.md`.
- OMEGA-PRODUCT-COMPILER-SOURCE remains open at TASKS.md — the Omega-written
  product compiler is still a lexer plus partial parser, so the gate's
  prerequisite has not arrived.

## The seam already exists (re-verified on this revision)

- `abstract-operations-to-abstract-operations/src/analyses/manager.rs` —
  `commit_revision(validate_retained)` cold-recomputes supposedly retained
  analyses and fails `UndeclaredInvalidation` on drift.
- `OptimizationWorkBudget` threaded through
  `validation/prephysical_manifest/{projection,validation}.rs` — all five
  axes (rule_evaluations, candidates, validation_steps, commits, iterations)
  bounded per pass.
- `pass_manager/` hosts `baseline.rs`, `external_policy/`, `accounting.rs`,
  `execution.rs` — deterministic `choose_baseline` over independently
  validated `ValidatedCandidateSummary` rows with duplicate-candidate
  rejection and strictly decreasing convergence measure; external_policy
  replays supplied decisions on exact context + row equality.
- `execution.rs` calls `validate_psi_rewrite_candidate` per iteration and
  reports `CandidateContractAxis::Input` for stale-input candidates.
- `cargo nextest run -p abstract-operations-to-abstract-operations --lib`:
  534/534 green at `75650d2e94` (Linux x86-64), including the
  `loop_invariant_scalar_motion` tests whose earlier failures GENERAL-LICM's
  leg repaired.

## Verdict

Authorization-gated: revalidation at scale is the existing behavior's
measured cost, not a missing mechanism, and the search itself needs
OMEGA-PRODUCT-COMPILER-SOURCE plus a concrete justification. No
implementable slice exists.
