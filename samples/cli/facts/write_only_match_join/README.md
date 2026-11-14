# Match-selected write-only loan

A match on one evaluation of a copyable flag selects a write-only borrow of one
slot, the join records that write-only loan, and the consumer stores 41 through
the recorded loan, leaving the unselected slot and the neighbor canaries
unchanged.

Parent: **MATCH-SELECTIVE-LOWERING** in [TASKS.md](../../../../TASKS.md), the
borrowed-join requirement "extend borrowed joins ... to write-only joins". This
row establishes that requirement only.

Each program owns two `Cell` records, `{3, 17, 5}` and `{7, 23, 9}`, whose
`before`/`after` fields are canaries around the `value` slot. `choose(counter,
flag)` requires `counter == 0` and sets it to 1, so the subject runs once. The
two arms take `&write chosen.value` and `&write other.value`; the join's type is
`&write i64`, and `selected = 41` is the only use of the joined loan. After the
loan ends, the program reads both slots and all four canaries. `main()` runs the
construct with `flag = true` (expect `chosen 41, other 23`) and `flag = false`
(expect `chosen 17, other 41`), so a join that stored through the wrong arm,
through both arms, through a copy, or at the wrong field offset traps.

## Places

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| local | [main](local/main.omg) | [invalid](invalid_local/main.omg) | both cells are locals of `store` |
| field | [main](field/main.omg) | [invalid](invalid_field/main.omg) | both cells are fields of a `Pair` record (`&write pair.chosen.value`) |
| parameter | [main](parameter/main.omg) | [invalid](invalid_parameter/main.omg) | both cells and the counter arrive as `&mut` parameters of `apply` |
| call result | [main](call_result/main.omg) | [invalid](invalid_call_result/main.omg) | both cells are produced by `make(...)` call results |
| match join | [main](match_join/main.omg) | [invalid](invalid_match_join/main.omg) | both cells are produced by `match true` joins |
| const name | [main](const_name/main.omg) | [invalid](invalid_const_name/main.omg) | both cells are copies of the named constants `CHOSEN` and `UNSELECTED` |
| other state | [main](other_state/main.omg) | [invalid](invalid_other_state/main.omg) | the nested machine `pick` performs the join over borrows of its parent's cells |
| other machine | [main](other_machine/main.omg) | [invalid](invalid_other_machine/main.omg) | `forward` passes the cells to the top-level `apply`, which performs the join |
| target body | [main](target_body/main.omg) | [invalid](invalid_target_body/main.omg) | Build selects `X86Apply` or `ArmApply` for `SelectedApply::apply`; each checks its target tag (17 / 29) before the join |

Pinned fixtures: `tests/omega/pass/facts/write_only_match_join_through_<place>_exit`
and `tests/omega/fail/facts/write_only_match_join_through_<place>_rejected`.

Every twin differs from its positive by one statement: it reads the joined loan
(`let previous: i64 = selected;`) before the store, and rejects with
``reads write-only parameter `selected`; `&write` permits replacement or exact
`&write` forwarding, never observation``. A join that weakened the two
write-only arms to a readable `&mut` or shared loan would accept that read.

## Nearest rows

- [`write_only_indexed_reborrow`](../write_only_indexed_reborrow/README.md):
  its `match_join` place joins two reborrows of the same parent, so the join has
  one origin. This row joins loans of two distinct roots, and the join must keep
  both candidate origins and the write-only mode.
- [`match_computed_root`](../match_computed_root/README.md) and
  [`descriptor_range_exclusive_loan`](../descriptor_range_exclusive_loan/README.md):
  the same join shape over exclusive `&mut` loans. Their twins write the source
  while the join is live (`while local borrow ... is still active`); no twin
  there can check the join's access mode, because `&mut` permits the read this
  row's twins add.

## Requirements and evidence

| Requirement | How this row establishes it |
| --- | --- |
| Establishment | The loan is formed by the arms' `&write` borrows of existing valid cells; `choose` establishes the once-only subject from its own `requires`. |
| Identity and lifetime | The join keeps both candidate roots; reading both slots after `selected = 41` shows the loan ended at its last use and the store reached the original storage. |
| Composition | The consumer's store uses the joined loan directly; the twins differ from the positives only in the consumer's read of that loan. |
| Soundness boundaries | `&write` stays write-only through the join (twin diagnostics). |
| Psi 06–08 | Terminal Psi execution agrees with native execution for both flag values in all nine places. |
| Omega 00–09 | macOS AArch64 native images run with exit 0; canaries show the store hits only the `value` field of the selected record. |

Evidence, 2026-11-01, base `c79bb826816`, generation producer built at that base,
host macOS AArch64: `omega run --both` exit 0 for the nine positives (native and
interpreter agree), `omega --check` rejects the nine twins with the fragment
above; `python3 tools/corpus_gate.py --both --filter facts/write_only_match_join_`
recorded 9 `built exit:0 interpreter:agrees` rows, and the check tier recorded 9
`checked` and 9 `rejected expected` rows. All places passed on that base without
a compiler change. Linux and Windows native execution are not observed here.

## Open

The rest of the parent's borrowed-join list (runtime-sized element-view
selections, case/reference/descriptor-bearing referents beyond their rows' open
items) and its affine-custody and rewrite-provenance bullets stay on the board.
The write-only diagnostic names a local loan a "parameter"; that wording is
shared by about 36 fixtures and is left as is.
