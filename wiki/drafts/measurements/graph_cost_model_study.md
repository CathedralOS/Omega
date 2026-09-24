# Graph cost-model study

Status: exploratory study of the question
[learned_optimization_policy.md](../reference/learned_optimization_policy.md) poses — whether
features of the compiler's typed operation/state graphs could rank optimization
candidates before expensive measurement. This study authorizes no
implementation and no model; it inventories the existing structural seams and
states what a measurement would have to show. Any future learned policy belongs
to the Omega-written product compiler and chooses through the existing
candidate and validation boundaries, not a second compiler path. Delete this
study once a measured workload corpus shows whether graph features beat
`predicted_cost_delta`, or when the learned-policy question closes.

Affected subject: [optimization selection and validation](../../spec/build/optimizations.md).

## The question

The reference compiler already declares an exact, deterministic candidate
surface. The empirical question is narrow: do graph features improve ranking
prediction over the scalar `predicted_cost_delta` the baseline policy consumes
today? Ranking is the only thing a model could ever do here — the spec's
nonauthoritative-policy section states that cost estimates rank candidates and
cannot establish eligibility or semantic correctness, and that independent
validation is what admits a candidate, not any producer or matcher.

## What exists today

The policy-visible row is already closed and exact:

- A `PsiRewriteCandidate` carries `predicted_cost_delta` (a signed scalar)
  alongside its bound identities
  (`omega-rust/omega/representations/optimization-unit/src/optimization_unit/rewrite/candidate/`).
- `validated_candidate_features` in the pass manager's external-policy seam
  projects a candidate only after the ordinary validator has admitted it,
  producing `ExternalCandidateFeatures` = validated summary (candidate
  identity, predicted cost delta) + required analyses + consumed facts
  (`.../04_abstract-operations-to-abstract-operations/src/pass_manager/external_policy/candidate_features.rs`).
  No graph shape crosses this boundary today.
- The model-free baseline is `min_by_key((predicted_cost_delta, candidate))`
  filtered to negative deltas, skipping as `NotProfitable`
  (`.../pass_manager/baseline.rs`).
- Supplied external decisions replay only on exact feature equality, and
  recording independently reconstructs the same surface
  (`external_policy/recording.rs`, `replay.rs`); decision rows bind input,
  candidate, verdict, validator, and budget as `BaselineDecisionRecord`s.

This is the structural readiness the spec requires: a future policy —
deterministic or learned — plugs into a boundary that already binds identity,
validation, and replay.

## Typed graphs available for features

Candidate features would come from the affected region of the typed graph each
stage already owns, computed deterministically from the immutable candidate
declaration and its declared analysis products:

- **Abstract-operations graphs** (consumed and produced by
  `abstract-operations-to-abstract-operations`): block/edge/operation
  structure within the candidate's affected region. Feature families: op-kind
  histogram, block and edge counts, backedge presence and loop-nesting depth,
  custody- and fuel-bearing edge counts, live-value density at the region
  boundary, region size.
- **Terminal Psi machine state graphs**: state/transition topology, fuel
  edges, entry/exit shape for candidates whose rule consumes the cyclic
  structure.
- **Optimization-unit region identity** already binds each candidate to the
  exact graph revision it was validated against — the feature projection can
  reference that identity rather than inventing a second region notion.

Feature extraction is a deterministic projection, never an oracle: every
feature must be recomputed by the replay path, so the same input revision
yields identical rows on any host.

## What a measurement would have to show

Per the source doc, agreement with recorded heuristic labels is not
optimization quality. A study-grade experiment needs:

- a versioned workload corpus with source-separated training, evaluation, and
  regression sets;
- identity-bound decision rows replayed exactly — the existing
  `ExternalDecisionAction`/`BaselineDecisionLog` machinery is the comparison
  substrate;
- measured execution, size, and compilation costs against the model-free
  baseline (`choose_baseline`), not prediction accuracy against the baseline's
  own choices;
- the cost of candidate checking at search scale treated as a measured
  quantity, not an assumption — a ranker that nominates more candidates still
  pays the independent-validation cost on each.

## Open questions

Which concrete workloads and cost objectives justify the work is undecided.
The useful boundary between high-level (source-level, custody/fuel) and
target-specific (instruction/pressure) features is open — graph features drawn
from abstract operations are target-neutral, while register-pressure shapes
belong to `selected-instructions` and later stages. How specialized variants
interact with code identity, deduplication, and component replacement is
likewise unresolved and is the neighboring board item's question.

## Conclusion

The structural substrate already exists: validated candidates, exact
feature projection, decision recording, and replay-on-equality are all
implemented seams. The gap the source doc names is not a missing model — it
is the missing evidence that graph features predict measured cost better
than `predicted_cost_delta` alone. That evidence requires the workload
corpus and instrumentation before any ranking model is worth building; this
study records the seam and the protocol and proposes no code.
