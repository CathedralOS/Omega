# Psi stage timing carrier — design note

Board: COMPILER-PASS-PROFILE-TIMINGS (TASKS.md "Remaining leg: decompose the
coarse boundary rows into per-stage rows — finer in-Psi rows need a Psi-owned
timing carrier because `terminal-production` cannot depend on `artifacts`
under `psi_does_not_depend_on_omega`"). This draft records the carrier design
the row asks for, what is already landed, and the one remaining call-site leg.

## Constraint

`--timings` stage rows live in the Omega-side accumulator
`artifacts::compile_timings::{CompileTimings, StageMeta, TimingCategory}`
(each row is a `StageMeta` — stage name, input form, output form, category —
plus elapsed microseconds and an `AllocationDelta`).

Psi crates cannot name that type: `omega-architecture-test` enforces
`psi_does_not_depend_on_omega`, and `terminal-production` sits under
`omega-rust/psi/`. So a Psi stage cannot call `timings.record_result(meta, …)`
directly — the meta vocabulary itself is Omega's.

## Design (landed)

The carrier is a Psi-owned row list that the owning Omega caller merges after
the boundary call returns.

- `terminal-production/src/stage_timings.rs` defines
  `enum TerminalProductionStage` — one variant per measured production leg
  (`MachineSelection`, `LedgerCheck`, `Lowering`, `Optimization`,
  `EntryReceipt`, `TerminalIdentity`, `ReceiverEligibility`, `Publication`,
  `BoundaryOperatorScope`) — and `struct TerminalProductionTimings`
  `{ enabled: bool, rows: Vec<(TerminalProductionStage, u128)> }`.
- `TerminalProductionTimings::record_result(stage, work)` wraps a fallible
  closure: when `enabled` it times the call and pushes `(stage, µs)`;
  repeated visits to one stage aggregate onto the first row so the ladder
  keeps call order. When `enabled` is false the work runs unmeasured, so
  every production path shares one instrumented body — no measured and
  unmeasured copies of the pipeline.
- `terminal_production.rs` threads `&mut TerminalProductionTimings` through
  the shared body and wraps each leg (`MachineSelection` at the selector,
  `LedgerCheck`/`Lowering`/`Optimization` in `lower_and_optimize_timed`,
  `EntryReceipt`, `TerminalIdentity`, `ReceiverEligibility`, `Publication`,
  `BoundaryOperatorScope` in `produce_program_entry_parts_timed`). The
  carrier rides on the produced artifact as `stage_timings`.
- The Omega side names the rows at merge time:
  `checked-compilation-to-terminal-artifact/terminal_artifact.rs`
  `terminal_production_stage_meta` maps each `TerminalProductionStage` to a
  `StageMeta` with its real input/output form names
  (`terminal-production/machine-selection`: `CheckedTrees → TerminalMachine`,
  `terminal-production/lowering`: `CheckedTrees → LoweredPsi`,
  `terminal-production/publication`:
  `OptimizedLoweredPsi → CanonicalTerminalArtifact`, etc.) and
  `TimingCategory::Pipeline`; `merge_terminal_production_timings` then
  `add_completed`s each row under the coarse `terminal-production` boundary
  row the caller already recorded. The boundary row stays the contract with
  downstream readers; the sub-rows are additive detail beneath it.
- `native_product.rs` merges `terminal.stage_timings()` back into the
  checked compilation's `CompileTimings` (`*checked.timings_mut() =
  terminal.stage_timings().clone()`), and `native_product/input_reuse.rs`
  records the `native-input-preparation` row on cache miss.
- `CompileReport::timings()` carries the ladder; `cli/compilation.rs`
  `--timings` prints command rows, then `report.timings()` stage rows, then
  `total elapsed`. Pinned by
  `timings_are_opt_in_stderr_output_without_debug_files` and
  `timings_request_carries_the_recorded_stage_ladder_to_the_report`.

Why a separate carrier rather than a shared vocabulary crate: the carrier
keeps Psi's own stage vocabulary (a `TerminalProductionStage` knows only Psi
production legs) and pushes all Omega naming to the one file that owns the
boundary (`terminal_artifact.rs`). Adding a measured Psi leg is a new enum
variant + one `StageMeta` arm; no layering waiver, no upward dependency, and
the disabled path is literally the same code (`work()` unwrapped).

## Remaining leg

- The prepared-project route does not yet thread the flag:
  `manager::operations::check_project::PreparedLocalProjectCheckRequest` and
  `PreparedLocalProjectNativeRequest` carry no `timings` field, and
  `review/candidate/compilation.rs`'s `compile_candidate_for_check` builds
  the shared accumulator unconditionally (`shared_timings:
  CompileTimings::default()` in `checking.rs` is the direct-route analogue —
  `CompileTimings::enabled()` when `request.timings`). Closing this leg is a
  request-field + pass-through on the manager request structs and the
  candidate-compile helper, plus a `--timings`-equivalent surface on the
  prepared route's caller.

## Verified at

`origin/main` (re-verified this leg, linux x86-64):
`terminal-production/src/stage_timings.rs` (carrier + 9 stages),
`terminal_artifact.rs` (`terminal_production_stage_meta`,
`merge_terminal_production_timings`), `native_product.rs:69`,
`input_reuse.rs:46-50`, `compile_report.rs::with_timings`,
`cli/compilation.rs` `--timings` print, and the absence of a timings field on
`PreparedLocalProjectCheckRequest`/`PreparedLocalProjectNativeRequest`.
