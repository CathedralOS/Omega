# slice_0099

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0760 | RH/recovery/structural_calls.rs:852 | REACHABLE | `decode_native_callback_argument` copies arbitrary commitment bytes from the wire with no recompute or compare. |
| A0761 | RH/recovery/structural_calls.rs:1476 | REACHABLE | `encode_result` omits `qualification_establishments`; decode always empties it, so wire can drop non-empty establishments without this codec re-checking them. |
| A0762 | SIA/fixed_precolored_split_requirements.rs:455 | REACHABLE | Failed `use_transition` still records a domain cut; `finish` accepts vacuously when nothing was marked used. |
| A0763 | SIA/live_ranges.rs:740-742 | REACHABLE | `register_fragments` skips unknown register indices instead of erroring, so operands for undeclared ids yield no fragments. |
| A0764 | SIA/live_ranges.rs:1145-1149 | REACHABLE | Duplicate virtual register ids set `has_duplicate_register_ids` but non-canonical rows get empty `fragments` with no error. |
| A0765 | SIA/machine_effects.rs:953 | CLOSED | In-repo construction always goes through `seal_pre_allocation_machine_effects`, which pairs `receipt(&plan)` before `new`. |
| A0766 | SIA/spill_choice.rs:329 | UNDECIDED | Whether `choice: None` can under-report register pressure on real functions was not traced upward from the cited return. |
| A0767 | SI/blocks.rs:346-363 | REACHABLE | Terminator embedded-instruction operands with `Def` and `fixed_view` never set `fixed_definition_bits`, so `has_fixed_definition` stays false. |

## A0760

**read** Area digest preamble (`digests/area_05.md:1-28`): `RH/` is `omega/compilation/representations/register-homes/src/`.

**read** `decode_native_callback_argument` (`omega-rust/omega/compilation/representations/register-homes/src/recovery/structural_calls.rs:817-865`): line 852 is `let registrar_application_commitment = cursor.array()?;` and line 864 stores those bytes on `TargetNativeCallbackArgument` with no hash recompute or equality check against other decoded fields.

**Caller search** pattern `decode_native_callback_argument` under `omega-rust`: **2** matches (definition and one internal decode site in the same file; read).

**read** `omega-rust/omega/compilation/representations/register-homes/src/recovery.rs:8-12`: recovery artifacts do not grant authority on decode alone; replay against retained roots is stated separately.

**read** `omega-rust/omega/compilation/realization/provider-admission/src/provider_admission/settlements/source_imports.rs:411`: `callback.registrar_application_commitment != application.commitment` is checked on an admission path, not in register-homes decode.

**Input that reaches the branch:** A fixed-view-copy wire payload for a native callback argument whose 32-byte commitment field is wrong but syntactically valid decodes successfully at line 852.

**REACHABLE:** A decoded callback record should be rejected as tampered at recovery decode, but any commitment bytes are accepted and carried forward until some other consumer compares them.

## A0761

**read** Digest item 98 and check 20 narrowing (`digests/area_05.md:468`, `:741`, `:749`; `records/unit_028.md:229`, `:266`, `:286`).

**read** `encode_result` (`structural_calls.rs:1443-1458`): writes place, type, multiplicity, qualifications, projected lists, and claims only; `qualification_establishments` is not encoded.

**read** `decode_result` (`structural_calls.rs:1460-1484`): line 1476 sets `qualification_establishments: Vec::new()` while decoding other fields from the wire.

**read** Companion drops cited in the row: `structural_declarations.rs:518` sets `projected_qualifications: Vec::new()` on `decode_target_parameter` (encode at `:501-508` has no matching field); `structural_contracts.rs:404-405` assigns `let _ = settlement.realization` during encode while decode path does not restore realization.

**Caller search** `decode_result(` under `register-homes`: **3** matches (read: `decode_read_result_and_layout` and structural decode helpers in `structural_calls.rs`).

**Input that reaches the branch:** Wire-encoded structural operation result bytes that would decode to non-empty `qualification_establishments` if the field were present, but the cursor never carries them.

**REACHABLE:** Two logical results that differ only in dropped wire fields round-trip to the same decoded struct; acceptance depends on whether outer plan identity hashes those fields (check 20 says selected-plan encoder does not bind `qualification_establishments`, inferred from digest).

## A0762

**read** `fragment` (`fixed_precolored_split_requirements.rs:407-486`): when shared view set becomes empty at a span point (lines 440-441), lines 443-457 push a segment cut and line 455 calls `cuts.use_transition(&prior, site, destination_view);` without using the `bool` return.

**read** `CutIndex::use_transition` (`:145-167`): returns `false` when `entry_transitions` has no matching source view or has ambiguous matches (lines 152-163).

**read** `CutIndex::finish` (`:169-188`): line 174-179 only verifies entries in `self.used`; when `used` is empty, `all` is vacuously true (read, Rust iterator semantics).

**read** `derive` for a register (`:398`): `cuts.finish(function, register)?` runs after all fragments; unauthenticated cuts with empty `used` can still pass.

**Caller search** `\buse_transition\b` in `omega-rust`: **2** production matches — definition `:145` and ignored call `:455` (read).

**Input that reaches the branch:** A virtual register whose fixed-precolored span shrinks shared candidates to empty at a fixed-use boundary while `entry_transitions` lacks (or doubly matches) the transition key for that site and destination view.

**REACHABLE:** Split-requirement analysis should reject an unauthenticated domain break, but the cut is recorded and `finish` can return `Ok` when no transition was marked used.

## A0763

**read** `register_fragments` (`live_ranges.rs:720-867`): every use of a register id goes through `register_index.get(register.0 as usize) && idx != u32::MAX` (e.g. lines 740-742, 773-775, 816-818, 837-839, 847-849) with no `else` error arm.

**read** `compute_function` (`:1024-1058`): `register_index` is sized to `max_reg_id + 1` with default `u32::MAX` (lines 1047-1056); undeclared ids stay `MAX` and are skipped by `register_fragments`.

**read** `reject_unsupported` (`:1185-1198`): checks only `UseDef` operands, not register table membership (read).

**Caller search** `register_fragments` under `omega-rust/omega/compilation/representations/selected-instructions/analyses`: **3** matches in `live_ranges.rs` (definition, import in function module, call at `:1058`; read).

**Input that reaches the branch:** Function liveness or operand rows that name a virtual register id with no corresponding `virtual_registers` roster entry (or index still `u32::MAX`).

**REACHABLE:** Live-range construction should fail on references to undeclared virtual registers, but those sites contribute no fragments and `compute_function` returns `Ok`.

## A0764

**read** `compute_function` (`live_ranges.rs:1047-1055`): second and later `virtual_registers` rows sharing the same `register.id` set `has_duplicate_register_ids = true` without returning an error.

**read** Virtual row assembly (`:1109-1166`): line 1145 `let fragments = if pos == canonical_idx { std::mem::take(&mut fragment_rows[canonical_idx]) } else { Vec::new() };` gives duplicate-id rows empty fragment vectors.

**read** `liveness.rs:337-350` (same area): duplicate ids are masked in `valid_register_bits` for edge flow but liveness analysis does not error (read).

**Caller search** `compute_function(` in `live_ranges.rs`: **3** matches (definition, internal test/import, pipeline via `compute_live_ranges`; read).

**Input that reaches the branch:** A `SelectedFunction` whose `virtual_registers` table lists the same `VirtualRegisterId` twice.

**REACHABLE:** Analysis should reject duplicate register identities, but duplicate rows are emitted with empty live-range fragments and no `LiveRangeError`.

## A0765

**read** `ValidatedPreAllocationMachineEffects::new` (`machine_effects.rs:953-960`): stores `plan` and `receipt` without verifying they correspond.

**read** Enclosing seal path `seal_pre_allocation_machine_effects` (`:877-881`): `let receipt = receipt(&plan); Ok(ValidatedPreAllocationMachineEffects::new(plan, receipt))` where `receipt` (`:999-1005`) hashes identities from `plan`.

**read** `analyze_pre_allocation_machine_effects` (`:903-921`): compute then seal; exported as `analyze_machine_effects` (`:1030-1034`).

**Caller search** `ValidatedPreAllocationMachineEffects::new` in repository: **1** match at `machine_effects.rs:881` (read).

**Caller search** `analyze_pre_allocation_machine_effects` / `analyze_machine_effects` production pipeline: `machine_effects.rs:921`, `lib.rs:1033`, pipeline `04` / `05` imports (read).

**CLOSED:** Every in-repo path that constructs `ValidatedPreAllocationMachineEffects` goes through seal with `receipt(&plan)` immediately before `new`; there is no other production caller of `new`.

## A0766

**read** `compute_function` in `spill_choice.rs:143-333`: when `first_free` finds a view for the incoming register (lines 206-216), the loop `continue`s without spill; when no contender needs spilling, lines 329-332 return `FunctionSpillChoices { choice: None, .. }`.

**read** `choose_spill_victims` (`:510-532`): seals the plan after per-function `compute_function` (read header only upward).

**Caller search** `choose_spill_victims`: **4** matches — definition in `spill_choice.rs`, pipeline `04` and `05` (read listing).

**UNDECIDED:** Whether a function can have true register pressure while every simplified-model step gets a free view (so `choice: None` hides a needed spill) was not traced through `first_free`, legality tables, or downstream `classify_pressure_recovery` consumers.

## A0767

**read** `BlockEntry::lookup` (`blocks.rs:190-363`): instruction loop lines 308-316 set `fixed_definition_bits` when `operand.fixed_view` is `Some` and access is not `Use`; terminator handling uses `let terminator = super::terminator_instruction(&self.block.terminator)` (line 218) and lines 346-363 iterate `terminator.operands` setting `fixed_views_mask` and `referenced_register_bits` but never call `set_register_bit` on `fixed_definition_bits` for `Def` operands.

**read** `has_fixed_definition` (`:717-721`): scans `fixed_definition_bits` only.

**read** Production consumers: `assignment.rs:1537` and `runtime_spill_and_split.rs:4951` gate splits on `has_fixed_definition` (read).

**Caller search** `has_fixed_definition` in `omega-rust`: **4** matches — definition, tests in `plan_identity.rs:1641-1664`, and the two pipeline sites above (read).

**Input that reaches the branch:** A block whose only fixed-view definition of a virtual register appears on the terminator's embedded instruction operand list (not in `block.instructions`), so lookup never sets the bit.

**REACHABLE:** Runtime split logic treats the register as lacking a fixed definition even though the terminator defines it with a fixed view, so the split path may accept when it should reject (or skip admissible splits).

## What I did not read

- Full `fixed_view_copy_codec.rs` transformed-identity hashing for every dropped field named in A0761.
- `first_free` and recovery-classification consumers for spill-pressure semantics (A0766).
- Whether the main compiler emitter can place the sole fixed-view `Def` only on a terminator operand (A0767), beyond the lookup construction difference.
- `git show 457adf4a11f:` snapshots for line drift; all cites re-located by symbol in the current checkout.

SLICE COMPLETE
