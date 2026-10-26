# dependent_subtract

`count - used` under `used <= count` is non-negative in exact `u32`.
The standing `count <= 11` bound makes the exact result conversion to `i32`
valid. Every place retains that fact, bound and operation.

The parent outcomes are `CONSERVATION-CONTRACT` and
`CANONICALIZE-SCALAR-RANGE-CONTRACTS` in `TASKS.md`. This row establishes exact
relational subtraction across the nine transport places below; it does not
close either parent outcome.

## Places and controls

| Place | Positive | Reject twin | Transport at the consuming operation |
| --- | --- | --- | --- |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | Copies the two fields into named locals before subtracting. |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | Subtracts the two field projections directly. |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | Checks the relation on the subtraction machine's parameters. |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | Subtracts the actual `used_amount(main)` result under its exact result postcondition. |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | Subtracts a match result justified on both arms; the runtime entry exercises both arms. |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | Subtracts `USED` directly under its relation to the field. |
| another state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | Carries count and used to `finish`, whose arrival contract justifies subtraction there. |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | `copied_used` transports the relation through its result contract into the consuming machine call. |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | Each of the four authored target bodies proves the same parameter subtraction. |

The root [main.omg](main.omg) is a runnable representative. Every positive and
the root bind `ProgramEntry` for `windows_x86_64`, `linux_x86_64`,
`macos_arm64` and `linux_arm64`, declare `Expected exit: 0`, and retain an
`expected_native_exit.txt` of zero. The nine places are pinned under
`tests/omega/pass/facts/dependent_subtract_through_<place>_exit/`; their
check-only reject bodies are pinned under
`tests/omega/fail/facts/dependent_subtract_through_<place>_rejected/`.
Sample and fixture main sources match byte for byte for each place.

The reject twins retain their original consuming operations. They remove the
relevant relational premise, not the transport construct: the call-result twin
removes `ensures result <= main.count`; the match twin still subtracts the
match result; the state twin still subtracts in `finish`. Parameter, machine
and target-body twins remove the subtraction machine's `used <= count`
precondition. The positive runtime entry checks 7 and 0 as output oracles;
those post-operation checks supply no fact to the earlier subtraction. Negative
bodies need no runtime entry because checking must refuse them before execution.

Run a place and its control:

```sh
omega run --both samples/cli/facts/dependent_subtract/call_result/main.omg
omega --check --verify samples/cli/facts/dependent_subtract/call_result/invalid/main.omg
```

A positive must report actual native exit 0 and Terminal exit 0. A reject must
exit 1 with ``local `remaining` may overflow `u32`:`` in the original consuming
machine/state. The target-body twin reports that obligation for all four bodies.
An import, setup or unrelated rejection does not count.

## Requirements established and limits

- **Establishment:** `requires` clauses establish `used <= count` before exact
  subtraction. The call-result postcondition establishes the bound on the exact
  returned value; every selected match arm justifies the joined bound. The data
  `where count <= 11` or the explicit parameter/state contract retains the
  conversion bound. See [dependent values](../../../../wiki/spec/language/dependent_values.md)
  and [scalar contracts](../../../../wiki/spec/language/domains.md).
- **Identity and lifetime:** locals, fields, parameters, call results and const
  values retain their exact scalar subjects. This row has no intervening source
  mutation; write invalidation remains separate acceptance.
- **Composition:** ordinary calls substitute their actual values and published
  result facts; match joins justify every arm; state transitions prove their
  arrival requirements. The operation consumes those facts directly. See
  [fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md).
- **Soundness boundaries:** each control removes the subtraction premise and
  refuses at that operation. A callee body cannot assume a relation merely
  because its current callers happen to supply related values. These programs
  add no authority, trusted arithmetic axiom or termination exception.
- **Checked-to-Terminal Psi:** the positive programs lower through the ordinary
  Terminal Psi route and run in its interpreter. Erased relational evidence
  justifies executable exact integer arithmetic; it is not a runtime assertion.
- **Provider settlement, layout and calling plans:** the macOS ARM64 runs cover
  the row's scalar/record layout, `ProgramEntry`, ordinary call and state
  argument/return placement. They do not establish unrelated stack-argument,
  aggregate-return or foreign-call lanes.
- **Instruction and frame realization:** macOS ARM64 execution covers the exact
  subtraction, conversion, calls and state branches needed by this row. It is
  not evidence for other ISA instruction families or physical ABI boundaries.
- **Encoding and native emission:** the generated macOS ARM64 image executes and
  agrees with Terminal Psi. Four hosted source bindings are present; execution
  on Windows, Linux x86_64 and Linux ARM64 remains unverified by this local run.

## Current evidence

On 2026-10-20, the original `9b2bf86d5b44c98a763fbf4074ba92f028a23ea6`
nine-place sources and oracles were frozen before restoring this row. Against
immutable `canonical-field-contracts-1`, all nine positives ran with native and
Terminal exit 0 and all nine original controls rejected at their intended
exact-arithmetic obligation. No compiler edit was required. The restored tracked
row was then run separately: the nine positives plus root returned 0 in both
engines, and the nine restored controls rejected as intended. Direct execution
of the pinned fixture paths also produced native/Terminal exit 0 for all nine
positives and the intended arithmetic refusal for all nine controls. The focused
18-fixture corpus check admitted nine positives and rejected nine controls; its
nonzero comparison exit records nine expected diagnostic-location changes from
the replaced helper to the restored consuming machine/state. Those golden
updates remain part of integration.

The CLI SHA256 is
`de3d601bfc1cdd4fdebbcdca7cc52d02e82051c7865513b6a2bf354a0ede24c2`,
with verified optimization level 1. Its source is base
`11f36ca90e8ea9ff93a52577592d8b8d60bca6be` plus the recorded
`canonical-field-contracts-1` patch, not a clean commit masquerading as that
producer. Raw baseline and restored-row records are retained under
`.codex/recovery/current-resume-20261020/dependent-subtract-original-9b2/`
and `dependent-subtract-restored-row-2/`, including source, SDK and producer
hashes. Pinned-path runtime records are in `dependent-subtract-fixture-runtime-1/`
and the corpus comparison is in `dependent-subtract-fixture-corpus-1/` under the
same recovery root. An interrupted mixed-source attempt is retained as
invalidated and supplies no acceptance.

These observations establish this frozen producer's behavior. Integrated
candidate qualification, independent review and authorized landing remain with
the owning integration. Variable array/slice lengths and nonlinear
multi-variable relational constraints remain outside this row's acceptance.
