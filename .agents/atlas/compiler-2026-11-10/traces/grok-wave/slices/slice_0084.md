slice_0084

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0642 | omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/literal_fold_rewrites.rs:2891-2913 | CLOSED | Minus-one fold only runs for unsigned `MaterializeI64` literals whose victim scalar admits that unsigned value; `ConstantOfLiteral` yields 0, which matches signed `WrappingRemainderI64` for divisor −1. |
| A0643 | omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/literal_fold_rewrites.rs:2571-2617;3526-3541;1717-1815 | UNDECIDED | `ConditionalBranchNonZero` in the equality-only reader walk is not traced to program acceptance; the other two sub-claims look like intentional retirement/renumbering. |
| A0644 | omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/rewrites.rs:144-191 | UNDECIDED | `literal_bits`/`immediate_bits` sign split is real, but callers that matter for compare folding were not fully traced to a bad acceptance. |
| A0645 | omega-rust/omega/compilation/pipeline/05_selected-instructions-to-register-homes/src/assignment.rs:490-491;970 | CLOSED | Manifest `target`/`selected` are checked when consumed (stage 06 and preservation); live-fragment subtraction assumes validated ranges with ordered endpoints. |
| A0646 | omega-rust/omega/compilation/pipeline/05_selected-instructions-to-register-homes/src/rewrites.rs:626-631 | CLOSED | All-`instruction_local` early return only skips cross-instruction materialization that cannot be required for those operands. |
| A0647 | omega-rust/omega/compilation/pipeline/05_selected-instructions-to-register-homes/src/home_assignment.rs:1988-2010 | UNDECIDED | Non-dense branch admits mask bits by view-table index plus class match only; did not prove whether bad masks can reach coloring. |
| A0648 | omega-rust/omega/compilation/pipeline/05_selected-instructions-to-register-homes/src/runtime_spill_and_split.rs:1651-1663 | UNDECIDED | `load_slot` falls back to `private`, but I did not prove a shared-store path can omit a loads-map row at spill rewrite. |
| A0649 | omega-rust/omega/compilation/pipeline/06_register-homes-to-post-allocation-machine/src/optimization.rs:304-344 | REACHABLE | Release builds can reuse the first candidate’s unit-liveness table across later instructions with only `debug_assert_eq!` guarding differing `extra_units`. |

### A0642

1. Read `WRAPPING_REMAINDER_MINUS_ONE_MATERIALIZE` at `literal_fold_rewrites.rs:2891-2913` (`PairImmediateBound::Exactly(u64::MAX)`, `ConstantOfLiteral` shape at `3715-3718`).
2. Read `derive_actions`, `register_admits`, and `admit_candidate` at `1214-1264`, `1298-1320`, `1437-1512` (callers: search `derive_actions(` in `omega-rust` → **2 hits**, both in `literal_fold_rewrites.rs`; read `derive_function_folds` caller, not every validation path).
3. Read `fold_immediate` at `3841-3848` (non-`SaturatingAdd` `ConstantOfLiteral` → immediate `0`) and `rewrite_consumer` at `3977-3986`.
4. **Input to accepting branch:** sole-use unsigned `MaterializeI64` with `u64::try_from(value)` ok, victim admitted as unsigned scalar, consumer `WrappingRemainderI64` with divisor operand matching `u64::MAX`, policy bit enabled.
5. **Verdict CLOSED:** `derive_actions` skips non-unsigned materializations (`1221-1225`); `register_admits` requires `integer.admits(IntegerValue::Unsigned(value))` (`1311`); instruction kind is `WrappingRemainderI64` (`instructions.rs:241`, read); fold result `0` matches signed remainder by −1. Entry’s unsigned `% MAX ≠ 0` case does not apply to this i64 wrapping remainder row without a shown unsigned operand path.
6. **Unsure:** Whether any production constraint row binds unsigned semantics to `WrappingRemainderI64` (not read in effect catalog).

### A0643

1. Read `equality_sensing_reader` and `swapped_unit_readers_equality_only` at `2571-2617` (search `equality_sensing_reader` → **2 hits**, `literal_fold_rewrites.rs` only).
2. Read `admits_dead_consumer_defs` at `3526-3541` and `admits_swapped_condition_defs` at `3543-3567` (search `admits_dead_consumer_defs(` → **2 hits**).
3. Read `apply_actions` / `redensify` at `1717-1815`, `1901-2023` (read `lower_selected_instruction`, not every IR edge type).
4. **Input (equality branch):** `PairUnitDefRelation::OperandSwapped` consumer with distinct operands; all implicit def units must pass `swapped_unit_readers_equality_only`, which treats `ConditionalBranchNonZero` like `MaterializeBooleanEqual` (`2574-2576`).
5. **Input (dead defs):** `RetiredWhenDead` with empty `implicit_uses` and each implicit def unit absent from `implicit_unit_used` (`3532-3537`, `2561-2568`).
6. **Verdict UNDECIDED:** Record claims live-out risk for branch-as-equality; I did not read flag/def propagation through `ConditionalBranchNonZero` to compilation acceptance. Dead-def and `redensify` paths look like conditional transforms, not silent acceptance of malformed input (entry partially misstates “only tables renumbered” — also successor bindings at `1994-2010`, read).
7. **Unsure:** Whether `ConditionalBranchNonZero` implicit unit uses are always equality-insensitive to swapped compare flags.

### A0644

1. Read `literal_bits`, `immediate_bits`, `materialized_bits` at `144-191` (search `materialized_bits(` → **9 hits**: `rewrites.rs` 6, `branch_and_compare_rewrites.rs` 3).
2. Read callers `constant_operands` (`212-248`), `constant_index` (`595-614`), `transport_defines` (`616-645`) in full; read `constant_boolean` `admit` through `resolved_operands` (`209-210`, read earlier `120-219`).
3. **Input:** register with exactly one in-function `MaterializeI64` def matching shape; bits via `literal_bits` (signed → `as u64` cast at `146`).
4. **Verdict UNDECIDED:** Signed `literal_bits` vs `immediate_bits` asymmetry is read at `144-156`, but I did not trace a complete compare-fold path that accepts a program that should fail. `constant_index` rejects successor block parameters via `transport_defines` (`610-611`); literal-fold `sole_use` rejects successor-carried victims (`1409-1410`, read). Entry’s block-parameter constant claim not shown on traced compare-fold path.
5. **Unsure:** Whether `materialized_bits` can feed a constant fold for a register that is still a live block argument on another edge without going through `transport_defines`/`sole_use` guards.

### A0645

1. Read `expected_record` at `370-517`, especially `441-450` and `490-491`.
2. Search `project_post_allocation_optimization_manifest(` → **7 hits** (`assignment.rs` 6, `rewrites.rs` 1`); read `assignment.rs:541-560` only.
3. Read consumers `preservation.rs:342-347` and `plan.rs:297-305` (read).
4. Read `live_points` subtraction at `967-971` and `1317-1321`.
5. **Input:** validated `ValidatedLiveRanges` + `ValidatedAllocationLegality` + `ValidatedRegisterHomes` with matching receipt identities; manifest copies `ranges.plan().target` and `.selected` into the record.
6. **Verdict CLOSED:** Downstream `manifest.target != selected.plan.target` / `environment.target()` rejects mismatch (`plan.rs:297-298`, `preservation.rs:345`, read). Receipt roots tied before write (`441-450`). Fragment `end.0 - start.0` on `ValidatedLiveRanges` data — well-formedness of fragments not re-derived here (inferred from stage contract).
7. **Unsure:** None for manifest fields; fragment ordering invariants not opened in analysis crate.

### A0646

1. Read `materialize_selected_fixed_occurrences` path at `600-632` and `807-809` (search `materialize_selected_fixed_occurrences(` → **2 hits**).
2. Read `bounded_settlements` at `578-589`.
3. Trace to allocation loop `assignment.rs:1690` (read line only, not full spill loop).
4. **Input:** occurrence plan where every roster register has `instruction_local: true`.
5. **Verdict CLOSED:** Returns `bounded_settlements` (settlement bound check only); non-local materialization is explicitly skipped for `instruction_local` operands (`807-808`). Accepting branch means “no cross-instruction clone needed,” not “ignore tied-to violations” (still validated earlier `605-620` when bound is zero).
6. **Unsure:** Did not read full `local_occurrence_registers` classification logic (`~912+`).

### A0647

1. Read `home_assignment.rs:1930-2013` (`dense_view_ids` branch vs `else` at `1981-2012`, read function header not found — mid-function read only).
2. **Input:** non-dense view ids, domain with empty `candidates`, nonempty `candidates_mask`.
3. **Verdict UNDECIDED:** Non-dense path uses `model.views.get(usize::from(bit))` filtered by `view.class == domain.leader_member.class` (`1988-1996`) without dense id equality check present in dense branch (`1965-1972`). Whether invalid bits reach this code depends on upstream domain construction (not read).
4. **Unsure:** Origin of `candidates_mask` when `candidates` is empty.

### A0648

1. Read `Assignment::load_slot` at `1651-1656` and `assign` load insertion at `2060-2067` (read `assign` `1719-2078` partially).
2. Search `load_slot(` → **3 hits** (`runtime_spill_and_split.rs` 2, `reference_result_custody.rs` 1); read use site `1189-1196`.
3. **Input:** spill rewrite reload at `(block_index, instruction_index)` when operand needs memory reload; `share` true when shared reload policy applies.
4. **Verdict UNDECIDED:** Fallback `unwrap_or(self.private)` is read at `1651-1656`. `assign` inserts explicit slots for marked load positions (`2065-2067`). I did not prove an inconsistency where a shared store slot is chosen but the corresponding use index is absent from `loads.instructions`.
5. **Unsure:** Full `window_of_position` / `emitted` alignment proof.

### A0649

1. Read `rewrite_declared_forms` at `277-367` and `extra_units` at `379-396` (search `rewrite_declared_forms(` → **3 hits**, all `optimization.rs`).
2. Caller `run_x86_select_xor_zero_i64_materialization` → `select_declared_form` (`215-274`, read).
3. **Input:** second+ candidate in same function where `extra_units(instruction, form)` differs from first table build.
4. **Verdict REACHABLE:** `table.get_or_insert_with` freezes first `extra` vector (`336-340`); later instructions only `debug_assert_eq!(*units, extra)` (`342`). In release, mismatched extra units yield wrong `table.live_after` → optimization may commit (`357-358`) when units are still live. Smallest bad outcome: two zero-materialization instructions in one function whose selected forms disagree on implicit extra units; second is unsoundly rewritten.
5. **Unsure:** How often two admitted forms differ in `extra_units` within one function for this rule (not enumerated).

### What I did not read

- Effect catalog row operand classes for `remainder_i64` (`catalog.rs` bodies beyond grep hits).
- Full `local_occurrence_registers` and `RuntimeSpillDerivation` assignment construction (`runtime_spill_and_split.rs` spill admission from the top).
- `UnitLivenessTable::compute` internals (`06` crate).
- `integer.admits` implementation (semantic vocabulary crate).
- Complete validation harness paths from `derive_function_folds` through final native emission.
- Entire `home_assignment` function containing the `1988-2010` branch (only surrounding read).

SLICE COMPLETE
