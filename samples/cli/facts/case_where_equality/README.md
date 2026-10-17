# Case-where equality

Case `Point(lo, hi) where lo == hi` makes `hi - lo` exact in `u64`, without
a guard or Wrapping. Every place copies this same fact and operation.

This is source-check acceptance only. The original `main.omg` and its build
entry bindings are preserved. The nine place projects bind no `ProgramEntry`
and have no runtime oracle; no native execution is claimed.

```sh
omega --check samples/cli/facts/case_where_equality/main.omg
omega --check samples/cli/facts/case_where_equality/<place>/main.omg
omega --check samples/cli/facts/case_where_equality/<place>/invalid/main.omg
python3 tools/sample_gate.py --cli <omega> --filter cli/facts/case_where_equality --jobs 2
```

| Place | Equality-bearing subject | Positive | Reject twin |
| --- | --- | --- | --- |
| `local` | copied local interval | checks | missing equality: exact `u64` overflow |
| `field` | borrowed context field | checks | missing equality: exact `u64` overflow |
| `parameter` | explicit interval parameter | checks | missing equality: exact `u64` overflow |
| `call_result` | identity machine result | checks | missing equality: exact `u64` overflow |
| `match_join` | two distinct interval fields joined by a match | checks | missing equality: exact `u64` overflow |
| `const_name` | named `Point { lo: 4, hi: 4 }` constant | checks | `hi: 3` violates the same case predicate |
| `other_state` | interval forwarded to another state | checks | missing equality: exact `u64` overflow |
| `other_machine` | interval forwarded to another machine | checks | missing equality: exact `u64` overflow |
| `target_body` | four hosted target bodies | checks | four exact `u64` overflow diagnostics |

Eight twins remove only `where lo == hi`; the const twin changes only `hi: 4`
to `hi: 3` and pins construction enforcement. Its rejection is distinct from
subtraction rejection. Expected diagnostic fragments are authored beside each
twin. Pass/fail mirrors live at
`tests/omega/{pass,fail}/facts/case_equality_through_<place>_compile`.

`Interval [copy]` explicitly permits the ordinary copies used by the place
programs. Unannotated owned data is affine even when all its fields are scalar;
the initial borrowed-storage transfer refusals were required ownership checks,
not arithmetic defects. The original root source remains unchanged.

On base `2ef2ab353498f0830f6023d8a023b63d21664cb9` (containing
`73ecbccba3`), eight place programs checked after that declaration. The const
place first rejected a complete `Point`/`Empty` dispatch as non-exhaustive, then
rejected exact subtraction after coverage was repaired. The compiler repair
compares complete aggregate dispatch subjects, binds substituted payload fields
to their exact case, and extends closed scalar projection through the complete
common/active constructor roster. It retains case predicates, ownership,
closed-sibling and Exact-policy obligations.

The focused corpus also checks constant-array dispatch, distinct aggregate axes,
common fields with same-named sibling payloads, and unsafe sibling arithmetic.
An additional maximum-`u64` construction probe still fails on the exact base
before this repair: its case predicate is not established. That existing wide
construction gap is outside this authored constant-4 witness and is not claimed
fixed by the nine-place source acceptance.
