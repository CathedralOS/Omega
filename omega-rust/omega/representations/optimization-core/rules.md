# Exact optimization rules

This inventory is checked against the owning selection and stage catalogs.
The [optimization contract](../../../../wiki/spec/build/optimizations.md#release-rollback-and-promotion)
defines selection and promotion. Every current row is explicit, opt-in, and experimental.
There is no debug/release bundle and no `O1`, `O2`, or `O3` alias.

## Exact rule inventory

<!-- exact-rule-inventory:start -->
| Exact rule | Phase | Applicability | Status | Rollback | Owner review |
| --- | --- | --- | --- | --- | --- |
| `ControlFlowCleanup` | Psi | Target-independent | Experimental | `--disable-optimization ControlFlowCleanup` | Required |
| `SparseConditionalConstantPropagation` | Psi | Target-independent | Experimental | `--disable-optimization SparseConditionalConstantPropagation` | Required |
| `CopyPropagation` | Psi | Target-independent | Experimental | `--disable-optimization CopyPropagation` | Required |
| `GlobalValueNumbering` | Psi | Target-independent | Experimental | `--disable-optimization GlobalValueNumbering` | Required |
| `DeadPureScalarElimination` | Psi | Target-independent | Experimental | `--disable-optimization DeadPureScalarElimination` | Required |
| `ProofCheckElision` | Psi | Target-independent | Experimental | `--disable-optimization ProofCheckElision` | Required |
| `SelectedIncomingU12ExactAddImmediate` | SelectedLowering | Target-independent | Experimental | `--disable-optimization SelectedIncomingU12ExactAddImmediate` | Required |
| `X86RelaxConditionalBranchesToRel8V1` | FunctionRelativeLayout | x86-64 | Experimental | `--disable-optimization X86RelaxConditionalBranchesToRel8V1` | Required |
| `SelectedIncomingU12ExactSubtractImmediate` | SelectedLowering | Target-independent | Experimental | `--disable-optimization SelectedIncomingU12ExactSubtractImmediate` | Required |
| `SharedEntryFixedViewCopyAfterCompareBeforeBranchV1` | AllocationRecovery | Target-independent | Experimental | `--disable-optimization SharedEntryFixedViewCopyAfterCompareBeforeBranchV1` | Required |
| `ActiveResidentImmediateU64MultiUseRematerializationV1` | AllocationRecovery | Target-independent | Experimental | `--disable-optimization ActiveResidentImmediateU64MultiUseRematerializationV1` | Required |
| `SelectedIncomingU12CompareImmediate` | SelectedLowering | Target-independent | Experimental | `--disable-optimization SelectedIncomingU12CompareImmediate` | Required |
<!-- exact-rule-inventory:end -->

The architecture test derives exact names and phases from `Optimization::ALL`'s
owning source vocabulary and applicability from each surviving rule-owning stage catalog.
Retired post-allocation machine rewrite spellings are rejected by native realization
and are not rollout entries.
A missing, duplicate, renamed, rephased, retargeted, or broad alias row fails
the repository gate.

## Supported composition policy

- The source vocabulary names six Psi suites. The current executable Psi phase
  supports `DeadPureScalarElimination`; other nonempty selections reject until
  their transformations are implemented and independently validated there.
- One allocation-recovery rule may precede the canonical frame and layout stages.
- x86-64 branch relaxation runs in the resolved-layout phase.
- Selected-lowering and retired post-allocation machine rewrite selections are
  rejected by the native pipeline. Remaining selected-lowering catalog entries
  describe isolated rule support, not native publication support.
- Psi selections do not choose another physical realization route.

Unsupported or wrong-target compositions fail before physical optimization;
they never silently drop a selected rule.

Catalog presence records selection vocabulary or isolated rule support, not
complete phase or program-shape support. Every row remains experimental, and native realization fails closed
when the selected carrier, target, or exact composition is unsupported.

## Release rollback procedure

Follow the [exact-rule rollback procedure](../../../optimization.md#operational-rollback). It keeps the
authored `build.omg` selection unchanged, applies one repeatable native-build
argument per affected exact row, captures the printed requested/applied/
effective receipt, and defines verification and restoration steps.

## Promotion rule

Changing any row from `Experimental` to `Recommended` or `Default` requires a
completed owner-reviewed record at
`promotions/<ExactRuleName>.md`. The architecture gate rejects the
status change unless that record names the exact rule and status and supplies
semantic/corruption, differential, deterministic bounded-work, supported
target, measurement, owner-approval, and exact rollback evidence.
