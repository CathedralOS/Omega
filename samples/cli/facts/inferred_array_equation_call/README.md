# Inferred array capacity

Fact: an omitted `Capacity` is inferred as 2 from the actual `[u8; 2]` type and `Length == [u8; Capacity]`, independently of element values.

The declared two-element array determines the omitted argument to `capacity`.
The program then observes the returned capacity as 2. Array element values do
not choose a type or an index.

The owning tasks are **STRUCTURAL-GENERIC-MATCHING** and
**BUILD-OWNED-TARGET-SELECTION** in [TASKS.md](../../../../TASKS.md).
This folder contains the direct call and three existing ways to produce its
input: a [forwarded result](forwarded_result/README.md),
[mixed explicit arguments](mixed_binders/README.md), and a
[checked constant call](typed_const_argument/README.md).
Their facts and consuming operations stay separate and unchanged.

## Places

| Place | Positive Program | Fact-sensitive Reject Twin | Carrier / Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | [`invalid/main.omg`](invalid/main.omg) | canonical root witness passing `[u8; 2]` actual directly to `capacity` |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | declared `[u8; 2]` array copied to local variable before `capacity` call |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | `[u8; 2]` array stored in and projected from `Holder::items` nominal field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | declared ordinary `[u8; 2]` parameter passed to `capacity` |
| call_result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | declared `[u8; 2]` array return value from helper function |
| match_join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms return `[u8; 2]`; joined array passed to `capacity` |
| const_name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | declared array constant `BASE: [u8; 2]` passed to `capacity` |
| other_state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | state arrival parameter `step(items: [u8; 2])` forwards to `capacity` |
| other_machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | forwarding machine routes `[u8; 2]` array to `capacity` |
| target_body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Build-selected named implementations perform the same inferred call |

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

## Current migration qualification

The original sources are fixed at `1f7bad99b8c86066daf01e4915538cb6d65a1d20`.
The inventory includes all 94 projects across the four productions, with
86 existing matching corpus fixtures. Four root positive/twin pairs have no
separate corpus pin. Prior runtime observations remain historical; none is
relabeled as execution of the current migration.

The current baseline and migration qualification are pending. Run records retain
the actual optimized producer, program revision, host and engine outcomes.
The migration aligns the remaining target-prefixed twins, replaces the typed
constant production's four identical bodies with one ordinary implementation,
and gives its reject pins matching Build companions. It does not change any
fact, capacity, selected constant body or consuming operation.

The parent tasks remain open. Open generic schemas, retained extensions, late
receivers, other binder combinations, named lifetimes and selected computed
calls keep their own acceptance.
