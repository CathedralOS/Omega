# Closed numeral order establishes an executing bound

Owner: **PROOF-KERNEL-CORE**. Checked numeral order establishes `value <= 5`
for exact `5u64 - value`. A mutable assignment of `3` into the route-free
`Small` domain is the source certificate producer; the consumer must return `2`.
The domain is exactly `0 <= value <= 5`. Each twin changes only that assignment's
input to `6` (the named constant in `const_name`) and rejects at the assignment.

The missing implementation was inside bounded denotation: a primitive closed
order was installed as an assumption of its conclusion, even though the existing
numeral-order producer could derive it from the fixed arithmetic laws. Primitive
leaves now share `closed_inhabitant` with predicate conversion. This introduces
no calculus rule, arithmetic axiom, or new source proof route. Unsupported
closed values retain their explicit decision fallback.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [local](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | [field](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | [parameter](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call_result | [call_result](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match_join | [match_join](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| const_name | [const_name](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| other_state | [other_state](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| other_machine | [other_machine](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| target_body | [target_body](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

Each place is mirrored under `tests/omega/{pass,fail}/facts/closed_integer_bound_through_*`.
All positives bind the four hosted targets. Execution evidence is Linux x86-64
only; foreign native hosts are not claimed.

## Requirements established

- Establishment: an actual bounded assignment certificate proves the literal
  lies inside the complete domain; the goal is not installed as an assumption.
- Identity and lifetime: each immutable transported value retains the bound
  established by the completed assignment. No intervening write changes it.
- Composition: the bound reaches the subtraction through all nine places;
  the only equality guards check the final runtime oracle. Both join arms run.
- Soundness: the source false twin rejects; kernel regressions retain the exact
  fixed-law closure and reject missing numeral definitions, changed operands,
  and false comparisons. Fixed arithmetic laws remain explicit assumptions.
- Psi 05–07: the checked domain permits exact subtraction; proofs are erased
  and Terminal interpretation executes the authored scalar consumer.
- Omega 00–02: ordinary hosted entry binding, scalar layouts, field placement,
  and call/state argument plans carry the value.
- Omega 03–06: subtraction, scalar calls, field loads, joins and state transfers
  realize the same consuming operation.
- Omega 07–09: the Linux ELF image and Terminal execution agree on exit zero.

## Evidence and open scope

Baseline `e5e69808ad62f42142200a461534378ee667448d`: nine native/Terminal exit-zero
positives and nine intended assignment rejections. Each positive emits one
kernel judgment with three bounded proof nodes, 11 declarations, closure size 7,
and 182 post-check arena slots. The baseline regression
`closed_primitive_order_does_not_assume_its_conclusion` fails at its decision-map
assertion. Exact producer hashes and raw observations are retained in the
worktree's `build/proof-kernel-evidence` and qualification records.

The candidate retains the same three bounded proof nodes, with 19 declarations,
closure size 12, and 1,617 post-check arena slots per positive. The closure grows
because reusable fixed laws replace conclusion-specific assumptions; it does not
claim assumption-free arithmetic or improved performance.

This row removes only the closed primitive-order decision assumption within the
existing numeral fragment. Open Boolean conjunction conversion, opaque operations,
construction-budget fallbacks and the broader task acceptance remain open.
