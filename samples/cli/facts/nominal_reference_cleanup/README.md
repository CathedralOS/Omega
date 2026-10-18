# Nominal reference cleanup

A nominal owner retains its exclusive output reference through a shared call, then normal cleanup writes captured code 7 to the original backing.

This establishes CML4's intact reference-bearing nominal receiver requirement
under [ordinary borrowed calls and cleanup](../../../../wiki/spec/language/ownership.md#consumers-and-cleanup)
and [nominal cleanup](../../../../wiki/spec/terminal-psi/ownership.md#nominal-cleanup).
The borrowed call preserves the owner's custody; cleanup substitutes that exact
owner into the hook's prerequisite and retains its original output reference.

The nine projects exercise local, field, parameter, call result, match join,
const name, another state, another machine, and another target body. Each copies
that sentence and lends the same fresh Keeper to the same shared Unit call.
Its hook requires code 7 and writes through the original exclusive reference.
The following check observes 7 in the original Counter, so a skipped hook,
wrong backing or copied referent cannot satisfy the runtime oracle.

Run `omega run --both <place>/main.omg`. Every positive and the original root
must return 0 in both engines. Each `invalid/` twin supplies code 6 while leaving
the hook's code 7 requirement intact; it must reject automatic cleanup. Producer
postconditions and parameter preconditions remain truthful in each twin.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [program](local/main.omg) | [code 6](local/invalid/main.omg) |
| field | [program](field/main.omg) | [code 6](field/invalid/main.omg) |
| parameter | [program](parameter/main.omg) | [code 6](parameter/invalid/main.omg) |
| call result | [program](call_result/main.omg) | [code 6](call_result/invalid/main.omg) |
| match join | [program](match_join/main.omg) | [code 6](match_join/invalid/main.omg) |
| const name | [program](const_name/main.omg) | [code 6](const_name/invalid/main.omg) |
| another state | [program](other_state/main.omg) | [code 6](other_state/invalid/main.omg) |
| another machine | [program](other_machine/main.omg) | [code 6](other_machine/invalid/main.omg) |
| another target body | [program](target_body/main.omg) | [code 6](target_body/invalid/main.omg) |

The [original witness](main.omg) and its [reject twin](invalid/main.omg) keep
the same obligations. Construction, parameter contracts and completed producer
contracts establish code 7; the match join supplies it on both arms. Each reject
twin instead truthfully supplies 6 and reaches the authored `Keeper::drop`
clause. The runtime check observes the original Counter after cleanup, testing
the retained reference's backing identity and lifetime through the shared call.

This row verifies existing reference-bearing nominal receiver and normal-call
cleanup support, rather than a new compiler repair. Only that obsolete exclusion
is removed from [CML4](../../../../TASKS.md). The writeback oracle does not count
hook invocations or establish crash, fuel/retry, suspension or cross-host
acceptance; CML4's remaining residual-cleanup and computed-call requirements stay
open. All four targets are bound, with actual native/Terminal exits and reject
diagnostics recorded for macOS ARM64 in the qualification and claim evidence.
