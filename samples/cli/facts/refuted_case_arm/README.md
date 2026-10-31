# Refuted case arm

A generic extractor lists every case of `Value<T>`, so the arm naming the case
the instance's default domain refutes (`Integer` on `Value<bool>`, `Boolean` on
`Value<i32>`) selects no value: `extract` returns `7` from `Value<i32>`, `true`
and `false` from `Value<bool>`.

Parent: `DOMAIN-DEFAULT-PREDICATE-UNIFICATION` in
[TASKS.md](../../../../TASKS.md). This row executes the generic typed extractor
over `Integer`/`Boolean` through native and Terminal Psi (`omega run --both`).
[case_dependent_default_domain](../case_dependent_default_domain/README.md)
carries the same default domain to a `Boolean`-only extractor and a guarded
`Range` consumer; this row carries the other half of that acceptance, the body
that must name both cases.

`Value<T>` carries its `requires` disjunction as the default domain. On the
closed instance `Value<bool>` the `T == i32` conjunct refutes `Integer`, and on
`Value<i32>` the `T == bool` conjunct refutes `Boolean`. Typing already counts
a refuted case as impossible, so a body over one instance may omit it. A body
generic over the family cannot: `extract<T [copy]>` is checked once for every
`T`, so it names both cases, and each instance of it carries one arm that no
value reaches.

Before the repair Terminal production built that arm anyway and failed after
checking succeeded. The recorded diagnostics, on `origin/main`
`0ff9f81b393`, were
`terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine:
"extract$specialized$…", reason: "field observation selected an absent case" })`
for the value-position `match`, and `reason: "case observation lost its exact
selected case"` for a `transition` over a `Value<bool>` with an `Integer` arm.
The lowered sum declares only the possible cases, so the arm's tag test and
payload read named a case it no longer had.

The requirement the producing layer dropped: the checked arm list is the
possible cases in authored order. Stage 04 now omits an arm whose case the
subject's instance refutes from the closed-sum dispatch roster, from the
structural scalar dispatch, and from the scalar computation roots of a
`transition` arm, through one predicate
(`proof::refuted_case`). Stage 05 keeps its integrity check that an
observation names a declared case.

Each of `local`, `field`, `parameter`, `call_result`, `match_join`,
`const_name`, `other_state`, `other_machine`, and `target_body` transports the
same `Value<i32>`/`Value<bool>` triple to the same `extract` consumer. The
`target_body` place crosses a boundary trait whose named implementation `Selected`
Build binds for each hosted target. All projects bind the four hosted targets
and require `omega run --both` native and Terminal agreement on exit 0.

Each `invalid/main.omg` constructs the refuted case on the instance
(`Value::Integer { value: true }` as a `Value<bool>`) and changes nothing else,
so the default domain must reject the construction. Positive and negative
counterparts are pinned as
`tests/omega/{pass,fail}/facts/refuted_case_arm_through_<place>_{exit,rejected}`.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [local/main.omg](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | [field/main.omg](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | [parameter/main.omg](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call_result | [call_result/main.omg](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match_join | [match_join/main.omg](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| const_name | [const_name/main.omg](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| other_state | [other_state/main.omg](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| other_machine | [other_machine/main.omg](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| target_body | [target_body/main.omg](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

## Requirements

- **Establishment:** each instance admits its active case by construction
  (`Value::Integer` on `Value<i32>`, `Boolean` on `Value<bool>`); the twin
  constructs the refuted case and the default domain rejects it.
- **Identity and lifetime:** the instance, not the template, decides which case
  is refuted. `extract` specializes once per `T`, and the two instances pick
  opposite arms.
- **Composition:** the triple crosses a local, a field, a parameter, a call
  result, a `match` join, a constant, a nested machine, another machine and a
  Build-selected named implementation unchanged.
- **Soundness boundaries:** rejecting construction of the refuted case is what
  makes dropping its arm sound; the arm is unreachable, not unchecked.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** the lowered sum declares
  the possible cases only, and the checked dispatch lists the arms that select
  one of them.
- **Runtime falsifier:** `main()` returns `7` through the `Integer` arm, `true`
  and `false` through the `Boolean` arm, three distinct outcomes, so a
  mis-lowered tag test or a selected refuted arm traps. `match_join` also drives
  both operands of each join (`7`/`9`, `true`/`false`).
- **Open:** `Omega 00–09` realization of the sum is unchanged by this row and is
  exercised only by the native run on the recorded host.

Governing contracts: [domains](../../../../wiki/spec/language/domains.md),
[data and literals](../../../../wiki/spec/language/data_and_literals.md),
[generics](../../../../wiki/spec/language/generics.md),
[patterns](../../../../wiki/spec/language/patterns.md).
