# Lexical visibility of nested machines

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../TASKS.md).
Specification: [nested machines](../../../../../wiki/spec/language/machines.md#nested-machines-and-identity)
("A helper is unavailable outside the machine whose scope declares it; qualification
cannot bypass this restriction. A sibling can name another visible sibling, but cannot
reach into that sibling's private descendants.").

A bare call names the machine visible from the call site: the private `hidden` inside
`outer`, the free `hidden` anywhere else, and no callee once the free one is absent.

`outer` declares a private `hidden` that returns its argument. A free `hidden` returns
107. `main` observes `outer(7) == 7` and `hidden(7) == 107`, so a compiler that resolves
either spelling to the other declaration reaches the `bad` arm and traps. The twin of
every place deletes only the free declaration. `main`'s spelling then matches nothing
visible and must be refused as an unresolved callee; the private helper may not stand in.

Run any place with `omega run --both <place>/main.omg`. Twins are checked with
`omega --check <place>/invalid/main.omg`.

| Place | Positive | Reject twin (no free `hidden`) |
| --- | --- | --- |
| Local | [source](local/main.omg) | [unresolved call](local/invalid/main.omg) |
| Field | [source](field/main.omg) | [unresolved call](field/invalid/main.omg) |
| Parameter | [source](parameter/main.omg) | [unresolved call](parameter/invalid/main.omg) |
| Call result | [source](call_result/main.omg) | [unresolved call](call_result/invalid/main.omg) |
| Match join | [source](match_join/main.omg) | [unresolved call](match_join/invalid/main.omg) |
| Const name | [source](const_name/main.omg) | [unresolved call](const_name/invalid/main.omg) |
| Nested machine | [source](other_state/main.omg) | [unresolved call](other_state/invalid/main.omg) |
| Other machine | [source](other_machine/main.omg) | [unresolved call](other_machine/invalid/main.omg) |
| Target body | [source](target_body/main.omg) | [unresolved call](target_body/invalid/main.omg) |

Each place changes only how the two results travel to the final comparison: a local, a
record field between non-zero canaries `19` and `23`, a call argument, the result of a
contract-carrying `identity` call, both arms of a `match` join, a named constant, a call
made three machines deep inside `main`, a call made by another top-level machine
(`relay`), and a call made beside named implementations `X86Probe` and `ArmProbe` of a
`boundary trait`, each with its own private `hidden`, selected by `Build::select_provider`
for the four hosted targets. Copies and oracles are pinned under
`tests/omega/{pass,fail}/facts/nested_machine_lexical_visibility_<place>_{exit,rejected}`.

## Baseline and repair

Optimized baseline `771fa8951` (`origin/main` when this row started; `dev profile
[optimized]`). All nine positives and the control below already ran there. The defect was
in the refusals:

- All nine twins were refused for an unrelated reason: ``call `entry` has an invalid
  argument correspondence: call has no retained parameter telescope``. Resolution had
  correctly found no callee, but checking re-selected the private helper by spelling
  through its by-name machine index (`TopLevelSymbols`, built from every machine
  including nested ones). The refusal came from the helper lacking a call telescope, not
  from visibility, so it was no evidence that visibility is enforced.
- [`controls/bare_name_value`](controls/bare_name_value/main.omg) names the private helper
  as a value (`let named: u64 = hidden;`). `omega --check` **accepted** it (exit 0) at
  baseline, because the same index made the spelling look like a known declaration.
  `omega run` then stopped in lowering with ``main has no executable body: this compiler
  does not lower a statement of it yet``. A spelling that matches nothing is refused by
  the checker as ``uses `nosuch`, which is not a declared local, parameter, field, or type``.

The leak was not confined to user code. The index also held the bundled library's private
helpers, so an unresolved user call was checked against whatever nested machine shared its
spelling. In `tests/omega/fail/modules/duplicate_imported_machine_overload_rejected`, whose
`seed();` is ambiguous between two imports, baseline `771fa8951` added the diagnostic
``call to `seed` discards its non-unit `AccessPlan` result`` because
`source/library/core/layout.omg` declares a private `seed` inside `AccessPlan::seed_bounded`.
The repaired producer no longer reports it, and the fixture's diagnostics again equal the
golden's recorded pair.

The repair is in `omega-rust/psi/compilation/checking/src/declarations/symbols/top_level.rs`.
Each roster entry records whether its machine is declared inside another machine, and the
two bare-name lookups (`machine`, `machine_symbol`) skip such entries. Resolution has
already stamped every in-scope use with its symbol, so no legitimate use reads the
spelling. Duplicate and overload analysis and symbol-based membership still see every
machine. With the repair the nine twins report ``value call `hidden(..)` does not resolve
to a callable machine entry or requirement`` in the machine that made the call, and
`bare_name_value` is refused at checking with the unknown-name diagnostic.

## Integration

Rebased onto `64fd22d07` after `origin/main` moved 39 commits, which conflicted only in
`TASKS.md` and the corpus golden. On that tree, optimized: all ten positives again report
native exit 0 and interpreter exit 0. Of the 542 fixtures upstream added or changed in that
window, 44 moved against the golden; the baseline runner (`64fd22d07` alone) and the
repaired runner agree on every one. Ten of them are already red on `64fd22d07`: the nine
twins in `tests/omega/fail/facts/nested_machine_invocation/` report a diagnostic from the
specialized helper (``machine `keep$specialized$…` state `entry` stores a boolean into a
`u64` return value``) instead of their pinned ``terminal expression expects `T`, got
`bool` ``, and `fail/providers/native_generic_binding_wrong_requirement` reports a missing
provider. This row does not change them.

## Controls

| Control | Result | Baseline `771fa8951` |
| --- | --- | --- |
| [`bare_name_value`](controls/bare_name_value/main.omg) | rejects: unknown name | **accepted by `--check`** |
| [`private_descendant`](controls/private_descendant/main.omg): `right` calls `left`'s private `deep` | rejects: unresolved call | rejected for the telescope reason |
| [`statement_call`](controls/statement_call/main.omg): `hidden();` from `main` | rejects: unresolved call | rejected for the telescope reason |
| [`terminal_call`](controls/terminal_call/main.omg): `transition { _ -> hidden(7) }` | rejects: no declared entry | already rejected for this reason |
| [`module_export`](controls/module_export/main.omg): `routines::hidden` after exporting `outer` | rejects: unresolved call | already rejected for this reason |
| [`local_named_like_helper`](controls/local_named_like_helper/main.omg): a local `hidden` beside `outer`'s private `hidden` | runs, exit 0 | runs, exit 0 |

A qualified call through the enclosing machine (`outer::hidden()`) is pinned by the
existing `tests/omega/fail/calls/nested_machine_private_descendant`.

## Requirement evidence

- **Establishment:** the declarations visible at a call are fixed by lexical scope: a
  nested helper by its enclosing machine body, a free machine by the package. No caller
  guard, argument or contract supplies visibility.
- **Identity and lifetime:** the call is bound once, during resolution, to an exact
  declaration symbol, and checking reads that symbol. Two machines spelled `hidden`
  coexist and every place observes both results, 7 and 107, in one program.
- **Composition:** the eight transports and the selected-implementation body carry
  the same two results. Lookup inside a machine nested three deep still reaches the free
  machine past an unrelated private `hidden`; a sibling cannot reach a sibling's
  descendant (`private_descendant`).
- **Soundness boundaries:** module qualification and export do not widen visibility
  (`module_export`); a local keeps its own meaning (`local_named_like_helper`); a bare
  helper name is not a value (`bare_name_value`).
- **Psi 05–07:** lexical nesting is erased into distinct flat machine identities; the
  interpreter returns exit 0 for all nine positives, so the two `hidden` identities
  survive Terminal Psi. These programs add no proof-only carrier, loan or cleanup.
- **Omega 00–02:** Build companions bind `ProgramEntry` for all four hosted targets;
  the target-body place also selects `X86Probe` or `ArmProbe` per target.
- **Omega 03–06, 07–09 and emission:** two `u64` calls and returns per place. No new
  stack-bound, layout or encoding claim.

Executable observations are from this host only, `linux_x86_64`, with `omega run --both`
reporting native exit 0 and interpreter exit 0 for the nine positives and
`local_named_like_helper`. The other three hosted bindings are authored and checked, not
run. The twins and controls are checked on this host.

## Open

The parent outcome remains open: complete call-contract and custody acceptance, joint
ranking and physical stack-bound evidence, product-compiler support, and migration and
removal of all retired source-state paths are not established by this row.

The refusal text still ends with the state-era hint "Internal states are transition
destinations, not ordinary callees". It is accurate for a transition destination but not
for a nested helper out of scope. Eight golden rows and
`tests/omega/fail/calls/terminal_state_jump_block_acknowledgement` pin that wording, so
rewording it is a separate change.
