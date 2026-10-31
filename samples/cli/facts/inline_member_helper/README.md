# Inline member helper

Owning task: **STATIC-MACHINE-MIGRATION** in
[TASKS.md](../../../../TASKS.md). The requirement exercised here is an inline
member that calls a shared helper. The helper does not satisfy the requirement,
so the member remains that call. A member whose only body calls a machine that
already satisfies the same requirement is that machine instead.

**Fact: Selected satisfies Sum and its inline combine calls shared helper add, preserving result == left + right.**

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
Each twin removes only the right operand bound and rejects because `apply`
cannot prove `combine`'s requires. `main` checks both `20 + 22 = 42` and
`10 + 12 = 22`. The match-join place takes the true arm for 42 and the false
arm for 22.

## Evidence

- Establishment: `add` and `combine` both require the operand bounds and ensure
  `result == left + right`. The member body is the call `add(left, right)`.
- Identity: `Selected` is the named implementation. `add` is a separate machine
  and does not satisfy `Sum::combine`.
- Composition: `apply` substitutes `Selected` and returns `Operations::combine`.
  Joins and transports copy that call.
- Soundness: the reject twin drops the right bound only. The helper call is
  not rewritten into a reference row.
- Checked lowering: the generic `<=` and `+` in `Sum` are schematic over
  `Value`. The concrete member uses `i32`.
- Calling plans: each positive binds `ProgramEntry` for the four hosted targets.
- Instruction and image: `omega run --both` on this Linux x86_64 host.
- Runtime falsifier: both sums are checked. A constant 42 fails the second call.
  Match join exercises both arms.

Open under STATIC-MACHINE-MIGRATION: repository-wide reference-row retirement,
`dropped by`, and source-visible `Binding<R>`.
