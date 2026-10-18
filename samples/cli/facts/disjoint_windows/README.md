# disjoint windows

Compile-scope probe. No standard-library dependency.

- `items[0..2]` and `items[2..4]` — literal disjoint bounds let two
  `&mut` subslices of one base borrow at once; writes through both.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/facts/disjoint_windows/main.omg
mbx run -p omega -- update --project samples/cli/facts/disjoint_windows
```

## Nine places

For an exclusive four-element base, `split == 2` makes `[0,2)` and
`[split,4)` disjoint, permitting simultaneous exclusive loans and writes
`a[0] = 11` and `b[1] = 41`. The checked bounds are
`0 <= 0 <= 2 <= 4`, `0 <= split <= 4`, `0 < 2`, and `1 < 2`.
The boundary `2` belongs to the right half only. Both loans remain live
through both writes and subsequent readback.

| Place | Positive project | Boundary transport |
| --- | --- | --- |
| local | `local/` | local scalar initialized to 2 |
| field | `field/` | record field initialized to 2 |
| parameter | `parameter/` | parameter requiring equality to 2 |
| call result | `call_result/` | result guaranteeing equality to 2 |
| match join | `match_join/` | both arms establish 2; caller exercises both |
| const name | `const_name/` | named constant 2 |
| another state | `other_state/` | successor parameter requiring 2 |
| another machine | `other_machine/` | forwarding call to machine requiring 2 |
| another target body | `target_body/` | four target-scoped bodies requiring 2 |

Every project has `controls/overlap/`. That twin changes only the transported
boundary to 1 (and consistent argument/contract occurrences). Its ranges
`[0,2)` and `[1,4)` remain in bounds, and both stores remain in bounds;
the second exclusive loan must reject because the first remains active.
Each positive has an exit-0 oracle that checks the borrowed writes and all
caller elements after return: `[11,20,30,41]`. The original root probe stays
unchanged and retains its check-only role.

Canonical contracts:
[borrowed range access and endpoints](../../../../wiki/spec/language/expressions.md#borrowed-range-access),
[collection bounds](../../../../wiki/spec/language/numeric_values.md#collection-bounds),
[borrow authority](../../../../wiki/spec/language/ownership.md#borrows-and-aliases), and
[captured loan places](../../../../wiki/spec/terminal-psi/loans.md#captured-places-and-available-premises).
Nonoverlap proves compatibility; it does not supply missing authority or extend
a lifetime. Each body reborrows an existing mutable caller slice.

Run the row with a fresh compiler:

```sh
python3 tools/sample_native_gate.py --filter cli/facts/disjoint_windows --cli <omega> --jobs 2
python3 tools/corpus_gate.py --filter disjoint_windows_through_
```

The mirrored corpus pairs are
`tests/omega/pass/facts/disjoint_windows_through_<place>_exit/` and
`tests/omega/fail/facts/disjoint_windows_through_<place>_overlap/`.
Source checking includes every authored target body. Native results establish
only the matching host target.
