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
| `Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1` | PostAllocationMachine | AArch64 | Experimental | `--disable-optimization Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1` | Required |
| `SharedEntryFixedViewCopyAfterCompareBeforeBranchV1` | AllocationRecovery | Target-independent | Experimental | `--disable-optimization SharedEntryFixedViewCopyAfterCompareBeforeBranchV1` | Required |
| `ActiveResidentImmediateU64MultiUseRematerializationV1` | AllocationRecovery | Target-independent | Experimental | `--disable-optimization ActiveResidentImmediateU64MultiUseRematerializationV1` | Required |
| `Aarch64SelectShortestMovnSeededI64MaterializationV1` | PostAllocationMachine | AArch64 | Experimental | `--disable-optimization Aarch64SelectShortestMovnSeededI64MaterializationV1` | Required |
| `X86SelectXorZeroI64MaterializationV1` | PostAllocationMachine | x86-64 | Experimental | `--disable-optimization X86SelectXorZeroI64MaterializationV1` | Required |
| `X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1` | PostAllocationMachine | x86-64 | Experimental | `--disable-optimization X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1` | Required |
| `X86SelectMovR64Imm32SignExtendedI64MaterializationV1` | PostAllocationMachine | x86-64 | Experimental | `--disable-optimization X86SelectMovR64Imm32SignExtendedI64MaterializationV1` | Required |
<!-- exact-rule-inventory:end -->

The architecture test derives exact names and phases from `Optimization::ALL`'s
owning source vocabulary and applicability from each rule-owning stage catalog.
A missing, duplicate, renamed, rephased, retargeted, or broad alias row fails
the repository gate.

## Supported composition policy

- Any explicit subset of the six Psi suites may run in canonical phase order.
- Both selected-lowering rules may be selected together.
- Selected lowering may precede one target-compatible post-allocation rule, or
  x86-64 function-relative branch relaxation.
- One allocation-recovery rule may run alone; active-resident immediate-U64
  multi-use rematerialization may also precede AArch64 MOVN materialization or
  x86 XOR-zero or either exact x86 imm32 materialization rule on its matching
  target.
- One post-allocation machine rule may run at a time and cannot compose with
  function-relative layout.
- Psi selections are orthogonal overlays and do not alter physical-route
  admission.

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
