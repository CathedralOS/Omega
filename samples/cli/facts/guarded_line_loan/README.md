# Guarded line loan

A byte owner written through a mutable loan and read through a shared loan
after a selected case edge observes the written byte. Every place copies that
sentence and runs the same operation: `receive` writes 37 into byte 0 through
an exclusive loan and returns a `ReadOutcome` case carrying 5, the selected
case edge runs, and the shared read after the edge observes `37 + 5`. The
program exits 0 only when the observation is 42.

The original `main.omg` remains the starting witness. It names the fact and its
result 42, but its build declares a package rather than an application, so it
was check-only and had no executable oracle. The nine places are applications
that run natively:

| Place | Project | Transport |
| --- | --- | --- |
| local | `local/` | the owner is a local byte array |
| field | `field/` | the owner is a record field |
| parameter | `parameter/` | the shared loan is passed as a parameter |
| call result | `call_result/` | the owner is returned by a call |
| match join | `match_join/` | either match arm yields the same shared loan |
| const name | `const_name/` | a named constant selects the written and observed byte |
| another state | `other_state/` | a successor state receives the shared loan |
| another machine | `other_machine/` | a forwarding machine performs the write |
| another target body | `target_body/` | each target body writes through the loan |

Run `omega run samples/cli/facts/guarded_line_loan/<place>/main.omg`; every
place exits 0.

Each place has a `controls/held_loan/` reject twin. The twin adds one
construct: a competing exclusive loan of that place's owner, held live across
the loan site. That breaks only the exclusive-liveness this fact rests on, so
the owner can no longer be written through one loan and read after the edge
through another. Check a twin with `omega --check <control>/main.omg`; it must
reject for the still-active local borrow, not for a package, provider, or
native-production failure. The field twin's diagnostic names `holder.line`,
and the another-machine twin's names the forwarding machine `forward`.

The fact needed no compiler repair. All nine places checked and ran natively
with exit 0 on their first attempt, and all nine twins rejected with two
loan-conflict diagnostics each naming the still-active borrow.

The matching fact fixtures are under `tests/omega/{pass,fail}/facts/`, named
`guarded_line_loan_through_<place>_exit` and
`guarded_line_loan_through_<place>_held`. The focused corpus selector for the
row is `guarded_line_loan`.