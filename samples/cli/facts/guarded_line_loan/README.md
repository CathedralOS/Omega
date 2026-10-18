# Guarded line loan

A byte owner written through a mutable loan and read through a shared loan
after a selected case edge observes the written byte. Every place copies that
sentence and runs the same operation: `receive` writes 37 into byte 0 through
an exclusive loan and returns a `ReadOutcome` case carrying 5, the selected
case edge runs, and the shared read after the edge observes `37 + 5`. The
program exits 0 only when the observation is 42.

The original `main.omg` remains the starting witness. It names the fact and its
result 42, but its build declares a package rather than an application, so it
stays a check-only probe with no ProgramEntry binding and no executable oracle.
The nine places are applications that run natively:

| Place | Project | Positive | Reject twin | Transport |
| --- | --- | --- | --- | --- |
| local | `local/` | `local/main.omg` | `local/controls/held_loan/main.omg` | the owner is a local byte array |
| field | `field/` | `field/main.omg` | `field/controls/held_loan/main.omg` | the owner is a record field |
| parameter | `parameter/` | `parameter/main.omg` | `parameter/controls/held_loan/main.omg` | the shared loan is passed as a parameter |
| call result | `call_result/` | `call_result/main.omg` | `call_result/controls/held_loan/main.omg` | the owner is returned by a call |
| match join | `match_join/` | `match_join/main.omg` | `match_join/controls/held_loan/main.omg` | either match arm yields the same shared loan |
| const name | `const_name/` | `const_name/main.omg` | `const_name/controls/held_loan/main.omg` | a named constant selects the written and observed byte |
| another state | `other_state/` | `other_state/main.omg` | `other_state/controls/held_loan/main.omg` | a successor state receives the shared loan |
| another machine | `other_machine/` | `other_machine/main.omg` | `other_machine/controls/held_loan/main.omg` | a forwarding machine performs the write |
| another target body | `target_body/` | `target_body/main.omg` | `target_body/controls/held_loan/main.omg` | each target body writes through the loan |

Every place binds `ProgramEntry` for all four hosted targets and declares
`Expected exit: 0`. Each place carries a `controls/held_loan/` reject twin that
adds one construct: a competing exclusive loan of that place's owner, held live
across the loan site. That breaks only the exclusive-liveness this fact rests
on, so the owner can no longer be written through one loan and read after the
edge through another. The field twin's diagnostic names `holder.line`, and the
another-machine twin's names the forwarding machine `forward`.

## Acceptance

Measured with a compiler built from this tree on macos_arm64. All nine places
run, and Terminal Psi and native execution agree:

| Place | `omega run --both` | observed |
| --- | --- | --- |
| local | 0 | native exit 0, interp exit 0 |
| field | 0 | native exit 0, interp exit 0 |
| parameter | 0 | native exit 0, interp exit 0 |
| call result | 0 | native exit 0, interp exit 0 |
| match join | 0 | native exit 0, interp exit 0 |
| const name | 0 | native exit 0, interp exit 0 |
| another state | 0 | native exit 0, interp exit 0 |
| another machine | 0 | native exit 0, interp exit 0 |
| another target body | 0 | native exit 0, interp exit 0 |

The oracle is discriminating, so the read genuinely observes the written byte
rather than passing on an unrelated predicate. Changing the write from 37 to 36
makes the shared read observe 41 and the program traps in both engines (`native
signal 5`, `interp crash: Trap`). Changing the authored expectation from 42 to
41 traps the same way, and restoring either returns exit 0. The observed 42 is
therefore computed from byte 0 as read through the shared loan under the guard.

The nine twins each reject with exit 1 carrying two diagnostics that name the
still-active borrow, for example `state `receive` receives `line` while local
borrow `held` is still active` plus `statement 2 mutates `line[0]` while local
borrow `held` is still active`. Both anchors are the guard/lending fact, not a
package, provider, or native-production failure.

```sh
python3 tools/sample_native_gate.py --filter cli/facts/guarded_line_loan
python3 tools/corpus_gate.py --filter facts/guarded_line_loan
python3 tools/corpus_gate.py --both --filter facts/guarded_line_loan
```

`sample_native_gate` exits 0 with 9/9 samples meeting their documented oracle
and 9/9 intended-invalid controls matched. The corpus check leg reports 0
structural moves over 18 fixtures (`checked` 9, `rejected` 9). The combined leg
reports all nine rows `built` with `exit:0`, `interpreter-exit:0` and
`interpreter:agrees`. Foreign hosted bodies are source checked; this trial does
not claim foreign native execution, so the four-target binding is a checking
obligation while only the host target runs here.

## Evidence

The matching fact fixtures are
`tests/omega/pass/facts/guarded_line_loan_through_<place>_exit` and
`tests/omega/fail/facts/guarded_line_loan_through_<place>_held`. Each positive
fixture carries `expected_native_exit.txt` and `expected_interpreter_exit.txt`,
both `0`; each twin pins its rejection fragments in `expected.txt`. The focused
corpus selector for the row is `guarded_line_loan`.

The fact needed no compiler repair: all nine places checked, ran natively and
agreed with Terminal Psi on their first attempt.

Canonical authority: [borrows and aliases](../../../../wiki/spec/language/ownership.md#borrows-and-aliases)
gives `&mut T` exclusive observation and mutation and `&T` shared observation
under its loan contract, and requires borrowed arguments to continue denoting
the original storage rather than a caller-side snapshot; its starting blob is
`7e6f9a8ad935edc4e66324dd6ced07fe0c5be7ad`.
[Loan resources and compatibility](../../../../wiki/spec/terminal-psi/loans.md)
requires a live exclusive loan to carry its exact captured place, access and
restoration obligation and rejects an incompatible overlapping use; its starting
blob is `3e2e33d7f04f0dc0e4e144a9ccf94ab045af61f3`. This row establishes only
the transport and carrier lowering of this one guarded-line-loan fact across the
nine places; the board's other acceptance remains open.