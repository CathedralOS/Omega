# Matched field windows

The shared fact and operation in every place: a match arm treats its integer
pattern as the subject's value, so in the arm `which == 0` the window
`items[0..which + 2]` has length 2, the same length as the other arm, and the
returned window publishes length 2. Every place copies that sentence and writes
the returned window through `write_pair`, which requires length at least 2.

Only the transport of the matched coordinate changes. Each place is an
application that exits 0:

| Place | Project | Transport |
| --- | --- | --- |
| local | `local/` | the pattern subject is a local copy |
| field | `field/` | the pattern subject is a record field |
| parameter | `parameter/` | the pattern subject is an explicit parameter |
| call result | `call_result/` | the window end is a call whose ensured result is 2 |
| match join | `match_join/` | the window end joins two spellings of that coordinate |
| const name | `const_name/` | the window end adds the named constant `SPAN` |
| another state | `other_state/` | a successor state receives the coordinate |
| another machine | `other_machine/` | a forwarding machine receives `which + 2` |
| another target body | `target_body/` | each of four target bodies publishes the same contract |

```sh
omega run samples/cli/facts/matched_field_windows/local/main.omg
omega run samples/cli/facts/matched_field_windows/field/main.omg
omega run samples/cli/facts/matched_field_windows/parameter/main.omg
omega run samples/cli/facts/matched_field_windows/call_result/main.omg
omega run samples/cli/facts/matched_field_windows/match_join/main.omg
omega run samples/cli/facts/matched_field_windows/const_name/main.omg
omega run samples/cli/facts/matched_field_windows/other_state/main.omg
omega run samples/cli/facts/matched_field_windows/other_machine/main.omg
omega run samples/cli/facts/matched_field_windows/target_body/main.omg
```

Expected exit: 0 for all nine. Every place binds `ProgramEntry` for all four
hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`,
`linux_arm64`) and passes `omega run --both`: the Terminal leg reports
`interp exit: 0` beside the native `exit 0`.

## Reject twins

Each place has one reject twin that breaks only this fact and still rejects.
Seven of them keep the pinning arm at length 2 and change the arm that does not
pin the pattern — `items[2..3]` instead of `items[2..4]` — so the two arms
disagree on published length. The remaining two break the pinned coordinate
itself: `call_result` makes the call's ensured result 3, and `other_machine`
hands `open_end` the coordinate 3 while that machine requires `end == 2`.

| Place | Twin | Pinned diagnostic |
| --- | --- | --- |
| local | `tests/omega/fail/facts/matched_field_windows_through_local_rejected` | `cannot prove ensures contract ... result.len == 2` |
| field | `tests/omega/fail/facts/matched_field_windows_through_field_rejected` | `cannot prove ensures contract ... result.len == 2` |
| parameter | `tests/omega/fail/facts/matched_field_windows_through_parameter_rejected` | `cannot prove ensures contract ... result.len == 2` |
| call result | `tests/omega/fail/facts/matched_field_windows_through_call_result_rejected` | `cannot prove ensures contract ... result.len == 2` |
| match join | `tests/omega/fail/facts/matched_field_windows_through_match_join_rejected` | `cannot prove ensures contract ... result.len == 2` |
| const name | `tests/omega/fail/facts/matched_field_windows_through_const_name_rejected` | `cannot prove ensures contract ... result.len == 2` |
| another state | `tests/omega/fail/facts/matched_field_windows_through_other_state_rejected` | `cannot prove ensures contract ... result.len == 2` |
| another machine | `tests/omega/fail/facts/matched_field_windows_through_other_machine_rejected` | `violates required fact `end == 2`` |
| another target body | `tests/omega/fail/facts/matched_field_windows_through_target_body_rejected` | `cannot prove ensures contract ... result.len == 2` |

The target-body twin breaks every one of the four target bodies, so it rejects
on every host rather than only where a body is realized.

`tests/omega/fail/slices/matched_open_window_end` is the related wildcard-arm
control: an arm that does not learn the coordinate cannot prove the open end.

## Pinned fixtures

The positive side is `tests/omega/pass/facts/matched_field_windows_through_<place>_exit`,
one fixture per place, each a byte-copy of that place's `main.omg` with
`expected_native_exit.txt` of 0. The negative side is
`tests/omega/fail/facts/matched_field_windows_through_<place>_rejected`, each
with the pinned `expected.txt` fragment above.

`tests/omega/corpus_outcomes.txt` records all nine positives as `checked` and
all nine twins as `rejected` with their fragment matched.
`corpus_native_macos_arm64.txt` records `exit:0` for all nine positives, and
`corpus_both_macos_arm64.txt` records `interpreter-exit:0 interpreter:agrees`.
All three were measured on a fresh producer built from the rebased candidate.

## What this evidence establishes

The owning requirement is the fact-transport rule in
[AGENTS.md](../../../../AGENTS.md#hard-rules): one proved fact must work through
a local, a field, a parameter, a call result, a match join, a const name,
another state, another machine, and another target body. This row establishes
the *match-pattern-as-value* form of that rule for a published window length,
in the checking stage and in both execution engines.

This row needed no compiler repair. On a compiler built from
`5de48c6e72e841825dd4eea37ea084529d202764`, all nine places already checked at
exit 0, ran `--both` with native and Terminal agreement, and all nine twins
already rejected with the diagnostics above. The contribution here is the twins
and the pins, not a behavior change; no invalid control was accepted and no
valid program was refused.

What remains open: this is coverage for an accepted behavior, not a compiler
capability. No parent task's acceptance is discharged by it.