# slice_0093

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0712 | UV:8246 | REACHABLE | `TransformContent` hits `continue` and never runs the issuer-requirements additions loop at UV:8248-8279. |
| A0713 | UV:8770-8772 | REACHABLE | Non-unit callees make `reference_result_memberships_match` return `true` without checking result projected qualifications. |
| A0714 | UV:8868-8870 | REACHABLE | Nonempty projected qualifications on the view result are accepted when payload evidence is `Empty`. |
| A0715 | UV:9999-10029 | UNDECIDED | `None` from edge mapping becomes `ReachingWrite::Unknown` in the atomic kernel; I did not show a accepting path for a provably incoherent unit. |
| A0716 | UV:10208-10210 | REACHABLE | Case-segment entry claim paths may be omitted from transfers while `validate_slot_claim_transfers` still returns `true`. |
| A0717 | UV:10220 | CLOSED | Every caller pairs `zip` with `structural_arguments_match`, which rejects length mismatch at UV:13217-13218 first. |
| A0718 | UV:10749 | CLOSED | The sole production caller is inside a `CallStructural` arm, so the non-structural early `true` is dead on that path. |
| A0719 | UV:10803 | REACHABLE | `exact_claim_free` skips multiplicity claim-shape rules at UV:10812-10819 after catalog validation only. |

## A0712

**Trace (read).** In `validate_structural_root_operations` (`UV:7190-7201`), the `O::MoveStructuralValue` / `O::TransformContent` arm (`UV:8093-8103`) runs payload and membership checks. For `TransformContent`, `validate_contract`, `validate_memberships`, and `validate_parent_memberships` run (`UV:8181-8244`), then `continue` at `UV:8246`. `MoveStructuralValue` falls through to the additions loop (`UV:8248-8279`) that rejects transfers adding domains with unsatisfied issuer establishment routes (`UV:8277-8279`). `validate_memberships` (`content_transformations.rs:2030-2082`, read) only checks equality of result qualifications to inherited plus derivation outputs; it does not call `missing_membership_requirements` like the additions loop (`UV:8265-8275`).

**Input.** An optimization unit whose function contains a `TransformContent` on an owned carrier whose derived result memberships satisfy `validate_memberships` but include a domain whose issuer requirements are not established (the additions loop would reject the same qualifications on `MoveStructuralValue`).

**Callers.** `validate_structural_root_operations` is called from `validate_function` at `UV:9610-9618` (read). Search: `rg '\bvalidate_structural_root_operations\b' omega-rust` → 2 matches (1 definition, 1 call) (read).

## A0713

**Trace (read).** `reference_result_memberships_match` (`UV:8754-8820`): on `CallStructural` with a structural result, if `callee.machine().and_then(|m| unit.function(m))` is `None` (`UV:8770-8771`), the function returns `true` without comparing `result.projected_qualifications` to the callee signature or running preservation analysis (`UV:8776-8819`).

**Input.** A function node with a structural call to a boundary or dynamic callee (no in-unit `PsiOptimizationFunction`) where the call result lists projected qualifications that would not match a resolved callee signature.

**Callers.** `UV:7308-7314` inside `validate_structural_root_operations`; failure returns `catalog_mismatch` (`UV:7315`). Search: `rg '\breference_result_memberships_match\b' omega-rust` → 2 matches (read).

## A0714

**Trace (read).** `view_memberships_match` (`UV:8822-8894`): for `EstablishElementView` / `ElementViewSubslice`, if `result.projected_qualifications` is nonempty (`UV:8835-8837`), evidence is computed via `element_payload_memberships_in` (`UV:8852-8864`). `match evidence` (`UV:8866-8893`): `None` → `false`; `ElementPayloadMemberships::Empty` → `true` (`UV:8868-8869`) even when projected qualifications were nonempty; `Present` checks `AllElements` paths (`UV:8876-8892`).

**Input.** An element-view establishment whose result carries projected qualifications while descriptor analysis reports empty payload memberships (no matching element payload contract).

**Callers.** `UV:7301-7307`; failure → `catalog_mismatch`. Search: `rg '\bview_memberships_match\b' omega-rust` → 2 matches (read).

## A0715

**Trace (read).** `validate_function_atomic_coherence` (`UV:9971-10067`) passes a `projection_before_edge` closure (`UV:9999-10028`) into `happens_before_atomic_coherence_violation` (`atomic.rs:414-506`, read). The closure returns `None` when parameter bindings on an edge are not exactly one (`UV:10008-10016`), when conflicting projections map to different storage (`UV:10023-10024`), or when blocks/predecessors are missing (`UV:10000-10001`). In `reaching_writes` (`atomic.rs:581-584`, read), `None` inserts `ReachingWrite::Unknown` into `reached`. `residency_edge_violation` (`atomic.rs:679-754`, read) compares `reached` sets to claimed frontiers; `Unknown` participates in exact set inequality checks (e.g. `atomic.rs:720`, `722-726`, `750`).

**Undecided.** I did not trace every `AtomicResidencyEdge` variant against a concrete malformed unit to prove acceptance (exit `Ok` at `UV:10065`) when coherence is genuinely violated; digest entry said the kernel was "not read" — re-located at current checkout, lines match digest `UV:9999-10029`.

**Callers.** `validate_values_and_bindings` at `UV:14470-14475` (read); called from per-function validation pipeline.

## A0716

**Trace (read).** `validate_slot_claim_transfers` (`UV:10178-10212`): requires transfers to cover owned arguments and listed `entry_claim_paths` except when `expected.path` contains a `Case` segment (`UV:10206-10210`), in which case the path need not appear in `paths` (`UV:10183-10186`).

**Input.** A dynamic `CallStructural` (`UV:12483-12546`) whose slot `entry_claim_paths` includes a claim path with a `Case` segment but `claim_transfers` omits that path; other conjuncts at `UV:10187-10205` can still hold.

**Callers.** `operation_structural_call_contract_matches` at `UV:12532-12536`; failure makes `validate_node_operation_contracts` reject via `StructuralCallContractMismatch` (`UV:10461-10478`). Search: `rg '\bvalidate_slot_claim_transfers\b' omega-rust` → 3 matches (1 def, 1 `pub(crate) use`, 1 call) (read).

## A0717

**Trace (read).** `validate_internal_claim_transfers` (`UV:10214-10317`) uses `arguments.iter().zip(&callee.structural_parameters)` at `UV:10220` for the non-owned access path comparison. Production callers: `UV:12481` (with `structural_arguments_match_at_invocation` at `UV:12473-12480`) and `UV:12688` (with `structural_arguments_match` at `UV:12680-12687`). `structural_arguments_match_at_invocation` returns `false` when `arguments.len() != parameters.len()` (`UV:13217-13218`, read) before any `zip` runs.

**Closing check.** Mismatched argument arity never reaches the truncating `zip`; only equal-length slices are zipped.

**Search.** `rg '\bvalidate_internal_claim_transfers\b' omega-rust` → 4 matches (1 def, 1 import, 2 call sites) (read).

## A0718

**Trace (read).** `payloadless_selected_evidence_surface_matches` (`UV:10735-10757`): non-`CallStructural` operations hit `else { return true; }` at `UV:10747-10749`. For structural calls, nonempty `selected_evidence` requires `exact_payloadless_structural_call` and callee contract ensures (`UV:10751-10756`).

**Closing path.** Only call site `UV:12624-12626` (read) sits inside the `O::CallStructural { result: AbstractStructuralCallResult::Structural { ... }` arm of `operation_structural_call_contract_matches` (`UV:12586-12641`); the operation is always `CallStructural` there, so `UV:10748-10749` is not taken on the acceptance path (inferred from control flow, read).

**Search.** `rg '\bpayloadless_selected_evidence_surface_matches\b' omega-rust` → 3 matches (1 def, 1 import, 1 call) (read).

## A0719

**Trace (read).** `validate_structural_call_result` (`UV:10759-10821`): signature and catalog checks through `UV:10801`; then if `exact_claim_free` is `true`, `return true` at `UV:10803-10804` before multiplicity-specific claim path rules (`UV:10812-10819`). `exact_claim_free` is the OR of plain-call predicates at `UV:12616-12620` inside `operation_structural_call_contract_matches` (`UV:12613-12622`).

**Input.** A plain structural call classified by one of those predicates (e.g. `exact_plain_affine_structural_call`, `UV:9841-9903` read) whose result multiplicity is affine but `result.claims` include an empty path claim (would fail `UV:10816-10817` if `exact_claim_free` were false).

**Callers.** Single production call `UV:12613`. Search: `rg '\bvalidate_structural_call_result\b' omega-rust/omega/compilation/representations/abstract-operations` → 3 matches in `unit_validation.rs` (read).

## What I did not read

- Full bodies of `element_payload_memberships_in`, `reference_call_preserved_memberships`, and `validate_contract` beyond signatures (not required to decide the cited branches).
- `wiki/spec` text on whether Case claim paths must transfer (spec not opened).
- Test modules under `atomic.rs` (`atomic.rs:756+`).

SLICE COMPLETE
