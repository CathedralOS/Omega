# Match join place read

A borrowed match join records the selected place, and a later projection read
uses that recorded referent. The consumer reads the joined field.

`choose()` runs once with the contract `ensures result == true`, so the match
`joined: &Pair = match choose() { true -> &chosen; false -> &spare }` statically
selects `&chosen` and records that selected place on the join. The later
projection read `read_field(joined)` carries the recorded place fact into the
callee's requirement `place.kept == 17` and reads `place.kept` through it; the
program observes `17` and takes `done()` (exit 0). Every place copies that
sentence and performs the same choose-join-read-observe sequence; only the
transport of the joined reference changes.

The reject twin changes only the consumer's own contract and projected field
(`place.kept == 17`/`place.kept` to `place.other == 17`/`place.other`). The
recorded place fact no longer discharges the call's requirement, and checking
fails closed with
``cannot prove requires contract for call read_field ... place.other == 17``,
so the row's oracle discriminates the transported place fact from an
unsupported field claim.

## Places

| Place | Positive Program | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the joined referents are two local `Pair` values |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the joined places are record fields (`&holder.chosen` / `&holder.spare`) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the join runs inside `apply` on `&Pair` parameters |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the joined places are `make_pair(...)` call results |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | the `Pair` values themselves arrive from match joins |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | the distinguishing values are named constants `KEPT`/`UNSELECTED` |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine `pick` receives the referents and performs the join/read |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine `forward` supplies the referents to `apply` |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target-scoped `apply` bodies performs the join/read |

## Execution

Every positive place binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and declares
`Expected exit: 0`. `omega run --both` reports Terminal Psi interpreter and
native execution agreement (`native exit 0` and `interp exit: 0`):

```sh
target/debug/omega run --both samples/cli/facts/match_join_place_read/local/main.omg
target/debug/omega run --both samples/cli/facts/match_join_place_read/field/main.omg
target/debug/omega run --both samples/cli/facts/match_join_place_read/parameter/main.omg
target/debug/omega run --both samples/cli/facts/match_join_place_read/call_result/main.omg
target/debug/omega run --both samples/cli/facts/match_join_place_read/match_join/main.omg
target/debug/omega run --both samples/cli/facts/match_join_place_read/const_name/main.omg
target/debug/omega run --both samples/cli/facts/match_join_place_read/other_state/main.omg
target/debug/omega run --both samples/cli/facts/match_join_place_read/other_machine/main.omg
target/debug/omega run --both samples/cli/facts/match_join_place_read/target_body/main.omg
python tools/sample_native_gate.py --facts --filter cli/facts/match_join_place_read
python3 tools/corpus_gate.py --filter facts/match_join_place_read
```

### The oracle discriminates

Exit `0` is strictly discriminative. Corrupting the selected place's payload
(`Pair { kept: 17, other: 5 }` to `Pair { kept: 41, other: 5 }`) removes the
recorded place fact, and the callee requirement `place.kept == 17` stops being
provable: checking fails closed before either engine runs
(``cannot prove requires contract for call read_field ... .kept == 17``). The
reject twins exercise the other half: the consumer asks for `place.other`
instead, and the same call refuses as unproved.

## Reject Twins

Each place has an `invalid/` reject twin that changes only the consumer's own
contract and projected field; everything else is identical to its positive
(verified with `git diff`). Compile-time verification fails closed with the
requirement fragment named in the twin's `expected.txt`:

| Place | Twin | Pinned Diagnostic Fragment |
| --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | `cannot prove requires contract for call read_field` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | `cannot prove requires contract for call read_field` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `cannot prove requires contract for call read_field` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `cannot prove requires contract for call read_field` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `cannot prove requires contract for call read_field` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `cannot prove requires contract for call read_field` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `cannot prove requires contract for call read_field` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `cannot prove requires contract for call read_field` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `cannot prove requires contract for call read_field` |

## Pinned Fixtures

The row is pinned by `tests/omega/{pass,fail}/facts/match_join_place_read_through_*`
fixtures; every positive records native exit 0 and Terminal agreement, and
every twin records its rejection with the expected fragments. The nine pass
fixtures are byte-identical copies of their place programs, and the nine fail
fixtures are byte-identical copies of their twins, so sample and fixture cannot
drift apart.

## Recorded runs

Every positive and reject row above is recorded on all three corpus legs,
measured with the generation pair built in this worktree at `93038434623`
(`corpus_runner`, opt_level 1, sha256 `24ef21394030`; sibling `omega` sha256
`bc33b4787586`) and anchored with the tree at the published base
`4fba1ca3cd5a`, so each row stamps `@4fba1ca3cd5a+wt`
(`python3 tools/corpus_gate.py --record --runner <pair>/corpus_runner`, then
`--native --record`, then `--both --record`, filtered to
`facts/match_join_multi_variant_bound,facts/match_join_place_read`):

| Leg | Rows | Result |
| --- | --- | --- |
| check | `pass/facts/match_join_place_read_through_<place>_exit` | `checked`, `check-target:macos_arm64` |
| check twins | `fail/facts/match_join_place_read_through_<place>_rejected` | `rejected`, `expected`; 0 rejected without the expected fragment |
| native | the same nine in `corpus_native_macos_arm64.txt` | `built`, `exit:0` |
| both | the same nine in `corpus_both_macos_arm64.txt` | `built`, `exit:0`, `interpreter-exit:0`, `interpreter:agrees` |

The three golden diffs are additions only: +36 keys and +24 continuation lines
in `corpus_outcomes.txt` (both match-join rows), +18 rows each in
`corpus_native_macos_arm64.txt` and `corpus_both_macos_arm64.txt`, 0 deletions
in all three.

Direct runs with the same pair's CLI (opt_level 1, stdin closed, macOS arm64):
all nine places exit 0 through `omega run --both` (`native exit 0`, `interp
exit: 0`), and the nine twins exit 1 with the pinned ``cannot prove requires
contract for call read_field`` and `place.other == 17` fragments. Over both
rows `tools/sample_gate.py --cli` reads 37/37 checked with 18 intended
rejection controls matched and `tools/sample_native_gate.py --facts --cli`
reads 19/19 samples at their documented oracle on macos_arm64 with 18/18
intended-invalid controls. Earlier sweeps with the coordinator's shared
generation pairs (`9d9ba4fb625`, `595562a958b`) and the shared
`.codex/worktrees/factsr1-producer` CLI at `9ff26a87021` measured the same 9/9
positives and 9/9 twins; those runs preceded the `.rs` movement that moved the
generation past `93038434623` and the record above is the measurement that
supersedes them.

## Requirements and Evidence

- **Parent outcome:** `MATCH-SELECTIVE-LOWERING` (`TASKS.md`), whose remaining
  borrowed-join acceptance names exactly this witness: "Still missing:
  write-only joins (**WRITE-ONLY-BORROW**), case/reference/descriptor-bearing
  referents, runtime-sized element-view selections, ranges, computed roots, and
  join-place reads/projections." The exact requirement this row establishes:
  a borrowed match join records its selected place, and a later projection
  read through the joined reference is discharged from that recorded place
  fact — carried through all nine transport places to native execution.

### Specification and Lowering Layers

1. **Establishment:** The selected place is established where it originates:
   `choose()` is called once with `ensures result == true`, the match join
   `match choose() { true -> &chosen; false -> &spare }` records `&chosen` as
   the selected place, and the projection read `read_field(joined)` carries
   that recorded place fact into the callee's requirement `place.kept == 17`.
   No caller equality guard substitutes for the obligation.
2. **Identity and lifetime:** The joined reference names the exact `Pair`
   storage of the selected place for the whole read; the unselected `spare`
   place is never read. The recorded place fact survives the join and the call
   boundary, and the read observes exactly the value that place holds
   (`observed == 17`).
3. **Composition:** The fact composes through local, field, parameter, call
   result, match join, const name, successor machine, forwarding machine, and
   target body; `<place>/main.omg` vs `<place>/invalid/main.omg` differ only
   in the consumer's own requirement/projection field (`place.kept` to
   `place.other`).
4. **Soundness boundaries:** All nine twins fail closed during compile-time
   proof checking
   (``cannot prove requires contract for call read_field ... place.other == 17``)
   when the consumer claims a field the recorded place does not establish; the
   positive places demonstrate that the same checker admits the read the join
   actually records. Corrupting the selected payload (`kept: 17` to
   `kept: 41`) refuses the same obligation from the other direction.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** The once-only subject
   evaluation, the borrowed join, the recorded selected place, and the
   projection read lower without loss into canonical target-neutral Terminal
   Psi (`PSITERM\0`); the Terminal Psi interpreter executes all nine positive
   places in complete agreement with native execution (`omega run --both`).
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):**
   Target realization joins Build companion rows (`ProgramEntry`, `ProviderPlan`,
   `CompilerBuiltinExecution`) across all four hosted targets. `Pair` layout
   geometry (two `i64` fields) is evaluated per target, and
   `CallPlan`/`StatePlan` carry the reference arguments (`joined`, and in the
   parameter/other-machine/target-body places the two `&Pair` parameters)
   across register placements.
7. **Instruction selection, register homes, and activation frame realization
   (`Omega 03–06`):** Backend lowering selects target ISA instructions for the
   reference materialization, the discriminating loads of `kept`, and the
   crash-path trap; register homes and disjoint frame slots are assigned under
   WCSU/bounded activation claims with entry prologues and epilogues.
8. **Form encoding, branch relaxation, relocations, and native image emission
   (`Omega 07–09` + emission):** Final encoding resolves symbolic displacements,
   lays out blocks with short/near branch relaxation, binds relocations, and
   emits direct PE/ELF/Mach-O images that execute with exit code 0 and agree
   with Terminal Psi (`omega run --both`) on this host.

Remaining open scope: the parent item's broader borrowed-join acceptance
(write-only joins, case/reference/descriptor-bearing referents, runtime-sized
element-view selections, ranges, computed roots) is carried by the other rows
of the same family; this row closes only the join-place read/projection
obligation.