# Contracted cursor steps in a private cycle

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../../../../../../TASKS.md).

Fact: a private helper preserves the invocation rank on a tail edge, and every
complete cycle strictly descends while advancing the explicit counter once.

This is a production of the existing [computed-upper row](../README.md), not a
new fact. The upper endpoint remains `stop + 1`, with the same initial interval
`[0, 7]`. The cursor step comes from an ordinary `advance` call whose independently
checked public contract states `result == value + 1`. Its private body is not the
caller's proof. Three cursor advances and four upper-input decrements leave
`Counts { value: 10, climbs: 3, shrinks: 4 }`; an empty invocation preserves all
three caller-owned fields. The distinct counts expose either branch being
skipped, repeated or confused with the other.

Every twin changes only the helper's result contract and body from `value + 1`
to `value + 0`. The call remains well formed, but the cycle can then preserve
rank forever on its cursor edge and must reject.

| Place | Positive | No-progress twin |
| --- | --- | --- |
| local | [source](../../../../../local/bounded_distance/moving_upper/computed_upper/called_step/main.omg) | [source](../../../../../local/bounded_distance/moving_upper/computed_upper/called_step/invalid/main.omg) |
| field | [source](../../../../../field/bounded_distance/moving_upper/computed_upper/called_step/main.omg) | [source](../../../../../field/bounded_distance/moving_upper/computed_upper/called_step/invalid/main.omg) |
| parameter | [source](main.omg) | [source](invalid/main.omg) |
| call_result | [source](../../../../../call_result/bounded_distance/moving_upper/computed_upper/called_step/main.omg) | [source](../../../../../call_result/bounded_distance/moving_upper/computed_upper/called_step/invalid/main.omg) |
| match_join | [source](../../../../../match_join/bounded_distance/moving_upper/computed_upper/called_step/main.omg) | [source](../../../../../match_join/bounded_distance/moving_upper/computed_upper/called_step/invalid/main.omg) |
| const_name | [source](../../../../../const_name/bounded_distance/moving_upper/computed_upper/called_step/main.omg) | [source](../../../../../const_name/bounded_distance/moving_upper/computed_upper/called_step/invalid/main.omg) |
| other_state | [source](../../../../../other_state/bounded_distance/moving_upper/computed_upper/called_step/main.omg) | [source](../../../../../other_state/bounded_distance/moving_upper/computed_upper/called_step/invalid/main.omg) |
| other_machine | [source](../../../../../other_machine/bounded_distance/moving_upper/computed_upper/called_step/main.omg) | [source](../../../../../other_machine/bounded_distance/moving_upper/computed_upper/called_step/invalid/main.omg) |
| target_body | [source](../../../../../target_body/bounded_distance/moving_upper/computed_upper/called_step/main.omg) | [source](../../../../../target_body/bounded_distance/moving_upper/computed_upper/called_step/invalid/main.omg) |

The fixed baseline `f45f51d86f5591a3d731f4b855ee4d09187857bb` rejects the frozen
parameter witness with `no exact private-coordinate assignment proves every edge
and complete cycle`. The optimized candidate executes that unchanged witness in
both engines at exit 0. All nine places and their nine no-progress twins were observed with the same
optimized Linux x86-64 producer. Full commands, producer digest, source digests
and per-engine results are retained in the publication record.

## Requirements

- Establishment: ordinary invocation formation independently proves `advance`'s
  `value < 7` requirement. The captured normal result carries the selected public
  equality. Removing that equality or strengthening the precondition remains a
  refusal; a proposed rank does not prove its own call.
- Identity and lifetime: a completed call retains each actual's exact occurrence
  and supplying scalar roster. A symbol alone is not a completed operand. Input
  snapshots remain separate from mutable storage and callee post-state values.
- Composition: the same value goes through all nine places. Nested helper calls
  and reordered arguments challenge the complete roster; the no-progress twins
  challenge the every-cycle descent judgment.
- Soundness: the shared contract importer retains its selected callable and
  independence checks. It reads no private helper body and imports no callee
  precondition as a premise. Unrepresented operands retain only their known
  interval. Ordinary ownership, effects and call-formation checks remain active.
- Psi 06–08: the cursor call is a real executed operand on the recurrent edge,
  with ordinary call/custody facts and constant-stack component lowering. The
  caller observes both active and empty paths; replacing the step result breaks
  the runtime counter oracle.
- Omega 00–02: every positive binds all four hosted entries and uses the ordinary
  explicit borrowed-counter and scalar-argument calling plans.
- Omega 03–06: the original caller-owned counter remains live across the private
  cycle. These bounded executions do not certify portable composed WCSU or a
  100,000-cycle physical stack limit.
- Omega 07–09: actual Linux x86-64 native and Terminal observations are recorded
  separately. Other hosted targets have authored bindings, not local execution
  evidence.

Pinned row fixtures are `facts/nested_machine_joint_rank_called_step_*` under
`tests/omega/{pass,fail}`. The broader parent remains open, including private
computed-upper inference, remaining declaration forms, product-compiler parity
and source migration. The original call-computed-upper probe remains unfinished;
this smaller production isolates its shared scalar-capture prerequisite.

## Composition controls

[Nested calls](controls/nested_call/main.omg) and
[reordered arguments](controls/reordered_arguments/main.omg) execute the same
seven steps. [Missing result evidence](controls/missing_result_contract/main.omg),
[an unmet call precondition](controls/missing_call_precondition/main.omg), and
[a constant result](controls/constant_result/main.omg) still reject.
The [ordered-operand positive](controls/ordered_operands/main.omg) mutates a
borrowed scalar in an earlier argument and consumes its later value; its
[stale-value twin](controls/stale_operand/main.omg) rejects the old value.
The existing operand-write barrier precedes these captures; a scoped roster
never authorizes rereading a retired storage observation.
