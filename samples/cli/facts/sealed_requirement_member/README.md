# Sealed requirement member

Owning task: **STATIC-MACHINE-MIGRATION** in
[TASKS.md](../../../../TASKS.md). This family retires `CallingPolicy::plan =
Existing::plan` rows whose callee already `satisfies CallingPolicy::plan`.
The inline member only calls that machine, so the member is that machine.

**Fact: Selected satisfies Sum and its inline combine only calls add, which already satisfies Sum::combine, preserving 0 <= result && result <= 100.**

## Canonical transport row

Every positive place declares `Expected exit: 0` and binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [main](local/main.omg) | [invalid](local/invalid/main.omg) |
| Field | [main](field/main.omg) | [invalid](field/invalid/main.omg) |
| Parameter | [main](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| Call result | [main](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| Match join | [main](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| Const name | [main](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| Nested machine | [main](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| Another machine | [main](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| Another target body | [main](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

`omega run --both <place>/main.omg` reports Terminal Psi and native exit 0.
Each twin removes only the right operand bound and rejects. The static
ensures is the closed range. `main` checks both `20 + 22 = 42` and
`10 + 12 = 22`.

Open under STATIC-MACHINE-MIGRATION: reference rows whose callee does not
already satisfy the requirement, including the other rows in
`interrupt_table_canary` and `secondary_processor_canary`.
