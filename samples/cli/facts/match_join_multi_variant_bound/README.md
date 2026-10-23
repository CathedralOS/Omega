# Multi-variant match join interval bound

Exhaustively matching a three-variant `case` value where every arm establishes `joined >= 20u64 && joined <= 60u64` preserves the joined interval bound across all nine places, discharging exact subtraction `60u64 - joined` and `joined - 20u64`.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

## Places

The nine places carry the joined interval bound through each language construct,
accompanied by `<place>/invalid/` reject twins:

| Place | Positive Program | Invalid Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | exhaustive match join establishes interval bound directly and discharges subtractions |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | match join result bound to local variable |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | match join result stored in and projected from record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | match join result passed as explicit machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | match join result returned directly from helper machine call |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | match join result converged across secondary match arms |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | match join bounds and mode instance declared in named constants |
| another state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | match join result passed across state transition boundary |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | match join result evaluated and consumed inside intermediate machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | match join result evaluated across target-scoped machine bodies |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four
hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`)
and declares `Expected exit: 0`. `omega run --both` reports Terminal Psi
interpreter and native execution agreement (`native exit 0` and `interp exit: 0`):

```sh
target/debug/omega run --both samples/cli/facts/match_join_multi_variant_bound/main.omg
target/debug/omega run --both samples/cli/facts/match_join_multi_variant_bound/local/main.omg
target/debug/omega run --both samples/cli/facts/match_join_multi_variant_bound/field/main.omg
target/debug/omega run --both samples/cli/facts/match_join_multi_variant_bound/parameter/main.omg
target/debug/omega run --both samples/cli/facts/match_join_multi_variant_bound/call_result/main.omg
target/debug/omega run --both samples/cli/facts/match_join_multi_variant_bound/match_join/main.omg
target/debug/omega run --both samples/cli/facts/match_join_multi_variant_bound/const_name/main.omg
target/debug/omega run --both samples/cli/facts/match_join_multi_variant_bound/other_state/main.omg
target/debug/omega run --both samples/cli/facts/match_join_multi_variant_bound/other_machine/main.omg
target/debug/omega run --both samples/cli/facts/match_join_multi_variant_bound/target_body/main.omg
python tools/sample_native_gate.py --facts --filter cli/facts/match_join_multi_variant_bound
python3 tools/corpus_gate.py --filter facts/match_join_multi_variant_bound
```

### The oracle discriminates

Exit `0` is strictly discriminative. The joined interval `[20u64, 60u64]` is
what discharges `60u64 - joined`; widening any one variant gate (`High` to
`val <= 61`) admits `joined == 61` and the exact subtraction refuses to compile
with the exact-arithmetic obligation
(``exact arithmetic in machine `main` state `entry` local `diff1` may overflow
`u64`: the operands are not provably in range (decision 17 -- exact arithmetic
is a proof obligation)``, pinned as ``may overflow `u64` `` in each twin's
`expected.txt`). Corrupting the observed
transition instead of the bound (`diff1 == 20 && diff2 == 20` to
`diff1 == 21 && diff2 == 20`) routes to `bad()`, which executes `crash Trap;`;
both engines abort non-zero (`native signal 5` with
`interp crash: Trap (host presentation agrees)`).

## Reject Twins

Each place has an `invalid/` reject twin that widens only the `High` variant's
payload gate (`val <= 60` to `val <= 61`); everything else is identical to its
positive (verified with `git diff`). Proof checking fails closed during
compile-time verification with the fragment named in the twin's `expected.txt`:

| Place | Twin | Pinned Diagnostic Fragment |
| --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | ``may overflow `u64` `` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | ``may overflow `u64` `` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | ``may overflow `u64` `` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | ``may overflow `u64` `` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | ``may overflow `u64` `` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | ``may overflow `u64` `` |
| another state | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | ``may overflow `u64` `` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | ``may overflow `u64` `` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | ``may overflow `u64` `` |

## Pinned Fixtures

The row is pinned by
`tests/omega/{pass,fail}/facts/match_join_multi_variant_bound_through_*`
fixtures; every positive records native exit 0 and Terminal agreement, and
every twin records its rejection with the expected fragment. The nine pass
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
| check | `pass/facts/match_join_multi_variant_bound_through_<place>_exit` | `checked`, `check-target:macos_arm64` |
| check twins | `fail/facts/match_join_multi_variant_bound_through_<place>_rejected` | `rejected`, `expected`; 0 rejected without the expected fragment |
| native | the same nine in `corpus_native_macos_arm64.txt` | `built`, `exit:0` |
| both | the same nine in `corpus_both_macos_arm64.txt` | `built`, `exit:0`, `interpreter-exit:0`, `interpreter:agrees` |

The three golden diffs are additions only: +36 keys and +24 continuation lines
in `corpus_outcomes.txt`, +18 rows each in `corpus_native_macos_arm64.txt` and
`corpus_both_macos_arm64.txt`, 0 deletions in all three.

Direct runs with the same pair's CLI (opt_level 1, stdin closed, macOS arm64):
the root and all nine places exit 0 through `omega run --both` (`native exit
0`, `interp exit: 0`), and the nine twins exit 1 with the pinned ``may overflow
`u64` `` fragment. Over both rows `tools/sample_gate.py --cli` reads 37/37
checked with 18 intended rejection controls matched and
`tools/sample_native_gate.py --facts --cli` reads 19/19 samples at their
documented oracle on macos_arm64 with 18/18 intended-invalid controls. Earlier
sweeps with the coordinator's shared generation pairs (`9d9ba4fb625`,
`595562a958b`) and the shared `.codex/worktrees/factsr1-producer` CLI at
`9ff26a87021` measured the same 19/19 positives and 18/18 twins; those runs
preceded the `.rs` movement that moved the generation past `93038434623` and
the record above is the measurement that supersedes them.

## Requirements and Evidence

- **Parent outcome:** `CANONICALIZE-SCALAR-RANGE-CONTRACTS` ([TASKS.md](../../../../TASKS.md)), whose settled proposition-based scalar range contract carries a **case payload `where`** gate through construction, matching, joins and exact arithmetic, and `MATCH-SELECTIVE-LOWERING`, which owns case-pattern dispatch and selected-arm execution. The exact requirement this row establishes: an exhaustive match join over a three-variant `case` whose every arm's `where` gate is `val >= 20 && val <= 60` preserves the joined interval `[20u64, 60u64]` and discharges the exact subtractions `60u64 - joined` and `joined - 20u64` across all nine transport places to native execution.
- **Requirement established:** Exhaustive multi-variant `case` sum match join interval intersection and bounded arithmetic discharge across all nine standard places (`Psi 05–07` / `Omega 00–09`), verifying that exhaustively matching a three-variant sum type (`case Mode { Low(u64), Mid(u64), High(u64) }`) where every variant arm establishes `joined >= 20u64 && joined <= 60u64` statically preserves the joined interval `[20u64, 60u64]`, discharging exact subtractions `60u64 - joined` and `joined - 20u64`.

Full 8-layer specification and lowering evidence:

- **Establishment:** The 3-variant sum `data Mode [copy] { case Low(val: u64) where val >= 20 && val <= 60; case Mid(val: u64) where val >= 20 && val <= 60; case High(val: u64) where val >= 20 && val <= 60; }` defines payload interval gates on all variants. Exhaustive `transition mode` dispatch unpacks the active variant payload, where each arm establishes an interval within `[20u64, 60u64]`. The compiler forms the join across all incoming arms (`Low`, `Mid`, `High`), proving that the unified joined result satisfies `joined >= 20u64 && joined <= 60u64`. This statically discharges both exact subtractions `60u64 - joined` and `joined - 20u64` without runtime traps or defensive caller guards.
- **Identity and lifetime:** The matched value is an active payload copy extracted from `Mode`. The resulting scalar `u64` value carries its proven static interval `[20u64, 60u64]` across local bindings, struct field storage, parameter passing, call returns, secondary match joins, named constants, state transitions, machine forwarding, and target-scoped execution bodies.
- **Composition:** The joined interval bound composes across all 9 standard places to statically discharge exact subtractions `60u64 - joined` and `joined - 20u64`:
  - `local`: match join result is bound to a local variable `local_joined` before subtracting.
  - `field`: match join result is stored in `JoinedCarrier.joined` record field and projected out.
  - `parameter`: match join result is passed to machine parameter `receive(val: u64)`.
  - `call_result`: helper machine `produce_joined()` returns the match join result across call boundaries.
  - `match_join`: match join result converges across secondary match arms.
  - `const_name`: bounds `UPPER` and `LOWER` and constant `MODE` are defined by compile-time named constants.
  - `other_state`: match join result is passed across a transition edge into successor state `step(val: u64)`.
  - `other_machine`: match join result is forwarded through intermediate machine `runner()`.
  - `target_body`: match join dispatch is executed across target-scoped machine bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in weakening the `High` variant payload gate to `val <= 61` (widening the joined interval to `[20, 61]`), proving that `60u64 - joined` strictly fails closed at compile time when any arm allows underflow.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `error: exact arithmetic in machine <machine> state <state> local \`diff1\` may overflow \`u64\`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation)` (the pinned ``may overflow `u64` `` fragment; the owner names move with the place — `main`/`entry` for local through const name, `step` for another state, `runner` for another machine, `exercise` for another target body), confirming that the arithmetic proof engine soundly detects that when `joined` can be 61, `60u64 - joined` may underflow `u64`.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Multi-variant sum types and exhaustive match joins lower into canonical target-neutral `PSITERM\0` representation, preserving interval arithmetic certificates and control-flow join structures. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Layout geometry for `Mode` allocates an 8-byte discriminant tag plus 8-byte payload slot (16 bytes total, 8-byte alignment), and calling plans assign register homes for scalar values across calling conventions.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA instructions (`x86_64` and `arm64`) for discriminant comparison, branch jumps, and 64-bit subtractions (`sub`). Disjoint activation stack frames and register homes are allocated with entry prologues and restoration epilogues honoring the `StatePlan` frame ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O images are emitted with valid machine-state footprint evidence, executing natively and agreeing with Terminal Psi (`omega run --both` reports `native exit 0` and `interp exit: 0`) across all hosted targets.

Remaining open under `CANONICALIZE-SCALAR-RANGE-CONTRACTS` / `MATCH-SELECTIVE-LOWERING`: non-exhaustive pattern coverage inference with unreachable default traps, multi-payload tuple-variant destructuring joins, and arbitrary disjoint interval-set unions. This row closes only the exhaustive three-variant case join whose arms share one interval gate.
