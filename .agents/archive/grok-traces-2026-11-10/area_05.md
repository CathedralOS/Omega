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

## Counts so far

| Verdict | Count |
| --- | --- |
| REACHABLE | 0 |
| CLOSED | 4 |
| UNDECIDED | 1 |

Entries not reached: 6 through 120.

## What I did not read

- The producer of `AcceptedObligationFact` and the bodies of `exact_add` / `saturating_add` / `wrapping_add` (entry 4).
- The phi-translated CSE validator above the `rewrite_scalar_value_uses` call (entry 5).
- Entries 6 through 120, including their functions and callers.
- Subagent drafts, if any are running: nothing from them is copied here.

Commit: filled in when this file is last pushed.
