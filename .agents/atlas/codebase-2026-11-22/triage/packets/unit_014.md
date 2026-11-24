# Packet: unit_014 (area 10, stage 06 analyses and selected rewrites)

Packet complete. None of the record's 26 correctness claims about stage 06 shows a wrong compiler result at revision `a0152d26`. One claim was already fixed by [#5386](https://github.com/CathedralOS/Omega/pull/5386). The others either fail closed in this crate or rely on a guarantee that a named producer or later check enforces. The duplicated-code and unused-API observations stay open as repair candidates. No overlapping open PR or claim exists, and this packet makes no code change.

The packet covers [`records/unit_014.md`](../../records/unit_014.md) for the 16 files in [`units/unit_014.tsv`](../../units/unit_014.tsv), area 10 only. The common baseline is `24ad2700685e`. Between it and `a0152d26`, no commit touches the stage 06 crate. Gap packets own the supplemental ranges. Each row starts from the merged [area 07–13 ledger](../07-13-psi-and-native.md) and [digest 10](../../digests/10-native-stages-04-10.md).

Paths are relative to `source-rust/subsystems/compiler/psi-to-native/pipeline/06_selected-instructions-to-selected-instructions/`; `A/` = `analyses/src/`, `S/` = `src/selected_rewrites/`. "Traced" means that this review read the code path named in the row. No compiler build or test was run. The review did not construct a fixture because no claim reached a wrong result.

## Fail-open claims

| Record section and claim | Code path | Outcome | Evidence |
| --- | --- | --- | --- |
| §1 risk 1; digest line 172, §10 item 8 (line 269): `UnsealedRecoveryFacts::seal` keeps a ranges-only liveness row when full liveness fails | `A/lib.rs` `seal` | Fixed | [#5386](https://github.com/CathedralOS/Omega/pull/5386) changed `seal` to call `compute_function(...).map_err(\|_\| AllocationLegalityError::RootMismatch)?`. A failed full liveness calculation now refuses the seal. The area ledger records the reanalysis fixture ("unsealed recovery liveness"). |
| §1 risk 2: `carried_digests` trusts the caller's `carried` flags | `A/lib.rs` `carried_digests`; callers in `live_ranges.rs` and `liveness.rs` `admit` | Intended (traced in the record) | The record traced one in-crate caller. `seal` computes `carried` itself from storage identity and equality. No caller passes outside flags. |
| §2 risk 1; digest lines 173 and 211: live ranges skip operands that name an undeclared virtual register | `A/live_ranges.rs` `register_fragments`, `compute_function` | Refusal preserved downstream | Stage 05 builds the register list and refuses non-positional IDs (`selection_scalar_graph.rs:7813`, `ids_are_positions`). Every allocation result passes `RetainedAllocation::admit` (stage 07 `allocation/output.rs:236`), which calls `CurrentAllocation::capture` → `analyze_machine_effects`. `check_operands` (`A/machine_effects.rs:212`) refuses an operand whose register is missing from `virtual_registers` or whose ID differs from its position. The skip is local. Stage 07 still refuses the program before publication. |
| §2 risk 2: duplicate virtual-register IDs get empty fragments and are not refused | `A/live_ranges.rs:1040-1048`, `:1137-1141` | Cannot arise from current producers | Stage 05 rejects any function whose register IDs differ from their positions, so duplicates cannot occur. Stage 06 rewrites keep the IDs positional. Literal folding checks `ids_are_positions` (`S/literal_fold_rewrites.rs:2139`). Copy removal deletes the row at its position and lowers each higher ID by one (`S/copy_and_extension_rewrites.rs:1062`, `:1242-1255`). |
| §2 risk 3: `copy_affinities::derive` trusts the precomputed per-block affinity row | `A/live_ranges.rs:576-617`; `representations/selected-instructions/src/blocks.rs` `lookup`, `detach` | Intended | `BlockLookup` is computed lazily from the block's own instructions in a `OnceLock` (`blocks.rs:185`). `get_mut`, `iter_mut` and `index_mut` call `detach`, which clears it (`:696-704`). A malformed `CopyI64` sets `has_malformed_copy`, which `derive` refuses. |
| §3 risk 1; digest line 174: a missing block transfer row gives empty gen/kill sets | `A/liveness.rs` `compute_domains` `block_virtual_transfer(...).unwrap_or(...)` | Intended (the fallback cannot be reached) | `block_virtual_transfer`, `block_unit_transfer` and `block_unit_masks` return `None` only for an out-of-range block index (`blocks.rs:853-880`). `compute_domains` passes only in-range indexes. The rows are recomputed from current contents after any mutation, as in the row above. |
| §3 risk 2: `validate_transports` returns early | `A/liveness.rs:1655-1665` | Intended (traced in the record) | It returns early only when the function has no edges, operation traps or invoke-no-normal terminators to validate. |
| §3 risk 3: `materialize_block` passes a non-parameter register through unchanged | `A/liveness.rs:1117-1129`, `:1861-1871` | Intended (traced in the record) | A register that is not a parameter of the target block keeps its identity across the edge. |
| §4 risk 1; digest line 177: ordinary compilation does not revalidate the ISA effect catalog | `A/machine_effects.rs:48-66` | Intended (stated) | The comment at `:52-53` makes the ISA owner responsible. Foreign-catalog validation is a separate trust boundary (AGENTS.md, "Verify at trust boundaries"). |
| §4 risk 2: implicit effects are copied after the check | `A/machine_effects.rs:311-340` | Intended (traced in the record) | The code copies them only after they equal the constraint row. |
| §4 risk 3: ordinary instructions borrow the catalog declaration | `A/machine_effects.rs:695-699`, `:741-746` | Intended | Only indirect and normalized foreign calls carry details specific to one call site. All other kinds are keyed by `(machine_semantic_kind, constraint)`, and an ambiguous key is refused (`:129-139`). |
| §5: recovery classification | `A/recovery_classification.rs` | No claim | The record found no fail-open path. `NoAdmittedRecovery` is a refusal. |
| §6: register writes ignore units that are not callee-saved | `A/register_writes.rs:233` | Intended | The header (`:1-5`) says the scan reports callee-saved witnesses, not allocation authority. |
| §7 risk 1; digest line 178: `charge_steps` checks one of five work axes | `A/rewriting.rs:249-259`; `S/rewrites.rs` `pass_usage`, `ensure_pre_allocation_budget` | Intended | `charge_steps` checks one rewrite's validation steps against the budget before work starts. Each pass counts all five axes in `pass_usage` (`S/rewrites.rs:1570-1586`), adds them with `work_accounting::combine` and checks the total with `work_accounting::admit`. |
| §8, §9, §10, §11, §12 | `A/spill_choice.rs`, `A/tied_registers.rs`, `A/work_accounting.rs`, `src/lib.rs`, `S/mod.rs` | No claim | The record reports no fail-open path. `TiedRegisters::new` drops early definitions outside a tie by design. The callers check them with `contains`. |
| §13 risk 1; digest line 179: an instruction without a memory row can pass the load-forwarding walk | `S/address_and_load_rewrites.rs:543-558`, `reject_unaccounted`; `S/rewrites.rs:2646-2712` `unrostered_reach` | Fails closed | The forwarding walk lets an instruction without a memory row pass only if `unrostered_reach` classifies it as `Private`. The `Private` kinds are loads, address arithmetic, register arithmetic, comparisons, materialization and `Store64` to spill or boundary slots. `Store`, `StorePacked`, other `Store64` slots, `CopyBytes` and `ZeroBytes` refuse as `AliasingWrite`. All other kinds fall to `Unaccounted`, and the rewrite refuses them as `UnsupportedInstruction`. |
| §13 risk 2: outgoing-argument writes never interfere | `S/address_and_load_rewrites.rs:854-895`; stage 05 `selection_scalar_graph.rs:630-700` | Intended | Stage 05 emits `WriteOutgoing` for stores to `FrameStorageSlotId::Outgoing` argument slots, which belong to one call site. These slots are separate from place storage, so writing one cannot change the forwarded place. The call that reads them is a barrier in `reject_barrier`. |
| §13 risk 3: `reject_barrier` lets unlisted kinds through | `S/address_and_load_rewrites.rs:1206-1228` | Fails closed | An unlisted kind still goes through its memory rows (`interferes`). Without rows, it goes through `unrostered_reach`, so `AtomicFence`, `RegisterAccess` and similar kinds without rows are `Unaccounted` and refused. |
| §13 risk 4 (inferred in the record): address folding leaves the memory row unchanged | `S/address_and_load_rewrites.rs:233-263`, `:446-470` | Intended; the record's inference was wrong | A fold changes how the instruction encodes the address, not the address itself. The memory row stays correct because it describes place, byte offset and byte count. Load forwarding then refuses or admits by comparing the encoded offset with that row. The record's prediction that every folded access would be refused does not follow, and no wrong result follows either. |
| §13 risk 5: a load that can reach itself | `S/address_and_load_rewrites.rs:699-761` | Intended (traced in the record) | The walk scans the tail after the load and treats a different stored value as `AliasingWrite`. |
| §14 risk 1: `reaching_event`, `resolved_operands`, `materialized_bits` were not read | `S/rewrites.rs:160-450` | Fails closed | `reaching_events` seeds the entry block with `Unknown` and refuses if `Unknown` or nothing reaches the read. `reaching_event` requires exactly one site. `materialized_bits` requires the register's only non-`Use` definition in the function to be a plain `MaterializeI64`. `resolved_operands` refuses a producer whose operand form differs from the rule's. |
| §14 risk 2; digest line 180: a constant branch drops the untaken edge, including its bindings and work rows | `S/branch_and_compare_rewrites.rs:847-850`, `:885-899` | Intended | Turning a decided branch into a `Jump` removes the untaken edge, so its bindings and work rows no longer describe any transfer. The kept successor is cloned with its own bindings. The record identified no wrong input. Removing an unreachable target block is a separate optimization. |
| §14 risk 3: `remove_compare` maps errors to `SourceMismatch` | `S/branch_and_compare_rewrites.rs:2003-2008`, `:2183-2188` | Intended (traced in the record) | This changes the error kind only. The program is still refused. |
| §14 risk 4: `decided_value` has an `unreachable!` | `S/branch_and_compare_rewrites.rs:410` | Intended (traced in the record) | Every rule in the fixed table names one of the five materialize consumers. |
| §15 risk 1; digest line 181, §10 item 7 (line 268): `copy_removal` does not check `ids_are_positions` | `S/copy_and_extension_rewrites.rs:861-1029`, `:1242-1255` | Duplicate of digest §10 item 7 (partly right; no wrong numbering) | Stage 05 guarantees positional IDs (see §2 risk 2). Lowering every higher ID by one keeps IDs unique and ordered, and keeps them positional when they were positional before. The full admission check also refuses uses outside the block, uses in successor or slot mentions, and any redefinition of the input before the last use. |
| §15 risk 2: copy or compare removal leaves gaps in instruction IDs, and a later literal fold would refuse the function | `S/selected_optimization.rs:191-260`; `S/rewrites.rs` catalog | Cannot occur | `optimize_selected_instructions` runs exactly one executable phase and refuses a second with `UnsupportedComposition`. Copy and compare removal belong to the pre-allocation phase, while literal folding is the selected-lowering phase. Both never run on the same program in stage 06. |
| §15 risks 3–4 | `S/copy_and_extension_rewrites.rs` | Intended (traced in the record) | These checks can only refuse a rewrite. |
| §16 risk 1; digest line 182: `fold_usage` skips charging an action whose consumer is missing | `S/literal_fold_rewrites.rs:2245-2267`, `:2351-2370` | Cannot be reached | `compute_terminal_literal_fold` calls `derive_function_folds` first. That call applies every action to the same source plan and refuses a missing consumer with `DecisionMismatch` (`:1710-1720`). By the time `fold_usage` runs, every action's consumer exists. |
| §16 risk 2: `derive_actions` passes function index 0 | `S/literal_fold_rewrites.rs:1245` | Intended (traced in the record) | The call is a filter, `.is_ok()`, so the error value is discarded. |
| §16 risks 3–4 | `S/literal_fold_rewrites.rs:2135-2158`, `:3283-3297` | Intended (traced in the record) | A function with non-positional IDs is refused. The rules name which operand flags their consumers may drop. |
| §16 cases: the U12 immediate bound `Encoding(4095)` also applies on x86_64 | `S/literal_fold_rewrites.rs` catalog | Intended | The bound is narrower than the x86_64 immediate range, so it only declines folds. It never admits an immediate that x86_64 cannot encode. |

## Structure and unused code (sections 1–16 and maintainer list)

These observations do not claim a wrong result. Each remains open for the [code repair queue](../../../../../OWNER_CODE_CONCERNS.md) under AGENTS.md rule 6 and needs no owner decision.

| Record section and claim | Outcome | Notes |
| --- | --- | --- |
| §2, §3, maintainer item 1; digest line 72: tied-operand and early-clobber legality is written twice (`A/liveness.rs:839-1033`, `A/live_ranges.rs:204-422`) | Still open (duplicate mechanism) | One shared check would serve both analyses. |
| §7, §16, maintainer item 1; digest line 72: the transport-slot walk is written six times | Still open (duplicate mechanism) | The copies are in `A/rewriting.rs:64-118` and `:427-490`, `A/liveness.rs:1623`, `S/literal_fold_rewrites.rs:1352-1396` and `:1938-1995`, and `S/copy_and_extension_rewrites.rs:1153-1220`. |
| §15, §16: two `redensify` implementations | Still open (duplicate mechanism) | `S/copy_and_extension_rewrites.rs:1066-1101` and `S/literal_fold_rewrites.rs:1885-2007`. |
| §14: `declared_producers` is written twice, and the flag-universe computation four times | Still open (duplicate code) | The duplicates live in `S/branch_and_compare_rewrites.rs`. |
| §14: a private `isolated_declaration` requires `ControlFlow`, while `rewriting::isolated_declaration` requires `None` | Intended semantics, misleading name | The branch rewrites need a control-flow barrier. Renaming the private helper would remove the ambiguity. |
| §3, §7: `block_instructions` is defined three times; `control::successors` and a private `terminator_successors_mut` copy existing helpers | Still open (duplicate code) | — |
| §5, §16: the producer-provenance test is written twice | Still open (duplicate code) | `A/recovery_classification.rs:196-210` and `S/literal_fold_rewrites.rs:1265-1280`. |
| §14, §16: `audit_dead_unit` and `swapped_unit_readers_equality_only` duplicate a forward walk | Still open (duplicate code) | — |
| §1, §2, §3, maintainer item 5; digest line 262: three `*_reusing` analyses have no callers | Duplicate of the area ledger row "analysis reuse" (performance candidate) | — |
| §1: "Omega 04/05" stage labels in comments | Duplicate of digest line 257 | The labels are documentation only. |
| §7: `RegisterAllocationRuleTargetApplicability` has one variant | Still open (cleanup) | — |
| Maintainer item 2: nothing in this unit revalidates the plan after a rewrite | Intended (stated) | `S/selected_optimization.rs:9-14` and AGENTS.md, "Verify at trust boundaries". Replay is optional. |
| Maintainer item 4: thin unit tests for the rewrites | Still open (test coverage) | The rewrite families have tests outside these files, which this review did not count. |

## The record's uncertainties

| Uncertainty | Resolution |
| --- | --- |
| Do the `S/rewrites.rs` helpers fail closed? | Yes. See §13 risks 1 and 3 and §14 risk 1. |
| Can precomputed block rows be absent? | No. They are computed from block contents and cleared on mutation. See §2 risk 3 and §3 risk 1. |
| In what order do the phases run? | Stage 06 runs one executable phase. See §15 risk 2. |
| What does `allocation_input.rs` contain? | It is outside this slice (representations area 12). |
| Are the call-site counts exact? | Not rechecked. No disposition depends on them. |

## Unresolved claims

All of these are code-quality items with no demonstrated wrong result. Each can be repaired without an owner decision.

1. The tied-operand and early-clobber legality check is written twice.
2. The transport-slot walk is written six times.
3. `redensify` is written twice.
4. `declared_producers` and the flag universe are duplicated, and the private `isolated_declaration` has a misleading name.
5. `block_instructions`, `control::successors` and the private `terminator_successors_mut` duplicate existing helpers.
6. The producer-provenance test is written twice.
7. The forward unit-reach walk is written twice.
8. `RegisterAllocationRuleTargetApplicability` has a single variant.
9. The rewrite families have few unit tests in their own files.
