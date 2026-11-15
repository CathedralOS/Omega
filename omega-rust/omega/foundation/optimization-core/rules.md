# Exact optimization rules

This table copies the rule names from the selection vocabulary and the stage
catalogs that own each rule; nothing checks it automatically.
The [optimization contract](../../../../wiki/spec/build/optimizations.md#release-rollback-and-promotion)
defines selection and promotion. Every rule today is explicit, opt-in and
experimental.
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
| `SelectedCopiedCallOperandFoldV1` | PreAllocation | Target-independent | Experimental | `--disable-optimization SelectedCopiedCallOperandFoldV1` | Required |
| `SelectedStoredLoadForwardingV1` | PreAllocation | Target-independent | Experimental | `--disable-optimization SelectedStoredLoadForwardingV1` | Required |
<!-- exact-rule-inventory:end -->

Names and phases come from the source vocabulary; applicability comes from the
stage catalog that owns each rule. Names of retired post-allocation rewrites
are not listed. The architecture inventory gate that once checked this table
is gone, so review any change against those sources by hand.

## Supported composition policy

- The source vocabulary names nine Psi rules. All nine run after Terminal Psi,
  in `PSI_PASS_CATALOG` in
  `01_abstract-operations-to-abstract-operations/src/rules.rs`.
- At most one allocation-recovery rule may run before the canonical frame and layout stages.
- x86-64 branch relaxation runs in the resolved-layout phase.
- The native pipeline rejects selected-lowering rules and retired
  post-allocation machine rewrites. Remaining selected-lowering catalog
  entries mean the rule works in isolation, not in native output.
- Selecting a Psi rule does not switch to a different native realization route.

An unsupported combination of rules, or a rule for the wrong target, fails
before physical optimization; a selected rule is never silently dropped.

A catalog entry means the name can be selected or the rule works in isolation,
not that it works across its whole phase or every program shape. Every row
stays experimental, and native realization refuses when the selected
representation, target or combination is unsupported.

## Release rollback procedure

Follow the [exact-rule rollback procedure](../../../optimization.md#operational-rollback). It leaves the
`build.omg` selection unchanged, passes one native-build argument per affected
row (repeat it for each), saves the printed requested/applied/effective
receipt, and says how to verify and restore.

## Promotion rule

Changing a row from `Experimental` to `Recommended` or `Default` needs the
owner's decision and the evidence the public promotion contract asks for.
Completed records go in [exact-rule promotion records](README.md#exact-rule-promotion-records); unfinished
investigation goes in its linked draft. A Markdown record alone proves neither
validation nor approval.
