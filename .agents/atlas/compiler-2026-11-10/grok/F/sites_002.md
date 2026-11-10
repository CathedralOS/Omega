# sites_002

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| F0004 | omega-rust/psi/compilation/validation/src/contract_entailment.rs:1831 | SOUND | Difference-bound closure derives a strictly positive lower bound on an atom minus itself (read arithmetic_domains.rs:1577-1578). |
| F0005 | omega-rust/psi/compilation/validation/src/contract_entailment.rs:4930 | SOUND | USE aborts carrier-range fitting with `CarrierFit::Unknown` instead of `Proven` when hypotheses are marked unsatisfiable (read 4930-4931, 37780, 53620). |
| F0006 | omega-rust/psi/compilation/validation/src/contract_entailment.rs:5435 | SOUND | SET flags a dispatch arm whose assumed match polarity contradicts a boolean literal condition (read 5383-5388, 5434-5435). |

## F0004

**Cited location:** `omega-rust/psi/compilation/validation/src/contract_entailment.rs:1831` (column 43, `requires_unsatisfiable`). Lines match the input TSV in this checkout (read).

**Kind:** SET (`self.requires_unsatisfiable |= …`, read 1831).

**Enclosing function:** `difference_matrix::impl Engine::close_matrix` (read 1830-1832), invoked from `refresh_difference_bounds` (read 1787-1790) after `seed_matrix` records difference seeds from bounds, intervals, and unsigned atoms (read 1793-1827).

**What sets the flag:** `close_matrix` ORs `requires_unsatisfiable` with the boolean returned by `DifferenceBounds::close(ZERO_ATOM)` (read 1831; `ZERO_ATOM` at 6165).

**Contradiction facts:** `DifferenceBounds::close` (in `omega-rust/psi/compilation/validation/src/arithmetic_domains.rs`, read 1517-1562) unions atoms linked by stored difference edges, runs `close_members` to propagate combined bounds (read 1565-1585), and returns `unsatisfiable = true` when for some atom index `from == to` the combined bound is neither negative nor zero—i.e. a strictly positive integer lower bound on “from minus from” (read 1576-1578).

**Satisfiable hypotheses with flag set:** Under the engine’s encoding, each matrix atom is a distinct difference-logic variable unless connected only through recorded bounds; if every stored edge is a sound lower bound on the corresponding atom difference, a positive reflexive bound is not satisfied by any integer assignment (read 1577-1578; difference-closure standard, read). I did not read a complete soundness proof for every path that calls `matrix.record_indices` / `record` (inferred gap only for over-approximated bounds, not observed).

**Downstream effect:** With `requires_unsatisfiable` true, other sites treat arithmetic obligations as vacuously discharged (e.g. `engine.requires_unsatisfiable \|\| engine.prove_at_least(…)` read 23648; entailment `proven.entry.extend_from_slice(&ensures)` read 90907-90909). This site only records contradiction in the matrix layer.

## F0005

**Cited location:** `omega-rust/psi/compilation/validation/src/contract_entailment.rs:4930` (column 49). Lines match the input TSV (read).

**Kind:** USE (`if self.requires_unsatisfiable {`, read 4930).

**Enclosing function:** `strict_bindings::impl Engine::integer_value_fit` (read 4921-4966).

**Goal affected:** Whether a substituted polynomial `value` is proven, refuted, or left unknown to lie in `[minimum, maximum]` (with optional exclusivity / address-model special case, read 4933-4965). When the flag is set, the function returns immediately with `CarrierFit::Unknown` (read 4930-4931) and does not reach the branches that return `CarrierFit::Proven` (read 4935-4941) or `CarrierFit::Refuted` (read 4943-4950).

**Read upward:** Callers include `scoped_arithmetic_value_fits_for_declaration` (read 37777-37786), which already bails out of interval extraction when `install_hypotheses` leaves `requires_unsatisfiable` (read 37737-37738), and structural integer fitting that calls `observations.engine.integer_value_fit` after installing premises (read 53615-53626). A contradictory hypothesis context therefore does not yield `CarrierFit::Proven` for range fitting through this path (read).

**Vacuous prove elsewhere:** The same flag on other paths does turn comparisons or implications into proven (e.g. `StrictArithmeticImplicationJudgment::Proven` read 37554-37555; `proof_integer_formation` read 23648). This site explicitly does not do that; it withholds a fit judgment.

**Verdict rationale:** Even if `requires_unsatisfiable` were ever set incorrectly, this USE returns `Unknown` rather than `Proven`, so it does not wrongly certify carrier fit here (read 4930-4931). When the flag is correctly set for unsatisfiable hypotheses, refusing to prove range membership is consistent with impossible premises (read).

## F0006

**Cited location:** `omega-rust/psi/compilation/validation/src/contract_entailment.rs:5435` (column 59). Lines match the input TSV (read).

**Kind:** SET (`self.requires_unsatisfiable |= value != positive`, read 5435).

**Enclosing function:** `strict_bindings::impl Engine::collect_condition_comparisons` (read 5427-5486), called from `install_dispatch_selection` when the dispatch condition is a `DispatchCondition::Predicate` (read 5377-5388).

**Contradiction facts:** For `ExpressionNode::Boolean(value)`, the engine is collecting comparisons under the assumption that the condition holds with polarity `positive` (for predicates, `positive` is `selection.matched`, read 5383-5386). If the literal is `true` but `positive` is `false`, or the literal is `false` but `positive` is `true`, then `value != positive` and the flag is set (read 5434-5436). That is the standard contradiction between assuming a literal boolean and assuming its negation in the match/non-match arm.

**Satisfiable hypotheses with flag set:** None for this rule alone: any assignment satisfies at most one of {condition true, condition false}. No aliasing or integer comparison is involved at this site (read).

**Downstream:** After collection, `install_hypotheses(comparisons)` may add further constraints (read 5388); with the flag already set, `requires_unsatisfiable` remains true through later vacuous-proof paths (read 37554-37555, 90907-90909 inferred from flag persistence on same engine).

## What I did not read

- Full `LowerBoundQueries` / `prove_at_least` implementation and every matrix seeding path beyond `difference_seeds` and `install_hypotheses` (omega-rust/psi/compilation/validation/src/contract_entailment.rs:6664-6743, read partially).
- `git show 457adf4a11f:…` digest snapshot (inferred unnecessary; line numbers matched).
- Tests under `#[cfg(test)]` for difference matrix or dispatch selection.
- Sibling field `hypotheses_contradictory` and its structural-judge wiring (search.txt lists separate sites).

SITES COMPLETE
