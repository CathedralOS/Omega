# as_slice_views

The ordinary `&main.entries[..]` range lends a shared view of noncopy entries
through the imported core `Slice::range` requirement. The sample reads its
extent and records whether it still covers all four entries.

The root `main.omg` runs with `// Expected exit: 0` and `crashes Trap` on failure:

```text
omega run --both samples/cli/facts/as_slice_views/main.omg
```

The range uses selected checked-provider supply and retains the bounds contract
and backing loan. The runtime root and fixtures expect exit 0 in both engines.

## Nine places

A whole-array shared view of four noncopy Entries has length 4; reading its extent satisfies require_four(view.len).

The checked operation is the call to `require_four`, whose requirement is
`length == 4`. Every positive reads the borrowed descriptor's extent rather
than supplying a literal. Each `controls/short_extent/` twin changes the
backing array extent and matching transport contracts to 3, retaining the
operation and its requirement of 4. The twin must fail at that call.
The root application and nine place projects all verify that extent.


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

## Requirement accounting

Owning task: **TERMINAL-SLICE-VIEW-VOCABULARY** (TASKS.md, P3). This row
discharges one requirement of its acceptance: a consuming non-byte `&[T]` view
call executes natively with Terminal interpreter agreement across the nine
places. The task's wider acceptance (restricted scalar-array constructors,
cyclic view forwarding, and its named sample obligations) remains open there.

- **Establishment:** the four-entry extent is established by construction of
  the fixed array and carried by the view's stored length; the reject twins
  break only that extent and fail at the `require_four` requirement.
- **Identity and lifetime:** the view borrows `entries` under its shared loan;
  descriptor construction cannot waive the backing loan. The anonymous
  provider call product inside the direct `&entries[..]` argument keeps its
  own evaluation custody: retirement planning answers its transfers from the
  authored call's retained operand facts when no established local carries its
  schedule.
- **Composition:** each place substitutes the view through its carrier (local,
  field, parameter, call result, match arms, const name, state arrival,
  machine parameter, target body) into the same `require_four` call.
- **Soundness boundaries:** the twins' weakened extent is a proof failure
  (`cannot prove requires contract`), distinct from a lowering refusal;
  authority and ownership obligations are unchanged by the transfer repair.
- **Terminal Psi and carrier lowering:** the view lowers as a borrowed
  descriptor with its stored extent; the anonymous `Slice::range` call
  evaluates through the ordinary structural value-call schedule.
- **Native realization:** every place binds `ProgramEntry` for the four hosted
  targets and agrees between engines on the matching host
  (`omega run --both`, exit 0). Foreign-target execution is not claimed.

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
