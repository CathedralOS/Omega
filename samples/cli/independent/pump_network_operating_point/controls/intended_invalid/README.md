This independent complete source is expected to fail frontend checking.
`branch_total` requires a shared borrow of a two-element fixed array, but its
caller supplies a one-element array literal. The array-formation rule is
specified in `wiki/spec/language/counts_and_addresses.md` and exercised by the existing
`tests/omega/fail/arrays/borrowed_array_call_argument_wrong_extent` control.
`expected.txt` names this extent violation; unrelated rejection is a failure.
There is no application import and no package dependency or native execution.
