# Inferred array capacity

Fact: an omitted `Capacity` is inferred as 2 from the actual `[u8; 2]` type and `Length == [u8; Capacity]`, independently of element values.

The declared two-element array determines the omitted argument to `capacity`.
The program then observes the returned capacity as 2. Array element values do
not choose a type or an index.

The owning tasks are **STRUCTURAL-GENERIC-MATCHING** and
**BUILD-OWNED-TARGET-SELECTION** in [TASKS.md](../../../../TASKS.md).
The canonical nine places below carry this fact. Computed array sizes in named
constants and trait arguments also have focused regression programs under
`tests/omega/{pass,fail}/generics`; they do not add another sample row.

## Places

| Place | Positive program | Reject twin | What the program carries |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`invalid_local/main.omg`](invalid_local/main.omg) | declared `[u8; 2]` array copied to local variable before `capacity` call |
| field | [`field/main.omg`](field/main.omg) | [`invalid_field/main.omg`](invalid_field/main.omg) | `[u8; 2]` array stored in and projected from `Holder::items` nominal field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`invalid_parameter/main.omg`](invalid_parameter/main.omg) | declared ordinary `[u8; 2]` parameter passed to `capacity` |
| call_result | [`call_result/main.omg`](call_result/main.omg) | [`invalid_call_result/main.omg`](invalid_call_result/main.omg) | declared `[u8; 2]` array return value from helper function |
| match_join | [`match_join/main.omg`](match_join/main.omg) | [`invalid_match_join/main.omg`](invalid_match_join/main.omg) | selected `[u8; 2]` passed to `capacity`, then its first byte is checked as 7 or 19 |
| const_name | [`const_name/main.omg`](const_name/main.omg) | [`invalid_const_name/main.omg`](invalid_const_name/main.omg) | declared array constant `BASE: [u8; 2]` passed to `capacity` |
| other_state | [`other_state/main.omg`](other_state/main.omg) | [`invalid_other_state/main.omg`](invalid_other_state/main.omg) | state arrival parameter `step(items: [u8; 2])` forwards to `capacity` |
| other_machine | [`other_machine/main.omg`](other_machine/main.omg) | [`invalid_other_machine/main.omg`](invalid_other_machine/main.omg) | forwarding machine routes `[u8; 2]` array to `capacity` |
| target_body | [`target_body/main.omg`](target_body/main.omg) | [`invalid_target_body/main.omg`](invalid_target_body/main.omg) | Build-selected named implementations perform the same inferred call |

The root [`main.omg`](main.omg) is an additional direct-call example.
Each positive has an authored exit-zero oracle and four hosted `ProgramEntry`
rows. Each twin supplies an explicit capacity of 3, which conflicts with the
declared two-element array. The twin must reject for that equation; a package,
parse or unrelated typing failure does not count.

## Target selection

The target place calls the public `SelectedConsume` requirement. Its ordinary
named implementations are chosen by target-keyed Build rows. The entry and
implementation each call a tag checker: 17 for x86-64 and 29 for ARM64. A wrong
entry, checker or consuming implementation changes the observed tag check and
traps. Both the positive and its twin have the same entries, providers and
contracts; only the twin's explicit capacity differs.

The tag checker's requirement has empty service reach. Its synchronous callers
also keep empty service reach and declare `invokes SelectedBodyCheck`, as required
by [direct synchronous invocation](../../../../wiki/spec/language/effects.md#direct-synchronous-invocation).
The checker contributes its published effects; its trait name is not another
service effect.

## Requirements and evidence

- The declared array type and the `requires` equation establish Capacity 2.
  A runtime equality guard does not select that argument.
- Copies, fields, parameters, call results, joins, constants and machine
  transitions retain that exact array type. No intervening write changes it.
- The consumer uses the inferred argument directly. Its twin changes the
  argument to 3 without adding a caller guard or changing the array.
- Normal ownership, access, callback and conformance checks remain required.
  A type equation grants no reference or execution authority.
- Successful execution must include actual native and Terminal Psi exits,
  through ordinary Build selection, array layout, calls and image emission.
  Authored target rows alone do not establish matching-host execution.

Positive and negative nine-place fixtures are pinned under
`tests/omega/{pass,fail}/facts/inferred_array_equation_call_through_*`.
The [structural inference contract](../../../../wiki/spec/language/generics.md#structural-type-equations-and-inference)
requires exact declaration-backed structure before ordinary compatibility.

## Current validation

Main deliberately retired the duplicate forwarded-result, mixed-argument and
typed-constant sample trees in PRs #1841 and #1844. This change preserves those
deletions. Their frozen programs and prior observations remain historical
source evidence, including the [original snapshot](https://github.com/CathedralOS/Omega/tree/f846bcc27a2c428c000906b377582ade6cd13249/samples/cli/facts/inferred_array_equation_call).
They are not current tracked acceptance.

The repair also addresses a real constant-evaluation failure. A valid named
constant with `IndexedPacket<[u8; 2], extent()>` was rejected because the compiler
treated its unfinished size call as a completed non-integer value. Later checks
also missed computed arguments in `satisfies` clauses. The compiler now retains
those computations, resolves them in their enclosing declaration, and checks
the evaluated arguments against the same parameter types and conditions as
other applications.

The retained regression programs cover the [original named constant](../../../../tests/omega/pass/generics/inferred_equation_typed_const_const_name_exit/main.omg)
and its [conflicting-size twin](../../../../tests/omega/fail/generics/inferred_equation_typed_const_const_name_conflict/main.omg).
One [focused positive](../../../../tests/omega/pass/generics/inferred_equation_computed_satisfaction_exit/main.omg)
also combines an evaluated packet type, a computed satisfaction argument and a
value-qualified constant. Its [wrong-size twin](../../../../tests/omega/fail/generics/inferred_equation_computed_satisfaction_extent_conflict/main.omg)
keeps the computed packet size at 2 and changes only the satisfaction argument
to 3. The [invalid-value control](../../../../tests/omega/fail/generics/inferred_equation_mixed_destination_invalid_value/main.omg)
changes only the ready constant from 7 to 8, so preserving an unfinished type
cannot suppress a condition on a completed value. These three projects use six
new source/Build files altogether.

Optimized compilers built at `35dd60f55878` pass the current selection on all
four matching hosts: 30 Source positives, 50 intended refusals, and 30 actual
native/Terminal exit-zero agreements per host. All 19 canonical projects pass:
the root and nine places execute at zero, and the nine twins reject as intended.
The match executes both branches and observes 7 or 19 after consuming capacity.

- [Linux x86-64](https://github.com/CathedralOS/Omega/actions/runs/41926662415)
- [Linux ARM64](https://github.com/CathedralOS/Omega/actions/runs/41931716718)
- [macOS ARM64](https://github.com/CathedralOS/Omega/actions/runs/41933212101)
- [Windows x86-64](https://github.com/CathedralOS/Omega/actions/runs/41933220369)

The latter three runs use program inputs `29a5c1cd3b26` with those unchanged
compilers. That input-only revision adds a missing Linux ARM64 entry to an
existing captured-Const preservation fixture; its Main and oracles are unchanged.
The earlier ARM run retains its real missing-entry failure. The focused ordinary
package checks also execute both the combined satisfaction case and captured-Const
case at zero on every host, with the corrected ARM entry checked in its full replay.
The source archives, original producer identities, profiles and input hashes are
preserved in [PR #1839](https://github.com/CathedralOS/Omega/pull/1839).

Records retain their measured source stamps. Linux's retired combined-results
file stays deleted; three genuine native-only observations update the existing
native baseline. The full combined results remain in the run evidence.
The value-index program was already green in the frozen baseline; its old
refusal record was stale. The named-constant program's non-integer refusal was
an actual compiler defect. Its conflict twin now reaches the intended size
mismatch after evaluation, rather than that earlier kind failure.

The parent tasks remain open. Open generic schemas, retained extensions, late
receivers, other binder combinations, named lifetimes and selected computed
calls keep their own acceptance.
