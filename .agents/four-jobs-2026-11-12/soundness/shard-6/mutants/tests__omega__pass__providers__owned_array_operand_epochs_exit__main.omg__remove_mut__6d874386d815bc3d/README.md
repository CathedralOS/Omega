# Owned array operand epochs

Prepare the package with `omega update --project .` and, if requested,
`omega update --project . --resume`, then run `omega run --both main.omg`.
Both engines must exit 0. The standalone Unit entrypoint needs no standard
library dependency; a wrong observation crashes `Trap` rather than succeeding.

The first selected boundary call receives a scalar write, an owned array,
and another scalar write. The first write changes byte 0 from 30 to 31;
the array must capture 31 before the last write changes the caller to 99.
This rejects both hoisting the copy before the earlier scalar operand and
delaying the copy until all scalar operands finish.

The second call receives two owned array operands from the same home with a
scalar write between them. Its provider must observe 31 in the first array
and 99 in the second. This rejects sharing a live source address or reusing
one snapshot across distinct argument occurrences. Both providers also check
all three unaffected bytes. The caller checks its final array and a cumulative
counter of three scalar write effects, so operand evaluation must execute once.

The boundary parameters are owned fixed arrays, not references. No loan to
the original array remains during a later write. Borrowed arguments retain
their different rule, pinned by the existing invalid
`boundary_nested_argument_conflicting_loan` and
`boundary_argument_later_write_under_element_loan` fixtures.
