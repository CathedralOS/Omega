# slice_0084

## Summary

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0642 | `S04/literal_fold_rewrites.rs:1221-1231` | CLOSED | Unsigned-only literal scan and `WrappingRemainderI64` consumer gate the minus-one fold. |
| A0643 | `S04/literal_fold_rewrites.rs:2571-2617` | UNDECIDED | `ConditionalBranchNonZero` in the equality walk is read; whether that is too weak for swap folds is not settled. |
| A0644 | `S04/rewrites.rs:158-191` | REACHABLE | `materialized_bits` ignores block-parameter incoming values when a lone `MaterializeI64` def exists. |
| A0645 | `S05/assignment.rs:490-491` | CLOSED | Manifest fields are receipt-linked; `u32` span math does not admit invalid programs here. |
| A0646 | `S05/rewrites.rs:626-631` | CLOSED | All-`instruction_local` early return only skips fixed-occurrence materialization, not legality checks. |
| A0647 | `S05/home_assignment.rs:1988-2010` | CLOSED | Non-dense mask walk only marks physical views used; it tightens homes, not acceptance. |
| A0648 | `S05/runtime_spill_and_split.rs:1651-1656` | CLOSED | `load_slot` defaults coincide with loads inserted for every emitted reload site. |
| A0649 | `S06/optimization.rs:304-344` | REACHABLE | Release builds can reuse the first `extra_units` liveness table across differing unit sets. |

## A0642

Digest at `457adf4a11f` cited `S04/literal_fold_rewrites.rs:2891-2913`; I read current `omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/literal_fold_rewrites.rs:2891-2913` (re-located by symbol `WRAPPING_REMAINDER_MINUS_ONE_MATERIALIZE`).

The rule pairs `MaterializeI64` with `WrappingRemainderI64`, bound `Exactly(u64::MAX)`, and `ConstantOfLiteral` rewrites to materialize `0` via `fold_immediate` (`:3846-3848`, read).

`derive_actions` (`:1214-1267`, read in full) only scans literals whose kind is `MaterializeI64 { value: IntegerValue::Unsigned(value) }` (`:1221-1225`, read); signed `-1` materializations never enter the fold. `literal_u64` is `u64::try_from(value)` (`:1230-1231`, read).

`register_admits` (`:1298-1321`, read) requires the victim register’s scalar type to `admits(IntegerValue::Unsigned(value))` (`:1311`, read).

`admit_candidate` selects rules with `rows.for_consumer(consumer.kind, use_operand, literal_u64)` (`:1437`, read); `WrappingRemainderI64` is selected only for signed remainder at stage 03 (`selection_scalar_graph.rs:4508-4513`, read). Unsigned remainder uses `ExactRemainderU64` (`:4515-4519`, read), so the `u64::MAX` rule does not attach to unsigned remainder consumers.

`rewrite_consumer` materializes the folded immediate through `scalar_materialize_value` with the result register’s scalar type (`:1847-1854`, `:3993-4003`, read).

Caller search: `\bderive_actions\b` in `omega-rust` — 1 definition, 1 call site (`literal_fold_rewrites.rs:2133`, read). `\bcompute_terminal_literal_fold\b` — 1 definition, 1 call (`:4078`, read).

**Verdict CLOSED.** The smallest unsound program described in the digest would need unsigned remainder with `x % u64::MAX` folded to zero; that path never reaches this rule because literals are unsigned-only and the consumer kind is signed remainder only.

## A0643

Digest cited `:2571-2617`, `:3526-3541`, `:1717-1815`, `:1901-2023`. I read current same file at those symbols (`equality_sensing_reader`, `swapped_unit_readers_equality_only`, `admits_dead_consumer_defs`, `apply_actions` / `redensify`).

`equality_sensing_reader` (`:2571-2577`, read) treats `ConditionalBranchNonZero` like `MaterializeBooleanEqual`.

`swapped_unit_readers_equality_only` (`:2579-2617`, read) walks forward from the consumer; any `implicit_uses` of the unit on a non-equality-sensing instruction returns `false` (`:2594-2597`, read).

Used only from `admits_swapped_condition_defs` when `unit_defs == OperandSwapped` (`:3543-3567`, read). Caller search: `\bswapped_unit_readers_equality_only\b` in `omega-rust` — 1 definition, 1 production call (`:3558`, read).

`admits_dead_consumer_defs` for `RetiredWhenDead` (`:3531-3537`, read) requires empty consumer `implicit_uses` and every `implicit_defs` unit to satisfy `!implicit_unit_used(function, *unit)` (`:2561-2568`, read).

`redensify` (`:1901-2023`, read) renumbers only removed ids after a committed action; admission is strictly before `apply_actions` (`:1717-1815`, read).

**Verdict UNDECIDED.** I read that `ConditionalBranchNonZero` is classified as equality-only for the swap walk, but I did not trace machine flag semantics for swapped compares through to a definite accepting branch. What would decide: a concrete selected function where an operand-swapped literal fold passes `admits_swapped_condition_defs` yet a later `ConditionalBranchNonZero` depends on flag bits beyond equality.

## A0644

Digest cited `S04/rewrites.rs:158-191` and `:144-156`. I read `omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/rewrites.rs:144-248` (re-located).

`literal_bits` sign-extends signed literals to `u64` (`:144-148`, read); `immediate_bits` rejects negative signed values for immediates (`:151-155`, read).

`materialized_bits` (`:158-191`, read) demands exactly one in-function def producer and that producer is `MaterializeI64`; it does not consult `VirtualRegisterOrigin::BlockParameter` or successor `bindings`.

`constant_operands` calls `materialized_bits` for compare operands (`:226-228`, `:237`, `:244`, read). `branch_and_compare_rewrites.rs` uses it in equivalence admission (`:1870`, `:1887-1888`, read).

`constant_index` skips registers when `transport_defines` is true (`:609-612`, read), but `materialized_bits` has no analogous guard.

Caller search: `\bmaterialized_bits\b` in `omega-rust` — 1 definition (`rewrites.rs`), 4 production references in `branch_and_compare_rewrites.rs`, 3 internal uses in `rewrites.rs` (count grep, read).

**Verdict REACHABLE.** Plain words: a virtual register that is fed different incoming edge values but also has a single `MaterializeI64` def on one path can be treated as a constant in compare or branch rewrites, folding a boolean that should depend on the block argument.

## A0645

Digest cited `S05/assignment.rs:490-491`, `:970`, `:1320`. I read `omega-rust/omega/compilation/pipeline/05_selected-instructions-to-register-homes/src/assignment.rs:438-520` and `:965-973`, `:1316-1322`.

`PostAllocationOptimizationManifest` stores `target` and `selected` from `ranges.plan()` without re-deriving them (`:490-491`, read), but the same function checks receipt linkage (`:441-448`, read). Later `preservation.rs:342-347` compares `manifest.target` to `environment.target()` (read).

Fragment length uses `u64::from(fragment.end.0 - fragment.start.0)` (`:970`, `:1320`, read) without an explicit underflow check.

**Verdict CLOSED** for ACCEPT unsoundness. Wrong manifest metadata is caught at preservation boundaries; inflated `live_points` from wrapped subtraction affects spill heuristics inside stage 05, not whether an ill-formed program is accepted as compiled.

## A0646

Digest cited `S05/rewrites.rs:626-631`, `:654-682`. I read `omega-rust/omega/compilation/pipeline/05_selected-instructions-to-register-homes/src/rewrites.rs:600-682` and `:807-808` (enclosing materialize loop).

When `constrained_operand_bound != 0`, if every roster register is `instruction_local`, the function returns `bounded_settlements(function)` (`:626-631`, read). Otherwise blocks without candidates copy through unchanged (`:654-682`, read). `instruction_local` marks skip materialization in `materialize` (`:807-808`, read).

**Verdict CLOSED.** This path skips fixed-occurrence copying for scratch-like registers; it does not bypass earlier constraint checks (`:605-624`, read) and returns errors on mismatch.

## A0647

Digest cited `S05/home_assignment.rs:1988-2010`. I read `omega-rust/omega/compilation/pipeline/05_selected-instructions-to-register-homes/src/home_assignment.rs:1931-2012` (re-located).

Dense path validates mask bits against class-specific valid masks (`:1954-1975`, read). Non-dense `else` branch (`:1981-2012`, read) for empty `candidates` walks `candidates_mask` and requires `view.class == domain.leader_member.class` at each bit index (`:1988-1996`, read).

**Verdict CLOSED** for unsound acceptance. The branch only records which physical views are in use for interference construction; at worst it rejects allocation later, it does not admit an illegal home plan.

## A0648

Digest cited `S05/runtime_spill_and_split.rs:1651-1663`. I read `omega-rust/omega/compilation/pipeline/05_selected-instructions-to-register-homes/src/runtime_spill_and_split.rs:1651-1663`, `:1968-2077`, and use site `:1189-1194`.

`load_slot` returns `loads[block].instructions[index]` or `self.private` (`:1651-1656`, read). Assignment construction inserts `loads` at every `window_of_position` emission point (`:2060-2067`, read). The only caller is reload insertion at an emitted instruction index (`:1193`, read). Caller search: `\bload_slot\b` in `omega-rust` — 1 definition, 1 call in this crate (grep, read).

**Verdict CLOSED.** A reload at an emitted instruction index always has a map entry; the private fallback applies where no load was scheduled, matching the private store default.

## A0649

Digest cited `S06/optimization.rs:304`, `:335-344`. I read `omega-rust/omega/compilation/pipeline/06_register-homes-to-post-allocation-machine/src/optimization.rs:277-360` and module comment `:19-21`.

Per function, `table: Option<(Vec<RegisterUnitId>, UnitLivenessTable)>` is built once from the first qualifying candidate’s `extra_units` (`:304`, `:336-340`, read). Later candidates call `table.live_after(..., &extra)` with their own `extra` (`:334-343`, read); equality of unit sets is only `debug_assert_eq!` (`:342`, read).

`live_after` uses precomputed `suffix_effects` for the first unit set; mismatch falls back to `block_effect(instructions, units)` for suffix only (`:489-504`, read), while `live_in` remains from the first set (`:446-486`, read).

`extra_units` (`:379-395`, read) varies with baseline defs or clobbers. Only executor rule is `X86SelectXorZeroI64MaterializationV1` (`:205-212`, read).

**Verdict REACHABLE.** Plain words: in a release build, a function with two xor-zero candidates whose baselines differ on whether flags are already clobbered can reuse a liveness table built for the other unit set and commit the optimizer form while a flags unit is still live.

Caller search: `\brewrite_declared_forms\b` in `omega-rust` — 1 definition, 2 in-module calls (`:267`, test `:759`, read).

## What I did not read

- Full `enabled_pair_rules` catalog beyond snippets needed for A0642 and A0643.
- Full spill assignment builder above `window_of_position` construction (A0648 assumed pairing with `:2060-2067`).
- Stage-03 obligation proofs that forbid the A0644 merge shape at selection time (would affect REACHABLE witnesses).
- `UnitLivenessTable::live_after` proofs for mixed unit sets beyond the implementation read for A0649.

SLICE COMPLETE
