# Native miscompile: value-call result as a binary operand (slot-name collision)

Found 2026-06-14 by the end-to-end sample lane.

## CORE BUG: FIXED 2026-06-14 (slot planner)

The core slot-name collision is fixed. `call_result_slot_symbol_and_name`
(omega-runtime-storage/body.rs) now names an AssignmentValue call-result slot
after the binding ONLY when the binding's initializer is a BARE call; when the
call is embedded in a larger initializer the scratch slot stays anonymous
(`__call_result`), so name-based reads resolve to the real LocalStorage slot.
Regression-free (full suite 271 → 272). Guarded by the RUN canary
`calls/value_call_embedded_in_binary_exit` (the isolated single-state repro).

## SECOND FACET: also FIXED 2026-06-14 (argument fold)

Passing a local whose initializer contains a call (`step2(r1)` where
`r1 = base + f(6)*3`) as a TRANSITION ARGUMENT folded the local back into its
initializer and re-materialized it in the TARGET state -- re-evaluating the
embedded call, whose result scratch lives in the SOURCE state's frame and is
unreachable there -- so the target's parameter slot was never written (native
exit 73). `initial_value_blocks_inline_fold` blocked the fold only for a
TOP-LEVEL call; a call nested in a binary slipped through. Fixed: block the
inline fold whenever the initializer CONTAINS a result-producing call
(`expression_contains_result_call`), so the local stays a place and its slot is
copied. `samples/value_call_in_expr` now exits 70. Guarded by
`calls/transition_arg_local_from_embedded_call_exit`.

## dual_accumulator_recursion: FIXED 2026-06-14 (state-storage liveness)

The SECOND sequential recursive value-call result local got no frame slot
because `local_data_requires_storage` (omega-state-storage/collection.rs) elided
a local unless a LATER STATEMENT IN THE SAME STATE referenced it -- but
`sum_sq_result` is read only in a transition TARGET state (`do_total`, which
shares main's frame), which the scan never traversed. Fixed by also keeping
storage when any OTHER state of the machine references the local
(`local_referenced_in_other_machine_states`, additive/sound). General soundness
fix: ANY local used only in a sibling state was silently miscompiled (read as
`Place Unknown`). `samples/dual_accumulator_recursion` now exits 70; guarded by
`interpreter_matches_native_on_dual_accumulator_sample`. Original analysis:

## (historical) dual_accumulator_recursion root-cause notes

`samples/dual_accumulator_recursion` (exit 73): two sequential recursive
value-returning calls bound to locals (`let sum_result = self.r.sum(s,0); let
sum_sq_result = self.r.sum_sq(s2,0)`), then `total = sum_result + sum_sq_result`.
Each local reads correctly in its own guard (those guards constant-fold, since
the inputs are constant), but the binary `sum_result + sum_sq_result` reads
`sum_sq_result` as `Place Unknown offset 0 bytes 0` -> garbage.

ROOT (from slots.txt): the SECOND sequential recursive value-call result local
(`sum_sq_result`) gets NO frame slot at all — only `sum_result` (the first) and
`total` are allocated. `append_branch_local_slot` faithfully allocates every
entry in `context.state_storage.locals`, so `sum_sq_result` is missing from
`state_storage.locals` UPSTREAM, in the omega-state-storage planning phase
(not the frame-slot allocator). The value-return keystone delivers the first
recursive call's terminal value to its local but the second's local is never
planned. Fixing it means tracing why state-storage drops the second
value-call-result local — a separate, deeper phase, deferred rather than
rabbit-holed. Repro: `samples/dual_accumulator_recursion`.

The original analysis below is retained for context.

## Symptom

A value-call result used as an operand of a BINARY expression computes the wrong
value, even though the call's result is correct in isolation:

```
let dv6 = self.calc.double_val(6);              // 12  -- correct alone
let r1  = self.base + self.calc.double_val(6) * 3;  // should be 46, reads wrong
```

## Root cause (verified from slots.txt + the emission)

A `let` binding whose initializer **contains a call but is not solely the call**
gets TWO frame slots that BOTH carry the binding's name/symbol:

- a `StateCallResult` scratch slot — the embedded call's result (e.g. `12`), and
- a `LocalStorage` slot — where the full binary expression's value is written
  (e.g. `46`).

Every place that resolves the binding by name (the dispatch GUARD via
`omega-state-guards/operands/layout.rs`, and the TRANSITION-ARGUMENT read via
`omega-instruction-selection ... storage_places.rs::find_runtime_frame_slot_for_path`)
does a first-match `find_map` over `frame_slots` and lands on the
`StateCallResult` scratch slot — which holds only the partial (call) value — not
the `LocalStorage` slot that holds the real value. So `r1` reads `12`, not `46`.

A BARE-call binding (`let dv6 = double_val(6)`) is immune because its call result
is copied into the local slot, so both slots hold the same value.

## Why the obvious fix is wrong

Making the two name-based resolvers "prefer `LocalStorage`" fixes
`value_call_in_expr` (→ exit 70) but **regresses 16 canaries** (including the
dungeon crawler): some call-result bindings populate ONLY the `StateCallResult`
slot (the value is never copied into the `LocalStorage` slot), so preferring
`LocalStorage` reads an uninitialized local. The slot-population invariant is
inconsistent across binding shapes, so a blanket read-preference is unsafe.
(The `omega-state-guards` half alone is regression-free but, on its own, only
moves `value_call_in_expr` from exit 72 to 73 — the transition-arg read is the
other half — so it is not worth landing without the matching selection fix.)

## Correct fix direction (targeted, not yet done)

Do NOT change the read-preference. Instead, at slot ALLOCATION, an embedded
call's `StateCallResult` scratch slot should NOT inherit the binding's
name/symbol when the initializer is a larger expression (only a bare-call
binding's call-result slot legitimately *is* the binding). The scratch slot is
referenced for WRITING by position (`call_result_slot_by_ordinal`,
`statement_index`), not by name, so dropping its name breaks nothing — and then
every name-based read resolves unambiguously to the real `LocalStorage` slot.
Alternatively, guarantee the call result is always copied into the binding's
`LocalStorage` slot (making the invariant consistent) and then prefer
`LocalStorage`. Either way it is a slot-planner change, with the f32/variant
canaries as the regression guard.

`dual_accumulator_recursion` (two bare-call results summed: `sum + sum_sq`) may
be a SEPARATE facet — both operands are bare-call locals, so the name-collision
analysis above does not obviously apply; investigate independently when fixing.
