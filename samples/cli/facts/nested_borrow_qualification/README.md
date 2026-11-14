# Nested borrowed qualification

A nested borrowed record preserves the final-word-is-zero qualification for a
consuming call. Every place uses the same four-word array, last index 3, and
`final_word` operation requiring zero. The two typed pointer crossings are
`Attributes.name` and `Name.words`; neither numerical pointers nor a caller
guard after transport can replace the carried fact.

Each place has its own `build.omg`, binds all four hosted targets, and declares
exit 0. Its `invalid/` twin removes only the `Terminated` qualification from
the stored words reference. The same consumer must then reject its unproved
zero requirement.

The consumer, its parameter types and the stored domain are public. Its body
therefore proves the consuming call from the field's declared qualification.
[Private helper inference](../../../../wiki/spec/language/state_contracts.md)
can use facts from every known caller; with only zero-tail callers, removing the
field qualification from a private helper still leaves a valid program.

Each positive also reads three distinct nonzero neighboring words through the
same nested reference. The runtime check fails if the consuming call returns a
nonzero result or a neighbor changes. This makes a wrong pointer or element
offset observable without adding a caller guard that proves the zero-tail fact.
The match join selects between two arrays with different first neighbors and
checks both selector values, so choosing the same arm twice fails at runtime.

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`invalid_local/main.omg`](invalid_local/main.omg) | the qualified attributes are reached through a local `view: &Attributes` binding |
| field | [`field/main.omg`](field/main.omg) | [`invalid_field/main.omg`](invalid_field/main.omg) | the view is stored in `Holder.view` and consumed through the field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`invalid_parameter/main.omg`](invalid_parameter/main.omg) | `parameter(&attributes)` passes the reference as an explicit parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`invalid_call_result/main.omg`](invalid_call_result/main.omg) | `keep(&attributes)` returns the same `&'scope Attributes` |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`invalid_match_join/main.omg`](invalid_match_join/main.omg) | `choose` returns the reference through both arm states |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`invalid_const_name/main.omg`](invalid_const_name/main.omg) | the zero written at word 3 arrives as the named constant `TERMINATOR = 0` |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`invalid_other_state/main.omg`](invalid_other_state/main.omg) | a successor machine `inspect` receives the reference |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`invalid_other_machine/main.omg`](invalid_other_machine/main.omg) | `relay` forwards the reference to `consume` |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`invalid_target_body/main.omg`](invalid_target_body/main.omg) | each of four target bodies forwards the reference |

Run a place through the public route:

```sh
omega run --both samples/cli/facts/nested_borrow_qualification/local/main.omg
omega --check samples/cli/facts/nested_borrow_qualification/invalid_local/main.omg
```

The places are local, field, parameter, call result, match join, const name,
a nested machine, another machine, and another target body. The constant names the
stored zero; the target bodies perform the same consuming call.

## Acceptance and parent scope

Parent: FILESYSTEM-PLATFORM-SERVICES. The immediate customer is Windows
handle-relative removal's nested borrowed native input contract.

On producer `d58b8ab0f8fed8e2cd06c6fe4197fd20013334e2`, all nine
positives execute with native and Terminal exit 0 on Linux x64, Linux ARM64,
macOS ARM64 and Windows x64. Each reject twin reaches the missing stored-field
qualification at the same public consumer. Nonzero neighbors and both match
choices make incorrect addresses, stores or branch selection observable.
The [PR 1880 validation records](https://github.com/CathedralOS/Omega/pull/1880)
identify the exact producer and each host run.

This row establishes nested qualification transport. The parent Windows
filesystem-provider acceptance remains separate; a foreign image alone does
not demonstrate execution on that host.

## Pinned Fixtures

The row is pinned by 18 fixtures: nine
`tests/omega/pass/facts/nested_borrow_qualification_through_<place>_exit`
fixtures, each a byte-identical copy of its place program and `build.omg`
(`ProgramEntry` for all four hosted targets, `Expected exit: 0`), and nine
`tests/omega/fail/facts/nested_borrow_qualification_through_<place>_rejected`
twins, each a byte-identical copy of its `invalid/` program pinning the
diagnostic `cannot prove requires contract`. Their source records in
[`corpus_outcomes.txt`](../../../../tests/omega/corpus_outcomes.txt) come from
direct clean fixture observations at `ee4db530ba1e`. The fresh d58 cohorts
check these eighteen fixture IDs as well as the corresponding sample projects.
Fixture and sample observations retain their own input identities.

## Requirements witnessed

- Establishment: writing word 3 to zero establishes `Terminated` before record
  construction; the consuming helper relies on the stored qualification.
- Identity and lifetime: nested references address the original array and name
  storage; state transport keeps their actual backing live. Additional
  [owned forwarding](../../../../tests/omega/run/state_owned_nested_borrow/main.omg)
  and [shared backing](../../../../tests/omega/run/state_shared_nested_backing/main.omg)
  controls exercise moved carriers and two loans ending in different states.
- Composition: the nine places transport that same fact without changing its
  array, index, required value, or consuming operation.
- Soundness: deleting the field qualification must reject before execution.
- Checked-to-Terminal lowering: finite qualification paths retain concrete
  crossed declarations, without expanding unrelated pointer targets or dropping
  membership requirements. Terminal execution must agree with native execution.
- Provider/layout/ABI: four hosted entry bindings and typed references retain
  their distinct pointer carriers and exact referent layouts.
- Instruction/register/frame realization: calls and state edges must preserve
  original storage and keep live backing homes distinct from descriptor homes.
- Encoding/emission: native products must meet the authored zero exit; producing
  a foreign-target image alone is not that target's runtime acceptance.

## Build-owned target-body migration

The `target_body` place and its twin carry no source target prefix.
`BUILD-OWNED-TARGET-SELECTION` replaces the prefixed family with ordinary named
implementations (`X86...` and `Arm...`) of a boundary-trait requirement, selected
by explicit target-keyed Build rows. The operation, inputs, contracts and exit
oracle are unchanged. The entry wrapper and each implementation check a tag
(17 for x86-64, 29 for AArch64), so a wrong Build row traps. Observed evidence is
Linux x86-64 native and Terminal Psi agreement for the positive and source
rejection for the twin; the other hosted targets keep their authored rows,
unexecuted here.
