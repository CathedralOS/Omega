# Area 05 open-entry trace

Read-only trace of every entry classed "open" in `.agents/atlas/compiler-2026-11-10/digests/area_05.md`. Citations were checked against the current tree; `git diff --stat 457adf4a11f HEAD` for these crates was empty before this branch, so the digest line numbers still match. Nothing was built or run.

This file is updated as entries are decided. Entries not listed in the table are not reached yet.

| # | Site | Verdict | Reason |
| --- | --- | --- | --- |
| 1 | `candidates.rs:1581` | CLOSED | Jump arguments already dominate the predecessor, and the fused node is revalidated. |
| 2 | `candidates.rs:1813` | CLOSED | The ordinal loop cannot drop an active function; the digest's missing-function case does not occur. |
| 3 | `candidates.rs:1834` | CLOSED | A reference the validator misses makes the candidate disagree with the only proposer, so nothing is deleted. |
| 4 | `candidates.rs:4225` | UNDECIDED | The exact obligation is matched by id, not re-proved against the arithmetic. |
| 5 | `candidates.rs:7315` | CLOSED | A structural case has no scalar payload operand; a leftover runtime index fails revalidation. |
| 6 | `candidates.rs:9824` | CLOSED | The call stays; a returned value is constant only if every feasible path agreed. |
| 7 | `candidates.rs:2274` | CLOSED | Skipping the store leaves the removed parameter in the use list, so revalidation rejects it. |
| 8 | `candidates.rs:10686` | CLOSED | Those tables are not rewritten except the prune roster, which is validated. |
| 9 | `current_ownership.rs:204` | UNDECIDED | A failed descriptor graph clears effects; not every failure is shown to be rejected later. |
| 10 | `current_ownership.rs:553` | UNDECIDED | Prefix fallback cannot authorize an extension of the same path; canonical locations were not read. |
| 11 | `current_ownership.rs:1069` | CLOSED | Scalar returns never carry moved places, and a structural return is not in this pattern. |
| 12 | `current_ownership.rs:1160` | CLOSED | Binding and parameter counts are required to be equal before this zip. |
| 13 | `current_ownership.rs:1620` | CLOSED | A missing or empty target fails validation, so the dropped edge never accepts a unit. |
| 14 | `current_ownership.rs:2207` | UNDECIDED | Non-owned sources skip owner liveness; the earlier transfer check was not finished. |
| 15 | `current_ownership.rs:2202` | CLOSED | Same equal-length check as entry 12. |

## 1. `candidates.rs:1581-1586`

**CLOSED.** Digest description is right that the closure is always true, and wrong that this skips a check the fusion needs.

Searches: `checked_scalar_substitutions(` — 3 calls (`candidates.rs:557`, `:720`, `:1581`) and the definition at `:2007`. `validate_shared_jump_fusion_candidate` — definition `:1720`, re-export `:1970`, match arm `:10393`. `validate_psi_rewrite_candidate(` — 1 caller, `pass_manager.rs:603`.

Read in full: shared-jump `replay::validate` (`candidates.rs:1528-1647`), `independently_post_dominates` (`:1649-1718`), `checked_scalar_substitutions` (`:2007-2036`), `independently_replacement_dominates_uses` (`:3304-3350`), `admit` / `admit_origin` (`:418-464`), `validate_node_uses` (`unit_validation.rs:14335-14396`), `accept_region_rewrite` (`candidates.rs:10662-10699`), `revalidate_rewrite_output` (`unit_validation.rs:18213-18227`), `validate_function` (`:9546-9673`). Read the `Jump` arm of `direct_uses` (`construction.rs:713`) and the call in `pass_manager.rs:584-603`. Did not read the rest of `direct_uses` or the rest of the pass manager.

`admit` (`candidates.rs:447`) calls `validate_psi_optimization_unit` (`:422`) before the replay. The fused node replaces the predecessor jump (`:1643`). The substitution arguments are that jump's binding arguments. `direct_uses` records those arguments as uses (`construction.rs:713`), and `validate_node_uses` (`unit_validation.rs:14351-14393`), reached from `validate_function` (`:9647`) via `index_and_validate_unit_catalogs` (`:1233`), already required each of them to dominate the predecessor. `accept_region_rewrite` (`candidates.rs:10680`) runs `revalidate_rewrite_output` (`unit_validation.rs:18225`), which runs the same check on the fused return. The always-true closure only skips the merge-path test that the replacement dominate every use of the target parameter (`candidates.rs:562-568`). Fusion rewrites only the cloned terminal (`:1605-1610`), not those other uses.

## 2. `candidates.rs:1813-1832`

**CLOSED.** The digest is wrong: a function is not dropped from the map, and the index at `:1783` does not panic for an active machine.

Searches: `validator_active_source_ordinals` — 2 matches in `candidates.rs` (call `:1778`, definition `:1813`). `source_ordinal` under `optimization-unit/` — no match in `unit_validation.rs`. The producer has a separate copy, `rule_active_source_ordinals` (`control_flow_cleanup.rs:1299`).

Read in full: `validator_active_source_ordinals` (`candidates.rs:1813-1831`) and its caller in `replay::validate` (`:1750-1811`); the machine-roster checks (`unit_validation.rs:1114-1151`); `rule_active_source_ordinals` (`control_flow_cleanup.rs:1299-1316`). Read `PsiTransformationLedger::new`'s ordinal loop (`ledger.rs:122-146`). Did not read the rest of the ledger codec.

The loop runs `functions.len() + pruned_machines.len()` ordinals (`candidates.rs:1823`). The pruned map has at most one key per pruned row. Each distinct in-range key suppresses one slot, so the number of slots that take the next function is at least the number of functions. Every active function is inserted. `active.next()` being `None` only skips surplus ordinals when a pruned ordinal is duplicated or outside that range, and those surplus ordinals are never indexed. Duplicate machine ids are rejected earlier (`unit_validation.rs:1114-1121`), and `admit` runs that validation first. The producer `expect`s a function on every non-pruned slot (`control_flow_cleanup.rs:1311-1313`), so a surplus slot panics in the proposer instead of producing a candidate.

## 3. `candidates.rs:1834-1879`

**CLOSED.** The digest is wrong that a machine the validator's walker misses would be pruned.

Searches: `unreachable_private_machine_complement` — 1 call (`candidates.rs:1760`) and the definition (`:1834`). `operation_machine_references(` — definition `lib.rs:252`, validator call `candidates.rs:1898`, producer call `control_flow_cleanup.rs:1355`. `call_machine_references(` — definition `lib.rs:225`, call-graph builder `control_flow_analyses.rs:100`. `validate_psi_rewrite_candidate(` — 1 caller, `pass_manager.rs:603`. `MachineId` in `operations.rs` — the hits listed below.

Read in full: `unreachable_private_machine_complement` (`candidates.rs:1834-1878`), `validator_machine_references` (`:1881-1900`), `operation_machine_references` (`lib.rs:252-281`), `call_machine_references` (`lib.rs:225-249`), `call_graph` (`control_flow_analyses.rs:85-119`), `rule_unreachable_private_machine_complement` (`control_flow_cleanup.rs:1320-1366`), `invoked_machine` (`ownership.rs:293-297`). Read `InvokeNoNormal` (`operations.rs:658-666`), `BoundaryCall` (`:428-436`), `Call` (`:449-457`), `EstablishNativeCodeLayout` (`:105-109`), `StructuralArgument` (`types_and_values.rs:2734-2738`), `CrashRouteBucket` (`proof.rs:283-286`), `ProofQuestion` (`optimization-unit` `proof.rs:192-204`). Did not read every `AbstractOperation` variant body; the `MachineId` search is the enumeration.

The validator marks a machine reachable from the entry, every dispatch row, every provider candidate, every attached function (`candidates.rs:1842-1858`), plus `InvokeNominal` cleanup machines and `operation_machine_references` (`:1887-1898`). That helper sees `EstablishNativeCodeLayout`, `Call`, `CallStructural`, and `SelectImplementation` (`lib.rs:256-270`). It does not see `InvokeNoNormal`. The only proposer builds its set from the call graph, which does see `InvokeNoNormal` (`lib.rs:230-231`, `control_flow_analyses.rs:93-100`) and the same cleanup and `operation_machine_references` edges (`control_flow_cleanup.rs:1347-1356`). Its unreachable set is therefore a subset of the validator's. The validator accepts only when the candidate's machine list equals its own set (`candidates.rs:1771-1776`). If the callee is referenced only by `InvokeNoNormal`, the lists differ and the rewrite is rejected, so the callee stays. When the lists are equal, the deleted machines are absent from that larger edge set. `BoundaryCall` and `ExternalCompletion` name a `BoundaryMachineId`, not a function machine (`operations.rs:431`, `:669`).

## 4. `candidates.rs:4225-4249`

**UNDECIDED.** The digest is right that the compatible-policy key drops the overflow policy.

Searches: `independent_compatible_policy_scalar_leader` / `_redundant` — used from the CSE admission match (`candidates.rs:4151-4166`) and defined at `:3366` and `:3386`. `independently_accepted_operation_fact` — definition `:3189`. `IntegerArithmetic` in `unit_validation.rs` — fact index `:5945` and scalar-type match `:12165` only.

Read in full: `compatible_policy_scalar_row` (`candidates.rs:3408-3464`), `arithmetic_key` (`:3487-3516`), `shift_key` (`:3466-3485`), the proof-class match (`:4182-4255`), `independently_accepted_operation_fact` (`:3189-3217`), `accepted_obligation_fact` (`proof.rs:127-136`), `IntegerOverflowPolicy` and `IntegerArithmetic::new` / `evaluate` (`integer_arithmetic.rs:24-207`), the fact-index arms for arithmetic and shifts (`unit_validation.rs:5937-5956`), and the exact-add type arm (`:12165-12186`). Did not read the pass that first stores `AcceptedObligationFact`, and did not read `exact_add` against `saturating_add`.

A leader must carry no obligation and the redundant operation must carry one (`candidates.rs:4225`). For add, subtract, and multiply, wrapping and saturating both produce no obligation, and exact produces one (`integer_arithmetic.rs:54-58`, `candidates.rs:3448-3453`). Divide and remainder never get a key (`:3512-3513`). The exact side is accepted when the function's fact index names that obligation and `accepted_obligation_facts` has a row with the same machine, operation, and obligation id (`candidates.rs:3195-3216`, `proof.rs:133-135`). Unit validation rebuilds the fact index from the obligation id (`unit_validation.rs:5945-5955`) and does not compare the fact's proposition bytes to an in-range goal. The scalar-type check admits an exact multiply of a fixed-width type without evaluating the operands (`:12183-12185`).

What would decide it: the code that creates `AcceptedObligationFact` for an exact add, subtract, multiply, or shift, and whether that proposition is still required to mean "the mathematical result fits in the carrier" at the moment this rewrite runs. If it is, wrapping and saturating of the same operands match the exact result and this arm is harmless. If a fact can sit on an exact operation whose operands can overflow, a saturating or wrapping leader replaces that operation and the values can differ.

## 5. `candidates.rs:7315`

**CLOSED.** The digest is wrong that a structural case carries a scalar use which would then be accepted unchanged.

Searches: `rewrite_scalar_value_uses(` — 7 calls (`candidates.rs:624`, `:795`, `:3087`, `:4409`, `:4573`, `:4919`, `:6021`) and the definition at `:7293`. Six write the output inside `accept_region_rewrite` (`:585`, `:746`, `:3066`, `:4370`, `:4897`, `:6005`). The call at `:4573` rewrites a clone used only to recompute an expression key (`:4579-4585`). `fn runtime_indices` — definition `operations.rs:955`.

Read in full: `rewrite_scalar_value_uses` (`candidates.rs:7293-7321`), `rebuild_node_metadata` (`:10511-10528`), `sweep_output_nodes` (`:10566-10579`), `AbstractStructuralCaseSuccessor` and `AbstractStructuralCasePayloadBinding` (`control_flow.rs:123-137`), `StructuralCase` (`operations.rs:618-622`), the `StructuralCase` arm of `structural_paths` (`operations.rs:1052-1058`), `runtime_indices` (`:955-960`), `StructuralPathSegment::RuntimeIndex` (`types_and_values.rs:1548-1551`). Read the six output call sites enough to see `accept_region_rewrite`, and read `:4564-4585` for the clone. Did not read the phi-translated validator above `:4564`.

A case payload binding stores the destination parameter id, a field id, and a scalar type. It has no value operand (`control_flow.rs:133-137`). The value ids a case can hold are `RuntimeIndex` indexes on the scrutinee path and on structural-binding paths (`operations.rs:1052-1058`, `types_and_values.rs:1548-1550`). `runtime_indices` collects those, and `operation_uses` appends them (`construction.rs:612-619`). `sweep_output_nodes` rebuilds `node.uses` from that list (`candidates.rs:10575`). `accept_region_rewrite` then revalidates (`:10680`). If an output rewrite deleted the value and the case still names it, `validate_node_uses` reports an undefined value (`unit_validation.rs:14344-14349`). If the definition was not deleted, the unre-written index still names the original value. The clone at `:4573` is not written back; a key that no longer matches returns `CandidatePatchMismatch` (`:4585`).

## 6. `candidates.rs:9824-9851`

**CLOSED.** The digest is wrong that the replay cannot catch a shared mistake. The validator recomputes the constant itself.

Searches: `fn call_result_constant` — two definitions, `candidates.rs:9824` and `specialization.rs:478`. Callers: `candidates.rs:9744` (1) and `specialization.rs:410` (1). `plan_dispatch(` — definition `:9643`, caller `:10155`. `cyclic_machines(` — calls `:238`, `:3039`, `:10147`.

Read in full: validator `call_result_constant` (`candidates.rs:9824-9851`), producer `call_result_constant` (`specialization.rs:478-509`), `cyclic_machines` and `has_cycle` (`candidates.rs:9584-9641`), state-specialization `validate` (`:10126-10188`). Read `plan_dispatch` from `:9643` through `:9759`, and `validator_scalar_constant_facts` from `:9223` through `:9428`. Did not read `delivered_argument` beyond the copy at `:9784-9821`, and did not read `fused_node` past the jump arm (`:9853-9927`).

`validate` rejects a caller with a block cycle (`:10147`) and requires the candidate's edges to equal `plan_dispatch` (`:10160`). That plan calls `call_result_constant` only after local constant facts miss (`:9742-9744`). The helper keeps a callee only when it has exactly one `Return`, then asks the validator's own constant propagation for that returned value. Function parameters start overdefined (`:9257-9258`), and a value is reported only when every merge left it constant (`:9400-9402`). A result that depends on an argument or on a recursive input does not stay constant. The rewrite replaces the predecessor terminator (`:10173-10186`), not the call, so the callee's other effects still run. When the call does return, that unique return value is the call result.

## 7. `candidates.rs:2274-2277`

**CLOSED.** The digest is right that one rewriter skips the store. That skip does not accept the candidate.

Searches: `rewrite_block_parameter_operation(` — definition `:2223`, calls `:2455` and `:7677`. `normalize_redundant_parameter_observation_operation` — definition `:2113`, call `:2098`. `WriteOnlyPrimitiveStore` in `construction.rs` — `direct_uses` arm `:689` and `operation_obligation` arm `:414`.

Read in full: `rewrite_block_parameter_operation` (`candidates.rs:2223-2283`), `normalize_redundant_parameter_observation_operation` (`:2113-2187`), the redundant-parameter output loop and checks (`:2426-2488`), `primitive_local_triple_pattern` (`operations.rs:1490-1495`), the `direct_uses` arm that uses it (`construction.rs:689-691`), and the observation event that clones the whole operation (`observation.rs:345-373`). Read `rewrite_value_operands` far enough to see the same macro replaces `value.value` (`candidates.rs:10248-10249`). Did not read `rewrite_scalar_substitutions`'s caller at `:7670` beyond the call into `rewrite_block_parameter_operation`.

The output rewriter leaves a store unchanged when `range_obligation` is `Some` (`candidates.rs:2274-2277`). The observation normalizer does not have that arm; its wildcard calls `rewrite_value_operands` (`:2181-2184`), which does replace the stored value. The stored value is a use (`construction.rs:689-691`). After the parameter is removed (`candidates.rs:2445`), revalidation (`:2463`) still sees that use and reports an undefined value (`unit_validation.rs:14344-14349`). The region observation also clones the operation (`observation.rs:347`), so the normalized input and the skipped output differ and fail the semantics compare (`candidates.rs:2478`) if revalidation did not already fail.

## 8. `candidates.rs:10686-10692`

**CLOSED.** The digest overstates what those lines do. The outside-function loop does not decide the tables.

Searches: `accepted_obligation_facts` in `candidates.rs` — 4 matches (`:4905`, `:6028`, `:6387`, `:8524`), none of them an assignment. `pruned_machines` — extend at `:1799` and reads at `:1817` and `:1823`.

Read in full: `accept_region_rewrite` (`candidates.rs:10662-10699`) and `revalidate_rewrite_output` (`unit_validation.rs:18213-18227`). Read the prune mutation (`candidates.rs:1796-1802`), the equality test before the scalar-identity edit (`:4904-4926`), and the equality test after `install_evaluated_constant` (`:6373-6388`). Did not read `install_evaluated_constant` itself.

No callback in this file assigns `accepted_obligation_facts`. The only write of `pruned_machines` is the unreachable-machine replay, which then calls `validate_psi_optimization_unit` (`:1802`). `admit` has already validated the input tables (`:422`). A region rewrite clones the input and does not touch either vector, so revalidation sees the same tables. The identity recompute seals whatever is present; it is not the check. The roster checks that do reject a bad prune list are the ones at `unit_validation.rs:1114-1151`.

## 9. `current_ownership.rs:204`

**UNDECIDED.** The digest's failure mode is real if a unit can pass later checks while the graph is absent. I could not show that it can, or that it cannot.

Searches: `origins_with_analysis(` — definition `:182`, callers `:173` and `:2883`. `validate_compatibility(` — definition `:711` only; the use sites are the re-exports at `:767`, `:1590`, and `:9965`. `validate_descriptor_compatibility(` — caller `unit_validation.rs:1250`. `fn adapt(` — `descriptor_results.rs:1215`.

Read in full: `BorrowedRegions::origins` (`current_ownership.rs:151-180`), the start of `origins_with_analysis` through the empty maps and the `if let (Some(graph), Some(analysis))` (`:182-209`), `validate_compatibility` through the empty-requirement return (`:711-718`), `catalog_graph` (`descriptor_results.rs:1089-1141`), and `adapt`'s signature and place-declaration setup (`:1215-1292`). Did not read the rest of `adapt`, so I did not classify its `return None` sites (`:1299` onward).

`CallAccesses::new` builds the view graph before per-function validation (`unit_validation.rs:1208`, `current_ownership.rs:2865`). If `adapt` returns `None`, `catalog_graph` returns `None` (`descriptor_results.rs:1123-1125`) and the `if let` at `current_ownership.rs:204` leaves sources and effects empty. Compatibility requirements are built from loans that need an incoming origin (`:1038`). With no origins, that list stays empty, and `validate_compatibility` returns `Ok` (`:716-717`). Duplicate machine ids cannot take this path: they are rejected at `unit_validation.rs:1114-1121` before `CallAccesses::new`.

What would decide it: read every `return None` in `adapt` and see whether each one is also rejected by `validate_function` or by a check that runs before the empty graph is trusted. If one `None` is possible for a unit that otherwise validates, a call whose stored effect conflicts with a live loan is accepted.

## 10. `current_ownership.rs:553-562`

**UNDECIDED.** The prefix fallback does not, by itself, authorize a longer path. The canonical location stored for that prefix was not traced.

Searches: `access_location(` — definition `:553`, callers `:116`, `:1184`, `:1279`. `fn contains` on `DescriptorLocation` — `descriptors.rs:569`. `fn location(` on the analysis — `descriptor_replay.rs:413`.

Read in full: `access_location` (`current_ownership.rs:553-562`), `DescriptorLoan::authorizes` (`:1233-1249`), `conflicted_edge`'s use (`:1183-1224`), `DescriptorLocation::contains` (`descriptors.rs:562-571`), and `DescriptorAnalysis::location` (`descriptor_replay.rs:413-424`). Read the write-recording caller (`current_ownership.rs:104-127`) and the observation caller (`:1272-1281`) enough to see both store the returned location and later pass it to `authorizes` (`:1111-1116`). Read the insert of canonical locations (`:306-324`). Did not read `canonical_storage`.

`contains` is "same root, and the other path starts with this path" (`descriptors.rs:569-570`). `authorizes` skips a conflict only when some authorized location contains the location it was given (`current_ownership.rs:1239-1244`). A recorded prefix is a prefix of the requested path. Any authorization that contains that prefix also contains the longer path. Falling back therefore cannot make `authorizes` true for a path it would have rejected. It can make `authorizes` false, which reports more conflicts.

What would decide the remaining gap: `canonical_storage`, called by `analysis.location` (`descriptor_replay.rs:422-423`). The map stores that canonical value, not the path that was queried (`current_ownership.rs:321-324`). If a shorter path's canonical location is not a path-prefix of the full path's canonical location, the contains argument above does not apply.

## 11. `current_ownership.rs:1069-1073`

**CLOSED.** The digest's return-that-moves-a-place does not go through this clear.

Searches: `conflicted_operation(` — definitions `:669` and `:1059`; calls `:1000`, `:2154`, `:2176`. `check_operation(` — definition `:2145`, call `:7937`. `return_or_return_unit_pattern` — `operations.rs:1278`.

Read in full: the descriptor `conflicted_operation` from `:1059` through the moved-list clear and the structural-result overlap (`:1059-1099`), `check_operation` (`:2145-2180`), the caller's structural-argument match and the `pins.check_operation` call (`:7831-7951`), and `return_or_return_unit_pattern` (`operations.rs:1278-1286`). Read the `ReturnStructural` arm (`current_ownership.rs:8579-8676`) and the `ReturnStructural` arm of `operation_place_accesses` (`unit_validation.rs:6867-6868`). Did not read `value_use.moving` past `:9542`.

The pattern is only `Return` and `ReturnUnit`. For those, the only caller builds `consumed_places` from structural arguments, and both operations fall through to `&[]` (`current_ownership.rs:7831-7841`). The cleared list is already empty. `ReturnStructural` is not matched, so this clear does not apply to it. Its source is checked by `value_use.moving` (`:8585`) and is visible to the exclusive-pin scan because `operation_place_accesses` returns it (`unit_validation.rs:6867`).

## 12. `current_ownership.rs:1160`

**CLOSED.** The digest's second line, `:1276`, is not a zip. That line is inside `operation_observations` (`:1272-1281`).

Searches: `.zip(` in `current_ownership.rs` — the binding/parameter zips are `:1160`, `:2202`, `:2276`, `:2467`, `:2556`, `:2589`. `structural_bindings.len()` — `unit_validation.rs:9045` and `:14287`.

Read in full: the zip in `conflicted_edge` (`current_ownership.rs:1157-1182`) and `structural_transfers::validate`'s edge loop (`unit_validation.rs:9036-9047`). Read `validate_function`'s order (`:9593` then `:9660`). Did not read the rest of `conflicted_edge` past `:1228` for this entry; it was read for entry 10.

`structural_transfers::validate` runs before ownership (`unit_validation.rs:9593`, `:9660`) and rejects an edge whose structural bindings and destination parameters differ in length (`:9045-9046`). `conflicted_edge` zips those same two slices (`current_ownership.rs:1157-1160`, destination from `:8717`). Equal length means the zip drops nothing.

## 13. `current_ownership.rs:1620-1623`

**CLOSED.** A dropped edge belongs to a unit that validation rejects.

Searches: `RangeRelations::new(` — `current_ownership.rs:146` and `descriptor_results.rs:2669`. `nodes.is_empty()` — `unit_validation.rs:5680`. `captured_scalar_ordering(` — caller `legalization_reference_custody.rs:1082`.

Read in full: the `edge` closure (`current_ownership.rs:1619-1641`) and the successor match through `StructuralCase` (`:1643-1668`), the empty-block rejection (`unit_validation.rs:5679-5685`), and `captured_scalar_ordering` (`descriptor_results.rs:2665-2672`). Read the `?` as a closure `Option`, not as a function error. Did not read `ScalarOrdering` or the pipeline caller past the call.

`positions` contains `(block, 0)` only for a block with at least one node (`current_ownership.rs:1608-1610`). A missing block or a block with no node makes the closure return `None`, and that successor is left out of the ordering graph. Empty blocks are rejected (`unit_validation.rs:5680-5684`). A missing successor is rejected by `structural_transfers::validate` (`:9038-9044`). Both run inside `validate_function`, which is the same `index_and_validate_unit_catalogs` that built `CallAccesses` (`:1208`, `:1233`). If the edge was dropped, that function still returns the error. The pipeline caller at `legalization_reference_custody.rs:1082` runs on a unit that has already passed that validation, so the target is present and nonempty and the closure returns `Some`.

## 14. `current_ownership.rs:2207-2211`

**UNDECIDED.**

Searches: the `continue` is in `check_edge` (`:2202-2211`). `exclusive_borrow_pair_pattern!` — no `macro_rules` under `abstract-operations/src`; the use at `unit_validation.rs:9182` is the related check. `structural_source_contract(` in `structural_transfers::validate` — `:9078` and `:9162`.

Read: `check_edge` from `:2202` through `:2226`, and `structural_transfers::validate` from `:9045` through `:9207`. Did not read the rest of that validate function.

The `continue` skips the owner-liveness tests when the argument has no source contract or the contract is not `Owned`. `structural_transfers::validate` runs first. Its exclusive-borrow arm (`:9182-9194`) allows a backing access that is not `Owned`. I stopped at `:9207`, before the expression is required to be true.

What would decide it: finish `structural_transfers::validate` and see whether an exclusive-borrow parameter can arrive at `check_edge` with a missing or non-owned source. If it cannot, the `continue` is unreachable for a validated edge. If it can, the skipped tests are the ones at `:2212-2226`, and a non-owned source of an exclusive borrow is accepted without them.

## 15. `current_ownership.rs:2202`

**CLOSED.** Same length check as entry 12.

Searches: the zip is the `for` at `:2202`. Caller of `check_edge(` — `:8711` only, besides the definition `:2183`.

Read: `check_edge` (`:2183-2202`) and the call (`:8711-8718`). The destination is `blocks[&edge.target].structural_parameters`. `structural_transfers::validate` compares `edge.structural_bindings.len()` with that same parameter list (`unit_validation.rs:9045`) before ownership runs (`:9593`, `:9660`). The zip does not drop a binding.

## Counts so far

| Verdict | Count |
| --- | --- |
| REACHABLE | 0 |
| CLOSED | 11 |
| UNDECIDED | 4 |

Entries not reached: 16 through 120.

## What I did not read

- The producer of `AcceptedObligationFact` and the bodies of `exact_add` / `saturating_add` / `wrapping_add` (entry 4).
- The phi-translated CSE validator above `:4564` (entry 5).
- `install_evaluated_constant` (entry 8).
- The rest of `adapt` after `descriptor_results.rs:1292`, including every `return None` (entry 9).
- `canonical_storage` (entry 10).
- `structural_transfers::validate` after `unit_validation.rs:9207` (entry 14).
- Entries 16 through 120.
- Subagent drafts: nothing from them is copied here.

Pushed through entry 10 as `5093f5033b35c0ed02590d5762396026fa4ee63c`. The final SHA is written on the last push.
