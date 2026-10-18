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

The matching regression pairs live under `tests/omega/{pass,fail}/facts`.
A separate nonhost-body control weakens only the Windows body. Borrow controls
also retain live-parent and detached-child exclusions.

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
