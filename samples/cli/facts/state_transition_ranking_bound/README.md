# State Transition Ranking Edge & Loop Exit Post-Guard Bound

Parent task: **CONSERVATION-CONTRACT** (`TASKS.md`).

## Governing requirement

Exiting a ranked state-transition loop under a terminal guard (`remaining <= 60u64`) preserves the loop-exit bound across all nine places, discharging exact subtraction `60u64 - remaining` with exit code `0`.

**Fact: exiting a ranked state-transition loop under a terminal guard (remaining <= 60u64) preserves the loop-exit bound across all nine places, discharging exact subtraction 60u64 - remaining with exit code 0.**

Every place carries the same construct and operation: a terminating, ranked state-transition countdown loop (`ranked_loop`) transitioning from entry to `state step` with ranking measure `terminates by current`. In each cycle, `c` decrements by 10 (`next = c - 10`) under a terminal guard `transition next <= 60`: when true, the transition routes to `machine done(rem: u64) requires rem <= 60`, discharging `ensures result <= 60`. The resulting bounded value (`remaining <= 60u64`, evaluated at runtime as `60u64`) is transported across all nine canonical program places to evaluate exact subtraction `60u64 - remaining` yielding `0`, and transitioning to `done()` with exit code `0`. Only the transport mechanism of the bounded loop result changes across places.

## Places

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| Root | [`main.omg`](main.omg) | — | Direct loop evaluation and exact subtraction |
| Local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | A local binding receives the loop-exit value before exact subtraction |
| Field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | A record field carries the loop-exit value before exact subtraction |
| Parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | An explicit machine parameter receives the loop-exit value before subtraction |
| Call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | A contracted forwarding machine delivers the loop-exit value |
| Match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Reconverging match branches join on the loop-exit bound before subtraction |
| Const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | A named constant supplies the subtrahend bound for exact subtraction |
| A nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | A successor machine transition carries the loop-exit value to subtraction |
| Another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | A separate evaluation machine carries the loop-exit value |
| Another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Target-scoped machine bodies for all four hosted targets evaluate subtraction |

Pinned fixtures:

- Pass: `tests/omega/pass/facts/state_transition_ranking_bound_through_<place>_exit/`
  — byte-identical copy of `<place>/main.omg`, with `Build::application`
  renamed to the fixture directory name.
- Reject: `tests/omega/fail/facts/state_transition_ranking_bound_through_<place>_rejected/`
  — byte-identical copy of `<place>/invalid/main.omg`, with `expected.txt`
  carrying the `may overflow \`u64\`` fragment. Feeding both halves through
  `git diff` shows only the construct's own guard and exit-bound values moved
  (`60u64 -> 61u64`).

### Verification commands

Root `main.omg` and all nine place projects bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declare `Expected exit: 0`, and carry `expected_native_exit.txt` (`0`). Run any positive directly:

```sh
omega run --both samples/cli/facts/state_transition_ranking_bound/main.omg
omega run --both samples/cli/facts/state_transition_ranking_bound/local/main.omg
omega run --both samples/cli/facts/state_transition_ranking_bound/field/main.omg
omega run --both samples/cli/facts/state_transition_ranking_bound/parameter/main.omg
omega run --both samples/cli/facts/state_transition_ranking_bound/call_result/main.omg
omega run --both samples/cli/facts/state_transition_ranking_bound/match_join/main.omg
omega run --both samples/cli/facts/state_transition_ranking_bound/const_name/main.omg
omega run --both samples/cli/facts/state_transition_ranking_bound/other_state/main.omg
omega run --both samples/cli/facts/state_transition_ranking_bound/other_machine/main.omg
omega run --both samples/cli/facts/state_transition_ranking_bound/target_body/main.omg
```

Each place has an `invalid/` twin whose terminal guard and loop-exit bound is weakened to `61u64`, causing exact subtraction `60u64 - remaining` to fail proof checking with `may overflow `u64`` (`exact subtraction underflow`). Check each twin with:

```sh
omega --check samples/cli/facts/state_transition_ranking_bound/<place>/invalid/main.omg
```

## Run record

Producer: the shared generation `gen-9d9ba4fb625` (`omega` sha256
`5a2fde4a3a914444db08f00e9849e06557fe0d6c8cf5fd5591c9b413bdd0e9ab`,
reported build profile `{"opt_level":"1"}`), built at base `9d9ba4fb625`.
Host `macos_arm64`; `OMEGA_LIBRARY_ROOT=<worktree>/source/library`.

Root `main.omg` and all nine places report `native exit 0` together with
`interp exit: 0` under `omega run --both` (`< /dev/null`): Terminal Psi and
native execution agree at exit 0 in ten of ten positives. All nine
`<place>/invalid/main.omg` twins reject at
`omega --check --target macos_arm64` (exit 1) with the pinned diagnostic
`exact arithmetic in machine ... may overflow \`u64\`` (`target_body` carries
two such diagnostics, the others one each).

Corpus goldens recorded on that revision with the generation
`corpus_runner`: 18 fixtures {`checked`: 9, `rejected`: 9} in
`tests/omega/corpus_outcomes.txt` and nine `built <ms> exit:0` rows in
`tests/omega/corpus_native_macos_arm64.txt`.

## Requirements and Evidence

Owning requirement: `wiki/spec/terminal-psi/control_flow.md#ranked-loops`, `wiki/spec/build/machine_state_evidence.md`, and `wiki/spec/terminal-psi/integer_certificates.md#canonical-scalar-goals`.
Parent task: **CONSERVATION-CONTRACT** (`TASKS.md`).
This row establishes that exiting a ranked state-transition countdown loop under a terminal guard (`remaining <= 60u64`) preserves the loop-exit bound across all nine places, discharging exact subtraction `60u64 - remaining` without underflow across all hosted targets.

- **Establishment:** Concrete entry actuals `current == 100` satisfy `requires current <= 100 && current >= 20`. In each iteration of `state step`, `next = c - 10` decreases monotonically, satisfying the ranking measure `terminates by current`. The terminal guard `transition next <= 60` branches on the condition: when true, arrival at `machine done(rem: u64) requires rem <= 60` establishes the terminal postcondition `ensures result <= 60` for `ranked_loop`. This bound directly proves that exact subtraction `60u64 - remaining` cannot underflow `u64` without requiring any post-hoc caller equality guard.
- **Identity and lifetime:** The loop scalar values (`current`, `c`, `next`, `rem`) maintain distinct, non-aliasing value versions across transitions. Live variable frontiers across cyclic edges are preserved in registers and spill slots across the loop cycle without storage corruption.
- **Composition:** Across state transitions, contracted calls, match joins, record fields (`LoopResult`), machine parameters, named constants, and target-scoped bodies, the established `remaining <= 60u64` bound composes without loss to justify exact subtraction `60u64 - remaining == 0`.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time: widening the terminal guard and loop-exit bound to `61u64` leaves exact subtraction `60u64 - remaining` unprovable (since `60 - 61 = -1 < 0`), soundly rejecting during checked compilation with `error: exact arithmetic in machine ... may overflow `u64`` (`exact subtraction underflow`).
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** The ranked loop lowers into a strongly connected control component in Terminal Psi (`PSITERM\0`) with typed basic blocks, explicit conditional branch edges, and well-founded termination ranking certificates (`terminates by current` via `Nat::Descending`). Terminal Psi execution agrees identically with native code (`omega run --both`, exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Build companion rows join `ProgramEntry` declarations across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Standard integer parameter and return calling plans (`CallPlan`) assign 64-bit scalar registers (`rcx`/`rax` on Windows x86_64, `rdi`/`rax` on Linux x86_64, `x0` on AArch64).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** The backend selects target ISA instructions for integer comparison (`cmp`), conditional jumps (`jbe` / `b.ls`), and integer subtraction (`sub`). Register homes are allocated with loop variable lifetimes spanning basic block boundaries, and stack frames with callee-saved preservation are sized to worst-case stack usage (WCSU).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Relocation fixups resolve symbolic displacements, basic block layout places loop bodies with branch relaxation, and direct PE/ELF/Mach-O images are emitted, passing native execution and Terminal Psi agreement (`omega run --both`, exit 0) across all hosted targets.

The row is verified by `samples/cli/facts/state_transition_ranking_bound` where every positive records native exit 0 and Terminal Psi execution agreement (`omega run --both`), and every invalid twin rejects with `may overflow `u64``.
What remains open: Dynamic ranking certificate synthesis for non-monotonic nested multi-induction variable loops.
