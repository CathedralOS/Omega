# Inline member helper

Owning tasks: **STATIC-MACHINE-MIGRATION** and **BUILD-OWNED-TARGET-SELECTION** in
[TASKS.md](../../../../TASKS.md). The requirement exercised here is an inline
member that calls a shared helper. The member and helper retain separate
callable identities; reuse is an ordinary checked call inside the member.

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
Each twin changes only its place's operand-bound declaration, ceiling constant,
or produced value. It rejects at the call that needs the missing bound (`step`
in the nested-machine place, `apply` elsewhere); the call-result twin also
disproves its supplier's range promise. `main` checks both `20 + 22 = 42` and
`10 + 12 = 22`. The match-join place takes the true arm for 42 and the false
arm for 22.

## Evidence

- Establishment: `add` and `combine` both require the operand bounds and ensure
  `result == left + right`. The member body is the call `add(left, right)`.
- Identity: `Selected` is the named implementation. `add` is a separate machine
  and does not satisfy `Sum::combine`.
- Composition: `apply` substitutes `Selected` and returns `Operations::combine`.
  Joins and transports copy that call.
- Soundness: each twin changes one bound, constant or produced value, retaining
  both runtime calls and all other contracts. Call-result suppliers expose
  their result ranges independently; returning 51 disproves the right range
  and the consuming call. Auxiliary sum equalities use `i64` so a missing
  operand bound is diagnosed at its call rather than as an overflowing `i32`
  expression in the postcondition. The helper remains an ordinary checked call.
- Checked lowering: the generic `<=` and `+` in `Sum` are schematic over
  `Value`. The concrete member uses `i32`.
- Calling plans: each positive binds `ProgramEntry` for the four hosted targets.
- Instruction and image: `omega run --both` on this Linux x86_64 host.
- Runtime falsifier: both sums are checked. A constant 42 fails the second call.
  Match join exercises both arms.

The target-body place declares ordinary `X86Probe` and `ArmProbe`
implementations. Its Build records explicit `TargetProbe` and `BodyTag`
provider rows for all four hosted targets. The entry checks its architecture's
tag, and the selected probe checks the same tag before calling the shared
`probe`. Selecting either mismatched implementation traps, independently of
the two sum checks. No source machine has a target prefix.

Open under STATIC-MACHINE-MIGRATION: reference-row retirement in the Omega-written producer,
`dropped by`, and source-visible `Binding<R>`.
