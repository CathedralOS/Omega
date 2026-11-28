# Calling a machine in a dependency

These nine programs import `ops.omg` from the package registered as `lib`. The call `|ops|::describe(@left, @right)` must prove that the left operand is positive. The exported machine accepts two references to `Wrapped` records and guarantees that its result equals the sum of their fields after conversion to `u64`. Both the record type and the machine are declared in the dependency. [Dependency source](../../support/fact_dependencies/qualified_call_lib/ops/ops.omg)

Each positive program supplies 250 and 10. The caller uses the result guarantee to satisfy `hold(total)`, which requires 260, and checks the actual sum before returning. Its invalid twin supplies 0 and 260. The sum stays the same, so the left operand is the reason the compiler must reject the call.

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`invalid_local/main.omg`](invalid_local/main.omg) | both operands are local bindings |
| field | [`field/main.omg`](field/main.omg) | [`invalid_field/main.omg`](invalid_field/main.omg) | both operands are fields of one record, borrowed as `@pair.left` |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`invalid_parameter/main.omg`](invalid_parameter/main.omg) | `observe` receives both operands and forwards its own `requires` and `ensures` |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`invalid_call_result/main.omg`](invalid_call_result/main.omg) | each operand is a call result whose `ensures` names its value |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`invalid_match_join/main.omg`](invalid_match_join/main.omg) | each operand is chosen by a `match` whose arms both yield the same record |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`invalid_const_name/main.omg`](invalid_const_name/main.omg) | each operand is copied from a named constant |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`invalid_other_state/main.omg`](invalid_other_state/main.omg) | a successor machine receives both operands and performs the call |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`invalid_other_machine/main.omg`](invalid_other_machine/main.omg) | `relay` forwards to `observe`, which performs the call |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`invalid_target_body/main.omg`](invalid_target_body/main.omg) | Build selects one implementation per hosted target; each performs the call |

Run both the programs and their invalid twins on Linux with `mbx xtask sample-gate --target linux_x86_64 --filter samples/cli/facts/dependency_type_qualified_call`.
