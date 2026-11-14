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
| local | [program](local/main.omg) | [invalid selection](invalid_local/main.omg) |
| field | [program](field/main.omg) | [invalid selection](invalid_field/main.omg) |
| parameter | [program](parameter/main.omg) | [invalid selection](invalid_parameter/main.omg) |
| call result | [program](call_result/main.omg) | [invalid selection](invalid_call_result/main.omg) |
| match join | [program](match_join/main.omg) | [invalid selection](invalid_match_join/main.omg) |
| const name | [program](const_name/main.omg) | [invalid selection](invalid_const_name/main.omg) |
| nested machine | [program](other_state/main.omg) | [invalid selection](invalid_other_state/main.omg) |
| another machine | [program](other_machine/main.omg) | [invalid selection](invalid_other_machine/main.omg) |
| Build-selected target implementation | [program](target_body/main.omg) | [invalid selection](invalid_target_body/main.omg) |

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

## Scope and recorded results

This change migrates the Build-selected target example and its matching fixtures.
It contains no Rust or library change. The nine reference-match programs and
root keep their values, contracts, consuming read and runtime checks.

On all four hosts, the nine corpus positives and root plus nine sample positives
run at exit 0 in both engines. All nine corpus twins and three independent-input
controls reject; eleven match their expected diagnostic. Eight sample twins
match. The nested corpus and sample expectations miss the same caller/state
labels, so the full-row runs remain failed.

| Host | Actual compiler source | Run |
| --- | --- | --- |
| Linux x86-64 | `43c8f36b1569` | [42267274369](https://github.com/CathedralOS/Omega/actions/runs/42267274369) |
| Linux ARM64 | `804d94179e6b` | [42271885697](https://github.com/CathedralOS/Omega/actions/runs/42271885697) |
| macOS ARM64 | `804d94179e6b` | [42271885697](https://github.com/CathedralOS/Omega/actions/runs/42271885697) |
| Windows x86-64 | `804d94179e6b` | [42271885697](https://github.com/CathedralOS/Omega/actions/runs/42271885697) |

Linux reuses the verified optimized compiler with run configuration
`b5d560ee2fc6`. The other hosts build the source shown above with run configuration
`146272abe22f`; their compiler and library inputs equal fixed main `43c8f36b1569`.
The final documentation and record changes preserve those tested program inputs.

Ordinary execution of the provider project also exits 0 in both engines on all
four hosts. Linux retains the [earlier exact-input replay](https://github.com/CathedralOS/Omega/actions/runs/42258417474)
with that same compiler and run configuration `6ab3b2410ace`; the other hosts run
it in the matrix above. Each capture verifies all 180 copied inputs and nine
product files, including `.omega-owner`.

## Unchanged failure in the broader row

The nested twin expects caller/state names `main`/`pick`; the diagnostic reports
`pick`/`entry` for the same missing 17 requirement. Its source, Build file and expectations are unchanged from fixed main.
The compiler and library sources are also unchanged.
The changed provider and control packages do not enter that nested program.
The failure therefore predates this migration and is outside its changed scope,
as allowed by [validation scope](../../../../tools/testing.md#validation-scope).

The expectation and strict validation rule are unchanged. Every failed report
remains failed. Correcting that expectation is separate work on the broader row;
this migration does not claim the row or either parent task complete.

Earlier experiments and four-host observations retain their actual producer
identities and failed conclusions in the
[historical PR records](https://github.com/CathedralOS/Omega/pull/1933#issuecomment-6691006128).
They are not relabeled as executions of the current source. Their original raw
records and product-retention limitations remain preserved.
