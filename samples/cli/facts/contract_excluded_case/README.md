# Contract-excluded case

A generic extractor whose contract states `T == i32` (or `T == bool`) owes no
arm for the case whose default domain states the other type, so `extract_int`
lists only `Integer` and `extract_flag` only `Boolean`: they return `7` and `9`
from `Value<i32>`, `true` and `false` from `Value<bool>`.

Owning task: none open. `DOMAIN-DEFAULT-PREDICATE-UNIFICATION` was deleted by
#1097, whose acceptance lists coverage, and this row found that coverage still
owed the excluded case. Exact requirement: coverage consumes the established
type equation, so a dispatch omits a case when selecting it contradicts the
facts in force there ([coverage and domain
tests](../../../../wiki/spec/language/patterns.md#coverage-and-domain-tests),
[static type equality](../../../../wiki/spec/language/generics.md#static-type-equality)).
The sibling [case_type_equation_recovery](../case_type_equation_recovery/README.md)
row runs the other direction (a case match recovers the equation); this row
runs equation to coverage, the half that row records as left undone.

## Baseline defect

On `origin/main` `a1efeec13ed` (#1097 merged) every place was refused by
checking, with two diagnostics, one per extractor:

```text
error: match does not cover its subject; cover every admitted case, both Boolean alternatives, or add a wildcard arm
```

`extract_int<T>(v: Value<T>) -> T requires T == i32` lists `Integer` only. The
open `Value<T>` carries both cases. #1097 computes the cases admitted at a match
from the complete default-domain predicate under each hypothetical tag
(`admitted_case_judgment`), and the `Boolean` trial must contradict the
contract's `T == i32` with the predicate's `T == bool`. It never did.

## What the layer records

The structural judge represents a type proposition as an opaque atom named by
its operands' identities, so `T == i32` and `T == bool` were unrelated facts and
no trial could be contradictory. Static type equality is exact, so a binder
equated to one closed type is not equal to any other. The atom's name now also
records which operands are closed, and when a type atom is bound true while
another atom already true equates the same open operand with a different closed
type, the hypotheses are contradictory (`note_type_atom`). The `Boolean` trial
then refutes, `admitted_cases` drops it, and coverage (which already reads
`admitted_match_cases`) owes no arm for it. No second evaluator and no per-case
clause is involved: the data's complete predicate is still the only source.

A binder that nothing equates to a closed type (the twins), a contract that
admits both alternatives (`T == i32 || T == bool`), and a contract that names the
case that was listed all leave the case owed.

## Places

All projects bind `ProgramEntry` for the four hosted targets, declare
`Expected exit: 0` and are run with `omega run --both`, which requires the
Terminal Psi interpreter and the native image to agree. `main` reads `7` and
`9` through `extract_int` and `true` and `false` through `extract_flag`, so an
extractor that returns a constant, the wrong payload, or the other case's
payload traps. The `target_body` place crosses a boundary trait whose named
implementation `Selected` Build binds for each hosted target.

Each `invalid/main.omg` deletes only `requires T == i32` from `extract_int`.
Without the contract nothing excludes `Boolean`, so the same `match` stops
covering its subject and checking refuses it.

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

`branch_world` is a tenth witness for another production of the same layer: the
equation comes from the enclosing branch (`transition T == i32 { true -> ... }`)
instead of a contract, and the `match` inside the true arm lists `Integer` only.
Its twin replaces the branch condition with `true`, so no equation reaches the
`match` and `Boolean` is owed again.

## Requirements

- **Establishment:** the contract (or branch) is the only source of `T == i32`
  here, and the caller must satisfy it: each call passes `Value<i32>` or
  `Value<bool>` to the extractor that states the matching equation.
- **Composition:** the pair of extractors is reached through a local, a field, a
  parameter, a call result, a `match` join, a constant, a nested machine,
  another machine and a Build-selected named implementation.
- **Soundness boundaries:** a case is dropped only by a contradiction among
  hypotheses. An unconstrained binder excludes nothing (the twins), a contract
  that admits both alternatives excludes nothing, and listing the wrong arm
  (`Boolean` under `T == i32`) leaves `Integer` owed; the last two were run as
  probes, not pinned.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** each specialization lowers
  a closed `Value<i32>` or `Value<bool>` whose refuted case is already absent
  from the sum; the omitted arm adds no roster entry.
- **Runtime falsifier:** `main()` observes four distinct results through two
  extractors, so a mis-selected arm or payload traps; flipping one expected
  value makes the program trap.
- **Open:** a `transition` over the same subject still owes the excluded case.
  Its coverage pass (`exhaustiveness.rs` in stage 03) runs on resolved trees,
  before any proof judgment exists, so it cannot see the contract
  (`match over Value does not cover Value::Boolean`). A dispatch inside a branch
  that a specialization makes unreachable keeps its refuted arm and is refused.
