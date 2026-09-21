# CANDIDATE-REVALIDATION-AT-SEARCH-SCALE — scope verification (2026-09-20, `9e80227d2b`)

Bare mined stub at `TASKS.md:7127`, already named a sibling re-mine of
the merged **BOUNDED-OPTIMIZATION-SEARCH** row (`TASKS.md:6187`,
resolved). Verified at `9e80227d2b`: the verdict stands.

## Verdict

Source doc `wiki/drafts/learned_optimization_policy.md` authorizes no
implementation — a search-scale revalidation-cost study is a far-future
extension gated on the Omega-written product compiler
(OMEGA-PRODUCT-COMPILER-SOURCE) plus concrete justification.

The seam it would measure already exists and re-verified at HEAD:

- every pass runs bounded on all five `OptimizationWorkBudget` axes
  (iterations, rule evaluations, candidates, validation steps, commits);
- `choose_baseline` selects deterministically over independently
  validated `ValidatedCandidateSummary` rows with duplicate-candidate
  rejection and a strictly decreasing convergence measure;
- `pass_manager/external_policy` replays explicitly supplied decisions
  on exact context and row equality;
- candidates are revalidated against the exact input revision they bind
  (`validate_psi_rewrite_candidate` per iteration —
  `pass_manager/execution.rs`; `CandidateContractAxis::Input` rejects
  stale-input candidates);
- `AnalysisManager::commit_revision(validate_retained)` cold-recomputes
  every retained analysis and fails `UndeclaredInvalidation` on drift
  (`analyses/manager.rs`).

Revalidation at scale is therefore the existing behavior's measured
cost, not a missing mechanism. The versioned workload corpus and
measurement protocol the study needs are gated under
WORKLOAD-CORPUS-AND-MULTIVERSIONING (live claim, Jarod /
swarm-w9-workload-corpus, 01:36Z), with the comparison protocol scoped
in `wiki/drafts/graph_cost_model_study.md`.

## Outcome

No code change — record only. Coordinator may fold the stub into
BOUNDED-OPTIMIZATION-SEARCH.
