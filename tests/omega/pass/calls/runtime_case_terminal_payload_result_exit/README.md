# Saved case payloads in terminal calls

Run `omega run --both main.omg`; both engines must exit 0.

Both cases bind a payload named `value`. Their selected terminal call passes
that saved payload to a record-returning machine. The program checks that the
returned records retain their distinct values, 70 and 71. A second dispatch
passes each saved payload through a nested identity call and checks 72 and 73;
that nested call retains its own ordinary operand-observation lifetime.
