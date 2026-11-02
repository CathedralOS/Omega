# Boolean helper requirements

Owner: [NESTED-MACHINE-UNIFICATION](../../../../../../TASKS.md).

Fact: a private helper preserves a signed invocation rank, and every complete
cycle strictly descends through zero while advancing the explicit counter once.
This is another production of the [signed-rank row](../../README.md), beside
its negated helper requirements. It keeps the input sequence 4→7, signed results
1, 0, -1, -2, interval -128..=127, counter 3→6, both selected array elements,
and canary 641. Only the helper requirement changes from `value <= 7` to
`(value <= 7) == true`.

All nine places live at `<place>/relation_requirement/main.omg`; each adjacent
`invalid/main.omg` changes only the selected increment from one to zero.
The corpus pins are `nested_machine_signed_rank_relation_requirement_<place>_exit`
and the corresponding `_rejected` projects under `tests/omega/{pass,fail}/facts/`.
Every positive binds all four hosted targets and declares exit 0.

The optimized fixed base `f45f51d86f5591a3d731f4b855ee4d09187857bb`
accepts the original and negated helper neighbors but refuses every new positive
at rank-range and source-complete cycle checking. The selected Boolean equality
was sent to integer operand formation. The repair forms Boolean truth paths in
source order, sharing the condition operation with `&&` and `||`; equality and
inequality always form both operands. Integer comparisons keep their own
arithmetic judgment. Raw baseline observations and the immutable producer are
retained under `build/boolean-requirement-baseline/` and `build/producers/baseline/`.

Establishment and soundness: every helper requirement must form and hold under
its exact actuals before its public result guarantee is imported. No requested
rank bound or result guarantee establishes the call that supplies it.
Identity and lifetime: rank inputs retain their captured scalar observations;
Boolean path hypotheses stay local and do not rewrite later storage.
Composition: each place carries the same helper contract and signed rank through
its complete private call component. Removing progress must still reject.

The runtime oracle checks all four signed helper results, both selectors, active
and already-complete arrivals, caller-visible counters and untouched neighbors.
Those observations exercise the existing source-to-Terminal calls, signed
carriers, register/stack homes and native image path. Requirements are erased;
Boolean proof formation adds no runtime branch or alternate emitter.

The supplemental controls isolate calls made by the declared measure; the nine
places retain their direct runtime helper-result checks:

- [Guarded reversed equality](../../controls/relation_requirement_guarded_boolean_equality/main.omg)
  forms a division under its strict `< 7` guard and uses the completed-value arm
  at 7. Its [twin](../../controls/relation_requirement_strict_boolean_right/main.omg)
  changes only `<` to `<=`, admitting the zero divisor; equality must still form
  its right operand.
- [Boolean inequality](../../controls/relation_requirement_boolean_inequality/main.omg)
  compares two predicates whose truth values exchange across the input range.
- [Boolean pair equality](../../controls/relation_requirement_boolean_pair_equality/main.omg)
  compares equivalent predicates. Its
  [twin](../../controls/relation_requirement_false_boolean_pair/main.omg) changes
  only the right boundary from 6 to 7, falsifying the helper requirement at 6.

Acceptance uses `tools/sample_native_gate.py --facts --filter relation_requirement`
with the retained optimized CLI, and the focused corpus filter
`nested_machine_signed_rank_relation_requirement`. The selected set has 12
positive native/Terminal exit-0 oracles and 11 intended source refusals. Corpus
programs and Build declarations are byte-identical to their sample counterparts.
Authored bindings cover all four hosted targets; execution observations in this
session are Linux x86-64. Other-host execution is not claimed.

The broader parent still owns other declaration bodies, normal-call proof
composition, invocation-contract obligations and product-source parity. The
unreduced guarded wrapper also refuses on the fixed base at its ordinary
`check_rank` call; that separate proof limitation is not this repair's acceptance.
