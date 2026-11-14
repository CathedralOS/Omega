# Inline implementation calling existing code

`Selected` implements `Sum` with an inline `combine` member that calls `add`.
Both require each input to be between 0 and 50 and promise a result between
0 and 100. `probe` selects `Selected` explicitly when calling generic `apply`.
The inline member and `add` keep distinct machine identities.

Owning tasks: **STATIC-MACHINE-MIGRATION**, replacing old member-to-machine
assignments with ordinary inline calls; and **BUILD-OWNED-TARGET-SELECTION**,
replacing target prefixes with explicit Build selections, in
[TASKS.md](../../../../TASKS.md).

**Fact: Selected satisfies Sum and its inline combine only calls add, which already satisfies Sum::combine, preserving 0 <= result && result <= 100.**

Every positive expects exit 0 and binds all four hosted targets. Each runs both
`20 + 22 = 42` and `10 + 12 = 22`. The match place exercises both choices with
different pairs, so selecting the wrong branch fails the runtime check.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [main](local/main.omg) | [invalid](invalid_local/main.omg) |
| Field | [main](field/main.omg) | [invalid](invalid_field/main.omg) |
| Parameter | [main](parameter/main.omg) | [invalid](invalid_parameter/main.omg) |
| Call result | [main](call_result/main.omg) | [invalid](invalid_call_result/main.omg) |
| Match join | [main](match_join/main.omg) | [invalid](invalid_match_join/main.omg) |
| Const name | [main](const_name/main.omg) | [invalid](invalid_const_name/main.omg) |
| Nested machine | [main](other_state/main.omg) | [invalid](invalid_other_state/main.omg) |
| Another machine | [main](other_machine/main.omg) | [invalid](invalid_other_machine/main.omg) |
| Another target body | [main](target_body/main.omg) | [invalid](invalid_target_body/main.omg) |

The paired invalid programs keep the same runtime checks and result promises.
Their inputs or producing contracts lack a range needed by `apply`, `step` or
`forward`. The call-result control keeps both supplier calls but removes the supplier
contracts and the right input bound. The const-name control widens the
shared ceiling from 50 to 100; it weakens both operand bounds. Each refusal must
identify the missing range at the call that needs it.

The selected target place calls `CombineProbe::probe`. One ordinary
`HostedProbe` implementation performs the same `block apply<Selected>(a, b)`
operation for all four platforms. Four explicit Build rows select it. Its
published contract retains both input bounds, the result range, empty service
reach and permission to block. The caller acknowledges blocking and names
`CombineProbe` in its direct-call ceiling. Shared code follows
[provider selection](../../../../wiki/spec/build/provider_selection.md#build-owned-target-selection),
[inline members](../../../../wiki/spec/language/conformances.md#declaration-and-selection)
and [call effects](../../../../wiki/spec/language/effects.md).

## Requirements

- Construction and call entry establish the operand bounds. Field domains,
  supplier contracts and named ceilings must justify them before the call.
- Copies preserve the values carried through locals, fields and calls.
  The final equality check observes the result; entry facts justify the call.
- Every place passes the same bounded inputs through `Selected::combine` to
  `add`. The selected target implementation keeps the same public promises.
- Explicit implementation selection and ordinary satisfaction checks provide
  the code. Build selection supplies no missing proof or hidden data argument.
- Portable lowering keeps the evaluated inputs, blocking calls and result.
  Native and Terminal execution must agree on both authored sums.
- Provider selection and ordinary i32 calling conventions preserve the inputs
  and result. The native arithmetic, calls, stack storage and emitted image
  must execute with the expected result; checking alone does not establish it.

## Validation and remaining work

The unchanged baseline `feef96dc297dbd12c2185973f1a216be927c060f` passes all
nine sample programs in native and Terminal execution on Linux x86-64. All nine
invalid projects reject as intended. Its target fixture pair also checks and
rejects as intended, and the positive runs with exit 0 in both engines:
[baseline run 41546993119](https://github.com/CathedralOS/Omega/actions/runs/41546993119).
These observations establish the old source's behavior.

The [migration run 41549454869](https://github.com/CathedralOS/Omega/actions/runs/41549454869),
source `d93716f3312454d024e4585ec999bcfe347f5c4e`, accepts all nine positive
fixtures and executes them with native and Terminal exit 0. All nine positive
sample projects also run successfully. Its eight unchanged invalid programs
reach their intended missing-range refusals.

Review then restored both supplier calls in the call-result invalid program.
[Run 41552699446](https://github.com/CathedralOS/Omega/actions/runs/41552699446),
source `6adf3a04cbb0e8592a922c2205e4085109a2d08a`, checks that corrected fixture
and its sample. Both reject because `supply(b)` cannot establish the required
right-input range. The compiler, library, gate tools and all other program
inputs are unchanged between the two sources; the verified CLI executable
hashes also match. Records retain the source each run actually used.

Together these observations cover all nine positives and all nine current
invalid controls. No compiler repair is part of this source migration.

This row does not close either parent task. Other target-prefixed sources and
old reference rows whose callee does not already satisfy the requirement remain
on their existing tasks. Native execution on the other hosts is unverified.
