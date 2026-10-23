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

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the qualified attributes are reached through a local `view: &Attributes` binding |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the view is stored in `Holder.view` and consumed through the field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `parameter(&attributes)` passes the reference as an explicit parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `keep(&attributes)` returns the same `&'scope Attributes` |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `choose` returns the reference through both arm states |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | the zero written at word 3 arrives as the named constant `TERMINATOR = 0` |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor state `inspect` receives the reference |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `relay` forwards the reference to `consume` |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target bodies forwards the reference |

Run a place through the public route:

```sh
omega run --both samples/cli/facts/nested_borrow_qualification/local/main.omg
omega --check samples/cli/facts/nested_borrow_qualification/local/invalid/main.omg
```

The places are local, field, parameter, call result, match join, const name,
another state, another machine, and another target body. The constant names the
stored zero; the target bodies perform the same consuming call.

## Acceptance and parent scope

Parent: FILESYSTEM-PLATFORM-SERVICES. The immediate customer is Windows
handle-relative removal's nested borrowed native input contract.

All nine places execute with native/Terminal exit 0 on macOS arm64. Each reject
twin removes only the stored qualification and rejects the same consuming
requirement. The state edge retains the actual transitive `Name` backing rather
than relying on retired stack bytes surviving native execution.

This establishes nested qualification transport, not completion of the Windows
portable filesystem provider. Windows runtime execution is unavailable and
explicitly deferred by the owner. Foreign image production is not runtime
acceptance on that target.

## Pinned Fixtures

The row is pinned by 18 fixtures: nine
`tests/omega/pass/facts/nested_borrow_qualification_through_<place>_exit`
fixtures, each a byte-identical copy of its place program and `build.omg`
(`ProgramEntry` for all four hosted targets, `Expected exit: 0`), and nine
`tests/omega/fail/facts/nested_borrow_qualification_through_<place>_rejected`
twins, each a byte-identical copy of its `invalid/` program pinning the
diagnostic `cannot prove requires contract`. Recorded rows: 18 `checked` with
`check-target:macos_arm64` and 18 `rejected` with `expected` in
[`corpus_outcomes.txt`](../../../../tests/omega/corpus_outcomes.txt), 9
`built exit:0 interpreter-exit:0 interpreter:agrees` in
[`corpus_both_macos_arm64.txt`](../../../../tests/omega/corpus_both_macos_arm64.txt).

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
