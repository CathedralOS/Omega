# Match-selected runtime view extent

A match on one evaluation of a copyable flag selects an element view whose
length is known only at run time, the join records the selected view's extent,
and the consumer stores 41 at the last element of that recorded extent, leaving
the unselected view and the neighbor canaries unchanged.

Parent: **MATCH-SELECTIVE-LOWERING** in [TASKS.md](../../../../TASKS.md), the
borrowed-join requirement "runtime-sized element-view selections". This row
establishes that requirement only.

Each program owns `chosen: [i64; 4] = [3, 17, 18, 19]` and
`other: [i64; 6] = [7, 23, 24, 25, 26, 27]`. The arms lend whole arrays of
different extents, `&mut chosen` and `&mut other`, to one `&mut [i64]` view, so
the view's length is 4 or 6 depending on the flag. `choose(counter, flag)`
requires `counter == 0` and sets it to 1, so the subject runs once. The consumer
computes `selected.len - 1` (proved from the join's extent, at least 4) and
stores 41 there. Afterwards the program reads `chosen[3]`, `other[5]` and the
canaries `chosen[0]`, `chosen[2]`, `other[0]`, `other[4]`. `main()` runs both
flag values: `true` expects `chosen[3] == 41, other[5] == 27`; `false` expects
`chosen[3] == 19, other[5] == 41`. A view carrying the wrong arm's length, a
constant length, or the wrong backing traps.

## Places

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| local | [main](local/main.omg) | [invalid](invalid_local/main.omg) | both arrays are locals of `store` |
| field | [main](field/main.omg) | [invalid](invalid_field/main.omg) | both arrays are fields of a `Pair` record |
| parameter | [main](parameter/main.omg) | [invalid](invalid_parameter/main.omg) | both arrays and the counter arrive as `&mut` parameters of `apply` |
| call result | [main](call_result/main.omg) | [invalid](invalid_call_result/main.omg) | both arrays are produced by `make_chosen()` / `make_other()` |
| match join | [main](match_join/main.omg) | [invalid](invalid_match_join/main.omg) | both arrays are produced by `match true` joins |
| const name | [main](const_name/main.omg) | [invalid](invalid_const_name/main.omg) | both arrays are copies of `CHOSEN` and `UNSELECTED` |
| other state | [main](other_state/main.omg) | [invalid](invalid_other_state/main.omg) | the nested machine `pick` performs the join over borrows of its parent's arrays |
| other machine | [main](other_machine/main.omg) | [invalid](invalid_other_machine/main.omg) | `forward` passes the arrays to the top-level `apply` |
| target body | [main](target_body/main.omg) | [invalid](invalid_target_body/main.omg) | Build selects `X86Apply` or `ArmApply` for `SelectedApply::apply`; each checks its target tag (17 / 29) first |

Pinned fixtures: `tests/omega/pass/facts/match_runtime_view_extent_through_<place>_exit`
and `tests/omega/fail/facts/match_runtime_view_extent_through_<place>_rejected`.

Each twin replaces the `true` arm's array with `let mut empty: [i64; 0] = [];`
and `&mut empty`, so one arm records extent 0. The consumer's `selected.len - 1`
then has no proof and the twin rejects with ``local `last` may overflow `u64`:
the operands are not provably in range``. The twin keeps both arms whole-array
loans, so it reaches the join's extent rather than the arm compatibility check.

## Repair

On base `c79bb826816` every place was refused with `match arms produce
incompatible values`: the dispatch check compared `[i64; 4]` against `[i64; 6]`
by exact type. A same-length variant passed checking but native lowering
refused it (`UnsupportedOperationInScalarFunction`), because Psi minted each
arm's exclusive view as unrestricted and then moved it onto the affine join
parameter.

- `checking/src/value_custody/expression_types/match_dispatch.rs`: loans of
  fixed arrays with the same access, lifetime and element identity and
  different extents are compatible arms; the destination's structural value
  adapts each arm to its `[T]` view, as a direct `&mut array` initializer does.
- `06_checked-trees-to-lowered-psi/.../emission/element_views.rs`: an exclusive
  view transferred onto an owned join parameter is established affine, the same
  custody as a direct view.

## Nearest rows

- [`descriptor_range_exclusive_loan`](../descriptor_range_exclusive_loan/README.md):
  its arms are fixed subranges (`[0..2]`, `[2..4]`) of equal length, formed by
  `ElementViewSubslice`. Here the arms are whole-collection
  `EstablishMutableElementView` loans of different extents, and the twins name a
  missing extent fact that its twins (source writes during the loan) do not.
- [`write_only_match_join`](../write_only_match_join/README.md): the same
  harness over scalar `&write` slots; no view length is involved.

## Requirements and evidence

| Requirement | How this row establishes it |
| --- | --- |
| Establishment | Each arm's view is formed from an existing array; its extent is that array's declared length. |
| Identity and lifetime | The view's backing is the selected array; reads after the loan show the store reached it and not the other array. |
| Composition | The join keeps only the fact both arms justify (`len >= 4`); the twin with an empty arm loses it. |
| Soundness boundaries | Access, lifetime and element identity must agree across arms (`&a` with `&mut b`, or `[i64]` with `[u64]`, still reject with `match arms produce incompatible values`). |
| Psi 06–08 | Terminal Psi carries one `EstablishMutableElementView` per arm, moved affine onto the join parameter; the interpreter agrees with native execution. |
| Omega 00–09 | macOS AArch64 native images read the runtime length from the descriptor and store at the selected array's last element. |

Evidence, 2026-11-01, host macOS AArch64: candidate compiler built from base
`c79bb826816` plus the two changes above. `omega run --both` exits 0 with
native and interpreter agreeing for all nine positives; `omega --check` rejects
all nine twins with the fragment above. The generation producer at the base
refuses all nine positives with `match arms produce incompatible values`.
Linux and Windows native execution are not observed here.

## Open

An array-typed destination (`let s: &mut [i64; 4] = match ... { &mut a4, &mut a6 }`)
is now refused by lowering (`no executable body ... local data`) instead of by
the dispatch check, the same refusal a plain `let s: &mut [i64; 6] = &mut a4`
gets on the base. The dispatch diagnostic still carries no source location for
borrow arms.
