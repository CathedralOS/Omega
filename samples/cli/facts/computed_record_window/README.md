# Computed record window

In the `which == 0` arm, `[0..which + 2]` has length 2; `hold` preserves that
extent in its returned `Held.values`, through which `write_pair` writes the backing.
Every place retains this fact, bound, operation, and `f64` backing. The original
root program remains unchanged.

The nine place programs are `local`, `field`, `parameter`, `call_result`,
`match_join`, `const_name`, `other_state`, `other_machine`, and `target_body`.
Each has a corresponding `controls/invalid_<place>` project. The twin changes
only the matched endpoint from 2 to 1 (including the endpoint machine's own
contract or the named constant). It must fail the unchanged length-2 promise;
the state twin instead fails admission to the state requiring that extent.
The controls match both the proof-refusal identity and its length proposition.

Each child exits 0 only after checking the four original backing elements with
exact float equality. It exercises both the zero arm and wildcard arm; the join
also exercises both Boolean alternatives. Writes must affect the selected two
elements and preserve the other two. The state place forms the view before the
transition and carries its extent into an explicit state contract. All four
host target bodies are checked; only the matching host is executed.

```sh
python3 tools/sample_native_gate.py --filter cli/facts/computed_record_window
python3 tools/corpus_gate.py --both --filter computed_record_window
```

## Measured oracle

The root, all nine places, and `case_payload` each carry
`expected_native_exit.txt` containing `0` beside an empty `input.txt`. Both
files are required: `tools/sample_native_gate.py` honors authored `expected_*`
files only for a project that carries `input.txt`, and otherwise scores the
header alone. Measured on `macos_arm64` with the gate CLI: 11/11 samples meet
their documented oracle with 0 unscored, and 10/10 intended-invalid controls
match their authored rejection.

The oracle discriminates on both halves of the fact. Against a byte-identical
copy of `local/main.omg`:

| Probe | Result |
| --- | --- |
| unmodified source | `native exit 0`, `interp exit: 0`, exit 0 |
| `write_pair` writes 5.0/7.0 instead of 0.0/1.0, every contract intact | native signal 5, interp crash Trap, exit 255 |
| zero arm becomes `samples[1..index + 3]` with the length-2 promise intact | native signal 5, interp crash Trap, exit 255 |

The second row shows the write through the returned record field is observable
rather than assumed; the third shows a wrong window endpoint traps even when
the promised length is unchanged.

The matching regression pairs live under `tests/omega/{pass,fail}/facts`.
`corpus_native_macos_arm64.txt` records all eleven positive fixtures at
`exit:0`, and the nine `_rejected` rows pin the refuse diagnostics. A separate
nonhost-body control weakens only the Windows body. Borrow controls
also retain live-parent and detached-child exclusions.

## Requirement coverage

Owning task: the **Computed calls and state transfers** item under
`TASKS.md` **SAMPLE-CORPUS**. The requirement discharged is that a window end
computed at runtime and carried out through a returned record field is proved
from the matched arm rather than from a literal `[0..2]` spelling, and that the
write through that returned field is observable at runtime.

- **Establishment.** The `ensures result.len == 2` contract on each window
  machine is what supplies the extent; `hold` consumes it as
  `requires samples.len == 2` and republishes `result.values.len == 2`. No
  caller-side guard substitutes for it — `exercise` branches only on the
  observed backing values.
- **Identity and lifetime.** `Held.values` is the returned record field and the
  sole write path to the backing; the four `f64` elements are read after the
  write in the same state.
- **Composition.** Each place moves the same computed length through a
  different carrier (local, field, parameter, call result, match join, const
  name, successor state, forwarding machine, target body). The positive and its
  twin differ only in the matched endpoint, never in a caller guard.
- **Soundness boundaries.** `hold` transfers the view into the record and
  `write_pair` consumes it; the borrow that supplies the window stays live
  across the call, which is the property the match-join witness pinned.
- **Terminal Psi and carrier lowering.** `omega run --both` requires native and
  Terminal Psi to agree on every place; both report exit 0.
- **Native realization, frame/layout encoding, and image emission.** All four
  hosted targets are checked; the row's eleven fixtures are recorded at
  `exit:0` in the native golden.

`tests/omega/run/slices/computed_record_window` binds all four hosted targets
and now carries its own exit 0 oracle, recorded as `built ... exit:0` in the
native golden alongside the eleven `_exit` fixtures.

The match-join witness exposed `hold(view)` rejecting its own supplying view:
`creates local borrow leading while local borrow view is still active`.
Returning an unprojected referent descriptor must retain its actual supplier,
even when a computed endpoint prevents a spatial containment proof. The repair
preserves source-derived descriptor identity; projected referent children keep
their independent lineage obligations.

Authority comes from [borrowed range access](../../../../wiki/spec/language/expressions.md#borrowed-range-access),
[match facts](../../../../wiki/spec/language/patterns.md), and
[loan transfer and parent lineage](../../../../wiki/spec/language/ownership.md).

Native realization also exposed two carrier-accounting gaps. Completion now
captures returned mutable-slice sources alongside reference records before
retiring local descriptors. View disposition uses the complete exact activation
roster: every locally established alternative must have one valid establishment
and the same release status. Multiple alternative claims realize one descriptor
retirement; missing mappings, mixed coverage, and duplicate events still reject.
The mixed-lifecycle regression exercises direct and call-produced views across
both drop and state-transfer exits.

These repairs implement the existing [return transaction](../../../../wiki/spec/terminal-psi/structural_access.md),
[loan lifecycle](../../../../wiki/spec/terminal-psi/loans.md), and
[call result custody](../../../../wiki/spec/terminal-psi/calls_and_outcomes.md).

The root program's transition `tail(&mut trace)` also exposed a home-namespace
gap: state loan chains resolved a lent local's completed home only among
discardable record results, so an array or byte-view local's binding answered
"no exact completed home". All structural result bindings share the one
establishment ordinal namespace; the lookup now names any of them.

`case_payload/main.omg` keeps the same computed length-2 window and original
backing write through `Outcome::Some { held: hold(window(samples, 0)) }`.
Its selected case requires `held.values.len == 2` before construction; matching
then carries that established condition into the state calling `write_pair`.
The invalid twin changes only the window extent to 1. The constructor must
consume completed operand snapshots and public call guarantees, without using
its own not-yet-established case condition as a premise.
