# Match-selected descriptor range loan

A match on one evaluation of a copyable flag selects a subrange of the chosen descriptor payload,
the join records that selected range, and the consumer writes 41 through the first element of that
recorded range, leaving the unselected payload unchanged.

`choose(counter: &mut u64)` requires `counter == 0` and increments it, so the flag subject is
evaluated exactly once per run; the join takes `&mut` subranges of the two `Slot::Present`
payloads — `&mut chosen.value[0..2]` on the true arm and `&mut other.value[2..4]` on the false
arm. The consumer writes 41 through the first element of the recorded range and reads it back, so
exit 0 holds exactly when the recorded range is the selected one and the flag ran once
(`observed == 41 && visits == 1`).

Every place copies that fact and performs the same operation on the same bound: one match over one
evaluated flag, one exclusive subrange loan, `selected[0] = 41`, then the read-back. Only the
transport of the two payloads and of the flag counter changes.

The final clause — the unselected payload is unchanged — is established by the mechanism rather
than by a second observation: the recorded range is the only writable path, and the exclusive loan
forbids touching the source while it is live; every twin pins exactly that. (A read of the
*unselected* payload is deliberately not part of the oracle: without an `ensures` on `choose` the
compiler cannot know which arm was taken, so the loan's candidate set legitimately covers both
payloads and such a read is refused conservatively rather than proved.)
Ordinary helper boundaries declare the two active `Slot::Present` cases and
`visits == 0` before they select a loan or call `choose`. Each caller proves
those entry requirements from its own construction; caller facts do not become
undeclared premises of a machine body. The matching reject twins carry the
same requirements and still replace a source while its selected loan is live.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) with [`build.omg`](build.omg) | — | canonical root witness running the same join and consumer |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | both `Slot` payloads are locals and the join takes `&mut` subranges of them |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | both payloads live in a `Pair` record's fields (`pair.chosen`, `pair.other`) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | both slots and the call counter arrive as parameters of `apply` |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | both slots are produced by `present(...)` call results |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both slots are produced by `match true` joins |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | both payloads are the named constants `CHOSEN` and `UNSELECTED` |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | the one evaluated flag and both payload loans reach the successor machine `pick`, which records the selected range there |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine (`forward`) supplies the loans to `apply` |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of the four target bodies of `apply` selects and writes |

## Execution

Every positive place, and the row's root `main.omg`, binds `ProgramEntry` for all four hosted
targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and declares
`Expected exit: 0`. `omega run --both` reports Terminal Psi interpreter and native agreement
(`interp exit: 0` and `native exit 0`) for each:

```sh
omega run --both samples/cli/facts/descriptor_range_exclusive_loan/main.omg
omega run --both samples/cli/facts/descriptor_range_exclusive_loan/local/main.omg
omega run --both samples/cli/facts/descriptor_range_exclusive_loan/field/main.omg
omega run --both samples/cli/facts/descriptor_range_exclusive_loan/parameter/main.omg
omega run --both samples/cli/facts/descriptor_range_exclusive_loan/call_result/main.omg
omega run --both samples/cli/facts/descriptor_range_exclusive_loan/match_join/main.omg
omega run --both samples/cli/facts/descriptor_range_exclusive_loan/const_name/main.omg
omega run --both samples/cli/facts/descriptor_range_exclusive_loan/other_state/main.omg
omega run --both samples/cli/facts/descriptor_range_exclusive_loan/other_machine/main.omg
omega run --both samples/cli/facts/descriptor_range_exclusive_loan/target_body/main.omg
python tools/sample_native_gate.py --facts --filter cli/facts/descriptor_range_exclusive_loan
```

## Reject Twins

Each place has an `invalid/` reject twin with an authored
[`sample.json`](local/invalid/sample.json) intended-invalid identity
(`"role": "intended-invalid"`, `"rejection_contains": [...]`) and a pinned
`tests/omega/fail/facts/descriptor_range_exclusive_loan_through_<place>_rejected` fixture. The twin changes only the
construct's own input or selection, and checking fails closed during compile-time verification:

| Place | Twin | What it changes | Pinned Diagnostic Fragment |
| --- | --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | `chosen` is reassigned while the range loan is live | `statement 4 mutates `chosen` while local borrow `selected` is still active (borrowed at statement 3; its last use is at statement 6)` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | `pair.chosen` is reassigned while the range loan is live | `statement 3 mutates `pair.chosen` while local borrow `selected` is still active (borrowed at statement 2; its last use is at statement 5)` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `chosen` is reassigned in `apply` while the range loan is live | `statement 1 mutates `chosen` while local borrow `selected` is still active (borrowed at statement 0; its last use is at statement 3 and it is released at state exit)` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `chosen` is reassigned while the range loan is live | `statement 4 mutates `chosen` while local borrow `selected` is still active (borrowed at statement 3; its last use is at statement 6)` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `chosen` is reassigned while the range loan is live | `statement 4 mutates `chosen` while local borrow `selected` is still active (borrowed at statement 3; its last use is at statement 6)` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `chosen` is reassigned while the range loan is live | `statement 4 mutates `chosen` while local borrow `selected` is still active (borrowed at statement 3; its last use is at statement 6)` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `chosen` is reassigned in `pick` while the range loan is live | `statement 1 mutates `chosen` while local borrow `selected` is still active (borrowed at statement 0; its last use is at statement 3)` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `chosen` is reassigned in `apply` while the range loan is live | `statement 1 mutates `chosen` while local borrow `selected` is still active (borrowed at statement 0; its last use is at statement 3 and it is released at state exit)` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each target body reassigns `chosen` while the range loan is live | `statement 1 mutates `chosen` while local borrow `selected` is still active (borrowed at statement 0; its last use is at statement 3 and it is released at state exit)` |

## Pinned Fixtures

The row is pinned by `tests/omega/{pass,fail}/facts/descriptor_range_exclusive_loan_through_*` fixtures — nine pass
fixtures (each a byte-identical copy of its place program plus `expected_native_exit.txt` `0`)
and nine fail fixtures (each a byte-identical copy of its place twin plus the pinned
`expected.txt` fragment). The sample and the fixtures cannot drift apart.

## Requirements and Evidence

- **Parent outcome:** `MATCH-SELECTIVE-LOWERING` (`TASKS.md`): "Lower value dispatch for owned,
  borrowed and linear results, structural/case/domain patterns and coverage through to native
  execution. Preserve once-only subject evaluation, authored first-match order, selected-arm-only
  execution, exact result/loan origins and death edges." Under *Borrowed joins* the task names
  case/reference/descriptor-bearing referents, ranges and join-place reads/projections as still
  missing, and its acceptance asks for corpus run fixtures for borrowed joins that run natively
  with interpreter agreement. This row supplies the case/descriptor-bearing referent and range part of that acceptance.

1. **Establishment:** the selected range is established by the join itself — one evaluation of
   `choose(&mut visits)` (its `requires counter == 0` rules out a second evaluation) selects
   `&mut chosen.value[0..2]` or `&mut other.value[2..4]`; `visits == 1` in the transition proves
   the once-only subject evaluation at runtime.
2. **Identity and lifetime:** the recorded range names one exact payload subrange; the write and
   the read-back go through that recorded range, and the twin shows that reassigning the source
   while the loan is live is rejected (`… mutates `chosen` while local borrow `selected` is still
   active …`), so the unselected payload cannot be reached through the source while the range is
   recorded.
3. **Composition:** the loan composes with local payloads, record fields, parameters, call
   results, match joins, constants, a successor machine, a forwarding machine and four target
   bodies; `<place>/main.omg` and `<place>/invalid/main.omg` differ only in the construct's own
   obligation (a write to the source while the range loan is live).
4. **Soundness boundaries:** every twin rejects during proof checking with its authored
   ``mutates … while local borrow `selected` is still active`` fragment; access authority of the
   `&mut` payload loans and the flag counter are preserved.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** the case-tagged `Slot` payloads, the
   subrange loans and the indexed write lower without loss into Terminal Psi and execute in the
   interpreter in agreement with native execution on all nine places plus the root.
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** each place
   binds `ProgramEntry` for the four hosted targets; the case-tagged payload and the `{base,
   extent}` range descriptor settle through layout geometry and the calling plan for each target.
7. **Instruction selection, register homes, activation frame realization (`Omega 03–06`):** the
   selected range's base/extent pair is materialized in register homes and the indexed store
   (`selected[0] = 41`) lowers to the target's element-width store, per target body family.
8. **Form encoding, branch relaxation, relocations, native image emission (`Omega 07–09` +
   emission):** the fixtures publish PE/ELF/Mach-O images that execute natively with exit 0 and
   agree with Terminal Psi; the nine `…_exit` fixtures carry `expected_native_exit.txt` `0` and
   are recorded in the host's native golden.

## Open items

- The row's fact sentence states the unselected payload stays unchanged; that clause is carried by
  the exclusive-loan obligation (each twin rejects with its authored fragment) rather than by a
  second runtime observation, for the reason given above.
- No requirement above is claimed closed beyond the nine places and nine twins of this row; the
  parent task's remaining acceptance (write-only joins, runtime-sized element-view selections,
  computed roots, join-place reads/projections, borrowed-result successor lifecycle) stays open on
  the board.

