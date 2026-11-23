# Record: unit 015 gap 002 (area 10-native-stages-04-10)

File: `source-rust/subsystems/compiler/psi-to-native/pipeline/07_selected-instructions-to-register-homes/src/allocation/runtime_spill_and_split.rs`, lines 541-5493 (4,953 lines). All read in full. Line 541 starts inside `pub(crate) mod runtime_spill` (declared at line 540, grepped); the file ends at 5493 with the close of `runtime_split`. Paths below are relative to `src/allocation/`.

## Overview

Stage 07 can free a register under pressure by "spilling" (store the value to a frame slot after each definition, reload it before each use) or "splitting" (insert a copy so a value pinned to a fixed register has a second, flexible name). This range holds the second half of the file: the spill module from `admission` onward, then the whole split module. Both are admit-then-rewrite-then-seal: a derivation function checks the victim register and refuses with an error enum; a rewrite function clones the plan and mutates one function; `seal` wraps the result with a receipt (`sealed!` macro, `runtime_spill_and_split.rs:3180`, `:5445`; macro defined elsewhere, not read).

## 1. `runtime_spill::admission` (541-801)

- Purpose: builds the instruction and register rows for one reload (frame-address, `Load64`, optional bits conversion) and one store (optional conversion, `Store64`) of a spilled value.
- Shape: `fresh` (549) bumps a counter with `checked_add`, error `IdentityOverflow`. `Reload` (557) with `into_streams` (567). `reload` (585). `Store` (691), `into_streams` (699), `store` (713). `frame` (798) wraps a slot as `FrameStorageSlotId::Local`.
- Inputs/outputs: reads a `RuntimeSpillDerivation` (3463) for the constraint rows (`address`, `load`, `store`), `carrier_class`, `bits_conversion`, `lineage`, `victim`. Returns fresh `VirtualRegister` and `SelectedInstruction` rows built through `crate::allocation::rewrites::instruction` (read at 665, 674, 681, 779, 786; defined outside this unit).
- Decides: whether a conversion is present, keyed on `admitted.bits_conversion` (593, 720). Register origin is keyed on `VictimLineage` (Scalar vs Structural) at 625, 647, 759. The store address scratch exists if the store constraint row has a second operand (733-750).
- Cases by shape: the reload always uses `Load64` and the store `Store64` at byte offset 0 (676, 788); only 8-byte slots. Conversion is at most one (float to bits) step.
- Fail-open: none seen.

## 2. `runtime_spill::case_payload_address` (802-941)

- Purpose: confirms that a case-payload load really reads the field of the matched case at the stated offset, by walking the address back through `AddressOffset` instructions to the case source (local frame slot or borrowed pointer).
- Shape: `case_payload_address_corresponds` (807, `#[allow(clippy::too_many_arguments)]`, 8 arguments). Returns `bool`.
- Inputs: the `SelectedFunction`, a block index, the load, `SelectedCaseDispatchSource`, an `EdgeId`, field offset, byte count, displacement. Reads `function.local_storage_slots` and `function.memory_accesses`.
- Decides on: exactly one memory-access row for the load with `origin == Edge(edge)`, `role == ReadPlace`, same place, offset and byte count (846-861); field offset aligned to its byte count and inside the slot size (839-845); every producer in the chain is an instruction whose provenance edges are exactly `[edge]` with no implicit uses, defs or clobbers (862-868); chain made only of `AddressOffset` and a terminal `FrameAddress` at offset 0 (900-938); the final summed offset equals `field_byte_offset` (891, 935). Judgment is by identity (edge id, place id, register id) and numeric offset.
- Cases by shape: only `AddressOffset` and `FrameAddress { byte_offset: 0 }` producers; `_ => return false` (937). The borrowed-pointer root must have no definition in the block (891).
- Same job elsewhere: not searched beyond the one caller. `grep -rn -F case_payload_address_corresponds source-rust`: 2 lines (definition 807 and the call at 4811).
- Fail-open: none seen. Every unmatched case returns `false`. Note: the returned `false` makes the caller refuse (4809-4828), so unknown shapes are refused.

## 3. `runtime_spill::rewrite` (942-1474)

- Purpose: executes the spill. Inserts a store after each definition, a reload before each use, rewrites terminator operands and successor bindings that name the victim, remaps boundary settlements, replaces block contents in the cloned plan.
- Shape: private `reload_for_use` (959). Public entries `spill_selected_runtime_value` (985, default policy `UnitWriteBounded`), `spill_selected_runtime_value_with_span_policy` (1002), `spill_selected_runtime_value_with_crossing_status` (1021, the real body; returns the sealed spill and a `crossed_unit_write` bool).
- Inputs/outputs: `derive_runtime_spill` (3486) gives the `RuntimeSpillDerivation`. The body clones `source.selected_plan()` and calls `functions.mutate_preserving_register_prefix` (1040-1042). Output `ValidatedRuntimeSpill` via `validation::seal_runtime_spill_with_span_policy` (1470).
- Decides on: block membership in `use_blocks` or having a definition (1055-1062); `shared_reload[block]` decides whether reloads are shared between uses (1063, 1196); an "active window" of instructions narrowed from `block_register_instruction_span_detail` plus definition anchors (1098-1142); if there are boundary settlements the whole block is the window (1098-1103). Operands are rewritten when `virtual_register == register`, access is `Use`/`UseDef`, and `(instruction id, operand)` is not in `retained_uses` (1174-1184). Terminator uses, `SelectedValueTransport::Registers`, `ScalarReference`/`Address` base registers, `WholeValue` structural bindings and case payload `Registers` are handled in turn (1333-1438). WholeValue bindings are matched to a `StructuralArgumentUse` by block, edge, place, source offset and size (1391-1397); a miss is `SourceMismatch` (1398).
- Cases by shape: fast path with window interior absent touches only the first and last window instruction (1259-1293). `unreachable!()` at 1356 and 1434 after a `matches!` check on the same transport.
- Same job elsewhere: the two edge-reload paths (settlement remap via `gap_tables`, `push_position`, `remap` from `rewrites::settlement_gaps`, imported at 957) are shared with other rewrites outside this unit (suspected, not read).
- Stated limits: none at this site.
- Fail-open: none seen. `has_settlements` with `remap(...).ok_or(SourceMismatch)` (1445-1446) refuses on mismatch. The `continue` at 1055-1062 skips blocks with no use and no definition, which is intended (no victim references there); traced: `use_blocks` is built by `derive_runtime_spill` from every block that references the register (3727-3732, 4049-4051), so a block that references the victim but is not in `use_blocks` would only arise from a reference that did not count as a use; every other access arm in `derive_runtime_spill` either counts a use, records a definition or returns `UnsupportedUse` (4031).

## 4. `runtime_spill::slot` (1475-2699)

- Purpose: chooses which 8-byte frame slot holds each spill window. It may reuse another victim's `Spill` slot when the two values never live in the same slot at once, otherwise declares a private slot named for the victim.
- Shape:
  - `reaching::DefinitionEntries` (1488), bitset dataflow with worklist (`compute` 1493), `replace`, `members`, and a unit test over word boundaries (1566-1590).
  - `BlockUsePositions` (1595; `unpinned`, `pinned`, `dedicated`, `end_of_block`) with `record_use` (1603).
  - `instruction_block` (1621), `Assignment` (1634), `BlockLoads` (1642), accessors `store_slot`, `load_slot`, `end_of_block_slot` (1648-1664). `load_slot` and `end_of_block_slot` fall back to the private slot when a row is missing (1656, 1663).
  - `assign` (1720): the main function. Union-find (`find` 2082) groups definitions that reach the same use into windows.
  - `summarize_window` (2096), `RawCandidateSlots::scan` (2177) and `get` (2355), `shareable` (2424), `loads_before` (2561).
  - `bits_transport` (2592), `entry_definitions` (2654), `physical_definition` (2665).
- Inputs: `SelectedFunction`, victim id, `StorageDefinition` list, per-block `BlockUsePositions`, `shared_reload`, `span_closes`, `reuse_slots`.
- Decides on:
  - A slot is a reuse candidate if it is a `LocalStorageSlotId::Spill` of another register, size 8, alignment 8 (1733-1741).
  - `RawCandidateSlots::scan` (2177) invalidates a candidate when its frame address register is shared between two slots, used by a non-load instruction, or reached by an `Invalidate` event (2229-2344). Judgment keyed on register id and slot id using per-block summaries (`block_slot_summary`, not read).
  - `shareable` runs a two-state (`EXISTING`/`NEW`, 1667-1668) dataflow per block and refuses if a victim load can see an existing store, or an existing load can see a new store (2459-2488), or at the end of a block (2481-2486).
  - `bits_transport` (2592): only `Binary32` and `Binary64` floats; checks the to-bits and from-bits constraint rows have exactly two operands, Use then Def, the expected classes, no fixed view, no tie, no early clobber, no implicit defs or clobbers (2619-2645).
  - `physical_definition` (2665): requires exactly one non-`Use` operand naming the register; none is `UnsupportedValue`, two or more is `UnsupportedUse` (2691-2696).
- Cases by shape: `bits_transport` `_ => return Err(UnsupportedValue)` for every scalar type but the two float formats (2611). Candidate slots must be 8/8 `Spill` (1734-1736).
- Same job elsewhere: slot reuse reasoning (stores and loads in a block, reaching definitions) resembles the liveness analysis in the analyses crate (suspected, not read).
- Stated limits: none by comment in this range.
- Fail-open:
  - `assign` when `candidate_slots` is empty and `definitions.is_empty()`: scans uses to decide `declares_private` (1743-1787). Traced: `definitions` is empty only for the entry-parameter case where `definition` is `None` and `entry_boundary` false... actually `parameter_definitions` returns `UnsupportedUse` if empty (4885), and `entry_definitions` returns one row, so the empty branch is reachable only from callers passing no definitions; I did not find such a caller in this range (inferred).
  - `window_of_position` windows with no mapped definition go to a sentinel `windowless` window whose slot defaults to `private` (1967-1976, 2011). Result: those loads read the private slot. If the private slot is not declared, `declares_private` is only set at 2046 when a window needs a slot; for a windowless window with loads, `has_loads` is true, `chosen` is looked up (2014-2042) and falls to `declares_private = true` (2046). So declared. Traced.
  - `RawCandidateSlots::scan`: `block_slot_summary` returning `None` is skipped via `continue` (2230-2232, 2293-2295), so a block without a summary contributes no accesses or invalidations. Whether `None` arises for a real block was not traced into `selected_instructions` (open).
  - `shareable` line 2513 `candidate.last_existing_store[block_index].unwrap()` over `store_blocks` (built from `last_store.is_some()`, 2394-2396): safe by construction (read).
  - `load_slot` falls back to `private` for a missing row (1656): `declares_private` may not be true then (open); intent appears to be that every emitted load has a row, since `assign` fills `assignment.loads` for all windows (2061-2078) in the shared branch, and the non-shared branch returns `loads: Vec::new()` with private for all.

## 5. `runtime_spill::snapshot_copy` (2700-3003)

- Purpose: for a spilled address that feeds a bulk `CopyBytes` (snapshot of a structural value), finds the instruction that actually names the victim as the source pointer, after checking that the copy length register was materialized as exactly `byte_size`.
- Shape: private `definition` (2706, backward walk with a step counter and `WorkBudgetExceeded`), `source_use` (2734, returns `Ok(Some(use_index))`, `Ok(None)` for "unrelated", `Err(UnsupportedUse)`), test module with four tests (2799-3002), using `baseline_target_register_environment(linux_arm64)`.
- Decides on: `CopyBytes` has exactly five operands, source and count are `Use` (2744-2751); direct pointer needs `source_offset == 0` (2752-2756); otherwise the pointer must be defined by `AddressOffset { byte_offset: source_offset }` with `source_offset != 0` from the victim (2769-2779); the count register must be defined by `MaterializeI64 { Unsigned(byte_size) }` (2787-2794). Keyed on register id, instruction kind and exact value.
- Cases by shape: five-operand `CopyBytes`; two-operand `AddressOffset`.
- Fail-open: `Ok(None)` for "no definition found" or "definition does not name the victim" (2758-2768): the caller `continue`s (4578-4580), skipping that access. Traced: skipped accesses do not add loads, so `offset != pending.byte_size` at 4655 refuses if the victim's bytes were not covered. A copy that reads the victim by another path is not counted, which could miss a use; the only guard is the later byte-count check and the `derive_runtime_spill` use scan, which refuses unknown operand uses with `UnsupportedUse` (4031). Traced: stated limit is the uncounted copy would still appear as an unmatched `Use` operand and be rewritten by the generic operand loop.
- Same job elsewhere: none seen in this unit.

## 6. `runtime_spill::unit_effects` (3004-3169)

- Purpose: refuses a spill whose inserted store or reload would clobber a hardware unit that is live at that point.
- Shape: `preserve_live_units` (3012). Calls `crate::allocation::analyses::liveness::compute::compute_function_units_filtered` (3061, defined outside this unit) restricted to blocks that need it.
- Decides on: clobber sets of the address/load/store constraint rows against `unit_live_in`/`unit_live_out` at each store site and each reload site (3098-3112, 3147-3152); end-of-block reloads check the terminator in and out (3154-3165). Keyed on register-unit identity.
- Fail-open: first guard returns `Ok(())` if no clobbers exist, or no block mentions any clobbered unit (3032-3043); traced: nothing to preserve in that case. Empty-liveness blocks skip with a position bound check only (3073-3097, 3119-3136); `block.instructions.is_empty()` in the liveness row is trusted to mean no unit is live (inferred; the filtered computation only covers `needed_blocks`).
- Callers: `grep -F preserve_live_units`: 2 lines (definition 3012, call 4668).

## 7. `runtime_spill::validation` and parent items (3170-3490)

- `seal_runtime_spill_with_span_policy` (3176) wraps `sealed!`.
- `RuntimeSpillSpanPolicy` (3207: `UnitWriteBounded`, `UnitWriteCrossing`), `RuntimeSpillError` (3217: `SourceMismatch`, `UnsupportedValue`, `UnsupportedControlFlow`, `UnsupportedUse`, `ConstraintMismatch`, `WorkBudgetExceeded`, `IdentityOverflow`), `VictimLineage` (3230), `StorageDefinition` (3235), `StoragePosition` (3242: `AfterInstruction`, `AfterUseDef`, `BlockStart`), `StructuralArgumentUse` (3251), `BitsConversion` (3263).
- Helpers: `stored_transport_size` (3270; descriptor scalar reference is 16 bytes, `Canonical` and `Address` give `None`), `next_chunk` (3286, widths 8, 4, 2, 1; computes `byte_size - offset` unchecked, so `offset > byte_size` would underflow; callers keep `offset <= byte_size` via the loop at 4608-4654, inferred), `surviving_home_exists` (3293), `control` (3325) and `control_mut`/`control_successors_mut` (3366, 3372) which match terminators through the macros `terminator_no_successor_pattern!` and `conditional_branch_less_pair_pattern!` (defined elsewhere), `next_instruction_id` (3354), `next_register_id` (3362), `require_dominated_uses` (3392).
- `surviving_home_exists` decides by unit sets: some allocatable view of the class has no unit in the implicit-use, pinned, written or reserved sets (3314-3320). A class with no row returns false (`is_some_and`, 3306).
- `require_dominated_uses` (3392): walks the block graph from the entry; if all uses are in the anchor block and the anchor is the entry, accepts at once (3409-3412). Otherwise confirms the anchor is reached and that no use is reachable when the anchor is bypassed (3415-3458). Refuses with `UnsupportedUse`. Callers: `grep -F require_dominated_uses`: 6 lines (definition, one call in this unit at 4165, two in `runtime_split::admission` at 4909 and 5085, others outside this unit, not inspected).
- Same job elsewhere: two mask/set paths for `surviving_home_exists` (set form 3293, mask form as closure 4330-4352) do the same check twice, one over `BTreeSet<RegisterUnitId>` and one over a `u128` mask limited to unit ids below 128. `grep -F surviving_home_exists`: 6 lines.
- Stated limits: hard-coded 128-unit mask (4181, 4189, 4284); falls to the set path when any id is 128 or more (4262-4263, 4289-4328).

## 8. `derive_runtime_spill` (3486-4716)

- Purpose: the admission check for a spill. Decides whether the victim can be spilled, collects every definition and use, decides which uses share a reload, and returns a `RuntimeSpillDerivation` (3463) holding everything the rewrite needs. Refuses with a `RuntimeSpillError` otherwise.
- Inputs: validated selected analysis, function index, victim register, register environment, span policy, work budget. Reads the function's blocks, memory accesses, local slots, virtual registers, structural contract and the environment's selected constraint keys (`frame_address`, `load64`, `store64`, float/bits keys), reservations and physical model.
- Output: `RuntimeSpillDerivation` with `use_blocks`, `shared_reload`, `dedicated_tie_uses`, `span_closes`, `crossed_unit_write`, `retained_uses`, `definitions`, `assignment`, `structural_uses`, id counters, constraint rows, `carrier_class`, `bits_conversion`.
- How it decides, in order:
  1. Victim origin (3515-3620): accepts `InstructionResult`, `BlockParameter` (not in the entry block, definition site must match the block and position, 3536-3543), `EntryParameter`, `StructuralParameter` (3561-3587), `AbiTransport` on an entry-boundary register, `StructuralObservation`, `AbiTransport` with an instruction. `_ => UnsupportedValue` (3619).
  2. Scalar type (3621-3641): `Callable`, `Boolean`, `IeeeFloat`, or integer of 8/16/32/64 bits. Plus a definition-site rule: FunctionParameter, BlockParameter, Node, or no site for the three structural origins.
  3. Refuses a second spill of the same register if `LocalStorageSlotId::Spill { register }` already exists (3642-3649).
  4. Use scan (3727-4052). Terminator operand uses: only `Use`, untied, not early-clobber, same class (3740-3745). Successor bindings: register transports, structural bindings (`ScalarReference`/`Address` base, `WholeValue` sized), case payloads. Instruction operands: `Def` (first definition or redefinition), `Use` (with tie and early-clobber rules, 3933-3995), `UseDef` (adds a definition, 3996-4030). Any other combination is `UnsupportedUse` (4031).
  5. Constraint rows (4056-4118): the frame address, `Load64` and `Store64` rows must have the expected operand count and access, no fixed view, no tie, early clobber only on the store's second operand; all operands in one `carrier_class`; no unit clobbered by one row and used implicitly by another.
  6. Work budget (4124-4164, 4528-4540, 4569-4577, 4677-4684): `steps` counted from `rewrite_scan` plus per-use terms; `WorkBudgetExceeded` if over `required_rewrite_budget(...).validation_steps()`.
  7. `require_dominated_uses` (4165).
  8. Shared-reload decision (4170-4369): needs any flexible use and a surviving allocatable home after excluding implicit-use, pinned (fixed-view) and reserved units.
  9. `UnitWriteCrossing` policy (4372-4458): lets a shared reload stay open across an instruction that writes units if a home still survives.
  10. `span_closes` (4460-4505): instructions that close a shared reload (redefinitions plus unit writers).
  11. Structural uses (4506-4667): matches each `WholeValue` edge transfer to its chunk loads by edge, place, offset range and read role; every load must be exactly the expected width and kind at the expected offset; `offset == byte_size` at the end; each is either dedicated-reload or shares.
  12. `unit_effects::preserve_live_units` (4668), then `slot::assign` (4685).
- Cases by shape (quotable): structural whole-value bindings are decomposed into loads of widths `[8, 4, 2, 1]` (3286-3290) or one `CopyBytes` snapshot; any other load shape is `UnsupportedUse` (4617, 4647). A byte size of 0 becomes a dedicated end-of-block reload (4543-4547). Integer sizes outside 8/16/32/64 are refused (3623).
- Same job elsewhere: use-scan logic is parallel to `runtime_split::admission::admit` (4921-5112), which scans uses in a simpler form for one fixed-view result. Also similar scan work is probably done in `runtime_rematerialization::admission` (lines 12-353, not in this range).
- Unused or one-caller: `derive_runtime_spill` called at 1029 in this unit; `grep -F derive_runtime_spill source-rust`: not run beyond this file, so unknown (open).
- Fail-open:
  - `RegisterOperandAccess::Use` arm at 3933-3995 is guarded by `defined || current_block_index != block_index`; a use before the definition in the defining block falls to `_ => UnsupportedUse` (4031). Traced: refused.
  - Uses counted as `retained_uses` (`established_stores`, 3940-3952) are left un-reloaded: store instructions that establish a local's address (`AddressLocal` access with same operation and place, 3689-3725) keep the original register as operand 0. Requirement for acceptance: operand 0 of a `Store`/`ZeroBytes` (3941-3947), otherwise `UnsupportedUse`. This is accepted because the address register is the frame address defined by the same instruction chain. Stated by structure, not comment (traced to the code only).
  - `shared_reload` is only a speed choice; when false each use gets its own reload (open question is none: refuses nothing).
  - `can_use_unit_mask` false route (4289-4328) rebuilds the sets without the 128 limit; correctness depends on both paths agreeing (read, not traced for equality).
  - The scan of `function.virtual_registers` for entry fixed views (4209-4220) does `if register.id.0 as usize >= 256 { scan tail from index id; break }` which assumes `virtual_registers[id]` is at index `id` (4213); `next_register_id` is `virtual_registers.len()` (3363), consistent with that (inferred). If ids were not positional the tail scan could skip registers and under-collect fixed views, accepting a shared reload that overlaps a pinned unit (open).

## 9. `parameter_definitions` (4718-4889)

- Purpose: for a block parameter being spilled, finds where each predecessor actually computes the argument, so a store can follow that computation.
- Inputs: function, destination block index, victim, source value. Output: `Vec<StorageDefinition>` (one per predecessor edge) or an error.
- Decides on: each predecessor must be an `EdgeTransfer` block ending in `Jump` with role `EdgeTransferContinuation` (4736-4742); if the edge has a structural case payload with this value, the payload must be a `Registers` transport whose argument is a `StructuralObservation` from a `Load32`/`Load64` that `case_payload_address_corresponds` validates (4743-4829); otherwise a single `Registers` binding with matching value and type whose argument is an `InstructionResult` defined in that same block (4838-4883). Empty result is `UnsupportedUse` (4885).
- Cases by shape: payload loads must be 32-bit or 64-bit integer (4799-4808, `_ => UnsupportedValue`).
- Fail-open: none seen. Every mismatch returns an error.

## 10. `runtime_split` (4892-5493)

- Purpose: "split" a victim register whose single definition writes a fixed physical view: copy the value into a fresh flexible register right after the definition and rename every later use to the copy. This frees the fixed register sooner. `before_fixed_use` instead copies just before the first fixed-view consumer.
- Shape:
  - `admission::admit` (4921) returns `Admission { block_index, definition_index, use_blocks, register, copy }` (4913). Helpers `copy_row` (5114, via `rewrites::integer_copy_row`), `copy_instruction` (5126, `CopyI64` through `copy_shaped`), `rebindable_use` (5146), `names_structurally` (5153), `visit_structural_registers` (5162), `charge` (5202), `next_instruction`/`next_register` (5222, 5228).
  - `before_use` (5232): `ValidatedRuntimeSplit::before_fixed_use` (5244).
  - `rewrite::split_selected_runtime_value` (5360).
  - `validation::seal_runtime_split` (5441). `RuntimeSplitError` (5469; `is_inadmissible` true for `UnsupportedValue` and `UnsupportedUse`, 5479-5481). `shifted_position` (5486).
- Inputs/outputs: same pattern as spill; mutates `blocks`, `virtual_registers`, `boundary_settlements` in a cloned plan.
- Decides on:
  - `admit`: victim origin must be `InstructionResult` (4937); scalar types `Callable`, `Boolean` or non-address integer of 8/16/32/64 bits; `IeeeFloat` is refused (4941-4947); no entry fixed view; definition site present (4948-4958); `has_fixed_definition(register)` (4960); the single defining operand must be `Def` with a fixed view, no tie, no early clobber, same class (4987-4997); every other mention must be an after-definition `rebindable_use` (4028); `parameter == register` in any binding refuses (5058); a structural mention refuses (5073-5075).
  - `before_fixed_use`: integer-only (5260); origin `InstructionResult`, `BlockParameter` or `EntryParameter` (5264-5269); picks the first fixed-view `Use` operand in a source or operation block, scanning blocks in order (5273-5298), refusing if tied (5301-5306). Keyed on position (first found), not on dominance or liveness.
- Cases by shape: one definition only; copy kind `CopyI64` only (5135). `before_fixed_use` rewrites exactly one operand (1 use charged, 5310).
- Same job elsewhere: use-scan and dominance check mirror `derive_runtime_spill`; both import `require_dominated_uses` and `control` from `runtime_spill` (4908-4910). The settlement shifting in `before_use` (5339-5346) and in `rewrite` (5422-5428) do the same job with a different rule: `before_use` shifts settlements at index `>= insertion` (5340), `rewrite` uses `shifted_position`, which shifts only positions strictly greater than the definition index (5486-5491, 5424).
- Unused or one-caller (grep -F, lines including definition): `split_selected_runtime_value` 6 lines; `before_fixed_use` 2 lines (definition 5244 plus one other); `is_inadmissible` 3 lines. Not traced to individual callers.
- Stated limits: none by comment.
- Fail-open:
  - `before_fixed_use` (5244): the victim is rewritten at the first matching consumer found in block order; only that single operand is renamed, with the copy inserted before it. Other uses keep the original register. Whether copying before only one use is always sound depends on the victim value not being redefined between the copy and the other uses; the check at 5264-5272 does not look at definitions of the victim (open; stated nowhere). Admission does not call `require_dominated_uses`. Open: no check that the definition dominates the chosen consumer, though the consumer already names the register so the definition must dominate it (inferred from register being defined once in SSA-like selected form; not verified).
  - `before_fixed_use` filters to `Source`/`Operation` blocks (5277-5281); other block origins are skipped silently. Wrong program through: a fixed-view use only in an edge-transfer block is not found, giving `UnsupportedUse` (5298), so refusal, not acceptance.
  - `admit` uses `charge` errors mapped as `IdentityOverflow` for `charge_steps` conversion at 5216 (`|| RuntimeSplitError::IdentityOverflow` is the first closure; read the call as written: first closure maps overflow, second maps budget).

## Unit tests in range

Two: `cyclic_joins_preserve_definition_bits_across_word_boundaries` (1567) and four in `snapshot_copy::tests` (2910, 2923, 2955, 2989). Count from reading. No other `#[test]` in 541-5493 (inferred from reading all lines).

## Coverage

- Read in full: `runtime_spill_and_split.rs` lines 541-5493 (4,953 lines).
- Skimmed: none.
- Not read: none. (Lines 1-540 belong to another reader's range of this file.)

## Five things a maintainer most needs to know about this unit

1. Spill and split are admit-clone-mutate-seal. `derive_runtime_spill` (3486) refuses unusual victims with `UnsupportedValue`, `UnsupportedUse` or `UnsupportedControlFlow`; the rewrite (985-1473) trusts those results and mutates only through `mutate_preserving_register_prefix`.
2. Slot reuse is decided in `slot::assign` (1720) by comparing the victim's store/load windows against every other `Spill` slot's existing accesses (`shareable`, 2424), with a union-find over reaching definitions. Slots outside 8-byte-aligned `Spill` rows are never reused (1734-1736).
3. A 128-unit bitmask fast path and a `BTreeSet` fallback compute the same "does an allocatable home survive" question (4330-4352 and 3293-3323; mask limits at 4181, 4263).
4. Structural whole-value edge transfers are only spillable when their loads decompose exactly into chunks of 8/4/2/1 bytes or a single matching `CopyBytes` snapshot (4608-4654, 2734-2796); anything else is `UnsupportedUse`.
5. Work is charged per use and per definition against `required_rewrite_budget(...).validation_steps()` (4124-4164); budget overflow is `WorkBudgetExceeded`, arithmetic overflow `IdentityOverflow`.

## What I am unsure about

- Whether `block_slot_summary` returning `None` (2230, 2293) can occur for a block that has slot accesses; if so those accesses are not seen by `RawCandidateSlots::scan`. I did not read `selected_instructions`.
- Whether the `assign` branch with empty `definitions` (1745) is reachable from `derive_runtime_spill`.
- The assumption that `function.virtual_registers[id]` is at index `id` (4213).
- Whether `before_fixed_use` needs a check that other uses of the victim remain valid (5244-5348).
- The macros `sealed!`, `validated_transformed_receipt!`, `rewrite_receipt!`, `failed_display!` and the callers of these public functions outside this file were not read, so call counts above are raw line counts from `grep -rn -F <name> source-rust`, including definitions.
