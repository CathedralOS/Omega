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
| local | [program](local/main.omg) | [invalid](invalid_local/main.omg) |
| field | [program](field/main.omg) | [invalid](invalid_field/main.omg) |
| parameter | [program](parameter/main.omg) | [invalid](invalid_parameter/main.omg) |
| call result | [program](call_result/main.omg) | [invalid](invalid_call_result/main.omg) |
| match join | [program](match_join/main.omg) | [invalid](invalid_match_join/main.omg) |
| named constant | [program](const_name/main.omg) | [invalid](invalid_const_name/main.omg) |
| nested machine | [program](other_state/main.omg) | [invalid](invalid_other_state/main.omg) |
| another machine | [program](other_machine/main.omg) | [invalid](invalid_other_machine/main.omg) |
| selected implementation | [program](target_body/main.omg) | [invalid](invalid_target_body/main.omg) |

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
[the original baseline](https://github.com/CathedralOS/Omega/actions/runs/41765895038)
checks all nine original positives, rejects all nine twins for their intended
contract, and runs all nine positive places in both engines at zero on Linux
x86-64. The source migration then exposed two checking gaps. Float preparation
looked for an entry named `entry`, missing the `observe` entry of the inline
implementation. The public call also could not read its captured field facts
without an executable callee. The repair uses the actual first state and the
public parameter's exact field and captured argument.

The completed observations use source `37bbc58593ae` and an optimized compiler
built from that exact source on each matching host:

| Host | Corpus positive pairs | Ordinary positive pairs and intended refusals | Root |
| --- | --- | --- | --- |
| [Linux x86-64](https://github.com/CathedralOS/Omega/actions/runs/41770117539) | 9 at 0/0 | [9 at 0/0 and 9 refusals](https://github.com/CathedralOS/Omega/actions/runs/41772775904) | 0/0 |
| [Mac ARM64](https://github.com/CathedralOS/Omega/actions/runs/41772529472) | 9 at 0/0 | 9 at 0/0 and 9 refusals | 0/0 |
| [Linux ARM64](https://github.com/CathedralOS/Omega/actions/runs/41772531846) | 9 at 0/0 | 9 at 0/0 and 9 refusals | 0/0 |
| [Windows x86-64](https://github.com/CathedralOS/Omega/actions/runs/41772534590) | 9 at 0/0 | 9 at 0/0 and 9 refusals | 0/0 |

Each pair is an actual native and Terminal exit. All nine positive pins check,
and all nine original reject pins match their intended contract on each host.
The Linux corpus job remains failed because an additional existing operator
control stops at an earlier diagnostic. The separate ordinary replay completed
the projects and root that this failed job did not reach.

A separate [wrong-entry control](target_body/invalid_entry/main.omg) supplies
`item.kept = 1.0` to the unchanged public requirement `item.kept == 7.0`. It is
pinned at `tests/omega/fail/facts/float_result_equality_selected_entry_field_rejected`.
[Source checking](https://github.com/CathedralOS/Omega/actions/runs/41773603050)
and [the ordinary project](https://github.com/CathedralOS/Omega/actions/runs/41774499444)
reject at that requirement using source `fa1d3b675c49`. The latter replay also
runs the root after adding this child project, with both exits zero. Its compiler
and runner digests equal the measured `37bbc58593ae` binaries; their actual
build and source records remain separate.

Two unchanged supplemental fixtures remain unverified for their intended checks:
`fail/float/guarded_range_requires_ieee_comparison` stops at a dispatch-storage
type error, and `fail/float/authored_heterogeneous_float_operator_requires` stops
at implicit domain weakening. Both diagnostics also occur with the pre-repair
producer at `9060de0ae551`. Their original programs, oracles and failed reports
are retained; neither is counted as evidence that its intended obligation holds.
The broader task and those checks remain open.

The measured source revisions are retained in the published replay archive on
PR #1786. Raw reports, optimized producer identities, consumed lock files,
source hashes, failed attempts and independent review receipts are retained in
`build/float-result-equality-selected-20261104`. Original notes and all 129
pre-migration files remain in that folder's frozen snapshot. Current records
name the source actually observed; they do not claim execution at later
metadata commits.
