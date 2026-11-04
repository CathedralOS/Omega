# Returned float equality

A returned f64 equals the live field it came from.

`read_kept` takes a shared view of `Ends` and promises that its returned float
is both `item.kept` and `7.0`. The caller passes that value and the same live
field to `agree`, which requires both equalities. The consuming call occurs
before the runtime comparison; the later comparison supplies no proof premise.

This migration belongs to `BUILD-OWNED-TARGET-SELECTION` in
[TASKS.md](../../../../TASKS.md). It replaces four identical target-prefixed
`observe` bodies with one ordinary `SelectedFloatObservation` implementation.
Four explicit Build rows select it through `FloatObservation`. The borrowed
input, result type, entry requirement and original implementation body remain.
The broader saved-observation work under `STATE-LOCAL-VALUE-FRONTIER` stays open.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [program](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | [program](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | [program](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call result | [program](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match join | [program](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| named constant | [program](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| nested machine | [program](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| another machine | [program](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| selected implementation | [program](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

Every twin changes only the producing read from `item.kept` to `item.other`.
The declared result contract remains unchanged and must refuse. Caller guards,
the marker helper and target selection cannot establish that missing guarantee.
The named-constant place retains `KEPT = 7.0` throughout its original contracts.
All eighteen Main and Build pairs match their pins under
`tests/omega/{pass,fail}/facts/float_result_equality_through_*`.

The match transport executes both Boolean choices. It carries the returned float
with route 17 or 23 in an ordinary record, then checks each route independently.
The producer retains its original signature and complete f64 guarantee; the
transport helper publishes that guarantee on its saved field. The caller proves
both `agree` calls before checking their runtime results. The neighboring
`item.other` field must still be `1.0`. Only this transport helper owns a match,
so choosing the wrong arm cannot hide behind identical float values.

The requirement checks are:

- Establishment: construction proves the input field is `7.0`. The producing
  machine proves its own normal-return guarantees from its read.
- Identity and lifetime: the shared input names live caller storage. The returned
  float is a copy, and this row performs no intervening write to that storage.
- Composition: calls substitute the exact input and result. Locals, fields,
  parameters, joins, constants, nested machines and ordinary implementations
  carry the same guarantees into `agree`.
- Soundness: returning the other field must still reject. A post-hoc comparison
  cannot justify the consuming precondition.
- Terminal Psi: scalar reads, returned values and contracts retain their exact
  source relationships through lowering and interpreter execution.
- Provider and calling plans: the ordinary selected method accepts `&Ends` and
  returns `f64`; four Build rows preserve target choice without source prefixes.
- Native realization: actual native and Terminal execution must agree at exit
  zero. The match markers and field check detect wrong branch or field lowering.

Because the input and output are both constrained to `7.0`, this row establishes
those published guarantees at that bound. It does not establish arbitrary float
equality, equality after overlapping writes, or every remaining saved-observation
requirement. Those obligations stay on the owning task board.

Governing contracts are [invocation contracts](../../../../wiki/spec/language/state_contracts.md),
[fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md),
[numeric values](../../../../wiki/spec/language/numeric_values.md) and
[Build-owned target selection](../../../../wiki/spec/build/provider_selection.md#build-owned-target-selection).

Every positive and [the root](main.omg) bind all four hosted targets and declare
exit zero. Run a place with `omega run --both <place>/main.omg`.

At fixed published base `db92349b9c2e`,
[baseline run 41765895038](https://github.com/CathedralOS/Omega/actions/runs/41765895038)
checks all nine original positives, rejects all nine twins for their intended
contract, and observes nine actual native/Terminal exit-zero pairs on Linux
x86-64. Its producer is built from that exact base at optimization level 1.
The original row already works; this is a source migration and completion of
branch observation, not a demonstrated compiler repair.

The migrated source remains unqualified. Focused execution, ordinary project
checks, independent review and verified publication remain open. The 129 original
source and oracle files, raw reports and producer profiles are retained in
`build/float-result-equality-selected-20261104`. Historical notes about other
constant spellings and artifact ingress are preserved in that original snapshot;
they do not establish current or broader behavior.
