# Case-recovered type equations

Matching `Value::Integer` or `Value::Boolean` recovers `T == i32` or `T == bool` from the data's default domain, so the generic extractor `extract<T>(v: Value<T>) -> T` returns each payload as `T`, and the specialization for `Value<i32>` or `Value<bool>` keeps only the arm its instance admits.

Owning task: **DOMAIN-DEFAULT-PREDICATE-UNIFICATION** in
[TASKS.md](../../../../TASKS.md). Exact requirement: a case match combines the
selected case with the established default domain to recover its type equation,
including when the result is checked against an enclosing generic return type
([case constraints](../../../../wiki/spec/language/data_and_literals.md#case-constraints),
[static type equality](../../../../wiki/spec/language/generics.md#static-type-equality)).
The sibling row [case_dependent_default_domain](../case_dependent_default_domain/README.md)
closes the closed-instance side (`Value<bool>` admits only `Boolean`, `Range`
recovers `lo <= hi`); this row closes the generic side that row never exercised.

## Baseline defect

Before this row, on `origin/main` 0402685c2f2, every place was refused by the
generic body alone, with six diagnostics, the first two being:

```text
error: match arms produce incompatible values
error: machine `extract` state `entry` return value expects `T`, but the value is delivered as `i32`
```

A second defect sat behind it. A `match` or `transition` that lists an arm for a
case its instance refutes (`Value::Integer` on `Value<bool>`) passed `--check`
and then failed Terminal production with `case observation lost its exact
selected case`.

## What the layer records

The data declaration states one type equation per case in its default-domain
`requires`. The checker's static type worlds now record, for each case test,
the equations of the selected case over the subject's applied arguments:

- On an open application (`Value<T>` with `T` rigid in `extract`) the selected
  case contributes `T == i32` or `T == bool`, so inside that arm `T` is `i32` or
  `bool` exactly as inside a `transition T == u32` branch. The `i32` payload is a
  `T` in one arm and the `bool` payload is a `T` in the other. The two arms hold
  different equations for one binder, so each is judged against the destination
  `T`, not against the other arm.
- On a closed instance (`Value<i32>`, `Value<bool>`) the instance already
  decided each equation, so the case it refutes can never hold. Its world is
  empty, the arm is unreachable, and typed preparation drops it, re-anchoring
  the match's completed subject on a surviving arm. No later stage rosters a
  tag, payload or successor for a case the instance cannot carry.

The consuming operation is `extract`'s dispatch, written once against that
recorded list. Every place only transports the `Value<i32>`/`Value<bool>` pair.

## Places

All projects bind `ProgramEntry` for the four hosted targets, declare
`Expected exit: 0` and are run with `omega run --both`, which requires the
Terminal Psi interpreter and the native image to agree. `main` observes `7`
and `9` from `Value<i32>` and `true` and `false` from `Value<bool>`, so an
extractor that returns a constant, the wrong payload, or the other case's
payload traps.

| Place | Transport | Positive | Reject twin |
| --- | --- | --- | --- |
| local | Immutable locals. | [local/main.omg](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | `Carrier.number` and `Carrier.flag`. | [field/main.omg](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | Explicit arguments; four probes distinguish payload and expectation. | [parameter/main.omg](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call_result | Completed calls return the values. | [call_result/main.omg](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match_join | Both joined values extract with distinct payloads in both orders. | [match_join/main.omg](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| const_name | Named immutable constants. | [const_name/main.omg](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| other_state | A nested machine receives the values at a terminal transfer. | [other_state/main.omg](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| other_machine | An ordinary forwarding machine receives the values. | [other_machine/main.omg](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| target_body | Build selects the checked named provider for all four hosted targets. | [target_body/main.omg](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

The tenth witness is the other production of the same layer. It extracts through
`transition v { Value::Integer { value } -> (value) ... }` rows instead of a
`match`, so the dead-row path that failed with the same lowering diagnostic is
pinned in this folder under the same sentence:
[transition_rows/main.omg](transition_rows/main.omg) and
[its twin](transition_rows/invalid/main.omg).

Each `invalid/main.omg` differs from its positive by one line: the `Boolean`
alternative of the default domain loses `T == bool`. That removes one
requirement from the recorded list, so matching `Boolean` no longer recovers the
equation and the `bool` payload is no longer a `T`. The diagnostic names the
missing requirement:

```text
error: machine `extract` state `entry` return value expects `T`, but the value is delivered as `bool`
```

Positive and negative counterparts are pinned as
`tests/omega/{pass,fail}/facts/case_type_equation_recovery_through_<place>_{exit,rejected}`
and `tests/omega/{pass,fail}/facts/case_type_equation_recovery_transition_rows_{exit,rejected}`.

## Requirement layers

- **Establishment.** Construction proves the whole default domain before a
  value exists: `Value::Integer` constructs only `Value<i32>`, kept by the
  sibling row's twins and `fail/dependent/case_where_generic_type_bound_impossible_case`.
  The dispatch itself establishes the equation; no caller guard stands in for it.
- **Identity and lifetime.** The equation is read at the arm's own expressions.
  It does not leak into the other arm or past the dispatch.
- **Composition.** Calls substitute the exact `Value<i32>`/`Value<bool>`
  actuals; joins and nested or forwarding machines carry them unchanged.
- **Soundness boundaries.** The twin shows the equation is a premise taken from
  the declaration, not assumed from the destination: without `T == bool` the
  generic body is rejected, and so is the `Value<i32>` specialization.
- **Checked-to-Terminal Psi.** `extract<i32>` and `extract<bool>` each lower a
  one-case dispatch over an instance whose other case is refuted; the dropped
  arm contributes no tag, payload or successor.
- **Provider settlement, layout, instruction selection, encoding.** No new
  mechanism: both specializations are ordinary one-case dispatches that the
  existing native path already emits, and the nine native images agree with
  Terminal Psi on this host.

## Evidence

Producer: the working tree at `origin/main` 0402685c2f2 plus this change,
built at `opt_level 1`. Host: Windows x86_64. `omega run --both` reports native
exit 0 and interpreter exit 0 for all ten positives; the ten twins reject with
the diagnostic above. Linux x86_64, Linux arm64 and macOS arm64 native images
are bound by the same `build.omg` and unverified here.

## Remaining open

A `requires T == i32` contract that excludes `Boolean` from a generic dispatch
without listing the arm (so `match v { Value::Integer { value } -> value }`
covers the subject) still reports `match does not cover its subject`. It is a
coverage rule for rigid parameters, not the type-equation recovery claimed here.
