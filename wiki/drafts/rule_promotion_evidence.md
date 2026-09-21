# RULE-PROMOTION-EVIDENCE — landed-state record

Claimed slice: `wiki/drafts/rule_promotion_evidence.md` (record), verified at
base `75650d2e94` on linux x86-64.

## Scope verification

The item re-mines WORKSPACE-ROLLOUT's exact-rule promotion territory
(TASKS_OPTIMIZER.md): the six staged records in
`omega-rust/omega/representations/optimization-core/promotions/` — one per
Psi-phase exact rule in the checked inventory `optimization-core/rules.md` —
must each carry the promotion contract's full evidence set
(`wiki/spec/build/optimizations.md#release-rollback-and-promotion`) before any
`Approved status` completes. The architecture gate
`exact_rule_rollout_is_complete_and_promotion_gated`
(`tests/architecture/optimizer_rollout/`) requires every schema label exactly
once, resolves every backticked citation, and requires each completed evidence
field to cite at least one repository artifact.

State at verification:

- `Semantic and corruption`, `Differential`, `Determinism and bounded-work`,
  `Target matrix`, and `Rollback` evidence were already complete on all six
  records; the `Rollback evidence` rejoin legs (PROMOTION-ROLLBACK-REJOIN-LEGS)
  had landed and no live claim remained.
- `Measurement evidence` was `PENDING` on all six. The recorded blocker — the
  BENCHMARKS native-realization failure — was lifted at `ff782bdf21` (the
  `cli_mvp` linux_x86_64 compile leg publishes native output again), so the
  leg became implementable on this host.
- `Approved status`, `Owner approval`, and the frozen workspace gate
  (`CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 mbx test --workspace
  --no-fail-fast`) remain owner/product decisions, not implementable slices.

## Landed change

`samples/cli/arithmetic/wrapping_square_sum/build.omg` now enables the complete
six-rule Psi selection through `builder.optimizations.enable(...)`, so the
dependency-free benchmark subject compiles under the promoted-cohort selection
and each rule's `--disable-optimization` row isolates its marginal cost. Seven
versioned records under `tools/benchmark/records/`, all measured on linux
x86_64 (3 compile + 5 runtime samples each):

| row | file | selection |
|---|---|---|
| baseline | `wrapping_square_sum__linux_x86_64__sel-44c60ac57c66.json` | all six enabled |
| ControlFlowCleanup off | `wrapping_square_sum__linux_x86_64__sel-bacb0af6ca52.json` | enabled six, disabled `ControlFlowCleanup` |
| CopyPropagation off | `wrapping_square_sum__linux_x86_64__sel-0b35b22c4221.json` | enabled six, disabled `CopyPropagation` |
| DeadPureScalarElimination off | `wrapping_square_sum__linux_x86_64__sel-2fe10c790188.json` | enabled six, disabled `DeadPureScalarElimination` |
| GlobalValueNumbering off | `wrapping_square_sum__linux_x86_64__sel-b936e1ff6607.json` | enabled six, disabled `GlobalValueNumbering` |
| ProofCheckElision off | `wrapping_square_sum__linux_x86_64__sel-2c860c734e64.json` | enabled six, disabled `ProofCheckElision` |
| SparseConditionalConstantPropagation off | `wrapping_square_sum__linux_x86_64__sel-b5ab8878609b.json` | enabled six, disabled `SparseConditionalConstantPropagation` |

Each promotion record's `Measurement evidence` now cites its rule's disabled
row beside the all-enabled baseline, satisfying the field's citation contract
with measured compile-time, peak-memory, code-size, and runtime legs.
`Approved status` and `Owner approval` stay `PENDING` by design — the gate
rejects a completed approval while the inventory rows remain `Experimental`.

The benchmark host-row matrix in `wiki/drafts/benchmarks.md` was regenerated
by `tools/benchmark/benchmark.py matrix` to cover the new rows (the guard test
`test_doc_embeds_the_current_matrix` requires it).

## Residual

- `Owner approval` and `Approved status`: owner review decisions; the inventory
  rows stay `Experimental` until then.
- The frozen workspace gate cannot pass while `canary_suite` and the
  known-baseline failures stay red — a product-level acceptance, not a slice.
- Other-host measurement rows (windows_x86_64, macos_arm64, linux_arm64,
  uefi_x86_64) remain open under BENCHMARKS row authorship; this host provides
  linux x86-64 only.
