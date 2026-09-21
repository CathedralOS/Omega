# LEARNED-OPTIMIZATION-COST-MODEL — record

Re-verified at `62a52db5ffd` on linux x86-64; re-verified unchanged at `94e764a6da` (same gate text, same residual owner).

## Status

The canonical board row was deleted by board sweep A (`091f5ba75c2`,
"55 resolved deep-mine rows with verified evidence") — its entry recorded
commits `50425f1c70`, `55ba7f6ab3`, `8a37f82686` plus the seam docs. The
assignment re-mines that consumed resolution.

## Gate still holds verbatim at this stamp

- `wiki/spec/build/optimizations.md` lines 206-211: "not an ML
  implementation… Do not build a trainer, training corpus pipeline, model
  evaluator, or inference integration in the Rust reference compiler."
- `wiki/drafts/learned_optimization_policy.md` line 6: "These notes
  authorize no implementation."
- The premature trainer stays removed (`55ba7f6ab3`); `predicted_cost_delta`
  remains the deterministic baseline (`pass_manager/execution.rs:417`).
- The gated residual (versioned workload corpus + measured comparison
  against the baseline) belongs to WORKLOAD-CORPUS-AND-MULTIVERSIONING,
  which is itself resolved-gated (`5b839c31ab`) on
  OMEGA-PRODUCT-COMPILER-SOURCE plus a concrete justification.

No linux_x86_64 implementable slice exists under this name. The alias row
LEARNED-COST-MODEL's stale pointer (TASKS.md:5889 reference, now deleted)
was updated to carry this record.

> Field note (163618c89557..b53c7ea26032 review): second ledger draft for the same gate as learned_optimization_cost_model_z142.md; keep one, fold the gate citation into LEARNED-COST-MODEL, delete the other.
