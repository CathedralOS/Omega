# Record: unit_014 (area 10-native-stages-04-10)

Paths are relative to `source-rust/subsystems/compiler/psi-to-native/pipeline/06_selected-instructions-to-selected-instructions/`
(`A/` = `analyses/src/`, `S/` = `src/selected_rewrites/`). Every file was read in full. Terms: "selected program" is
the machine-instruction program with virtual registers that stage 05 produces and stage 06/07 consume; "unit" is a
register unit (one physical register footprint bit); "fold" is a rewrite that replaces a producer/consumer pair
with one instruction.

Test counts (grep `#\[test\]` per file): A/lib.rs 2, live_ranges 6, liveness 3, machine_effects 5, recovery_classification 1,
tied_registers 1, work_accounting 4, S/address_and_load 1, S/literal_fold 1. Zero in register_writes, rewriting,
spill_choice, branch_and_compare, copy_and_extension, S/mod.rs, src/lib.rs.

---

## 1. A/lib.rs (1157 lines)

**Purpose.** Root of the shared analysis crate. It holds a block of `macro_rules!` that stamp out receipt, carrier and
error boilerplate for every analysis, the allocation-recovery rule catalog, the `SelectedOptimizationRecord`, the
`SelectedProgramView` trait, and `UnsealedRecoveryFacts`, which lets stage 07 re-analyze only the functions a
spill/split step changed and then seal the result.

**Shape.**
- Macros (exported): `shared_analysis_identity_accessors!` (:49), `receipt_accessor_rows!` (:77), `copy_receipt!` (:90),
  `staged_producer_accessors!` (:117), `validated_analysis_accessors!` (:169), `work_defect_conversion!` (:184),
  `validated_plan_carrier!` (:195), `shared_plan_carrier!` (:216), `seal_plan!` (:243), `failed_display!` (:263),
  `validated_transformed_receipt!` (:276), `receipt_selected_identities!` (:305), `rewrite_receipt!` (:325),
  `rewrite_receipt_identity!` (:362).
- `allocation_recovery_catalog` (:393): `AllocationRecoveryRule` enum (`FixedViewCopies`, `PressureRematerialization`, :411),
  `ALLOCATION_RECOVERY_RULE_CATALOG: [_; 2]` (:447), `selected_allocation_recovery_entry` (:483).
- `SelectedOptimizationRecord` (:528): `selected_lowering_completion`, `pre_allocation_completion`, `transformations`.
- `SelectedOptimizationInput` (:573): program, custody, register environment, selections, per-pass budget, record.
- `SelectedProgramView` trait (:653) with impls for `SelectedProgram` (:707), `SelectedProgramRef` (:685), `OwnedSelectedProgram` (:748).
- `carried_digests` (:803), `UnsealedRecoveryFacts` (:827) with `from_validated` (:835), `reanalyze_changed_functions` (:857), `seal` (:935).
- `#[path]` include of `representations/selected-instructions/allocation_input.rs` as `mod allocation_input` (:553).

**Inputs/outputs.** The catalog reads `OptimizationPhaseSelections`; `UnsealedRecoveryFacts` reads the previous and next
`SelectedProgramView`, an allocator availability, physical model and reservation profile, and returns new
`ValidatedLiveness`/`ValidatedLiveRanges`/`ValidatedAllocationLegality` (:935-1036).

**How it decides.**
- Catalog lookup is keyed on the `Optimization` enum value: zero selected -> `None`, one -> table `find`, more than one -> `UnsupportedComposition` (:492-501), unknown -> `UnsupportedSelection`.
- A function counts as "changed" if it neither shares storage with the previous function nor compares equal (:870-876). A function counts as "carried" when its program row, its liveness row, its range row and its legality row are each pointer-equal or equal to the anchor's (:963-996). Keyed on `Arc` identity first, then value equality.
- `carried_digests` (:803-817) reuses the previous digest only when `carried[index]` is true and a previous digest exists at that index; otherwise recomputes.

**Cases by shape.** Catalog has exactly two rows (:447-472): `SharedEntryFixedViewCopyAfterCompareBeforeBranchV1` and `ActiveResidentImmediateU64MultiUseRematerializationV1`; any other selected allocation-recovery optimization is refused.

**Same job elsewhere.** `reanalyze_changed_functions` reuses per-function rows the same way `analyze_liveness_reusing` (A/liveness.rs:2097), `analyze_live_ranges_reusing` (A/live_ranges.rs:1631) do for whole-plan reuse. Two reuse mechanisms exist side by side.

**Unused / one-caller code.**
- `UnsealedRecoveryFacts`: grep `UnsealedRecoveryFacts` across `source-rust/`: 3 hits in A/lib.rs, 8 in stage 07 `allocation/assignment.rs`; stage 07 is the only outside user.
- `analyze_liveness_reusing`, `analyze_live_ranges_reusing`, `analyze_allocation_legality_reusing`: grep over `source-rust/` shows only the definition and the `pub use` re-export (A/lib.rs:619,626,770), so 0 call sites outside definition and re-export. (Their `compute_terminal_*_reusing` helpers are referenced only by them.)
- `selected_allocation_recovery_entry` / `ALLOCATION_RECOVERY_RULE_CATALOG`: 5 hits in A/lib.rs, 2 in `S/rewrites.rs`, 2 in stage 07 `register_allocation.rs`.
- The crate comment (:22-32) says housing analyses in one crate lets stage 06 and stage 07 "share the exact dataflow and interference engines". Grep of `Cargo.toml` files for `selected-instruction-analyses` gives 5 manifests: itself, stage 06, stage 07, `native-compiler` (`psi-to-native/Cargo.toml`) and `verification/published-objects`. So two pipeline stages plus a verifier plus the aggregate crate use it.

**Stated limits.** "Omega 04 / Omega 05" labels (:42-45, :407-409) are stage names that do not match folder numbers 06/07. Doc :408-409: "it keeps no second table from optimization names to procedures."

**Fail-open risks.**
1. `UnsealedRecoveryFacts::seal` (:955-962): `if !materialized && let Some(function) = ... && let Ok(full) = liveness::compute::compute_function(index, function)` swallows an `Err` from `compute_function`. A function whose liveness was recomputed in "ranges-only" form (no per-instruction live sets, `liveness_materialized[index] = false`, set at :889) stays in that form if the full computation fails, and is then sealed as `ValidatedLiveness`. Class: open. Input that reaches it: a changed function whose full liveness fails but whose ranges-only liveness (`compute_function_for_ranges`, same validation prefix at liveness.rs:760-762) succeeded; both call `reject_unsupported_constraints` and `validate_transports` first, so a failure difference is not obvious. Not traced further.
2. `carried_digests` (:812-816) trusts the caller's `carried` flag and reuses a stale digest without checking it; `seal` computes `carried` itself (:963), so only a different caller could misuse it. Class: traced (one caller in this crate, `admit` paths at live_ranges.rs:1554, liveness.rs:2067).

---

## 2. A/live_ranges.rs (1859 lines)

**Purpose.** Turns the liveness rows into live-range fragments per virtual register and per register unit, plus tied pairs, early-clobber constraints, copy affinities, edge register transfers and an interference list. Also seals the result and stages it for custody.

**Shape.**
- `compute` module (:13-1515) with sub-modules `architectural_units` (:15), `constraints` (:198, `derive_early_clobbers` :204, `derive_tied_pairs` :363), `copy_affinities` (:571), `edge_transfers` (:619), `fragments` (:676, `register_fragments` :712, `before_point` :951 `after_point` :962), `function` (:1008, `compute_function` :1016), `interference` (:1193, `interference` :1261), `compute_terminal_live_ranges` (:1440), `compute_terminal_live_ranges_reusing` (:1462).
- `validate` (:1516-1613): `FunctionCounts` (:1523), `admit` (:1548), `require_liveness_of` (:1602).
- `analyze_live_ranges` (:1622), `analyze_live_ranges_reusing` (:1631).
- `staging` (:1654-1784): `stage_optimized_live_ranges` (:1722), `admit_optimized_live_ranges` (:1705), `StagedOptimizedLiveRanges` (:1735).
- `LiveRangeError` (:1809), 14 variants.

**Inputs/outputs.** Reads `SelectedFunction` and `FunctionLiveness` (from liveness.rs); produces `FunctionLiveRanges` rows held in `Arc`, wrapped in `LiveRangePlan` (:1452) then `ValidatedLiveRanges` with a digest per function and per-function counts.

**How it decides.**
- Point numbering: each instruction position `p` gets points `2p` (before) and `2p+1` (after) (:951-972). Overflow is an error (`PointOverflow`).
- Fragments are built by one backward walk per block, keyed on operand access (`Def`/`Use`/`UseDef`) and on whether the register appears in the instruction's `virtual_uses`/`virtual_defs` vectors; when both vectors are empty it falls back to the operand rows (:744-806). A fragment is closed at a def only if the same instruction does not also use the register (:814, :780).
- Interference: two registers interfere if any pair of their non-empty fragments in the same block overlaps (interval tree over sorted starts, :1209-1259; `Interval` is exactly 16 bytes, asserted :1207).
- Tied pair rule: definition `Def`, source `Use`, definition operand number > source operand number, different register, same class, source not itself tied (:392-398).
- Early-clobber rule: one long boolean (:298-330) with the exact shape described by `UnsupportedEarlyClobber`.
- Carry decision in `compute_terminal_live_ranges_reusing`: carry a function row only when the earlier ranges named the earlier selected program/liveness identity, the same optimization unit, work schedule and target, the function count matches, and either storage is shared or the function compares equal, and the liveness row compares equal (:1471-1495).

**Cases by shape.**
- `reject_unsupported` refuses any `UseDef` operand (:1177-1191).
- Tied pairs and early clobbers must match the shapes at :298-330 and :392-398, else refused.
- `operand_point` returns `UnsupportedUseDef` for `UseDef` (:943).
- Edge transfers exist only for block parameters that are live-in at the target (:649-651).

**Same job elsewhere.**
- `liveness.rs:839-1033` (`reject_unsupported_constraints`) checks the same tied-operand and early-clobber shapes (including a `TiedRegisters` chain check) that `derive_early_clobbers`/`derive_tied_pairs` here check again (:204-422). Two near-duplicate implementations of the same legality rules, in two files of one crate.
- Block-boundary and point arithmetic duplicated: `architectural_units` (:22-196) and `register_fragments` (:712-860) repeat the same backward fragment walk, one over units and one over virtual registers.
- `spill_choice.rs:482 interferes` re-does a lookup on the interference list this file builds.

**Unused / one-caller code.** `analyze_live_ranges_reusing`: 0 call sites outside its definition and the re-export (grep, see section 1). `append_point`, `fragments_from_points` (:863, :890) are `#[cfg(test)]` only.

**Stated limits.** None as comments; the refusals are `Unsupported*` error variants.

**Fail-open risks.**
1. `register_fragments` (:732, :765, :798, :808, :829, :839) and `compute_function` (:1054, :1067, :1090) test `register_index.get(r) && idx != u32::MAX` and silently skip operands or live sets that name a register not declared in `selected.virtual_registers`. A function where an instruction operand names an undeclared virtual register gets no fragment, no occurrence and no interference entry for it. Class: open. A different analysis (`machine_effects.rs:227-235 check_operands`) refuses undeclared operand registers, and edge propagation in liveness refuses undeclared registers that are live across an edge (liveness.rs:406-408), but whether that check always runs before this analysis is not traced.
2. Duplicate virtual-register ids: `has_duplicate_register_ids` (:1040-1048) is computed, and rows after the first duplicate get empty `fragments` (:1137-1141) but nothing is refused. Class: open. Liveness marks duplicate ids invalid only for edge propagation (liveness.rs:338-349).
3. `copy_affinities::derive` (:576-617): a block with no precomputed affinity row and a `CopyI64` with operand list not exactly two returns `FunctionMismatch` (closed). With a precomputed row it trusts `block_copy_affinities` (:582-592). Class: open (precomputed row not read; lives in representations).

---

## 3. A/liveness.rs (2309 lines)

**Purpose.** Computes backward liveness over virtual registers and over register units for each function of the selected program: per block live-in/out, per instruction live-in/out, and per successor edge live sets, with block parameters substituted through edge bindings. It also validates that edge value transports (bindings, structural cases, operation continuations) are self-consistent.

**Shape.**
- `compute` (:11-1193): `control::successors` (:23), `flow::FunctionFlow` (:72, `compute_domains` :142) as a bitset worklist solver (`VecDeque`, :380-501), `compute_terminal_liveness` (:544), `compute_terminal_liveness_reusing` (:563), `compute_function_units_filtered` (:622, unit-only liveness for chosen units), `compute_function` (:741), `compute_function_for_ranges` (:748, skips per-instruction sets), `reject_unsupported_constraints` (:839), `materialize_block` (:1035).
- `edge_values` (:1194-2052): `FunctionIndex` (:1529), `has_edge_use` (:1623), `validate_transports` (:1648), `incoming_argument` (:1851), `validate_case_transport` (:1972), `case_payload::observed_source` (:1433).
- `validate::admit_validated_liveness` (:2061), `analyze_liveness` (:2090), `analyze_liveness_reusing` (:2097), `staging` (:2111-2241), `LivenessError` (:2260, 11 variants).

**Inputs/outputs.** Reads `SelectedFunction` (blocks, terminators with successors and bindings, virtual registers, `block_virtual_transfer`/`block_unit_masks`/`block_unit_transfer` precomputed rows on `function.blocks`); produces `FunctionLiveness` (:831) with `OperandPosition` arena, and `ValidatedLiveness`.

**How it decides.**
- Positions: dense, one per instruction including the terminator, from block order (:764-780).
- Edge propagation substitutes a successor's block-parameter registers by the argument registers named in the edge's `bindings` or `structural_case.payloads` (:241-262, :423-428); register ids are the key.
- `validate_transports` keys on `SelectedBlockOrigin` x `SelectedSuccessorRole` combinations (:1733-1783) and on `psi_edge`/`source_target`/provenance fields.
- `incoming_argument` (:1851-1970) checks a binding by `source_value` equality, `parameter_index == binding_position`, scalar type and register class equality.
- `case_payload::observed_source` accepts a case payload register only if it is a `StructuralObservation` register, or a signed 8/16/32-bit `AbiTransport` register defined by a matching `SignExtendI8/16/32` whose input is a matching `Load8/16/32` register in the same edge-transfer block (:1433-1526).

**Cases by shape.**
- Dense-vs-sparse block lookup: `if max_block_id < 65536` uses a dense `Vec`, else a `BTreeMap` (:164-180). A size threshold, no comment.
- Unit masks: used when every block's high-unit flag is false and all units are `< 64`; otherwise a `BTreeSet` path (:221-232, :297-333). Register units above 63 take the slow path.
- `observed_source`: only widths 8, 16, 32 and only signed (:1456-1464).
- `reject_unsupported_constraints`: refuses `UseDef` (:853), more than one early-clobber definition unless all operands are `Use`/early `Def` with no ties (:871-893).

**Same job elsewhere.** See live_ranges.rs section: tied/early-clobber legality is duplicated (:839-1033 here, live_ranges.rs:204-422). `control::successors` (:27) duplicates `selected_instructions::terminator_successors`, which is also used in the same file (:1662, :1723, :1625 re-derives the list by hand inside `has_edge_use`). `block_instructions` is re-defined at :1182 here, at `A/rewriting.rs:41`, and at `A/recovery_classification.rs:319`.

**Unused / one-caller code.**
- `compute_function_units_filtered` (:622): grep over `source-rust/`: definition plus 1 caller, stage 07 `runtime_spill_and_split.rs:3061`.
- `analyze_liveness_reusing`: 0 outside call sites (see section 1).
- `has_edge_use` (:1623): used by `recovery_classification.rs:163`.
- `FunctionIndex::unique_register`, `parameters_of`, `block_index` are used inside this file and by `live_ranges.rs:638-658`.

**Stated limits.** None as comments.

**Fail-open risks.**
1. `FunctionFlow::compute_domains` (:281-288) `function.blocks.block_virtual_transfer(block_index).unwrap_or((&[], &[]))`: a block with no precomputed transfer row is treated as using and defining no virtual registers. Likewise `block_unit_transfer` returning `None` leaves empty unit sets (:313-332), and `block_unit_masks` `None` only falls to the slower path (:306-308). Wrong input: a block whose cached transfer row is absent gets empty gen/kill, so liveness under-approximates. Class: open (rows come from `selected_instructions`, not read).
2. `validate_transports` (:1655-1665): returns `Ok(())` without checking when no block is operation-originated, no terminator is an operation trap or invoke-no-normal, and no terminator has successors. Wrong input reaching it: a function whose only terminators are returns/crashes of source origin; those have no edges to validate. Class: traced (the early exit tests exactly the conditions under which the loop below has nothing to examine).
3. `materialize_block` (:1117-1129): when `edge.substitutions.is_empty()` and every target live-in register is valid, uses the live-in list as-is; otherwise runs `incoming_argument` which returns `destination` itself for any register that is not a block parameter of the target (:1861-1871). Class: traced (registers that are not parameters of the target block are passed through unchanged, as intended).

---

## 4. A/machine_effects.rs (1370 lines)

**Purpose.** For each instruction of the selected program, checks that its operands and implicit unit effects match its register-constraint row and looks up the ISA effect declaration (memory, trap, barrier, call, cleanup, encoding alternatives) for its (semantic kind, constraint key) pair. Produces `ValidatedPreAllocationMachineEffects`.

**Shape.**
- `analysis::analyze` (:20), `catalog::prepared_catalog` (:48; per-architecture, `canonical_machine_effect_catalog` with `x86_64_machine_effect_catalog` or `aarch64_machine_effect_catalog`), `facts::compute` (:84-653): `compute_terminal_pre_allocation_machine_effects` (:105), `validate_catalog_roots` (:191), `check_operands` (:212), `compute_instruction` (:279), `exact_declaration` (:343), `machine_semantic_kind` (:366, a 160-arm `match` from `SelectedInstructionKind` to `MachineSemanticKind`), `machine_alternative_is_transparent` (:643); `foreign_call` module (:654-766); `ValidatedPreAllocationMachineEffects` (:830); `MachineEffectError` (:860, 15 variants).
- Public entry points: `analyze_machine_effects` (:929), `prepared_machine_effect_catalog` (:919), `analyze_pre_allocation_machine_effects` (:798).

**How it decides.**
- Declaration lookup is keyed on `(machine_semantic_kind(kind), instruction.constraint)` through a `BTreeMap`; duplicate keys collapse to `None` and refuse as `AmbiguousDeclaration` (:129-139, :352-363).
- Operand check compares each operand to the constraint row field by field: operand number, access, class, fixed view, tied operand, early clobber, virtual-register id equal to the register's own id, declared class (:212-262).
- Implicit uses/defs/clobbers must equal the constraint's (:311-318).
- `Store` byte size must be 1, 2, 4 or 8 (:289-293).
- Calls: indirect calls (`indirect_code_offset().is_some()`) and `NormalizedForeignCall` instantiate a per-call constraint and declaration (:669-765); other instructions use the catalog row borrowed.
- Register environment identity must match the recomputed identity (:118-127); catalog target, constraint identity and selected keys must match (:191-209).

**Cases by shape.** `machine_semantic_kind` is an exhaustive match; each `SelectedInstructionKind` variant has an arm (:366-641); a new variant would fail to compile. `RegisterAccess` is dispatched on its operation payload (:381-436).

**Same job elsewhere.** The constraint-row-versus-operand check `check_operands` repeats what the rewrites do in their own admission (for example `agree_compare_removal_row` in S/branch_and_compare_rewrites.rs:1804 and the row checks in S/address_and_load_rewrites.rs:111-124). Suspected (rewrite checks are local, per-rewrite).

**Unused / one-caller code.** `machine_alternative_is_transparent`: grep `machine_alternative_is_transparent|...` gives uses in S/branch_and_compare (2 via import), S/copy_and_extension (2) and the file itself. `machine_semantic_kind` is used in S/branch_and_compare_rewrites.rs, S/copy_and_extension_rewrites.rs, S/literal_fold_rewrites.rs.

**Stated limits.** Comment at :52-53: "The ISA owner establishes its own output. Foreign-catalog validation is a separate boundary, not a second construction during ordinary compilation." So ordinary compilation builds the catalog from `x86_64_machine_effect_catalog`/`aarch64_machine_effect_catalog` without validating it.

**Fail-open risks.**
1. `prepared_catalog` (:48-66): the catalog the ISA crate returns is not re-validated during ordinary compilation. Class: stated (comment :52-53). A wrong ISA table would be trusted.
2. `compute_instruction` (:326-340) copies `instruction.implicit_uses/defs/clobbers` into the effects after checking they equal the constraint row (:311-318). Class: traced (equal by check).
3. Both `foreign_call::constraint` and `::declaration` return `Cow::Borrowed(base)` for any instruction that is neither an indirect call nor a normalized foreign call (:695-699, :741-746): the declaration is the catalog's declaration unchecked against instruction-specific details. Class: stated implicitly by the catalog design; not traced further.

---

## 5. A/recovery_classification.rs (588 lines)

**Purpose.** For the one virtual register chosen as a pressure victim (`spill_choice`), decides whether it can instead be recreated by re-materializing its constant just before a later use ("immediate u64 rematerialization candidate") or why it cannot.

**Shape.** `classify_pressure_recovery` (:505) -> `compute_terminal_recovery_classifications` (:429); `function_classification::classify` (:25), `victim_role` (:90); `immediate_eligibility::classify` (:134), `unique_definition` (:232), `future_uses` (:261), `is_fixed_unsigned_u64` (:330); `work_usage::required` (:386); `RecoveryClassificationError` (:574, 8 variants).

**How it decides.** Keyed on the victim register's scalar type, `VirtualRegisterOrigin` and range shape:
- Not unsigned 64-bit, non-address, known width -> `UnsupportedScalarType` (:142, :330-338).
- Origin `EntryParameter`/`StructuralParameter` -> `EntryParameter`; `BlockParameter`, `SpillAddress`, `StructuralObservation`, `ScalarAbiAddress`, `InstructionScratch`, `AbiTransport` -> `UnsupportedRangeShape` (:145-157). Only `InstructionResult` proceeds.
- Range must be a single fragment in the choice's block, no edge connectors and no edge use (:163-169).
- A fixed-register operand at or after the choice point -> `FutureFixedUse` (:170-177).
- Defining instruction: the proof-bearing arithmetic kinds are refused (`ProofBearingDefinition`, :180-189); only `MaterializeI64` is accepted; anything else -> `NonMaterializeDefinition` (:190-191).
- Provenance of the defining instruction must have one operation, one value equal to the register's source value, no edges, no obligations and nonempty work all at that operation (:196-210).

**Cases by shape.** Single-victim, single-block, single-fragment, constant-materialized unsigned-64 only; `RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1` is the only policy (:438-440). `unreachable!` at :194 after a match that established the kind.

**Same job elsewhere.** `literal_fold_rewrites.rs::derive_actions` (S/literal_fold_rewrites.rs:1198-1252) re-derives sole-use/producer admission for `MaterializeI64` with provenance checks at :1265-1280 that are the same provenance shape as here (:196-210). Duplicate producer-record test.

**Unused / one-caller code.** `classify_pressure_recovery` used in S/literal_fold_rewrites.rs:581, `S/mod.rs:170` and stage 07 (`recovery_classification` grep via `pressure_rematerialization_recovery`, inferred).

**Stated limits.** None as comments; `NoAdmittedRecoveryReason` values name each refusal.

**Fail-open risks.** `no_recovery` returns `Ok(NoAdmittedRecovery{..})` (:340-344): that is a refusal of recovery, not an acceptance. Classification mismatch in `future_uses` returns an error (:282, :302). Class: none seen.

---

## 6. A/register_writes.rs (348 lines)

**Purpose.** Walks a selected function with its register homes and records, per callee-saved register unit, which operands or implicit effects write it (witnesses), plus work counts.

**Shape.** `DirectTraversal` (:67, fields `physical`, `callee_saved`, `functions`, counters), `walk_function` (:119), `scan_instruction` (:150), `scan_implicit` (:205), `scan_units` (:225), `HomeIndex` (:26), `usage` (:259), `extent` (:289), `AllocatedCalleeSavedRequirementError` (:311).

**Inputs/outputs.** Reads `SelectedFunction`, `FunctionRegisterHomes`; produces `Vec<FunctionAllocatedCalleeSavedRequirements>` with `AllocatedCalleeSavedUnitRequirement{unit, witnesses}`.

**How it decides.** Only operands with access `Def` or `UseDef` count as writes (:161-165). The write units come from the home's physical view `write_units`; a unit counts only if it is in `callee_saved` (:233). Home must exist for each written register (`MissingHome`), be unique (`DuplicateHome`), match class and fixed view (:168-185).

**Cases by shape.** None beyond the above.

**Unused / one-caller code.** grep `DirectTraversal|register_writes::` over `source-rust/`: 6 in this file, 5 in stage 07 `preservation.rs`, 3 in `verification/published-objects/src/allocation.rs`. So two outside users (derivation and the independent verifier).

**Stated limits.** Header comment (:1-5): "These are witnesses and work counts, not allocation authority."

**Fail-open risks.** `scan_units` ignores written units not in `callee_saved` (:233): by design. Class: none seen.

---

## 7. A/rewriting.rs (659 lines)

**Purpose.** Shared helper functions for rewrites and for stage 07's recovery rewrites: block/edge mention probes, operand probes, work-step estimates, copy-row checks, byte builders for rewrite identities, and the bitset `PreAllocationPolicy`.

**Shape.**
- `block_edges` (:8): `member_position`, `function_at`, `block_instructions`, `block_instruction_ids`, `edge_surface`, `local_slot_mentions`, `successor_mentions`.
- `operand_probes` (:120): `effect_declaration` (unique declaration or None, :129), `last_definition_before`, `isolated_declaration`.
- `selected_operands` (:164): `selected_operand`, `copy_shaped`.
- Free functions: `rewrite_scan` (:222), `required_rewrite_budget` (:226), `function_transports` (:239), `function_scan` (:243), `charge_steps` (:249), `integer_copy_row` (:261), `declares_pair` (:285), `carrier_pair` (:299), `cone_propagation` (:330, `ConePropagation` :322), `plan_function` (:369), macros `receipt!` (:383) and `sealed!` (:396), `instruction` (:404), `register_occurs_once` (:427), `rewrite_bytes` (:492), `shifted_boundary_settlements` (:508), `constant_fold_steps` (:536).
- `PreAllocationPolicy` (:569): 10 named bits, `from_canonical_bits` rejects unknown bits (:653-658).
- `RegisterAllocationRuleTargetApplicability` (:218): a single-variant enum, `TargetIndependent`.

**How it decides.** `effect_declaration` returns `None` when zero or more than one declaration matches (:134-141). `carrier_pair` requires exactly two operands numbered 0 and 1, first `Use`, second the given access, no implicit uses/defs/clobbers and no fixed view, tie or early clobber (:299-320). `charge_steps` compares a single step count to `budget.validation_steps()` only (:255-257); it does not accumulate. `shifted_boundary_settlements` lowers an index `> removed` by one and errors when an index is past the block body (:508-534).

**Cases by shape.** `carrier_pair`/`declares_pair`: exactly two-operand `Use`+`Def`/`Use` rows. `PreAllocationPolicy`: a fixed set of ten rule bits (:574-593).

**Same job elsewhere.** `successor_mentions` (:64-118) and `register_occurs_once` (:427-490) both enumerate every register slot in successor bindings/structural bindings/case payloads; `liveness.rs:1623 has_edge_use` and `S/literal_fold_rewrites.rs:1352-1396 sole_use` enumerate the same slots again; `S/copy_and_extension_rewrites.rs:1153-1220 lower_successor` and `S/literal_fold_rewrites.rs:1938-1995` enumerate them a fifth and sixth time for renumbering. Six hand-written walks of the same transport slots.

**Unused / one-caller code (grep, `source-rust/` `*.rs`).**
- `required_rewrite_budget`: defined :226; 5 call sites, all in stage 07 (`rewrites.rs:551`, `runtime_spill_and_split.rs:272, 4159, 5214` and one more hit in `rewrites.rs`); none in stage 06 files.
- `function_transports`: 1 outside use (stage 07 `rewrites.rs:475`) plus 1 inside `required_rewrite_budget`.
- `shifted_boundary_settlements`: 2 call sites (S/branch_and_compare_rewrites.rs:1758, S/copy_and_extension_rewrites.rs:1054).
- `register_occurs_once`: 1 call site (S/literal_fold_rewrites.rs:2604).
- `rewrite_bytes`: 1 call site, inside the `rewrite_receipt_identity!` macro (A/lib.rs:368).
- `block_instruction_ids`: 4 outside uses (S/literal_fold_rewrites.rs:2091/2147, stage 07 `fixed_view_copy_recovery.rs:63/166`, `pressure_rematerialization_recovery.rs:445/478`).
- `RegisterAllocationRuleTargetApplicability`: only one variant; used as a field in the catalogs at A/lib.rs:425 and S/literal_fold_rewrites.rs:987-1015.

**Stated limits.** Header comment names these as primitives "used by selected-instruction optimizations and register-pressure recovery rewrites."

**Fail-open risks.**
1. `charge_steps` (:249-259) checks `steps > budget.validation_steps()` only against the validation-step axis, not the other four axes. Class: open. Wrong input: none that changes the program; it affects only the work bound (an under-charged rewrite is accepted).
2. `member_position`/`function_at`: return `None`/error when not found; callers map to `SourceMismatch` (closed).

---

## 8. A/spill_choice.rs (613 lines)

**Purpose.** For one function, simulates an in-order linear scan assigning register views; at the first register that cannot be seated it records which resident or the incoming register should be spilled ("contenders") and picks the victim with the latest end, ties by highest register id.

**Shape.** `choose_spill_victims` (:510) -> `compute_terminal_spill_choices` (:48); `compute_function` (:143), `first_free` (:452), `early_clobber_conflicts` (:401), `common_candidates` (:374), `validate_local_shape` (:335), `interferes` (:482); `ValidatedSpillChoices`; `SpillChoiceError` (:565, 14 variants).

**How it decides.** Process registers sorted by `(start, register id)` (:165-170); candidates are the intersection of every span's candidate views (:385-391, and early-clobber points :389). First free candidate = first in `BTreeSet` order with no overlapping active footprint among interfering registers and no early-clobber conflict (:464-479). On failure the incoming register and every resident for which freeing a view works become contenders; `max_by_key((exclusive_end, register id))` picks the victim (:310-314). Only one choice per function is produced; the loop `return`s at the first pressure failure (:315).

**Cases by shape.**
- Refuses tied pairs (`UnsupportedTiedOperands`, :109-111); early clobbers only when no pressure arises (:219-223 refuses once a spill is needed); registers with entry transitions (:187-192).
- Pressure shape: everything in one block, single fragment, no edge connectors (:225-271, :335-357) else `UnsupportedPressureShape`.
- Only policy `SingleBlockFarthestEndThenHighestVregV1` (:68-70).

**Same job elsewhere.** The first-fit view search also exists in stage 07 (`home_assignment.rs`, not read); suspected.

**Unused / one-caller code.** `choose_spill_victims` is called at S/literal_fold_rewrites.rs:569 and re-exported at S/mod.rs:170.

**Stated limits.** None as comments.

**Fail-open risks.** none seen. Errors for every unsupported shape.

---

## 9. A/tied_registers.rs (121 lines)

**Purpose.** Union-find over the "tied" edges (source register -> definition register) so early-clobber validation can count how many early definitions are connected through chains of ties.

**Shape.** `TiedRegisters` (:6), `new` (:19), `contains` (:68), `is_only_early_definition` (:72), `component_root` (:87).

**How it decides.** Keyed on virtual-register id. `is_only_early_definition(source, definition)` is true when the pair is a direct edge and the definition's component has exactly one early definition (:72-84).

**Unused / one-caller code.** `TiedRegisters::` grep: used by liveness.rs:1010 and live_ranges.rs:254 (2 users).

**Fail-open risks.** `new` ignores early definitions that are not in any tie edge (`if let Ok(index) = ...`, :55-60) by design; they are handled by `!ties.contains(...)` at the callers. Class: traced.

---

## 10. A/work_accounting.rs (179 lines)

**Purpose.** Counters for five work axes (`rule_evaluations`, `candidates`, `validation_steps`, `commits`, `iterations`) with overflow-checked adds and a final admission against a budget.

**Shape.** `WorkDefect` (:12), `count` (:22), `combine` (:26), `admit` (:43), `Work` (:55) with `finish` (:84).

**Unused / one-caller code.** `combine`: used at S/literal_fold_rewrites.rs:269 (1 site). `admit`: used at S/literal_fold_rewrites.rs:277 and :2325, and inside `Work::finish` (:88). `Work` is `pub(crate)`; used by recovery_classification.rs:392 and spill_choice.rs:71.

**Fail-open risks.** none seen. Overflow returns `Overflow`; over budget returns `BudgetExceeded`.

---

## 11. src/lib.rs (11 lines)

**Purpose.** Declares crate attributes, `#[macro_use] extern crate selected_instruction_analyses` (:3-4) so the analysis macros are visible, and re-exports `SelectedInstructionOptimizationError`, `SelectedInstructionOptimizationOutput`, `SelectedOptimizationReplay`, `optimize_selected_instructions` from `selected_rewrites` (:8-11).

---

## 12. S/mod.rs (207 lines)

**Purpose.** Module declarations and re-exports for the rewrite families; defines the sealing trait `ValidatedSelectedAnalysis`.

**Shape.** `selected_input` module (:45-143): a private `Sealed` trait (:61-63), `ValidatedSelectedAnalysis: Sealed + SelectedProgramView` (:65), implemented for `SelectedProgram`, `OwnedSelectedProgram`, `SelectedProgramRef` and, via the `validated_selected_analysis!` macro (:74), for the ten `Validated*` rewrite outputs plus `ValidatedPreAllocationTransformation` (:112).

**How it decides.** The only way to produce a rewrite input is a type that implements the sealed trait; it is an allow-list of eleven types (:100-142).

**Cases by shape.** Ten rewrite families named at :47-51: address fold, constant boolean, constant branch, copied call operand, copy removal, dead compare, equivalent compare, literal fold, redundant compare, redundant extension, stored load forwarding (11 counting stored load forwarding at :134).

**Unused / one-caller code.** `ValidatedSelectedAnalysis`: used as a bound in every `fn admit`/`fold_*`/`remove_*` in the four rewrite files (grep `ValidatedSelectedAnalysis` in S/: many; users are only S/ files).

**Stated limits.** Header comment (:14-25): "unchecked ... can never enter the shared selected-program analyses" and "The default compile does not replay the finished run".

**Fail-open risks.** none seen.

---

## 13. S/address_and_load_rewrites.rs (1429 lines)

**Purpose.** Two rewrites: (a) address folding, which merges an `AddressOffset` (base+constant) into a following load/store/address-offset by adding the constants into the consumer's offset field and reading the base register directly; (b) stored-load forwarding, which replaces a load by a copy/zero-extend of the register that was stored to the same bytes earlier.

**Shape.**
- `address_fold` (:11-346): `admission::admit` (:82), `consumer_shape` (:34), `admitted_offset` (:61), `rewritten` (:233), `fold_selected_address` (:250), `seal_address_fold` (:292), `AddressFoldError` (:333, 9 variants).
- `load_forwarding` (:348-1429): `admission::admit` (:420, very long), `forwarding_source` (:897), `place_store_value` (:988), `sequence_source` (:1015), `edge_preserves` (:1059), `registers_untouched` (:1115), `merged_parameter` (:1131), `candidate_accesses` (:1187), `reject_barrier` (:1206), `reject_unaccounted` (:1230), `interferes` (:854), `forward_selected_stored_load` (:1314), `StoredLoadForwardingError` (:1416).

**Inputs/outputs.** Each takes a `ValidatedSelectedAnalysis` source, a function index, an instruction id, a `ValidatedTargetRegisterEnvironment` and a work budget; returns a `Validated*` carrying the transformed plan and a receipt. `address_fold` rewrites one instruction; `load_forwarding` rewrites the load into a copy and removes its `memory_accesses` row (:1326).

**How it decides.**
- address_fold: consumer kinds `Load8/16/32/64`, `Store` of size 1/2/4/8, `AddressOffset` (:34-58); producer must be the last in-block definition of the pointer register and an `AddressOffset` (:125-137); between producer and consumer the base register must not be redefined (:168-174); sum of offsets must be accepted by `admitted_offset` (:61-72).
- load_forwarding: walks backward through the CFG from the load keyed on `memory_accesses` rows (place id, byte offset, byte count, access role) via `interferes` (:854-895); a store of equal place/offset/size is the forwarding source (:918-945); merges at joins by equal resolved registers or a block parameter all predecessor edges bind to that register (:595-697, :1131-1185).

**Cases by shape (quoted/cited).**
- `admitted_offset` (:61-72): `Aarch64` -> `combined.is_multiple_of(scale) && combined / scale <= 4095`; `X86_64` -> `combined <= i32::MAX`.
- Forward width cases: `match forwarded.byte_count { 8 => CopyI64, 4 => ZeroExtendU32, 2 => ZeroExtendU16, _ => ZeroExtendU8 }` (:797-802).
- `reject_barrier` (:1206-1228): calls, hosted ops, returns, jumps and conditional branches are barriers; every other kind passes this check.
- `forwarding_source` accepts only `Store` and `Store64{slot: Local}` (:921-985).

**Same job elsewhere.** The row-class checks (`declares_pair`, class equality) repeat in S/copy_and_extension (`copy_removal` :918-929, `redundant_extension` :1557-1568) and here (:111-124, :154-167). `place_store_value` and the `Store64` operand check (:962-983) both extract the stored value; two ways to read a store's value operand.

**Unused / one-caller code.** `fold_selected_address` and `forward_selected_stored_load` are `pub(crate)`; their callers are in `rewrites.rs` (not read). `address_fold::measured_steps`/`validation::measured_steps` are used in `rewrites.rs` (grep `measured_steps|surface_sizes` S/rewrites.rs: 28 hits across the family).

**Stated limits.** Header only (:1-6). `AddressFoldError::UnsupportedOffset`/`UnsupportedUse` are the refusals.

**Fail-open risks.**
1. `candidate_accesses` + `reject_unaccounted` (:543-558): an instruction with no `memory_accesses` row is classified by `unrostered_reach` (defined in `S/rewrites.rs:2646`, not read): `Private` lets the walk pass it. Class: open (depends on `unrostered_reach`, not read). Wrong input: a memory-writing instruction kind that `unrostered_reach` treats as `Private` and that has no row would be walked over, and the forwarded value could be stale.
2. `interferes` (:854-895): `WriteOutgoing`, `AddressOutgoing`, all `Read*` roles and `ReadLocal` return `false` for interference. Class: open (no comment says why outgoing-argument writes cannot alias a place).
3. `reject_barrier` passes every kind not in its list (e.g. `AtomicFence`, `RegisterAccess` operations, `CopyBytes`, `ZeroBytes`, `InvokeNoNormal`) to the row-based check. Class: open; effects of those kinds depend on their memory rows or on `unrostered_reach`.
4. Inferred, not traced: `fold_selected_address` (:250-263) changes the instruction's byte offset but not its `memory_accesses` row, while forwarding requires `read.byte_offset == encoded_offset` (:460) and `byte_offset == forwarded.byte_offset` (:928); a folded access therefore fails those equalities and is refused rather than misjudged.
5. `head_looped` (:699-761): when the load can reach itself the tail after the load is additionally scanned; written as `AliasingWrite` if a different value is stored. Class: traced.

---

## 14. S/branch_and_compare_rewrites.rs (2403 lines)

**Purpose.** Condition-flag rewrites on the selected CFG: (a) `constant_boolean` turns `MaterializeBoolean*` into `MaterializeI64 0/1` when the reaching compare has constant operands; (b) `constant_branch` turns a conditional branch into a `Jump` to the decided successor; (c) `dead_compare`, `redundant` and `equivalent` compare removal delete a flag-producing compare that nobody reads, repeats an identical earlier compare, or yields an equivalent result.

**Shape.**
- `constant_boolean`: `admission::admit` (:60), `pair::ConditionMaterializationRule` (:307) with 15 const rules (:326-384, table :451-467), `rewritten` (:262), `fold_selected_constant_boolean` (:516), `surface_sizes` (:557), `measured_steps` (:592), `ConstantBooleanError` (:655).
- `constant_branch`: `admit` (:716), `pair::TerminatorPairRule` (:926) with 9 rules (:948-981, table :1101-1111), `decided_successor` (:1018), `rewritten` (:885), `fold_selected_constant_branch` (:1163), `ConstantBranchError` (:1298).
- `dead_compare` (:1314-2403): `admission::admit` (:1670, uses `audit_dead_unit` :1632), `admit_shadowed_compare` (:1474, shared by redundant/equivalent), `audit_stable_paths` (:1387), `agree_compare_removal_row` (:1804), `compare_removal_steps` (:1579), `apply` (:1745), `remove_compare` (:1770); `equivalent` (:1840) with `compare_sides`/`sides_equal` (:1867-1930); `redundant` (:2104); `DeadCompareError` (:2392).

**Inputs/outputs.** Same shape as other rewrites; additionally the effect catalog for constant folds (`PreparedMachineEffectCatalog`). Compare removals delete one instruction and shift boundary settlements (:1758).

**How it decides.**
- Constant folds key on the machine semantic kinds of (compare, consumer) pairs from a rule table (`condition_materialization_for`, `terminator_pair_for`, :187-191, :834-838) and on the unique reaching compare site found by `reaching_event` (in rewrites.rs) for each flag unit the consumer reads; `resolved_operands` gets the constant operand values (:209-211, :845-846).
- Decided value: `Equal` `left == right`; unsigned/signed less-than and less-or-equal compare as `u64`/`i64` (:399-413). Branch: `NonZero` -> `left != right` picks `when_nonzero` (:1032-1036); less-than -> `left < right` picks `when_less`.
- Removals key on the units the compare defines/clobbers: dead compare requires no later read before a kill on any path (`audit_dead_unit` :1632-1668); shadowed compares require every reaching event of each defined unit to be an equivalent compare and that registers in the compare are not rewritten on the paths (`audit_stable_paths` :1387-1450).

**Cases by shape.** `CompareI64`, `CompareI64Immediate`, `CompareI64Zero` as producers (`declared_producers`, :481-487, :1125-1132); consumers: `MaterializeBoolean{Equal, U64LessThan, I64LessThan, U64LessOrEqual, I64LessOrEqual}` and branches `NonZero`, `U64LessThan`, `I64LessThan` (:948-981). `compare_operand_arity`: 2 for `CompareI64`, 1 for immediate/zero (:1344-1351). `equivalent::compare_sides`: `CompareI64`, `CompareI64Immediate`, `CompareI64Zero` only (:1867-1884).

**Same job elsewhere.**
- `declared_producers()` is defined twice with identical bodies (:481-487 and :1125-1132). The "flag universe" computation (union of implicit_defs of the three compare rows) is written four times: :136-149, :561-575, :758-771, :1207-1221.
- `isolated_declaration` private copy at :1134-1140 (requires `barrier == ControlFlow`) next to `rewriting::isolated_declaration` (A/rewriting.rs:156, requires `barrier == None`): same name, different meaning.
- `audit_dead_unit` (:1632) and `swapped_unit_readers_equality_only` (S/literal_fold_rewrites.rs:2503) are the same forward unit-reach walk with a different reader predicate.
- `dead_compare::measured_steps` (:2310) duplicates the step arithmetic inside `admit` (:1705-1730).

**Unused / one-caller code.** `measured_steps`/`surface_sizes`: 14 hits here and 28 in `S/rewrites.rs` (grep). `ConditionMaterializationRule::{producer,consumer,rewritten}` used by the `declared_kind_matchers!` macro; `TerminatorPairRule::{unit_flow, control}` used at :1072, :1084.

**Stated limits.** Header only (:1-7). `UnsupportedLiteral` error for non-constant operands.

**Fail-open risks.**
1. `reaching_event`/`reaching_events`/`resolved_operands`/`materialized_bits` (in `S/rewrites.rs`, not read) decide whether a compare is "constant" and "reaches". Class: open (not read).
2. `constant_branch::admit` (:716-883) keeps only the decided successor and drops the other edge (:847-850, :885-899), including the untaken edge's `work` rows and bindings. Class: open. Wrong input: none identified; the untaken block may become unreachable and is not removed here.
3. `remove_compare` error mapping (:2003-2008, :2183-2188): every `DeadCompareError` except `IdentityOverflow` becomes `SourceMismatch`. Class: traced (only `apply` produces these).
4. `decided_value` has `_ => unreachable!` (:410): reached only if a rule is constructed with a consumer outside the five materialize kinds; the const table has none. Class: traced.

---

## 15. S/copy_and_extension_rewrites.rs (1748 lines)

**Purpose.** Three rewrites: `copied_call_operand` replaces a direct-call argument that is a same-block copy by the copy's source register; `copy_removal` deletes a same-block `CopyI64` by renaming its output uses to its input and renumbering all later register ids; `redundant_extension` turns a zero/sign extension into a plain copy when its producer already guarantees the upper bits.

**Shape.**
- `copied_call_operand` (:11-758): `admission::admit` (:40), `pinned_view_written_by_call` (:264), `mentioned_after_call` (:302), `pair::CopiedCallOperandRule` (:379, rules `COPY_CALL_UNIT/SCALAR/AGGREGATE` :400-414, table :523-527), `retained_call` (:553), `call_alternative` (:583), `fold_selected_copied_call_operand` (:628), `CopiedCallOperandError` (:745).
- `copy_removal` (:760-1385): `admission::admit` (:861), `apply` (:1031), `redensify` (:1066), `lower_register` (:1242), `lower_successor` (:1153), `remove_selected_copy` (:1265), `copy_scan` (:1350), `CopyRemovalError` (:1337).
- `redundant_extension` (:1387-1748): `Extension` (:1413), `Normalization` (:1431), `producer_normalization` (:1438), `identity` (:1482), `admit` (:1503), `remove_selected_redundant_extension` (:1646), `RedundantExtensionError` (:1733).

**How it decides.**
- copied_call_operand: producer must be a `CopyI64` earlier in the same block that is the last definition of the destination; consumer must be a `CallUnit/CallScalar/CallAggregate` whose operand list is `n` uses then up to two defs, every row operand with a fixed view (:459-471); at most one fixed view across all uses of the source (:192-213); source not redefined between copy and call (:214-223).
- copy_removal: only same-block uses after the copy, all plain `Use` operands with no fixed view/tie/early clobber (:964-985); no use in successor bindings, case payloads or frame/hosted slots (:961-992); input register not redefined before the last use (:997-1011).
- redundant_extension: unique defining instruction of the value, kind-based normalization table (`Load8` -> zero-extended 8, `LoadPacked` -> meaningful low `width*8` only if `defined_operand == 1`, `ZeroExtendU32` -> meaningful low 32, `Float32ToBits` -> meaningful low 32, `MaterializeBoolean*` -> zero-extended 1, `MaterializeI64` -> its literal pattern, :1442-1480); `identity` compares extension form to normalization (:1482-1501).

**Cases by shape.** Calls limited to results roster `Empty/Scalar/Aggregate` with 1 or 2 aggregate results (`results_operands.len() <= 2`, :441-445). `copy_removal` works on `InstructionResult` or `InstructionScratch{operand: 1}` registers only (:903-910).

**Same job elsewhere.** `copy_removal::redensify` (:1066-1101) and `literal_fold::function_rewrite::redensify` (S/literal_fold_rewrites.rs:1885-2007) both renumber ids after removing a register and walk every id-bearing field; the first handles registers only, the second also instruction ids. Same job twice. `terminator_successors_mut` is defined privately at :1114 while `S/literal_fold_rewrites.rs:1693` imports `selected_instructions::terminator_successors_mut`.

**Unused / one-caller code.** Entry functions are `pub(crate)`; callers in `S/rewrites.rs` (not read). `copy_scan` (:1350) used at :1013 and :1292.

**Stated limits.** Header (:1-6) only.

**Fail-open risks.**
1. `copy_removal::lower_register` (:1242-1255) decrements every id greater than the removed one and `apply` removes by position (`contracts.virtual_registers.remove(admitted.register_index)`, :1062), assuming register ids equal their positions. `admit` (:861-1029) never calls `ids_are_positions()`; `literal_fold` does check it (S/literal_fold_rewrites.rs:2139). Class: open. Wrong input: a function with non-dense register ids would be renumbered incorrectly.
2. Both `copy_removal::apply` (:1061) and `dead_compare::apply` (S/branch_and_compare_rewrites.rs:1766) remove an instruction without renumbering instruction ids; `literal_fold::validate_dense_identifiers` (S/literal_fold_rewrites.rs:2135-2158) then refuses a function whose instruction ids are not `0..count`. Suspected interaction (order of phases is in `rewrites.rs`, not read): a literal fold after a copy removal in the same function would error, not skip.
3. `mentioned_after_call` and `pinned_view_written_by_call` only affect refusal. Class: none seen.
4. `redundant_extension::admit` (:1569-1581) searches only `block.instructions` for the producer, not terminators, so a register defined by a terminator yields `UnsupportedProducer` (closed).

---

## 16. S/literal_fold_rewrites.rs (4197 lines)

**Purpose.** The "selected lowering" phase: folds a constant (`MaterializeI64` of a u64 value) into its only consumer, replacing the consumer with an immediate form or a constant/copy and deleting the constant's instruction and register. The rule catalog has 23 optimization entries (`SELECTED_LOWERING_RULE_CATALOG: [_; 23]`, :1022; counted by hand at lines 1023-1148). The phase loops: choose a pressure victim, classify it, fold, re-analyze, until a pass applies nothing.

**Shape.**
- `literal_folds` (:11-979): `accounting` (:23-305: receipts, completion identity `omega.selected-lowering-optimization-completion.v5`, :132), `execution` (:306-696: `execute_selected_lowering_optimizations` :329, `validate_selected_lowering_optimization_custody` :415, `build_attempt` :558, `complete_attempt` :615, `replay_step` :655, `charge_step_measure` :679), `run_selected_lowering_optimizations` (:724), `StagedOptimizedLiteralFoldStep` (:740), `StagedSelectedLoweringOptimizationRun` (:782), custody receipts (:845-937), `OptimizedLiteralFoldCustodyError` (:940, 20 variants).
- `selected_lowering` (:981-4197): `catalog` (:982-1166), `literal_fold` (:1167-4161): `compute::actions` (`derive_actions` :1198, `sole_use` :1307, `admit_candidate` :1409), `compute::constraints` (`select_admitted_pairs` :1621, `validate_immediate_row` :1655), `compute::function_rewrite` (`apply_actions` :1701, `rewritten_consumer` :1801, `redensify` :1885), `compute::functions` (`derive_function_folds` :2093, `validate_dense_identifiers` :2135), `compute::usage` (`fold_usage` :2193), `compute_terminal_literal_fold` (:2351), `pair_rule` (:2393-3973: `PairOperandShape`, `PairTailCustody`, `PairImmediateBound`, `PairMachineEffects`, `PairUnitEffects`, `SelectedInstructionPairRule` :3709, ~60 named rule constants :2611-3229), `receipt` (:3974), `fold_selected_incoming_literal` (:4035), `LiteralFoldError` (:4137), `resolve_selected_lowering_rules` (:4179).

**Inputs/outputs.** Reads the staged legality stage (selected program, ranges, legality, availability, selections, budget). Per iteration builds spill choices, recovery classification, a `ValidatedLiteralFold`, then fresh liveness, ranges and legality; the run records every step and a terminal "attempt" that applied nothing. `record()` (:824) lists the applied folds as `PostAllocationSelectedTransformation::LiteralFold`.

**How it decides.**
- Candidate: a `MaterializeI64{Unsigned(v)}` whose single operand is a plain `Def`, with provenance of exactly one operation, one value, no edges/obligations and work only at that operation (:1265-1280); register type is a non-address integer admitting `v`, no entry fixed view, origin `InstructionResult` of this instruction (:1282-1305).
- Sole use: exactly one `Use` of the register in the whole function, in the same block, in an instruction body after the literal, with no appearance in any successor binding, structural binding or case payload (:1307-1406).
- Pair rule match by consumer machine kind, victim operand position and immediate bound; exactly one enabled rule may match (`for_consumer`, :1581-1597). `SelectedInstructionPairRule::admits_immediate` (:3757-3763): `Encoding(limit)` `value <= limit`; `EncodingNonZero`; `Exactly(exact)`.
- Effect admission: the ISA effect declarations of producer, consumer and rewritten kind must have isolated/transparent shape (`admits_producer`, `admits_consumer`, `admits_rewritten`, :3385-3522); fault-discharge by literal (divisor 1, -1, or dividend 0 with an obligation) is a flag on the rule (:3525-3534).
- Cleanup tail: operands after the grammar must be auxiliary zero-defined uses or single-occurrence dead scratch defs (`PairTailCustody`, :3598-3611, `auxiliary_zero_defined` :2578, `dropped_def_is_dead` :2600).
- Operand-swapped compare (`COMPARE_LEFT_*`) requires that every later reader of the consumer's flag units is an equality reader (`MaterializeBooleanEqual` or `ConditionalBranchNonZero`, :2495-2541) unless both operands are the same register (:3467-3492).
- Loop termination: iteration bound = initial virtual register count, each step must reduce the register count by exactly the applied count (:679-695, :362-368); the terminal attempt must apply nothing (:386-390).

**Cases by shape.**
- Catalog row families (:1022-1149): U12 add/sub/compare/zero compare (left and right literal), six extension folds, `Load8Indexed` offset, copy, byte-view address offset, exact divide by 1 and 0 dividend, wrapping remainder by 1, -1 and zero dividend, bitwise and/xor/add with zero or all-ones, saturating add/sub/divide with identities per carrier (U8..U64, I8..I64), upper-bound saturation per unsigned carrier.
- `fold_immediate` (:3765-3791): result kinds `SurvivingOperand`/`SwappedOperand` use the literal, `ConstantOfLiteral` -> `carrier.maximum_bits()` for `SaturatingAdd` else 0, `LiteralRecompute` per extension kind.
- `rewritten_row_roles` (:3735-3756) admits only 4 (result_kind, result) combinations, `None` otherwise.
- `immediate_constraint_key` (:3801-3820): `ExactAddI64Immediate`, `ExactSubtractI64Immediate`, `CompareI64Immediate`, `CompareI64Zero`, `MaterializeI64`, `Load8`, `AddressOffset`, `CopyI64`; other rewritten kinds -> `None` -> `ImmediateConstraintMismatch`.
- `rewrite_consumer` (:3822-3914) is an exhaustive `(rewritten, consumer kind)` match with `_ => None`.
- `scalar_materialize_value` (:3962-3972): integer type of 64 bits or less, non-address.
- Target rule: all 23 rows are `TargetIndependent`; the immediate bound `Encoding(4095)` (u12) is applied on every target, including `x86_64`.

**Same job elsewhere.**
- Producer provenance test at :1265-1280 is the same test as recovery_classification.rs:196-210.
- `apply_actions::redensify` duplicates the id renumbering of S/copy_and_extension_rewrites.rs:1066-1101.
- `sole_use` (:1307-1406) duplicates the transport-slot walk in rewriting.rs:64-118, :427-490 and liveness.rs:1623.
- `swapped_unit_readers_equality_only` (:2503) duplicates `audit_dead_unit` (S/branch_and_compare_rewrites.rs:1632).
- `validate_selected_lowering_optimization_custody` (:415-528) replays the whole run; the run execution (:329-413) is the same loop. Two copies by design (replay).

**Unused / one-caller code.** `build_step` (:531) only called by `replay_step` (:663). `PairLiteralPosition::Sole` etc. all used. `for_position` (:1599) used in `usage::fold_usage` (:2269). `StagedOptimizedLiteralFoldAttempt` accessors `choices()/recovery()/fold()` used in custody validation. `selected_lowering_custody_receipt` is called twice (:393, :512).

**Stated limits.** Header (:1-6): "Optional replay re-executes each committed step with the producer to check determinism." Error variants name refusals; `TerminalAttemptApplied`, `SelectedLoweringIterationBoundExceeded`.

**Fail-open risks.**
1. `usage::fold_usage` (:2251-2267): when an action's consumer or victim operand cannot be found it `continue`s, so that action is not charged for the operand-swapped audit. Class: open. Wrong input: none that changes the program; the work counts would be under-reported for an action whose consumer is missing (which `apply_actions` later refuses with `DecisionMismatch`, :1718-1720).
2. `derive_actions` (:1245) calls `rewritten_consumer(0, ...)` with a constant function index 0 as a filter, so errors carry the wrong function number but acceptance is by `is_ok()`. Class: traced (affects error values only).
3. `derive_function_folds` (:2115, :2135-2158): refuses functions whose ids are not dense positions with `FunctionMismatch` (closed, so a non-dense function stops the phase).
4. `PairUnitEffects::admits_consumer` (:3283-3297) rejects tied operands always and fixed-view/early-clobber operands unless the rule says they are dropped with the operand list. Class: stated by the named constants (`BOUND_CONSUMER_OPERANDS`, `BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS`).

---

## Coverage

Read in full (all lines of each file):
- `analyses/src/lib.rs` 1-1157 (1157 lines)
- `analyses/src/live_ranges.rs` 1-1859 (1859)
- `analyses/src/liveness.rs` 1-2309 (2309)
- `analyses/src/machine_effects.rs` 1-1370 (1370)
- `analyses/src/recovery_classification.rs` 1-588 (588)
- `analyses/src/register_writes.rs` 1-348 (348)
- `analyses/src/rewriting.rs` 1-659 (659)
- `analyses/src/spill_choice.rs` 1-613 (613)
- `analyses/src/tied_registers.rs` 1-121 (121)
- `analyses/src/work_accounting.rs` 1-179 (179)
- `src/lib.rs` 1-11 (11)
- `src/selected_rewrites/address_and_load_rewrites.rs` 1-1429 (1429)
- `src/selected_rewrites/branch_and_compare_rewrites.rs` 1-2403 (2403)
- `src/selected_rewrites/copy_and_extension_rewrites.rs` 1-1748 (1748)
- `src/selected_rewrites/literal_fold_rewrites.rs` 1-4197 (4197)
- `src/selected_rewrites/mod.rs` 1-207 (207)

Total read in full: 19,198 lines. Skimmed: 0. Not read: none. (Greps and small reads outside the unit were limited to counting callers; I did not read `rewrites.rs`, `selected_optimization.rs`, or the `selected_instructions`/`register_homes` representation crates that these files call.)

## Five things a maintainer most needs to know about this unit

1. The same legality rules for tied operands and early clobbers are written twice: `liveness.rs:839-1033` and `live_ranges.rs:204-422`, and the transport-slot walk (register slots in successor bindings, structural bindings, case payloads) is hand-written at least six times (`rewriting.rs:64,427`, `liveness.rs:1623`, `literal_fold_rewrites.rs:1352`, and the two renumbering walks `copy_and_extension_rewrites.rs:1153`, `literal_fold_rewrites.rs:1938`). A new transport slot kind has to be added in each.
2. Each rewrite is a three-step trio: `admission::admit` (all judgment), a mechanical `rewritten`/`apply`, and a `seal_*` that only builds a receipt and identity (`sealed!` at rewriting.rs:396). Nothing in this unit re-validates the transformed plan after the edit; acceptance rests entirely on `admit`. Replay of the pass is opt-in and lives in `selected_optimization.rs`/`literal_fold_rewrites.rs:415`.
3. Id density is assumed unevenly: `literal_fold` requires dense register and instruction ids (:2139) and refuses otherwise, `copy_removal` renumbers registers assuming ids equal positions without checking (copy_and_extension_rewrites.rs:861-1029, :1249), and `copy_removal`/`dead_compare` leave instruction-id gaps (copy_and_extension_rewrites.rs:1061, branch_and_compare_rewrites.rs:1766).
4. Unit-testing is thin on the rewrites: S/ has 2 `#[test]` functions total (1 in `address_and_load_rewrites.rs`, 1 in `literal_fold_rewrites.rs`); the analyses have 22. The rewrite families are covered outside this unit (not read).
5. `UnsealedRecoveryFacts::seal` (lib.rs:955-962) swallows a failed full liveness recomputation and keeps the ranges-only row; three `*_reusing` analyses have no callers (grep shows definition and re-export only); `compute_function_units_filtered` has a single caller in stage 07 (`runtime_spill_and_split.rs:3061`).

## What I am unsure about

- Whether `reaching_event`, `resolved_operands`, `materialized_bits`, `unrostered_reach` (all in `S/rewrites.rs`, not read) fail closed; several fail-open items above (address_and_load #1, #3; branch_and_compare #1) depend on them.
- Whether the precomputed block rows read through `function.blocks.block_virtual_transfer`, `block_unit_masks`, `block_unit_transfer`, `block_copy_affinities` (in `selected_instructions`, not read) can ever be absent for a valid program; liveness.rs:285, :297, :313 and live_ranges.rs:582 treat absence as empty/fallback.
- The order in which stage 06 runs the SelectedLowering and PreAllocation phases (in `rewrites.rs`/`selected_optimization.rs`), which decides whether the instruction-id-gap interaction in "five things" item 3 can happen.
- The `mod allocation_input` file included by `#[path]` at lib.rs:553 (`representations/selected-instructions/allocation_input.rs`) was not read; `SelectedAllocationInput` is defined there.
- Call-site counts for `choose_spill_victims`, `classify_pressure_recovery`, `fold_selected_*` etc. were taken from my reading and spot greps, not an exhaustive count per name.
