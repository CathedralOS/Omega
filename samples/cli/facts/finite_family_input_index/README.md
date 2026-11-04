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
The `dyn Family` argument is ordinary data for this outer boundary call, then
selects the inner family member. Neither selection captures implicit data.

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

The complete checked call, family table, selected Build row and result must
survive Terminal production, interpretation, calling plans and native execution.
The new source is pending independent review and fresh qualification. The
baseline below does not establish execution of the migrated programs, Product
producer parity, all-host support or closure of either parent task.

At fixed published source `60cb5e1f79dd05465f77775990e46bcc2e789470`, the
[optimized Linux baseline](https://github.com/CathedralOS/Omega/actions/runs/41798897195)
observed 12 checked corpus positives, 13 intended corpus refusals and 12 actual
native/Terminal exit-zero agreements. Ordinary projects had 12 zero agreements,
ten matching refusals and three failures caused by missing authored rejection
text. Those three diagnostics were the intended family-membership refusals.
Their existing corpus fragments are now copied into the samples and must be
scored in fresh validation. The original job remains failed.

This is migration of already working target-prefixed programs, not a claim of
an existing compiler defect. Both baseline executables use optimization level 1;
actual input and compiler identities remain distinct when a verified compiler
is reused. Windows execution and the wider parent requirements remain open.

<details>
<summary>Earlier source and measurements</summary>

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
