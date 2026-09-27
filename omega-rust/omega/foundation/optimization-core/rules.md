# Exact optimization rules

This inventory records the owning selection and stage catalogs; it is not an
automatically checked release receipt.
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
| `SelectedIncomingLiteralExtensionElimination` | SelectedLowering | Target-independent | Experimental | `--disable-optimization SelectedIncomingLiteralExtensionElimination` | Required |
| `SelectedIncomingU12Load8IndexedOffset` | SelectedLowering | Target-independent | Experimental | `--disable-optimization SelectedIncomingU12Load8IndexedOffset` | Required |
| `SelectedIncomingLiteralCopyMaterialization` | SelectedLowering | Target-independent | Experimental | `--disable-optimization SelectedIncomingLiteralCopyMaterialization` | Required |
| `SelectedIncomingU12ByteViewAddressOffset` | SelectedLowering | Target-independent | Experimental | `--disable-optimization SelectedIncomingU12ByteViewAddressOffset` | Required |
| `SelectedIncomingExactDivideIdentityCopy` | SelectedLowering | Target-independent | Experimental | `--disable-optimization SelectedIncomingExactDivideIdentityCopy` | Required |
| `SelectedIncomingWrappingRemainderOneZeroMaterialization` | SelectedLowering | Target-independent | Experimental | `--disable-optimization SelectedIncomingWrappingRemainderOneZeroMaterialization` | Required |
| `SelectedIncomingBitwiseAndZeroMaterialization` | SelectedLowering | Target-independent | Experimental | `--disable-optimization SelectedIncomingBitwiseAndZeroMaterialization` | Required |
| `SelectedIncomingBitwiseXorZeroIdentityCopy` | SelectedLowering | Target-independent | Experimental | `--disable-optimization SelectedIncomingBitwiseXorZeroIdentityCopy` | Required |
| `SelectedIncomingWrappingAddZeroIdentityCopy` | SelectedLowering | Target-independent | Experimental | `--disable-optimization SelectedIncomingWrappingAddZeroIdentityCopy` | Required |
| `SelectedIncomingBitwiseAndOnesIdentityCopy` | SelectedLowering | Target-independent | Experimental | `--disable-optimization SelectedIncomingBitwiseAndOnesIdentityCopy` | Required |
| `SelectedIncomingWrappingRemainderZeroDividendZeroMaterialization` | SelectedLowering | Target-independent | Experimental | `--disable-optimization SelectedIncomingWrappingRemainderZeroDividendZeroMaterialization` | Required |
| `SelectedIncomingExactDivideZeroDividendZeroMaterialization` | SelectedLowering | Target-independent | Experimental | `--disable-optimization SelectedIncomingExactDivideZeroDividendZeroMaterialization` | Required |
| `SelectedIncomingSaturatingAddZeroIdentityCopy` | SelectedLowering | Target-independent | Experimental | `--disable-optimization SelectedIncomingSaturatingAddZeroIdentityCopy` | Required |
| `SelectedIncomingSaturatingSubtractZeroIdentityCopy` | SelectedLowering | Target-independent | Experimental | `--disable-optimization SelectedIncomingSaturatingSubtractZeroIdentityCopy` | Required |
| `SelectedIncomingSaturatingDivideOneIdentityCopy` | SelectedLowering | Target-independent | Experimental | `--disable-optimization SelectedIncomingSaturatingDivideOneIdentityCopy` | Required |
| `SelectedIncomingSaturatingDivideZeroDividendZeroMaterialization` | SelectedLowering | Target-independent | Experimental | `--disable-optimization SelectedIncomingSaturatingDivideZeroDividendZeroMaterialization` | Required |
| `SelectedIncomingSaturatingSubtractZeroMinuendZeroMaterialization` | SelectedLowering | Target-independent | Experimental | `--disable-optimization SelectedIncomingSaturatingSubtractZeroMinuendZeroMaterialization` | Required |
| `SelectedIncomingSaturatingAddUpperBoundMaterialization` | SelectedLowering | Target-independent | Experimental | `--disable-optimization SelectedIncomingSaturatingAddUpperBoundMaterialization` | Required |
| `SelectedIncomingWrappingRemainderMinusOneZeroMaterialization` | SelectedLowering | Target-independent | Experimental | `--disable-optimization SelectedIncomingWrappingRemainderMinusOneZeroMaterialization` | Required |
| `SelectedIncomingSaturatingSubtractUpperBoundSubtrahendZeroMaterialization` | SelectedLowering | Target-independent | Experimental | `--disable-optimization SelectedIncomingSaturatingSubtractUpperBoundSubtrahendZeroMaterialization` | Required |
| `CheckedTreeProductPruning` | CheckedTrees | Target-independent | Experimental | `--disable-optimization CheckedTreeProductPruning` | Required |
| `StateSpecialization` | Psi | Target-independent | Experimental | `--disable-optimization StateSpecialization` | Required |
| `RepresentationSpecialization` | Psi | Target-independent | Experimental | `--disable-optimization RepresentationSpecialization` | Required |
| `LoopInvariantScalarMotion` | Psi | Target-independent | Experimental | `--disable-optimization LoopInvariantScalarMotion` | Required |
| `SelectedSameBlockCopyI64RemovalV1` | PreAllocation | Target-independent | Experimental | `--disable-optimization SelectedSameBlockCopyI64RemovalV1` | Required |
| `SelectedRedundantExtensionRemovalV1` | PreAllocation | Target-independent | Experimental | `--disable-optimization SelectedRedundantExtensionRemovalV1` | Required |
| `SelectedAddressOffsetFoldV1` | PreAllocation | Target-independent | Experimental | `--disable-optimization SelectedAddressOffsetFoldV1` | Required |
| `SelectedConstantBooleanFoldV1` | PreAllocation | Target-independent | Experimental | `--disable-optimization SelectedConstantBooleanFoldV1` | Required |
| `SelectedConstantBranchFoldV1` | PreAllocation | Target-independent | Experimental | `--disable-optimization SelectedConstantBranchFoldV1` | Required |
| `SelectedDeadCompareRemovalV1` | PreAllocation | Target-independent | Experimental | `--disable-optimization SelectedDeadCompareRemovalV1` | Required |
| `SelectedRedundantCompareRemovalV1` | PreAllocation | Target-independent | Experimental | `--disable-optimization SelectedRedundantCompareRemovalV1` | Required |
| `SelectedEquivalentCompareRemovalV1` | PreAllocation | Target-independent | Experimental | `--disable-optimization SelectedEquivalentCompareRemovalV1` | Required |
<!-- exact-rule-inventory:end -->

Exact names and phases come from the source vocabulary and applicability from
the rule-owning stage catalogs. Retired post-allocation rewrite spellings are
not rollout entries. The former architecture inventory gate is absent; changes
to this table must be reviewed against those source owners rather than treated
as mechanically verified.

## Supported composition policy

- The source vocabulary names eight Psi suites; the executable
  `lowered-psi-to-lowered-psi` phase runs every member through
  `run_psi_optimization` over the complete validated carrier.
- One checked-tree product-pruning rule may follow authored checking when the
  compilation binds an exact product root; it prunes unreachable machine
  declarations and retains the identity-bearing product selection evidence.
  A checked-tree selection without a bound product root rejects before
  mutation.
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

Changing a row from `Experimental` to `Recommended` or `Default` requires the
owner decision and evidence specified by the public promotion contract.
Completed records belong in [promotions/](promotions/README.md); unfinished
investigation belongs in its linked draft. Markdown presence alone establishes
neither successful validation nor approval.
