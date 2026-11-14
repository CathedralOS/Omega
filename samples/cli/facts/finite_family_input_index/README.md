# Keep the ordinary input index through a selected function

These programs select a function at runtime, then pass its result through calls,
storage and state transitions. The result must retain the captured value of the
ordinary `limit` argument. Its payload remains 7.

Owning tasks: **BUILD-OWNED-TARGET-SELECTION** and **FINITE-GENERIC-DISPATCH**
in [TASKS.md](../../../../TASKS.md).

A finite-family result retains the captured ordinary input index through selection, forwarding and storage, and its payload remains 7.

The public `Family::row` contract returns `Indexed<limit>` for widths 16 and 32.
The helper renames those parameters to `Span`, `payload` and `bound`; matching
must use their declarations and positions. It cannot substitute another ordinary
argument merely because its carrier is also `u32`.

Each program saves `limit == 16`, evaluates an input helper, then supplies the
current limit to the selected member. A positive helper leaves that limit alone.
Its twin changes the limit to 32 before the second ordinary argument is read.
The result can no longer have the saved index 16. That twin must refuse the
index mismatch; a caller guard does not establish a missing index relationship.
The wrong-provider control instead declares its result against `payload`, rather
than `bound`, and must also refuse.

## Places and existing controls

| Place | Positive | Index-sensitive twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [reject](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [reject](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [reject](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [reject](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [reject](match_join/invalid/main.omg) |
| Const name | [program](const_name/main.omg) | [reject](const_name/invalid/main.omg) |
| Nested machine | [program](other_state/main.omg) | [reject](other_state/invalid/main.omg) |
| Other machine | [program](other_machine/main.omg) | [reject](other_machine/invalid/main.omg) |
| Selected target body | [program](target_body/main.omg) | [reject](target_body/invalid/main.omg) |

The original three parameter compositions remain:

- [Local width](parameter/controls/local_width/main.omg) copies the input before
  selecting a row. Its [twin](parameter/controls/local_width/invalid/main.omg)
  removes only the establishing membership contract.
- [Replaced width](parameter/controls/replaced_width/main.omg) lets an earlier
  operand write 32 into the row-selection variable. Both initial widths must
  observe row 32, the unchanged neighboring limit 16 and payload 7. Its
  [twin](parameter/controls/replaced_width/invalid/main.omg) changes the write
  and its own postcondition to 64, outside the public family.
- [Reordered roster](parameter/controls/reordered_roster/main.omg) declares
  widths 32, 16 and 16 while the caller retains its 16-or-32 contract. Its
  [twin](parameter/controls/reordered_roster/invalid/main.omg) admits 64 only at
  the caller; every admitted alternative must satisfy the public requirement.

The [wrong-provider control](wrong_provider/invalid/main.omg), all nine twins
and those compositions stay in scope. All 25 discovered projects have matching
corpus Main/Build companions. No project is removed because its outcome is red.

## Observe the actual calls

Both implementations still return the same payload, so that value alone cannot
expose a wrong implementation. One ordinary mutable array records what actually
ran. It is explicit data, separate from the `dyn Family` selector:

| Element | Written by | Checked value |
| --- | --- | --- |
| 0 | The selected member | 31 for `Selected`, 47 for `Other` |
| 1 | The renamed helper | Its actual family width |
| 2 | The renamed helper | Its actual ordinary limit, 16 |
| 3 | The match helper | 3 or 7 for the chosen arm; 0 elsewhere |
| 4 | The input helper | One call, counted with wrapping arithmetic |

Each positive exercises both widths with both named implementations. The match
caller supplies its expected arm marker independently; inverting the match or
always taking one arm must trap. The nested-machine place carries the current
observation array as an ordinary state argument after the producing call.

These checks observe execution after the existing result transport. They add no
input precondition, change no `Indexed` carrier and prove no static index fact.
The public result type supplies that fact before any runtime check runs. Every
positive retains the four hosted entry bindings and `Expected exit: 0`.

## Build selects the relay

The four identical target-prefixed `relay` bodies become one ordinary
`PortableInputIndexRelay` implementation. Four explicit Build rows select it for
Windows x86-64, Linux x86-64, macOS ARM64 and Linux ARM64. The portable caller
uses the public `InputIndexRelay::relay` requirement.

That requirement has empty service reach. The caller names its synchronous
entry with `invokes InputIndexRelay`; selecting code adds no service identity.
Build chooses the relay implementation. The explicit `dyn Family` selector
parameter passes through that relay and selects the inner family member.
Both calls keep their data arguments explicit.

## Review the selected result type

The selected relay returns `Indexed<16>`. Its `Limit: u32` declaration is a
value parameter, so a literal 16 is a valid argument. Package review must retain
that exact type just as the checker and both execution engines do. It must not
mistake the declared value parameter for a type-only parameter.

The retained Linux compiler at `60cb5e1f79dd` checks and executes the selected
target program, but ordinary `omega packages update` rejects its package review with
`package review rejects a const expression outside one exact declared const-parameter slot`.
The source and oracle were frozen before changing the compiler. The shared
review validator now recognizes the exact declaration's value or const
parameter, as the existing conformance and calling-signature review already do.
It retains declaration ownership, argument position, expression identity and
binder checks. A literal in a type-only parameter is still ineligible.

Independent source review accepted this correction. Linux and macOS now pass
the full focused scope below. The existing static inline-result target and twin
also confirm that ordinary const-parameter review is preserved.

## Requirements and validation

The governing contracts are [runtime index identity](../../../../wiki/spec/language/generics.md#runtime-index-identity-and-storage),
[finite specialization](../../../../wiki/spec/language/generics.md#finite-specialization-boundary),
[finite dynamic families](../../../../wiki/spec/terminal-psi/dynamic_dispatch.md#finite-generic-method-families),
[dependent values](../../../../wiki/spec/language/dependent_values.md),
[provider selection](../../../../wiki/spec/build/provider_selection.md#build-owned-target-selection)
and [service reach and invocation](../../../../wiki/spec/language/effects.md).

Establishment comes from the public family and exact ordinary argument.
Identity belongs to that argument's captured value, rather than a later value
of the same variable. Calls, fields, joins and state transitions must preserve
it. The reject twins challenge only their own index or membership obligation.
Ordinary borrow and cleanup rules still apply to the observation argument;
recording a value grants no extra access or lifetime.

The current shared repair lets calls that cannot replace stored references
check without reconstructing storage details from earlier proof-only
statements. Unknown reference effects still reject. This removes the shared
`Nat.mod` refusal exposed by a later integration, with every program unchanged.

The [complete Linux replay](https://github.com/CathedralOS/Omega/actions/runs/41867447392)
at compiler source `972297978286bb346cb46a4a7979286f22c6920f` observes
13 checking accepts, 14 intended refusals, 13 native/Terminal zero agreements
and all 27 ordinary outcomes: 13 zero agreements and 14 intended refusals.
It uses the verified optimization-level-1 CLI and corpus runner from
[the focused producer run](https://github.com/CathedralOS/Omega/actions/runs/41860200412),
without another compiler build or changed limits. The
[source archive](https://github.com/CathedralOS/Omega/pull/1826)
retains that actual compiler. All 182 program and oracle files remain fixed.
The later published callback and checker changes have a separate applicability
review for these value-only family parameters. These results remain executions
at `972297978286`; this documentation update is not a new-source execution.

The changed-limit twins still refuse the index mismatch, and the replaced-width
case still observes row 32 after the earlier write. Proving that a call cannot
replace references does not discard its ordinary scalar writes. Published
records remain unchanged because their status, facts and complete diagnostics
match these observations; current CLI evidence is retained separately.

The complete checked call, family table, selected Build row and result survive
Terminal production, interpretation, calling plans and native execution on the
four hosted targets at the measured sources below. The focused scope includes
this row's 12 positive projects and 13 reject controls, plus the existing static
inline-result target and twin.

The [Windows x86-64 replay](https://github.com/CathedralOS/Omega/actions/runs/41830011701)
and [Linux ARM64 replay](https://github.com/CathedralOS/Omega/actions/runs/41827408170)
completed the remaining matching-host measurements at compiler source
`2dcf275fa50f3b59a0700b391bd84c2f187b48b5`. Each checks 13 positives and
14 intended refusals, observes 13 native/Terminal exit-zero agreements, and
passes all 27 ordinary projects: 13 zero agreements and 14 intended refusals.
This includes every place and existing composition control, plus the unchanged
const-parameter control for the shared package-review check.

Each replay authenticates the exact optimized binaries from its small
[Windows seed](https://github.com/CathedralOS/Omega/actions/runs/41823975939) or
[ARM seed](https://github.com/CathedralOS/Omega/actions/runs/41823980405).
The selected-target program completes ordinary package review and executes at
zero on both hosts. The full replays perform no second compiler build and
change no gate limits. The [compiler source archive](https://github.com/CathedralOS/Omega/pull/1808)
retains the measured source; compiler profiles use optimization level 1.

The earlier [Linux x86-64 qualification](https://github.com/CathedralOS/Omega/actions/runs/41813105626)
and [macOS ARM64 replay](https://github.com/CathedralOS/Omega/actions/runs/41817261002)
observe the same outcomes at compiler source
`6877d16bd42c53f2c3dd9a9cf4f733a6ffdbd550`. The later source adds an already
published floating-point proof repair. Independent review verified that it
affects only floating-point catalog selection; this row's integer, Boolean,
index and selector consumers are unchanged. Those earlier results retain their
actual source and profiles, with that applicability argument. They are not
new-source executions. Their [measured source archive](https://github.com/CathedralOS/Omega/pull/1805)
and macOS [binary seed](https://github.com/CathedralOS/Omega/actions/runs/41813109293)
remain available.

PR #1814 added only the measured Windows and ARM corpus records. Source, Linux
and macOS records stayed unchanged. The shared repair also preserves all
published records; their actual observations and ordinary CLI results remain
attached to the measured sources above.

The selected-relay migration, ordinary package-review repair and this fact's
matching-host acceptance are now observed across all four hosted targets.
Product producer parity and the broader acceptance of both parent tasks remain
open.

<details>
<summary>Earlier source and measurements</summary>

The [first integrated Linux replay](https://github.com/CathedralOS/Omega/actions/runs/41834854874)
passed the full 27/13/27 scope at `89984b4d4607`. The later
[integration run](https://github.com/CathedralOS/Omega/actions/runs/41841704932)
at `977be69fd10c` failed before any positive executed: `Nat.mod` reported
`stored call has no complete statement-successor effects`. That unrelated
refusal also masked 13 intended invalid causes. The original failed run is
preserved; it is not relabeled as passing after the shared repair.

The target-prefixed programs were working migration inputs, not a demonstrated
compiler defect. At fixed published source
`60cb5e1f79dd05465f77775990e46bcc2e789470`, the
[optimized Linux baseline](https://github.com/CathedralOS/Omega/actions/runs/41798897195)
observed 12 checked positives, 13 intended corpus refusals and 12 actual
native/Terminal zero agreements. Ordinary projects had 12 zero agreements,
ten matching refusals and three failures caused by missing authored rejection
text. Those diagnostics were the intended family-membership refusals. Their
existing corpus fragments were copied into the samples; the original job
remains failed.

With the strengthened inputs at `09e488e7eb34` and that same compiled producer,
[the input-only run](https://github.com/CathedralOS/Omega/actions/runs/41811098484)
checked all 25 cases and observed all 12 corpus zero agreements. Ordinary
projects had 11 zero agreements, 13 intended refusals and one valid
selected-target package-review failure. The frozen target was replayed to
capture its exact diagnostic before the one-file compiler repair. The source,
producer and failure remain historical rather than being relabeled as success.

The [previous README](https://github.com/CathedralOS/Omega/blob/60cb5e1f79dd05465f77775990e46bcc2e789470/samples/cli/facts/finite_family_input_index/README.md)
retains the original index-binding repair and membership investigation. Fixed
base `6e5131c510b9e6227b4592652c9a5a39e350a970` and optimized producer
`96a6da9b73e3b863bcf2238aeca3db1cbb61268d5ce0c4a8ab44d39abad2eab0`
refused the parameter membership call before its repair.

At `7b6988120e7ed1d2a87b396676600fd78f378042`, the earlier Linux scope had
17 checked and 17 intended rejected corpus cases; 16 native/Terminal pairs
agreed at zero and the existing nested-result neighbor agreed at 70. A
corrupted replaced-width expectation trapped in both engines. Those results
remain historical and do not qualify the new observation arguments.

The [original evidence archive](https://github.com/CathedralOS/Omega/blob/e84e6e4660226bd086c7b9accf17f5bba8fc4f7c/wiki/drafts/measurements/dynamic_family_membership_2026_11_01/raw-evidence.tar.gz)
retains commands, logs, producer identities and the source bundle. Its SHA-256
is `5c0c4ba9c5e4b05f9aebe1092293c629b9d5b7e4f9fa3e316a65a7959f5fd089`.
The recorded optimized CLI SHA-256 is
`15a75295c6e8eafd66c290ba0834660d85f07b8a2117f8087b831bacfed676d1`.
These observations do not establish Product parity or other-host execution.

</details>
