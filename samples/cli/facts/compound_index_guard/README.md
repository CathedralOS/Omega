# Compound index guard

The live fact `0 <= b * 20 + a < 200` permits storing `7` at
`grid[b * 20 + a]`.

This is a compile-scope probe, with no standard-library dependency or native
exit oracle. The root and nine place projects bind no `ProgramEntry`; check them rather
than running them. Source acceptance is distinct from native execution.

Each place repeats the same fact sentence in its source. The index remains a
two-variable `i32 in Wrapping` computation, the grid has 200 elements, and the
operation remains an indexed store. Inputs remain unknown so acceptance must
use the guard rather than constant folding. The historical root probe retains
its original initialized fields and state/store shape.

| Place | Evidence carried to the store |
| --- | --- |
| `local` | Copied coordinates and their compound guard cross the transition. |
| `field` | The guard bounds the same field computation at the store. |
| `parameter` | Explicit coordinate parameters carry the compound requirement. |
| `call_result` | A checked coordinate result equals the guarded coordinate. |
| `match_join` | Both arms preserve the guarded coordinate. |
| `const_name` | Named stride 20 and ceiling 200 preserve the same computation. |
| `other_state` | The guarded field computation survives arrival in `put`. |
| `other_machine` | A separate store machine requires the same compound bound. |
| `target_body` | Every hosted target body checks the same guard and store. |

Each `invalid/` twin removes only the strict upper-bound proposition, including
its corresponding parameter requirement where present. The lower bound stays;
these controls must reject for the store's missing index upper bound.

```sh
omega --check samples/cli/facts/compound_index_guard/<place>/main.omg
omega --check samples/cli/facts/compound_index_guard/<place>/invalid/main.omg
```

The baseline call-result place rejects with `cannot prove index
main.b * 20 + a is within length 200 in Main::main::put` (diagnostic quoting
omitted here). The checked coordinate equality must survive in the compound
wrapping observation.

Source acceptance covers all nine places and all nine reject twins. The
historical root also checks. The corpus mirrors the row under
`tests/omega/{pass,fail}/facts/compound_guard_through_*_compile` and adds
mutation, unequal-match, changed-result, lower-bound, grouping, and shorter-slice
controls. No native execution is claimed.

The checker retains each opaque wrapping observation, relates constructors
through already checked coordinate equalities, and reads the exact extent of
an always-full fixed array. Runtime slice lengths remain independent of backing
capacity.

```sh
python3 tools/sample_native_gate.py --cli <omega> --filter cli/facts/compound_index_guard
python3 tools/corpus_gate.py --filter facts/compound_guard
```

The sample gate reports ten check-only positives (root plus nine places), nine
intended-invalid rejections, and no native score.
