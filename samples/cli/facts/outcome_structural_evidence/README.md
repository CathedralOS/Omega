# Outcome structural evidence

Selecting `Success` from `pick(Gate { threshold: 5 })` transports exact `above(gate.threshold)`
evidence and preserves threshold `5`.

The root `main.omg` is the canonical witness. Each row uses the same `Gate` value (`Gate { threshold: 5 }`),
`Success` guarantee, `selected` selector, and threshold comparison (`gate.threshold == 5`). Only the `Gate`'s
transport changes.

## Places

| Place | Positive Program | Reject Twin (invalid) | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical root probe passing `Gate { threshold: 5 }` to `probe` and `pick` |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | a local `Gate` copy |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | `Holder.gate` record field projection |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | an explicit machine `Gate` parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `make_gate()` contracted helper call result |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both Boolean match arms construct `Gate { threshold: 5 }` and join |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `NAMED_GATE` constant definition |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `inspect` successor machine receives `Gate` parameter across transition |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `deliver()` forwarding machine passes `Gate` to consumer machine |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | target-scoped machine bodies on all four hosted platforms |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and declares `Expected exit: 0`.
`omega run --both` reports Terminal Psi interpreter and native execution agreement:

```sh
omega run --both samples/cli/facts/outcome_structural_evidence/main.omg
omega run --both samples/cli/facts/outcome_structural_evidence/local/main.omg
omega run --both samples/cli/facts/outcome_structural_evidence/field/main.omg
omega run --both samples/cli/facts/outcome_structural_evidence/parameter/main.omg
omega run --both samples/cli/facts/outcome_structural_evidence/call_result/main.omg
omega run --both samples/cli/facts/outcome_structural_evidence/match_join/main.omg
omega run --both samples/cli/facts/outcome_structural_evidence/const_name/main.omg
omega run --both samples/cli/facts/outcome_structural_evidence/other_state/main.omg
omega run --both samples/cli/facts/outcome_structural_evidence/other_machine/main.omg
omega run --both samples/cli/facts/outcome_structural_evidence/target_body/main.omg
```

Every positive place and the root witness observe `native exit 0` and `interp exit: 0`.

### The oracle discriminates

`Main` transitions on `(probe()) && failure_arm()`. Each row calls `pick_failure()`, exercises the
nonmatching arm without manufacturing a `Success` witness, and traps if that dispatch or the preserved
threshold is wrong. If `gate.threshold` is corrupt (not 5) or outcome evidence is absent, the transition
evaluates to false and enters `machine bad() { crash Trap; }`, terminating with non-zero exit/signal.

Measured on producer `bcd7c58d3a26` for `macos_arm64`. Two probe classes were exercised, and they are
distinguished by where they die:

| Probe | Mutation | Result |
| --- | --- | --- |
| static, boundary establishment | `pick` body `{ selected = Fresh; Outcome::Success }` → `{ Outcome::Success }` | **checker refusal**, no native production: `outcome-specific evidence \`selected\` is not definitely assigned on the matching ordinary exit through pick::entry` |
| static, opposite establishment | `pick` body `{ selected = Fresh; Outcome::Success }` → `{ selected = Fresh; Outcome::Failure }` | **checker refusal**: `outcome-specific evidence \`selected\` is assigned on a nonmatching ordinary exit through pick::entry` |
| runtime, bound value | `Gate { threshold: 5 }` → `Gate { threshold: 4 }` or `{ threshold: 6 }` (comparison stays `== 5`) | **both engines trap**: `native signal 5` / `interp crash: Trap (host presentation agrees)` |
| runtime, consuming operation | `gate.threshold == 5` → `gate.threshold == 6` (literal stays `5`) | **both engines trap**: `native signal 5` / `interp crash: Trap (host presentation agrees)` |

An unmodified copy placed under the same `build.omg` in the probe directory returns `native exit 0` /
`interp exit: 0`, so the trap rows are the mutation and not the probe scaffolding. The establishment
probes never reach native production because the guaranteed outcome contract is checked before
realization; the value probes reach runtime because the comparison is an ordinary operation whose
operands the prover does not constant-fold into the outcome contract. Together they show the row is
enforced statically at establishment and observably at execution.

### Invalid controls and reject twins

The `invalid` child of each place keeps the place's program shape and removes only `selected = Fresh;` from `pick`.
It must reject at the producer's applicable ordinary exit with:

```
outcome-specific evidence `selected` is not definitely assigned on the matching ordinary exit
```

Selection is fact-sensitive: removing establishment invalidates the exact named guarantee consumed at the
`Success` operation, even though the runtime comparison itself does not execute a proof.

```sh
omega --check samples/cli/facts/outcome_structural_evidence/local/invalid/main.omg
omega --check samples/cli/facts/outcome_structural_evidence/field/invalid/main.omg
omega --check samples/cli/facts/outcome_structural_evidence/parameter/invalid/main.omg
omega --check samples/cli/facts/outcome_structural_evidence/call_result/invalid/main.omg
omega --check samples/cli/facts/outcome_structural_evidence/match_join/invalid/main.omg
omega --check samples/cli/facts/outcome_structural_evidence/const_name/invalid/main.omg
omega --check samples/cli/facts/outcome_structural_evidence/other_state/invalid/main.omg
omega --check samples/cli/facts/outcome_structural_evidence/other_machine/invalid/main.omg
omega --check samples/cli/facts/outcome_structural_evidence/target_body/invalid/main.omg
```

## Requirements and Evidence

- **Parent outcome:** `OUTCOME-SPECIFIC-CALL-EVIDENCE` (`TASKS.md`) / [contracts](../../../../wiki/spec/proofs/contracts.md).
- **Pinned fixtures:**
  - Positive passes: `tests/omega/pass/facts/outcome_structural_evidence_through_<place>_exit` (9 places pinned, each byte-identical to `<place>/main.omg` and recording native exit 0 and Terminal interpreter agreement).
  - Negative controls: `tests/omega/fail/facts/outcome_structural_evidence_through_<place>_rejected` (9 places pinned, each byte-identical to `<place>/invalid/main.omg`, rejecting with `outcome-specific evidence \`selected\` is not definitely assigned on the matching ordinary exit`).

- **Measured run record** (producer revision `3d4cd69c2bc1`, `macos_arm64`, library root `source/library`):
  - `python3 tools/sample_native_gate.py --filter outcome_structural_evidence` — 10/10 samples meet their documented oracle; 9/9 intended-invalid controls match their authored rejection.
  - `python3 tools/corpus_gate.py --native --filter facts/outcome_structural_evidence` — clean, 9 fixtures all `built`, executed 9 with exit codes `{'0': 9}`, 0 stdout differing from expected. Nine `built ... exit:0` rows are pinned in `tests/omega/corpus_native_macos_arm64.txt`.
  - Discrimination probes recorded under [The oracle discriminates](#the-oracle-discriminates). This row was **coverage**, not a repair: all nine places already ran green on both engines, so no compiler source changed. Only the golden rows were missing.
- **Pin canonicalization re-measurement** (producer revision `9ff26a870215` for the sample gate; opt-level 1 corpus runner built from the landed tree for the goldens; host `macos_arm64`, `--jobs 2`): the fixture families were renamed to the canonical `<fact>_through_<place>_{exit,rejected}` shape and their golden rows key-renamed to match, then re-measured.
  - `python3 tools/sample_native_gate.py --cli <producer> --filter cli/facts/outcome_structural_evidence,cli/facts/placed_stable_write --jobs 2` — 19/19 samples meet their documented oracle (including the root `main.omg`), 18/18 intended-invalid controls match their authored rejection, 620 s wall.
  - `python3 tools/corpus_gate.py --record --filter facts/outcome_structural_evidence,facts/placed_stable_write` at `88723a749c16` — 36 check rows (18 `checked` with `check-target:macos_arm64`, 18 twins `rejected` with their authored fragment), 18 native rows `built ... exit:0`, and 18 both-leg rows `exit:0 stdout:match interpreter-exit:0 interpreter-stdout:match interpreter:agrees`.

### Specification and Lowering Layers

1. **Establishment:** Constructing the outcome variant `Outcome::Success` with `selected = Fresh;` satisfies the callee contract `ensures Outcome::Success -> { selected: above(gate.threshold); }`. Matching the outcome in `transition pick(gate) { Outcome::Success { ; selected: proof } -> ... }` establishes the structural variant fact and payload bound directly without any post-hoc caller equality guard (`transition x == 100`).
2. **Identity and lifetime:** The matched outcome payload fact stays attached to the exact extracted field/value version (`gate.threshold == 5`) across moves, copies, and borrows. The owned reference-free argument `gate: Gate` represents an evaluated value snapshot; no intervening mutation alters or invalidates `gate.threshold` between snapshot creation, callee evaluation, and matching.
3. **Composition:** The outcome structural evidence composes across all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming operation (`gate.threshold == 5`). Each positive `<place>/main.omg` vs. `<place>/invalid/main.omg` differs only in the omission of `selected = Fresh;` in `pick`, isolating the exact structural evidence guarantee under test.
4. **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed with compilation error `outcome-specific evidence \`selected\` is not definitely assigned on the matching ordinary exit` when a branch or variant payload does not establish the required structural bound. Nonmatching exits (`Outcome::Failure`) owe no `Success` conclusion, and callers cannot manufacture a `Success` witness without taking that arm.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Tagged outcome discriminant branching and payload extraction lower into canonical target-neutral `PSITERM\0` operations. Proof-only obligations (`selected: above(gate.threshold)`) are erased while preserving declaration-owned subject rosters (distinct scalar and structural parameter namespaces carrying authenticated field paths). Both Terminal Psi interpreter and native execution agree in `omega run --both`.
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles uniformly across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Tagged sum/outcome layout plans (discriminant tag + aligned payload offsets) and `Gate` layout (`u32` 4 bytes) validate target plans. `CallPlan` and `StatePlan` lower outcome arguments and returns across registers and stack slots according to target calling conventions.
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects discriminant tag comparisons, conditional branches, and payload field loads, assigns register homes and disjoint stack-frame storage, composes worst-case stack usage, and inserts entry prologues and multi-return epilogues honoring the `StatePlan` frame ceiling.
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements resolve for tagged outcome homes, basic blocks and arm branches relax with offset reflow, machine-state footprint evidence validates transitive usage, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across host targets (`windows_x86_64` and `macos_arm64`).
