# Read through a match-selected reference

A match selects one of two references. The selected reference must retain its
known contents when it is saved, passed to a helper, and read.

Every positive program selects the reference to 17 and passes it to
`read_through`, which requires 17. Every reject twin selects the reference to 5
instead. The values, selection contract and consuming operation stay the same
across all nine places. A wrong selection or read takes the trapping branch;
exit 0 checks the runtime result as well as the static requirement.

The target example replaces target-prefixed functions with ordinary named
providers selected in `build.omg`. All four providers pass both references to
the same `apply` helper. The public requirement and each provider require
`chosen == 17`, so independent inputs must establish the read condition.
This proves Build selection and reference transport;
it does not claim different operating-system behavior.

This row contributes to `BUILD-OWNED-TARGET-SELECTION` and the reference-result
part of `STATE-LOCAL-VALUE-FRONTIER` in [TASKS.md](../../../../TASKS.md).
Neither parent task is closed by this row.

## Programs and reject twins

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [program](local/main.omg) | [invalid selection](local/invalid/main.omg) |
| field | [program](field/main.omg) | [invalid selection](field/invalid/main.omg) |
| parameter | [program](parameter/main.omg) | [invalid selection](parameter/invalid/main.omg) |
| call result | [program](call_result/main.omg) | [invalid selection](call_result/invalid/main.omg) |
| match join | [program](match_join/main.omg) | [invalid selection](match_join/invalid/main.omg) |
| const name | [program](const_name/main.omg) | [invalid selection](const_name/invalid/main.omg) |
| nested machine | [program](other_state/main.omg) | [invalid selection](other_state/invalid/main.omg) |
| another machine | [program](other_machine/main.omg) | [invalid selection](other_machine/invalid/main.omg) |
| Build-selected target implementation | [program](target_body/main.omg) | [invalid selection](target_body/invalid/main.omg) |

The [root program](main.omg) checks the same operation. Positive programs bind
`ProgramEntry` for Windows x86-64, Linux x86-64, macOS ARM64 and Linux ARM64 and
expect exit 0. Each place has matching programs under
`tests/omega/{pass,fail}/facts/match_reference_referent_through_*`; the fail
fixture's `expected.txt` and sample's `invalid/sample.json` pin its rejection.

## What establishes the result

`choose()` guarantees `true`, so the selected arm is known before
`read_through` is checked. The reference's contents come from its original
binding or the producing call's contract. The final equality test is only the
runtime oracle; it does not establish the call's precondition.

The selected reference identifies one referent and remains live until the read.
The programs do not write that referent before reading it. Reject twins select
the other referent and must fail the same requirement, rather than a parser,
import or ownership setup error. See
[fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md) and
[loans](../../../../wiki/spec/terminal-psi/loans.md).

Private helpers can use caller facts only when every incoming call establishes
them after argument substitution. Public entries, callbacks and selected
provider members must work for their independently admitted inputs. Existing
controls reject a [bad second caller](../../../../tests/omega/fail/contracts/match_reference_private_second_arrival/main.omg),
an [unsupported public input](../../../../tests/omega/fail/contracts/match_reference_public_input/main.omg),
and an [unsupported provider input](../../../../tests/omega/fail/contracts/match_reference_selected_provider_input/main.omg).
The governing rule is [invocation contracts](../../../../wiki/spec/language/state_contracts.md).

Acceptance carries the checked match, both candidate borrows and the consuming
call through Terminal Psi and native execution. Calls must preserve the
reference and read result across the selected target's calling convention,
registers and stack storage. All required places must run with both engines
and exit 0; emitted artifacts alone do not establish execution.

## Recorded results and remaining checks

The optimized baseline `64376af6ae978b4ddb2d2d50874724226ccb0646`
rejected the root and all nine positive places at the requirement that the
referent equal 17. These failures were observed on macOS ARM64 on 2026-11-05.

The following repaired producers ran the root and all nine positive places at
exit 0 in both engines. Each run also passed 15 corpus positives and ordinary
execution of the target-provider project. All 29 negative controls rejected;
28 matched their expected diagnostic.

| Host | Actual producer | Run |
| --- | --- | --- |
| Linux x86-64 | `195774d028943e0adfc06e7bee331a9a187da740` | [42138842550](https://github.com/CathedralOS/Omega/actions/runs/42138842550) |
| Linux ARM64 | `47307200e73621ba104defb0300cc6c2447dc6ce` | [42145265423](https://github.com/CathedralOS/Omega/actions/runs/42145265423) |
| macOS ARM64 | `47307200e73621ba104defb0300cc6c2447dc6ce` | [42145219092](https://github.com/CathedralOS/Omega/actions/runs/42145219092) |
| Windows x86-64 | `47307200e73621ba104defb0300cc6c2447dc6ce` | [42145268258](https://github.com/CathedralOS/Omega/actions/runs/42145268258) |

These results retain their actual producer identities. Both revisions use the
reviewed compiler source `bb51de149e26`; their workflow cache settings differ.
They do not establish behavior of the newer integrated candidate.

The nested twin still expects caller/state names `main`/`pick`; the diagnostic
reports `pick`/`entry` for the same missing 17 requirement. The frozen expectation
is unchanged, so all four earlier runs remain failed and the row remains open.
The exact label correction still needs approval under the existing diagnostic-pin
rule.

A [bounded current-main replay](https://github.com/CathedralOS/Omega/actions/runs/42258417474)
uses optimized compiler `43c8f36b1569` with run configuration `6ab3b2410ace`.
It checks the frozen local positive and rejects its twin for the unchanged pin.
The ordinary target-provider project also runs at exit 0 in both engines.
Current main already supplies the compiler support those checks need, so the
remaining compiler addition was removed. This PR now changes the Build-selected
provider example, its fixtures, controls, records and documentation. The other
places and hosts retain the earlier producer identities above; the bounded
replay does not establish new full-row qualification.

Earlier ordinary-execution uploads retained eight product files and reported
nine product digests. The hidden `.omega-owner` file was omitted from those
uploads; its bytes are not claimed verified. The current-main replay retained
all nine product files, including that marker.

Older experiments at `9ff26a870215` also reported gaps when a helper returns a
reference-valued match, or a match arm names a saved reference-valued match.
Those experiments are outside this row's acceptance and have not been verified
against current main. The parent task retains the broader borrowed-result and
successor-lifetime work.
