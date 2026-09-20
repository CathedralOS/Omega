# COMPILER-PASS-PROFILE-INSTRUMENTATION — scope verification (2026-09-20, `0f5ae41e7d`)

Mined stub at `TASKS.md:7006`. **Scope verified: the item re-mines the
remaining legs already enumerated on the sibling `COMPILER-PASS-PROFILE-
TIMINGS` row (TASKS.md:7007), and every leg's touch surface is fenced by a
live sibling claim this wave. No independent unclaimed slice exists.**

## Landed substrate (verified live at `0f5ae41e7d`)

- `artifacts::compile_timings` owns the phase ladder: `CompileTimings`
  (enabled/disabled accumulator), `PhaseTiming`, `AllocationDelta`,
  `StageMeta`, `TimingCategory`.
- `CheckedCompilation::timings_mut` carries the accumulator through the
  checked record.
- `checked-compilation-to-terminal-artifact/src/terminal_artifact.rs`
  records `terminal-production`, `terminal-verification`, and
  `native-realization-proposal` rows via take/put-back;
  `ProgramEntryTerminalArtifact::stage_timings` carries the direct-route
  row; `NativeInputReuse` records `native-input-preparation` on cache miss.

## Open legs and their fences (18:50Z)

| Leg | Surface | Live claim |
| --- | --- | --- |
| Enable the accumulator (`CompileTimings::enabled()` driven by the
  `--timings` request instead of `default()`) | `compile_timings/`,
  `assembled-syntax-to-checked-compilation/src/checking.rs` |
  INTERNAL-PASS-PROFILE-TIMINGS (Jarod / swarm-w9-passtimings, 22:34Z) |
| Print rows under `--timings` | `omega/src/cli/arguments/compile{,.rs}`,
  `cli/compilation.rs` | COMPILER-OBSERVATION-OUTPUTS (00:41Z) |
| Merge the stage ladder into `CompileReport` | `compilation-report` —
  nominally unfenced (only `terminal_product/integer_comparisons.rs` is
  claimed), but with both consumer legs fenced the merge is dead plumbing
  this wave | — |
| Decompose coarse boundary rows into per-Psi-stage rows | needs a
  Psi-owned timing carrier — `terminal-production` cannot depend on
  `artifacts` under `psi_does_not_depend_on_omega`; the conversion seam
  (`compile_timings`, `checking.rs`) is the same fenced surface | — |

## Outcome

No code change made. The instrumentation substrate is landed; the residual
legs belong to INTERNAL-PASS-PROFILE-TIMINGS (enable), COMPILER-OBSERVATION-
OUTPUTS (print), and a Psi-carrier design decision on the parent
COMPILER-PASS-PROFILE-TIMINGS row. Coordinator may collapse this stub into
that row.
