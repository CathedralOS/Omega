# Match loan last use

A match on one evaluation of a copyable flag selects an exclusive loan, that
loan dies at its last use, and the source is usable again after that death.

`choose(&mut visits)` runs exactly once (`visits == 1` after), and the single
evaluation selects the exclusive range loan `chosen.value[0..2]` (or, on the
false arm, `other.value[2..4]`). The write `selected[0] = 41` goes through that
loan; the read `let written = selected[0]` is the loan's **last use**; after
that death the source `chosen` is replaced whole (`chosen = Slot::Present
{ value: [7, 0, 7, 0] }`) and its new payload is observed (`value[0] == 7`).
Every place copies that sentence and performs the same match-select-write-read-
replace-observe sequence; only the transport of the fact changes.

The reject twin uses the source while the loan is still live — the source write
moves before the loan's last use — and is rejected by the borrow checker
(``while local borrow `selected` is still active``), so the row's oracle
discriminates a correct death point from a premature one.

Ordinary helper boundaries declare the two active `Slot::Present` cases and
`visits == 0` before they select a loan or call `choose`. Each caller proves
those entry requirements from its own construction; caller facts do not become
undeclared premises of a machine body. The matching reject twins carry the
same requirements and still replace a source while its selected loan is live.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) with [`build.omg`](build.omg) | — | canonical root witness |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the source is a local `Slot` value and the loan is a range of its case payload |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the source is a record field (`Box.chosen`) and the loan is a range of that field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the source arrives as a `&mut Slot` parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `select(&mut chosen, ...)` returns the exclusive range loan |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | the copyable flag arrives from a match join (`visits == 1` still holds) |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | the flag is the named constant `DECIDE` |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives the source and performs the death/replace sequence |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine supplies the source to the computing machine |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target bodies performs the sequence |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four
hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`)
and declares `Expected exit: 0`. `omega run --both` reports Terminal Psi
interpreter and native execution agreement (`native exit 0` and `interp exit: 0`):

```sh
target/debug/omega run --both samples/cli/facts/match_loan_last_use/main.omg
target/debug/omega run --both samples/cli/facts/match_loan_last_use/local/main.omg
target/debug/omega run --both samples/cli/facts/match_loan_last_use/field/main.omg
target/debug/omega run --both samples/cli/facts/match_loan_last_use/parameter/main.omg
target/debug/omega run --both samples/cli/facts/match_loan_last_use/call_result/main.omg
target/debug/omega run --both samples/cli/facts/match_loan_last_use/match_join/main.omg
target/debug/omega run --both samples/cli/facts/match_loan_last_use/const_name/main.omg
target/debug/omega run --both samples/cli/facts/match_loan_last_use/other_state/main.omg
target/debug/omega run --both samples/cli/facts/match_loan_last_use/other_machine/main.omg
target/debug/omega run --both samples/cli/facts/match_loan_last_use/target_body/main.omg
python tools/sample_native_gate.py --facts --filter cli/facts/match_loan_last_use
python3 tools/corpus_gate.py --filter facts/match_loan_last_use
```

### The oracle discriminates

Exit `0` is strictly discriminative. Corrupting the written value (`selected[0]
= 41` to `selected[0] = 40`) leaves the loan obligation intact but makes the
`kept(written, ...)` transition false, and `wrong()` executes `crash Trap;`:
both engines then abort non-zero: `native signal 5` and `interp crash: Trap
(host presentation agrees)`, exit 255. The reject twins exercise the other half: moving the source write
before the loan's last use refuses to compile.

## Reject Twins

Each place has an `invalid/` reject twin that moves only the source write
ahead of the loan's last use; everything else is identical to its positive
(verified with `git diff`). Compile-time verification fails closed with:

| Place | Twin | Pinned Diagnostic Fragment |
| --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | ``while local borrow `selected` is still active`` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | ``while local borrow `selected` is still active`` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | ``while local borrow `selected` is still active`` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | ``while local borrow `selected` is still active`` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | ``while local borrow `selected` is still active`` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | ``while local borrow `selected` is still active`` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | ``while local borrow `selected` is still active`` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | ``while local borrow `selected` is still active`` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | ``while local borrow `selected` is still active`` |

## Pinned Fixtures

The row is pinned by `tests/omega/{pass,fail}/facts/match_loan_last_use_through_*`
fixtures; every positive records native exit 0 and Terminal agreement, and every
twin records its rejection with the expected fragment. The nine pass fixtures
are byte-identical copies of their place programs, and the nine fail fixtures
are byte-identical copies of their twins, so sample and fixture cannot drift.

## Requirements and Evidence

- **Parent outcome:** `MATCH-SELECTIVE-LOWERING` (`TASKS.md`), whose remaining
  borrowed-join acceptance names exactly this witness: "End loans at last use:
  `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/current_ownership/address_joins.rs`
  currently pins origins through the whole dominated region" and "borrowed-result
  successor lifecycle". The exact requirement this row establishes: an
  exclusive loan selected by a match dies at its last use, and the lending
  source is afterward usable again — carried through all nine transport places
  to native execution.

### Specification and Lowering Layers

1. **Establishment:** The loan is established where it originates: the single
   evaluation `choose(&mut visits)` fixes the first-match arm (requires/ensures
   `visits == 0`/`true`), the arm expression `&mut chosen.value[0..2]` forms the
   exclusive range loan of the source's case payload, and the match join binds
   it to `selected`. The death point is established at the loan's last use
   (`let written: i64 = selected[0]`), and only after that does the source write
   `chosen = Slot::Present { … }` occur. No caller equality guard substitutes
   for either obligation.
2. **Identity and lifetime:** The loan names the exact storage of
   `chosen.value[0..2]` (case-payload range) for its whole live window; the
   source write is invalid while it is alive (twin) and valid once it dies.
   After replacement the source names a new value version and the row observes
   the new payload (`value[0] == 7`), never a stale one. The unselected
   `other` payload is never written.
3. **Composition:** The fact composes through local, field, parameter, call
   result, match join, const name, successor machine, forwarding machine, and
   target body; `<place>/main.omg` vs `<place>/invalid/main.omg` differ only in
   the position of the source write relative to the loan's last use.
4. **Soundness boundaries:** All nine twins fail closed during compile-time
   borrow checking (``while local borrow `selected` is still active``) when the
   source is disturbed while the loan is live; the positive places demonstrate
   that the same checker admits the write after the death point. Ownership and
   access authority are preserved; the generated write goes through the loan,
   and the replaced source is observed exactly.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** The match, the exclusive
   range loan, its last-use death, and the successor source write lower without
   loss into canonical target-neutral Terminal Psi (`PSITERM\0`); the Terminal
   Psi interpreter executes root and all nine positive places in complete
   agreement with native execution (`omega run --both`).
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):**
   Target realization joins Build companion rows (`ProgramEntry`,
   `ProviderPlan`, `CompilerBuiltinExecution`) across all four hosted targets.
   `Slot` layout geometry (variant tag + `[i64; 4]` payload) and the range loan's
   element view descriptor are evaluated per target, and `CallPlan`/`StatePlan`
   carry the machine calls (`choose`, and, in the call-result place, `select`)
   across register placements.
7. **Instruction selection, register homes, and activation frame realization
   (`Omega 03–06`):** Backend lowering selects target ISA instructions for the
   range-loan address computation, the element store `selected[0] = 41`, the
   discriminating loads, and the whole-source replacement store; register homes
   and disjoint frame slots are assigned under WCSU/bounded activation claims
   with entry prologues and multi-return epilogues.
8. **Form encoding, branch relaxation, relocations, and native image emission
   (`Omega 07–09` + emission):** Final encoding resolves symbolic displacements,
   lays out blocks with short/near branch relaxation, binds relocations, and
   emits direct PE/ELF/Mach-O images that execute with exit code 0 and agree
   with Terminal Psi (`omega run --both`) on this host.

Remaining open scope: the parent item's broader borrowed-join acceptance
(write-only joins, case/reference/descriptor-bearing referents, runtime-sized
element-view selections, ranges, computed roots, join-place reads/projections)
is carried by the other rows of the same family; this row closes only the
match-selected exclusive-loan last-use death and its successor source write.
