# Skipped short-circuit call

A left operand proved false skips the right-hand bump call: the conjunction is false and the counter stays 0.

Parent outcome: **STATE-LOCAL-VALUE-FRONTIER** (`TASKS.md`), acceptance clause
"operands execute left-to-right once, skipped calls never run", under the
[evaluation schedule](../../../../wiki/spec/language/expressions.md#evaluation-schedule)
and [argument and result ordering](../../../../wiki/spec/terminal-psi/calls_and_outcomes.md#argument-and-result-ordering)
("Short-circuit control remains selective").

Every place evaluates `let first: bool = <left> && bump(&mut counter);` and
then calls `untouched(counter)`, whose `requires counter == 0u64` is proved
only because the checker knows `bump` did not run. Only the transport of the
left operand changes: a local, a record field between canaries, a parameter
whose `requires` fixes it, a call result whose `ensures` fixes it, a match
join whose arms both produce it, a named constant, a nested machine's
parameter, another machine's parameter, and the result of a Build-selected
implementation.

## Requirement recorded by the producing layer

Flow checking records an evaluation roster on each short-circuit occurrence:
the left capture, then the right capture only when some route selected it.
When the left operand is proved to choose the skipping route, the roster holds
the left capture alone and no call fact exists for the right operand. The
scalar computation builder read only literal left operands, so it tried to
build `bump` as a call, found no call fact and declined the whole local.
The repair reads the recorded roster: when every evaluation of the occurrence
at that statement captured only its left operand, the conjunction is the left
computation itself, keeping the left operand's effects and never forming the
right call.

Baseline `91458311fe6` (optimized producer built from that tree, macOS arm64):
`local`, `field`, `call_result`, `match_join` and `other_state` refused with
``has no executable body: this compiler does not lower a statement of it yet
(statement sequence: local data (last declined step: statement sequence: local
data: scalar local: pure initializer))``. `const_name` (a literal after
substitution), `parameter`, `other_machine` and `target_body` already ran.

## Places

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [local/main.omg](local/main.omg) | [invalid](invalid_local/main.omg) |
| field | [field/main.omg](field/main.omg) | [invalid](invalid_field/main.omg) |
| parameter | [parameter/main.omg](parameter/main.omg) | [invalid](invalid_parameter/main.omg) |
| call result | [call_result/main.omg](call_result/main.omg) | [invalid](invalid_call_result/main.omg) |
| match join | [match_join/main.omg](match_join/main.omg) | [invalid](invalid_match_join/main.omg) |
| const name | [const_name/main.omg](const_name/main.omg) | [invalid](invalid_const_name/main.omg) |
| a nested machine | [other_state/main.omg](other_state/main.omg) | [invalid](invalid_other_state/main.omg) |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [invalid](invalid_other_machine/main.omg) |
| another selected target implementation | [target_body/main.omg](target_body/main.omg) | [invalid](invalid_target_body/main.omg) |

Each twin changes only the left operand's value (and the contract that fixes
it) from `false` to `true`. `bump` then runs, the counter is no longer known to
be 0, and the twin rejects with `cannot prove requires contract for call
untouched ...: counter == 0`. Pins: `tests/omega/{pass,fail}/facts/skipped_short_circuit_call_through_*`.

Nearest rows: `copied_call_argument_snapshot` and `copied_case_before_later_argument`
order completed operands of one call; neither has a selective operand, and
their twins name the copied value, not a skipped call's write.

## Requirements and evidence

- **Establishment:** the left operand's value comes from its own construct
  (initializer, field, contract, join, constant, selected implementation); no
  caller guard re-establishes the counter fact.
- **Identity and lifetime:** `counter` keeps its exact pre-statement version
  because the skipped call writes nothing; the twins show the run call
  invalidates it.
- **Composition:** the counter fact reaches `untouched`'s `requires` through
  every place; the conjunction result reaches the `transition first` arms.
- **Checked-to-Terminal Psi lowering:** the conjunction lowers to the left
  computation; the right call is absent from Terminal Psi and the interpreter
  agrees with native execution.
- **Omega 00–09:** ordinary scalar locals, calls and branches; no new layout,
  ABI or encoding form.
- **Runtime falsifier:** `main` traps if `first` is true or if `counter` or
  the value returned by `untouched` is not 0, so a lowering that ran `bump`
  fails both engines. The field place also checks its two canaries.

Open: transition-guard and call-argument occurrences of a skipped right
operand use the same builder route but are not separately pinned here; the
remaining `STATE-LOCAL-VALUE-FRONTIER` bullets stay on the board.
