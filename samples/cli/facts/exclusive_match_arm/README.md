# Exclusive match arm

A match on one evaluation of a copyable flag returns the exclusive loan of the
selected primitive, writes 41 through that loan, and leaves the unselected
primitive unchanged.

`choose(&mut visits)` runs exactly once (`visits == 1` after) and the single
evaluation selects the exclusive loan `&mut chosen` (or, on the false arm,
`&mut other`). The write `selected = 41` goes through that loan, the consumer
reads it back (`observed == 41`), the selected primitive holds the written
value (`chosen == 41`), and the unselected primitive is unchanged
(`other == 0`). Every place copies that sentence and performs the same
match-select-write-observe sequence; only the carrier of the two primitives and
the transport of the loan change.

The reject twin uses the source while the loan is still live — the source write
moves between the match and the write through the loan — and is rejected by the
borrow checker (``while local borrow `selected` is still active``).

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) with [`build.omg`](build.omg) | — | canonical root witness |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the primitives are local `let`s |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the primitives are record fields (`Pair.left`, `Pair.right`) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the primitives arrive as `&mut i64` parameters |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `select(&mut chosen, &mut other, …)` returns the exclusive loan |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | the copyable flag arrives from a match join |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | the flag is the named constant `DECIDE` |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives the primitives and returns the loan |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine supplies the primitives |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target bodies performs the sequence |

A nonstandard probe at [`case_payload/main.omg`](case_payload/main.omg)
(positive + twin) transports the same sentence through a case payload
(`Slot::Present { value }`); it is retained intact as a mechanism probe that
records how far the case-payload carrier reaches. It is not one of the nine
places and never counts toward the row.

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four
hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`)
and declares `Expected exit: 0`. `omega run --both` reports Terminal Psi
interpreter and native execution agreement (`native exit 0` and `interp exit: 0`):

```sh
target/debug/omega run --both samples/cli/facts/exclusive_match_arm/main.omg
target/debug/omega run --both samples/cli/facts/exclusive_match_arm/local/main.omg
target/debug/omega run --both samples/cli/facts/exclusive_match_arm/field/main.omg
target/debug/omega run --both samples/cli/facts/exclusive_match_arm/parameter/main.omg
target/debug/omega run --both samples/cli/facts/exclusive_match_arm/call_result/main.omg
target/debug/omega run --both samples/cli/facts/exclusive_match_arm/match_join/main.omg
target/debug/omega run --both samples/cli/facts/exclusive_match_arm/const_name/main.omg
target/debug/omega run --both samples/cli/facts/exclusive_match_arm/other_state/main.omg
target/debug/omega run --both samples/cli/facts/exclusive_match_arm/other_machine/main.omg
target/debug/omega run --both samples/cli/facts/exclusive_match_arm/target_body/main.omg
python tools/sample_native_gate.py --facts --filter cli/facts/exclusive_match_arm
python3 tools/corpus_gate.py --filter facts/exclusive_match_arm
```

### The oracle discriminates

Exit `0` is strictly discriminative. Corrupting the written value (`selected =
41` to `selected = 40`) leaves the loan obligation intact but makes the
transition false and `wrong()` executes `crash Trap;`; both engines abort
non-zero: `native signal 5` and `interp crash: Trap (host presentation
agrees)`, exit 255. The reject
twins exercise the other half: writing the source while the loan is live
refuses to compile.

## Reject Twins

Each place has an `invalid/` reject twin that moves only the source write ahead
of the write through the loan; everything else is identical to its positive
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

The row is pinned by `tests/omega/{pass,fail}/facts/exclusive_match_arm_through_*`
fixtures; every positive records native exit 0 and Terminal agreement, and every
twin records its rejection with the expected fragment. The nine pass fixtures
are byte-identical copies of their place programs, and the nine fail fixtures
are byte-identical copies of their twins, so sample and fixture cannot drift.
The `case_payload` probe is not pinned and never counts toward the row.

## Requirements and Evidence

- **Parent outcome:** `MATCH-SELECTIVE-LOWERING` (`TASKS.md`), whose remaining
  borrowed-join acceptance names exactly this witness: "Still missing: …
  primitive/projected exclusive arms". The exact requirement this row
  establishes: the match returns the exclusive loan of the selected primitive
  (including primitive/projected arms), the write goes through that loan, and
  the unselected primitive is unchanged — carried through all nine transport
  places to native execution.

### Specification and Lowering Layers

1. **Establishment:** The loan is established where it originates: the single
   evaluation `choose(&mut visits)` fixes the selected arm (`requires counter
   == 0` / `ensures result == true`), and the arm expression `&mut chosen`
   (a primitive place, a field, a parameter, a call result, a join, a const
   value, or a nested machine's parameter) forms the exclusive loan bound by the
   match join. No caller equality guard substitutes for the loan.
2. **Identity and lifetime:** The loan names the exact storage of the selected
   primitive for its whole live window; the source write is invalid while it
   is alive (twin) and the unselected primitive is never touched. After the
   write, the same loan reads back the written value (`observed == 41`) and
   the selected primitive reflects it (`chosen == 41`).
3. **Composition:** The fact composes through local, field, parameter, call
   result, match join, const name, successor machine, forwarding machine, and
   target body; `<place>/main.omg` vs `<place>/invalid/main.omg` differ only in
   the position of the source write relative to the write through the loan.
4. **Soundness boundaries:** All nine twins fail closed during compile-time
   borrow checking (``while local borrow `selected` is still active``) when the
   source is disturbed while the loan is live; the positives show the same
   checker admits the loan write, preserves exclusivity, and leaves the
   unselected primitive untouched.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** The match, the primitive
   exclusive loan, and its write/read lower without loss into canonical
   target-neutral Terminal Psi (`PSITERM\0`); the Terminal Psi interpreter
   executes root and all nine positive places in complete agreement with native
   execution (`omega run --both`).
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):**
   Target realization joins Build companion rows (`ProgramEntry`,
   `ProviderPlan`, `CompilerBuiltinExecution`) across all four hosted targets;
   the primitive carrier's geometry and the `CallPlan`/`StatePlan` register
   placements for the match-selected loan (and, in the call-result place,
   `select`) are evaluated per target.
7. **Instruction selection, register homes, and activation frame realization
   (`Omega 03–06`):** Backend lowering selects target ISA instructions for the
   arm selection, the exclusive-address store `selected = 41`, and the
   discriminating loads; register homes and disjoint frame slots are assigned
   under WCSU/bounded activation claims with entry prologues and multi-return
   epilogues.
8. **Form encoding, branch relaxation, relocations, and native image emission
   (`Omega 07–09` + emission):** Final encoding resolves symbolic displacements,
   lays out blocks with short/near branch relaxation, binds relocations, and
   emits direct PE/ELF/Mach-O images that execute with exit code 0 and agree
   with Terminal Psi (`omega run --both`) on this host.

Remaining open scope: the parent item's broader borrowed-join acceptance is
carried by the other rows of the same family; the `case_payload` probe records
the case-payload carrier's reach and stays a probe. This row closes only the
primitive/projected exclusive arm selected by a match.
