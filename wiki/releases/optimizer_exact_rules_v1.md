# Optimizer Exact-Rule Release Notes V1

This is the published inventory for the first exact-name optimizer rollout.
It is governed by the [optimizer rollout brief](../design_briefs/optimizer/rollout.md).
Every row is explicit and opt-in, and every V1 row is currently experimental.
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
<!-- exact-rule-inventory:end -->

The architecture test derives exact names and phases from `Optimization::ALL`'s
owning source vocabulary and applicability from each surviving rule-owning stage catalog.
Retired post-allocation machine rewrite spellings are rejected by native realization
and are not rollout entries.
A missing, duplicate, renamed, rephased, retargeted, or broad alias row fails
the repository gate.

## Supported composition policy

- Any explicit subset of the six Psi suites may run in canonical phase order.
- One allocation-recovery rule may precede the canonical frame and layout stages.
- x86-64 branch relaxation runs in the resolved-layout phase.
- Selected-lowering and retired post-allocation machine rewrite selections are
  rejected by the native pipeline. Remaining selected-lowering catalog entries
  describe isolated rule support, not native publication support.
- Psi selections do not choose another physical realization route.

Unsupported or wrong-target compositions fail before physical optimization;
they never silently drop a selected rule.

Catalog presence records implemented rule support, not universal program-shape
support. Every row remains experimental, and native realization fails closed
when the selected carrier, target, or exact composition is unsupported.

## Release rollback procedure

Follow the [exact-rule rollback runbook](optimizer_rollback.md). It keeps the
authored `build.omg` selection unchanged, applies one repeatable native-build
argument per affected exact row, captures the printed requested/applied/
effective receipt, and defines verification and restoration steps.

## Promotion rule

Changing any row from `Experimental` to `Recommended` or `Default` requires a
completed owner-reviewed record at
`optimizer_promotions/<ExactRuleName>.md`. The architecture gate rejects the
status change unless that record names the exact rule and status and supplies
semantic/corruption, differential, deterministic bounded-work, supported
target, measurement, owner-approval, and exact rollback evidence.
