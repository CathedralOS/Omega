# as_slice_views

The ordinary `&main.entries[..]` range lends a shared view of noncopy entries
through the imported core `Slice::range` requirement. The sample reads its
extent and records whether it still covers all four entries.

Check this probe with `omega --check samples/cli/facts/as_slice_views/main.omg`.
The corresponding executable regression checks the extent with a trap oracle:

```text
omega run --both tests/omega/pass/slices/runtime_imported_range_views_exit/main.omg
```

The range uses selected checked-provider supply and retains the bounds contract
and backing loan. The runtime fixture expects exit 0 in both engines.

## Nine places

A whole-array shared view of four noncopy Entries has length 4; reading its extent satisfies require_four(view.len).

The checked operation is the call to `require_four`, whose requirement is
`length == 4`. Every positive reads the borrowed descriptor's extent rather
than supplying a literal. Each `controls/short_extent/` twin changes the
backing array extent and matching transport contracts to 3, retaining the
operation and its requirement of 4. The twin must fail at that call.
The original root probe above remains unchanged.

| Place | Project | Fact transport |
| --- | --- | --- |
| local | `local/` | whole-array shared view in a local |
| field | `field/` | whole-array shared view in a record field |
| parameter | `parameter/` | view parameter with exact extent requirement |
| call result | `call_result/` | borrowed result with exact extent guarantee |
| match join | `match_join/` | both arms return the whole-array view; both execute |
| const name | `const_name/` | named constant fixes the backing array extent |
| another state | `other_state/` | view arrives under an exact extent requirement |
| another machine | `other_machine/` | helper receives the view under its extent requirement |
| another target body | `target_body/` | all four hosted bodies read the extent |

Each executable positive expects exit 0 and traps if its measured extent differs
from 4. Cross-target checking is distinct from matching-host execution.
The mirrored corpus pairs are
`tests/omega/pass/facts/as_slice_views_through_<place>_exit/` and
`tests/omega/fail/facts/as_slice_views_through_<place>_short_extent/`.

Canonical anchors: [borrowed range access and endpoints](../../../../wiki/spec/language/expressions.md#borrowed-range-access),
[exact dependency transport](../../../../wiki/spec/language/dependent_values.md#establishment-and-witness-loans),
and [call requirements](../../../../wiki/spec/proofs/contracts.md#machines-and-bundles).
The four-entry extent supplies a mathematical fact; descriptor construction
cannot waive its backing loan or supply missing authority.

```sh
python3 tools/sample_native_gate.py --filter cli/facts/as_slice_views --cli <omega> --jobs 2
python3 tools/corpus_gate.py --filter as_slice_views_through_
python3 tools/corpus_gate.py --native --filter as_slice_views_through_
```
