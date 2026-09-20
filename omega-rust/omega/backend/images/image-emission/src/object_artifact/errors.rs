//! Every way object-artifact construction can fail.

use semantic_vocabulary::MachineId;
use target_operations::CallSiteOwner;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectError {
    EmptyPlan,
    DynamicConformanceCommitmentCollision,
    DynamicConformanceDataSizeOverflow,
    InvalidDynamicConformanceTable,
    InvalidForwardedDynamicDescriptorEvidence {
        caller: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    InvalidDynamicParameterCallEvidence {
        caller: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    InvalidForwardedDynamicParameterCallEvidence {
        caller: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    ForwardedDynamicDescriptorCommitmentCollision,
    DuplicateForwardedDynamicDescriptorAdapter,
    InvalidForwardedDynamicDescriptorTable,
    UnknownDynamicConformanceTarget(MachineId),
    NonCanonicalFunctionOrder {
        previous: MachineId,
        current: MachineId,
    },
    EmptyFunction(MachineId),
    InvalidPrivateFunctionIdentity,
    EmptyPrivateFunctionSymbol,
    PrivateFunctionSymbolCollision,
    InvalidPrivateFunctionAbi,
    InvalidPrivateFunctionBody,
    UnsupportedPrivateFunctionBody,
    MissingX86ScalarFmaFragment,
    InvalidX86ScalarFmaProviderAdmission,
    MissingX86ScalarFmaProfile(MachineId),
    X86ScalarFmaUnsupportedTarget(MachineId),
    NonCanonicalX86ScalarFmaOrder(MachineId),
    InvalidX86ScalarFmaInterval {
        machine: MachineId,
        offset: usize,
    },
    InvalidX86ScalarFmaEncoding {
        machine: MachineId,
        offset: usize,
    },
    InvalidX86ScalarFmaCustody {
        machine: MachineId,
        offset: usize,
    },
    InvalidX86ScalarFmaSemanticCustody(MachineId),
    InvalidX86ScalarFmaFloatingControl(MachineId),
    MissingX86ScalarFmaCustody {
        machine: MachineId,
        offset: usize,
    },
    NonCanonicalInternalCallOrder(MachineId),
    NonCanonicalForeignCallOrder(MachineId),
    NonCanonicalSemanticCodeAttributionOrder(MachineId),
    SemanticCodeAttributionOutsideFunction(MachineId),
    InvalidSemanticCodeAttribution(MachineId),
    NonCanonicalPortEffectOrder(MachineId),
    NonCanonicalBoundarySettlementOrder(MachineId),
    InvalidStructuralReturnEvidence(MachineId),
    StructuralReturnEvidenceConflict(MachineId),
    StructuralReturnBytesMismatch(MachineId),
    UnknownInternalCallTarget {
        caller: MachineId,
        target: MachineId,
    },
    InvalidInternalCallSite {
        caller: MachineId,
        owner: CallSiteOwner,
        offset: usize,
    },
    InvalidForeignCallSite {
        caller: MachineId,
        owner: CallSiteOwner,
        offset: usize,
    },
    InvalidForeignCallArgument {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    InvalidDynamicCallEvidence {
        caller: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    InvalidUnitDynamicDescriptorJoin(MachineId),
    InvalidCallbackAddressCustody {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    MissingCallbackPrivateFunction {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    InvalidForeignCallFloatingControl {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    ForeignCallOwnerNotInProvenance {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    DuplicateForeignCallOwner {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    ForeignCallTargetMismatch {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    ForeignStackProviderPlanMismatch {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    UnsupportedForeignStackAlignment {
        caller: MachineId,
        owner: CallSiteOwner,
        admitted_alignment: u64,
        physical_alignment: u32,
    },
    ForeignCallOverlapsInternalCall {
        caller: MachineId,
        offset: usize,
    },
    ForeignLocatorIdentityCollision {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    MissingForeignImportSymbol {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    InvalidInternalUnitCallEvidence(MachineId),
    InvalidInternalUnitScalarCallEvidence(MachineId),
    InvalidUnitScalarFunctionAbi(MachineId),
    InvalidInstalledProviderUnitScalarCallEvidence(MachineId),
    InvalidUnitWriteOnlyPrimitiveStoreEvidence(MachineId),
    InvalidUnitStructuralScalarFieldStoreEvidence(MachineId),
    InvalidScalarStructuralScalarFieldStoreEvidence(MachineId),
    InvalidUnitAffineCleanupEvidence(MachineId),
    InternalCallOperationNotInProvenance {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    DuplicateInternalCallOperation {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    MissingUnitCallStackEvidence {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    UnexpectedUnitCallStackEvidence {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    MissingScalarCallStackEvidence {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    UnexpectedScalarCallStackEvidence {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    InvalidUnitStackAlignment {
        machine: MachineId,
        alignment: u32,
    },
    ConflictingTerminalStackEvidence(MachineId),
    InvalidScalarStackAlignment {
        machine: MachineId,
        alignment: u32,
    },
    InvalidScalarConditionalEvidence {
        machine: MachineId,
        offset: usize,
    },
    ScalarConditionalCallOutsideArm {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
        offset: usize,
    },
    UntypedScalarInternalCall {
        machine: MachineId,
        offset: usize,
    },
    InvalidScalarCallStackEvidence {
        caller: MachineId,
        owner: CallSiteOwner,
        offset: usize,
    },
    MisalignedScalarCalleeEntry {
        caller: MachineId,
        owner: CallSiteOwner,
        caller_live_bytes: u32,
    },
    NonCanonicalScalarStackMutationOrder(MachineId),
    InvalidScalarInstructionEncoding {
        machine: MachineId,
        offset: usize,
    },
    NonLinearScalarControlFlow {
        machine: MachineId,
        offset: usize,
    },
    UnclaimedScalarStackMutation {
        machine: MachineId,
        offset: usize,
    },
    UnsupportedScalarStackMutation {
        machine: MachineId,
        offset: usize,
    },
    InvalidScalarStackEvidence {
        machine: MachineId,
        offset: usize,
    },
    MissingBalancedScalarReturn(MachineId),
    ScalarStackArithmeticOverflow(MachineId),
    ScalarStackReleaseExceedsAllocation {
        machine: MachineId,
        offset: usize,
    },
    UnitCallStackArithmeticOverflow {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    InvalidUnitStackEncoding {
        machine: MachineId,
        owner: Option<CallSiteOwner>,
        offset: usize,
    },
    InvalidUnitInstructionEncoding {
        machine: MachineId,
        offset: usize,
    },
    DuplicateUnitStackAdjustment(MachineId),
    UnclaimedUnitStackAdjustment {
        machine: MachineId,
        offset: usize,
    },
    UnclaimedUnitStackMutation {
        machine: MachineId,
        offset: usize,
    },
    MisalignedUnitCalleeEntry {
        caller: MachineId,
        owner: CallSiteOwner,
        caller_live_bytes: u32,
    },
    MissingX86UnitCallStackAdjustment {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    MissingAarch64UnitReturnLink {
        caller: MachineId,
        operation: Option<semantic_vocabulary::OperationId>,
    },
    UnaccountedTerminalStack(MachineId),
    TerminalStackCycle(MachineId),
    TerminalStackCompositionOverflow {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    PortEffectOutsideFunction {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    PortEffectOperationNotInProvenance {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    DuplicatePortEffectOperation {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    PortEffectBytesMismatch {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    BoundarySettlementOutsideFunction {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    BoundarySettlementOperationNotInProvenance {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    DuplicateBoundarySettlementOperation {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    InvalidBoundarySettlementArgumentPath {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    InvalidCompletionReceiptArgumentIndex {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    InvalidCompletionReceiptCustody {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    InvalidCompletionProviderCustody {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    BoundaryRealizationMismatch {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    EntryFunctionMissing(MachineId),
    TextSizeOverflow,
}

impl std::fmt::Display for ObjectError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ObjectError {}
