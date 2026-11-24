# Atlas packet unit_025: rewrite candidate validator

Packet complete. Every claim in [record unit_025](../../records/unit_025.md) and its area-12 digest lines has a
disposition below. One claim was a real defect and is fixed in this PR. When a block merge, empty-block thread,
common-subexpression rewrite or redundant-parameter rewrite replaced a scalar used as a runtime array index, native
optimization aborted. The validator left the old value in the structural path and then refused its own output. One
claim stays open (claim 4, a range-checked store fed by a redundant block parameter). The validator refuses that
case, and no fixture shows the proposer offering it.

Testing also found a `main` build break in this unit's crate. `optimization-unit-semantics` did not compile after
#5393. That is repaired here too; see "Build break repaired in the same PR".

Scope: area 12 only. The unit is one file,
`source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/optimization-unit-semantics/src/candidates.rs`,
cited below as `CAND:<line>` with the record's line numbers. The original survey read revision `4f3c255d`. The
review baseline is `24ad2700685e`. At `a0152d26` and at `3f9707e6` (the `main` this branch is rebased on) the file
still has 10,799 lines and every cited line holds the code the record names. After this PR's fix, lines after about
`CAND:2180` move down by up to 20. `D12` = [digest 12](../../digests/12-native-representations.md).

## Fixed

| # | Original claim | Code path | Contract, fixture or trace | Outcome |
| --- | --- | --- | --- | --- |
| 5 | Record §9 item 5; `D12:175`: `rewrite_scalar_value_uses` does nothing for `StructuralCase` (`CAND:7411`); the record asks whether a structural case can read a scalar. | `rewrite_accounting::substitutions::rewrite_scalar_value_uses`; `value_operand_rewrites::rewrite_value_operands` (`CAND:10309`); `AbstractOperation::runtime_indices` (`abstract-operations/src/operations.rs`) | A case reads a scalar whenever its dispatch path has a `StructuralPathSegment::RuntimeIndex`. `scalar_uses` counts every runtime index as a use (`operation_metadata.rs:197-203`). However, none of the substitutions rewrote structural paths, for any operation. Fixture `block_merging_substitutes_a_forwarded_runtime_index` (stage 03 `control_flow_analyses_tests.rs`): block 1 jumps to block 2 and passes the index `v1` as parameter `v5`, and block 2 dispatches on `array[v5]`. The stage 03 `AdjacentBlockMergeRule` proposes the merge. Before the fix, `validate_psi_rewrite_candidate` returned `UndefinedValue { machine: 1, block: 1, value: 5 }`. `pass_manager.rs:643` turns that error into `OptimizationRunError::CandidateValidation`, which ends the optimization run. | **Reproduced and fixed.** `structural_paths` and the new `structural_paths_mut` now share one macro body, and `for_each_runtime_index_mut` visits exactly the indices `runtime_indices` reports. `rewrite_value_operands` substitutes them for every operation. The `Jump`, `Conditional` and `StructuralCase` arms of the three functions that special-case edge terminators do the same. After the fix, the merged dispatch reads `array[v1]`. |

## Intended or failing closed

| # | Original claim | Code path | Contract, fixture or trace | Outcome |
| --- | --- | --- | --- | --- |
| 1 | Record §9 item 1; `D12:171`: `reconstruct_adjacent_merge_ownership_witness` returns an empty witness when none of the three frontier facts exist (`CAND:6959-6963`); same shape at `CAND:7578-7581`. | `CAND:6943-6984`; `unit_validation.rs` `reconstruct_ownership_frontier_facts_with_admitted_cycle_machines` (`:18234`) and `validate_retained_ownership_authority` (`:1205`) | The validator derives frontier facts from its own ownership replay (`catalogs::index_and_validate_unit_catalogs`). The proposer does not supply them. The empty witness is allowed only when the function has no structural parameters, entry claims or declared places. Without a place, there is no ownership to cross a frontier. If only some of the facts exist, or their snapshots differ, the function returns `None`. `revalidate_rewrite_output` then replays ownership over the merged output. | **Intended.** |
| 2 | Record §9 item 2; `D12:172`: shared terminal jump fusion passes `\|_, _\| true` where the merges test dominance (`CAND:1585`). | `control_flow_cleanup::shared_jump_fusion` (`CAND:1528-1647`) | The target must be a one-node block whose node is `Return`, `ReturnUnit`, `ReturnStructural` or `Crash`, with no successors (`CAND:1550-1563`). The validator copies that node into the predecessor and substitutes the jump's own arguments, which are already live at the predecessor's terminator. The target has no successors, so no other block can use its parameters. The target block itself remains for the other incoming edges. Whole-unit revalidation still checks dominance for every use (`unit_validation.rs` `validate_node_uses`). | **Intended.** The record's partial trace is confirmed. |
| 3 | Record §9 item 3; `D12:173`, `D12:388`: `validate_outside_region` compares only input blocks, and the function check covers only input functions. | `accept_region_rewrite` (`CAND:10762-10799`); `outside_region::validate_outside_region` (`CAND:5908`) | The output is `input.clone()` changed by the family closure written in this file. The candidate never supplies the output unit. No closure in `candidates.rs` adds a block or a function (a search for `functions.push`, `blocks.push` and similar finds only local lists of block IDs). The output is then fully revalidated. | **Intended.** No candidate can add a block or function. |
| 6 | Record §9 item 6; `D12:176`: `call_result_constant` makes a caller's value constant when the callee has one `Return` with a constant fact (`CAND:9840-9843`, `:9922`). | `state_specialization::call_result_constant` | The constant comes from `validator_scalar_constant_facts`, the validator's own SCCP pass over the callee. That pass treats the callee's parameters as unknown, so a constant fact on the single `Return` value holds on every normal return. A callee that never returns passes no value to the caller. Only `O::Call` (a direct scalar call) qualifies. | **Intended.** |
| 7 | Record §9 item 7 | `validate_unreachable_private_machines_candidate`, `CAND:1957` | Pruning whole machines changes no block, so the validator requires empty `affected_blocks` and substitutions. | **Intended.** |
| 8 | Record §9 item 8 | `state_specialization::plan_dispatch`, `CAND:9876-9878`; caller `CAND:9255` | When every incoming edge resolves, the plan is cleared and the caller rejects the empty plan. | **Intended** (fails closed). |
| 9 | Record §9 item 9 | `ProofCheckValidationRoute::for_rule`, `CAND:6584-6597`; router `CAND:6892-6908` | An unknown rule yields `None`, and the router returns `CandidateAnalysisContractMismatch`. | **Intended** (fails closed). |
| 10 | Record §9 item 10 | `validate_psi_rewrite_candidate`, `CAND:10466` | A proof-check rule goes to the proof-check router regardless of patch kind. Each routed validator unpacks its own patch variant with `candidate_patch!` and refuses any other. | **Intended** (fails closed). |
| 11 | Record §9 item 11 | `independent_cse_expression`, `CAND:3187` | `CompatiblePolicy` yields `None`. Its callers use `independent_compatible_policy_scalar_leader` instead (`CAND:4127-4130`). | **Intended** (fails closed). |
| 12 | Record §9 item 12: `.expect("... exists")` at `CAND:317`, `:320`, `:760`, `:1436`, `:2440`, `:4403` | Cited lines | Each follows a `candidate_function` or `candidate_block` lookup on the same input and runs on that input's clone. | **Intended.** These calls cannot panic. |
| 13 | Record §4 and five-things item 2; `D12:85`: rule and validator version suffixes differ for one family (`constant-conditional-fold` rule v5 vs validator v4; `redundant-block-parameter` rule v1 vs validator v2). | `contract` table `CAND:404-416`; validator byte strings `CAND:929`, `:2494` | The rule identity (`omega.psi-rule.*`) is admitted against the rule table. The validator identity (`omega.validator.*`) only names the checker in `ValidatedPsiRewrite` and the decision record. The two names are separate, so each version changes on its own. | **Intended.** |
| 14 | `D12:146`, record §5: merges, threads and fusion refuse structural block parameters; case-membership, field-value and state-argument specialization refuse any machine with a cyclic block graph (`CAND:1994`, `:9682`). | Cited lines | Each restriction returns a `Candidate*Mismatch` error. These are scope limits on what the optimizer may rewrite, not wrong results. | **Feature gap** (fails closed). Widening any of them needs its own replay. |

## Still open

| # | Original claim | Code path | What is known | Next discriminating check |
| --- | --- | --- | --- | --- |
| 4 | Record §9 item 4; `D12:174`, `D12:388`: `rewrite_block_parameter_operation` leaves `WriteOnlyPrimitiveStore { range_obligation: Some(_) }` unchanged (`CAND:2274-2277`). | `copy_propagation::operation_rewrite::rewrite_block_parameter_operation`; `normalize_redundant_parameter_observation_operation`; `unit_validation.rs` `require_scalar_receiving_fact` (`:2528`); stage 03 `rules.rs` `redundant_block_parameter::propose` (`:759`) | This **fails closed**. The removed parameter stays as the stored value, and revalidation refuses it with `UndefinedValue` (`validate_node_uses`). Substituting it would not pass either: the store's accepted range fact records the exact received `ValueId` (`verified_unit.rs:286-345`), and `require_scalar_receiving_fact` compares that value. The observation normalizer does substitute the value, so the two sides of the region comparison disagree on this store. The stage 03 proposer does not filter such parameters. If it proposes one, the refusal ends the optimization run, as in claim 5. That proposal has not been reproduced. | Build a stage 03 fixture: a block parameter whose incoming edges all pass the same argument, stored through a range-restricted `WriteOnlyPrimitiveStore` with an accepted receiving fact. If `RedundantBlockParameterRule` proposes it, either make the proposer skip parameters received under a range obligation, or carry the receiving fact through the substitution. |

## Duplicate code and inventory

| Original entry | Current source | Disposition |
| --- | --- | --- |
| Record §6; `D12:102-103`: dominators and post-dominators computed independently (`CAND:3226`, `:1649`) and in stage 03 and `optimization-unit` | Cited lines | **Duplicate** code by design. The checker recomputes what the proposer claims. |
| Record §6; `D12:97`: stage 03 imports `reconstruct_linear_thread_*` and the pruning helpers from this file | `control_flow_cleanup.rs:472`, `:540`, `:548`, `:919`, `:977`, `:983` | **Duplicate** lead. For these helpers, the proposer and the checker share one computation, so a fault there would reach both. No wrong result is known. |
| Record §6: repeated blocks within the file (two merges, two threads, CSE leader selection, obligation-fact checks, materialized-literal checks) | Cited lines | **Duplicate** code. Every copy compares against a recomputed value and refuses on mismatch. |
| Record §7; `D12:315`, `D12:403`: the three specialization validators have no external caller | `lib.rs:240-243` | **Intended.** Inventory only: the digest already marked this matched, and `validate_psi_rewrite_candidate` is the single production entry (`pass_manager.rs:643`). |

## Build break repaired in the same PR

| Cause | Source | Trace | Outcome |
| --- | --- | --- | --- |
| #5393 added `Proposition::DeclarationIdentity`. #5427 repaired the `optimization-unit` encoder, but the checker's `proposition_structural_roots` still matched the old variant list. | `optimization-unit-semantics/src/unit_validation.rs`, `proposition_structural_roots::collect` | On `3f9707e6`, `mbx test -p abstract-operations-to-abstract-operations --lib` failed with `error[E0004]: non-exhaustive patterns: &Proposition::DeclarationIdentity(_) not covered` at `unit_validation.rs:10668`. Open PRs #5432 and #5435 change only `optimization-unit/src/identity.rs`. | **Fixed.** The new arm reports the roots of `ClauseOperand::Input` subjects and `ClauseOperand::Scalar` terms. Formals and applications only name other clause values by position. |

## Validation

- Before the fix, on this branch with only the fixture applied:
  `CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx test -p abstract-operations-to-abstract-operations --lib forwarded_runtime_index`
  failed with `UndefinedValue { machine: MachineId(1), block: BlockId(1), value: ValueId(5) }`.
- After the fix:
  `CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx test -p abstract-operations -p optimization-unit-semantics -p abstract-operations-to-abstract-operations --lib`:
  11 passed (this includes the new `mutable_paths_visit_every_runtime_index_in_order`); 24 passed; and 11 passed with
  2 ignored (this includes both indexed-dispatch merge fixtures).
- `mbx clippy -p abstract-operations -p optimization-unit-semantics -p abstract-operations-to-abstract-operations --all-targets -- -D warnings`:
  clean.
- `rustfmt --check` on the changed files reports only diffs that `origin/main` already has: one in `operations.rs`
  and four in `unit_validation.rs`, none in the changed lines.
- No global compiler build, sample gate or native run was performed.
