//! The checked-program root and its concept owners.
//!
//! `CheckedTrees` pairs the typed program with `CheckFacts`. The facts retain
//! borrowing, control flow, proof, value, and operator judgments; they are not
//! a history of the validation passes that established them.

pub use typed_trees::byte_predicates;
pub use typed_trees::ranking;
pub use typed_trees::typed_trees::ClosedConformanceConstArgument;
pub use typed_trees::{
    data, domain, expression, finite_family, identity, machine, name, proof_only, proposition,
    signature, state, trait_definition, types, wire,
};

pub mod admissibility;
pub mod borrow;
pub mod facts;
pub mod flow;
pub mod operators;
pub mod proof;
pub mod service_parameter;
pub mod statement;
pub mod values;

pub use admissibility::{
    AcceptanceCheck, AcceptanceCheckProvenance, AcceptanceCheckVerdict, AcceptanceDimension,
    AcceptanceSummary, AcceptanceVerdict, AcceptanceView, CallAcceptance, ExitAcceptance,
    OperatorAcceptance, StateAcceptance, StateOperationAcceptance, StateOperationAcceptanceKind,
    StatementAcceptance,
};
pub use borrow::{
    BorrowAccessKind, BorrowArgumentAccessFact, BorrowCallCompatibilityOperand,
    BorrowCallCompatibilitySubject, BorrowCallFact, BorrowCompatibilityConclusion,
    BorrowCompatibilityDerivation, BorrowCompatibilityFormation, BorrowCompatibilityPlaceSide,
    BorrowCompatibilityPremise, BorrowCompatibilityPremiseRelation,
    BorrowCompatibilityPremiseSource, BorrowCompatibilitySelectorPosition,
    BorrowCompatibilitySelectorSnapshot, BorrowCompatibilitySelectorValue, BorrowFacts,
    BorrowLoanFact, BorrowLoanLineage, BorrowLoanOwnerSegment, BorrowRootKind,
    BorrowWritableRootFact, CapturedPlace, CapturedPlaceCompatibility, CapturedPlaceContainment,
    CheckedBorrowCallCompatibilityCertificate, CheckedBorrowCompatibilityCertificate,
    CheckedBorrowMutationCertificate, CheckedBorrowResourceDispositionTarget,
    CheckedBorrowResourceLifecyclePhase, CheckedDirectBorrowLoanResource,
    CheckedDirectBorrowParentLifetime, CheckedDirectBorrowRestorationObligation,
    CheckedParentBorrowResource, CheckedReborrowAccessEffect,
    CheckedReborrowContainmentCertificate, CheckedReborrowContainmentKind,
    CheckedReborrowLoanResource, CheckedReborrowParentEndStatus,
    CheckedReborrowParentSuspensionBoundary, CheckedReborrowResourceDisposition,
    CheckedReborrowResourceDispositionEvent, CheckedReborrowRestorationObligation,
    CheckedReborrowRestoredCallUseCertificate, CheckedRetiredParentResourceDispositionStep,
    ParentLexicalStatusAtChildEnd, StateBorrowFact,
};
pub use facts::{
    BlockingFacts, BuildBoundProgressDemand, CallServiceReachRows, CarryFacts, CheckFacts,
    CheckedBoundaryAdapterDispatch, CheckedCallbackPlacementIdentity,
    CheckedCallbackResourceReceipt, CheckedCrashCallSite, CheckedCrashOperatorSite,
    CheckedCrashSite, CheckedEntryResourceEnvelope, CheckedFactCallProjection,
    CheckedIntrinsicCallFact, CheckedMachineContractEnvelopeIdentity,
    CheckedMachineContractRefinement, CheckedMachineResourceEnvelopes, CheckedNominalMachineUse,
    CheckedPlacedViewInput, CheckedProofRankingRelation, CheckedProofRecursiveCallSite,
    CheckedProofRecursiveComponent, CheckedProofRecursiveEdge, CheckedProofRecursiveMember,
    CheckedProofRecursiveTransitionLane, CheckedRequirementCallMachineSelection,
    CheckedRequirementCallSpecialization, CheckedRequirementCallTypeBinding,
    CheckedResourceAxisAnchor, CheckedResourceDerivationObligation, ClaimCarryPolicyFact,
    ClosedFloatRangeRequirement, ClosedIntegerRangeRequirement, ClosedScalarContractValue,
    ClosedScalarValueContractPlan, ContainedMachineFieldFact, ContainedMachineTargetFact,
    ContentIdentityReshuffleFact, ContentPartitionCompositionFact,
    ContentPartitionInputClaimBinding, ContentPartitionPlaceSubstitution,
    ContentPartitionResultRewrite, ContentProjectionFacts, CrashCallSiteLocation, CrashCause,
    CrashContractCapsule, CrashInterface, CrashPlan, CrashPredicateExpression,
    CrashPredicateIdentity, CrashRouteBucket, CrashRouteBucketId, CrashRouteGuard,
    CrashSiteLocation, DataCarryFact, DomainDependencyFact, DomainDependencyPathFact,
    DomainElementFact, DomainFacts, DynamicConformanceBindingFact, DynamicConformanceBindingFacts,
    DynamicConformanceCandidateFact, DynamicConformanceFacts, DynamicConformanceRowFact,
    DynamicConformanceRowSource, DynamicConformanceSelectionFact, DynamicDescriptorStorageFact,
    IndexCompatibilityDischarge, IndexCompatibilityFact, IndexCompatibilityFacts,
    MachineActivationCarryFact, MachineBlockingFact, MachineBuildBoundProgressDemands,
    MachineCarryTopologyFact, MachineContractCommitment, MachineContractIdentity,
    MachineContractPlan, MachineContractPlans, MachineMutationFact, MachineQualifications,
    MachineServiceReachRows, MachineSuspensionFact, MachineSynchronousInvocationFact,
    MachineTerminationFact, MutationFacts, NominalMachineUseFacts, NominalMachineUseSite,
    OperatorSiteLocation, ProgressDemandCallSite, QualificationFacts,
    RealizedMachineContractEnvelope, RequirementCallSpecializationFacts, RetainedBorrowCustodyFact,
    ServiceReachFacts, StateServiceReachRows, StateWriteFramePlan, SuspensionCrossingCarryFact,
    SuspensionCrossingLiveValueFact, SuspensionCrossingStorage, SuspensionCrossingValueOrigin,
    SuspensionFacts, SynchronousInvocationFacts, TerminationFacts, VacuousQualificationUse,
    canonical_suspension_crossing_id, contract_identity, contract_report_fingerprint,
};
pub use flow::{
    CheckedAffineConstructionElementPlan, CheckedBoundaryMachinePlan,
    CheckedBoundaryMachineResultPlan, CheckedBoundaryScalarReturnMachinePlan,
    CheckedBoundaryScalarReturnPlans, CheckedByteSequenceCarrier, CheckedByteSequenceStoreValue,
    CheckedByteSequenceWritePlan, CheckedCallScalarArgument,
    CheckedClaimFreeAffineStructuralReturnMachinePlan, CheckedClosedSumCaseSuccessorPlan,
    CheckedClosedSumPayloadTransferPlan, CheckedComposedUnitControlMachinePlan,
    CheckedComposedUnitControlStatePlan, CheckedComposedUnitControlTerminatorPlan,
    CheckedControlResultPlan, CheckedDynamicDescriptorTransferEdge,
    CheckedDynamicDescriptorTransferPath, CheckedDynamicDescriptorTransferPlan,
    CheckedDynamicDescriptorTransferSource, CheckedDynamicDispatchPlans,
    CheckedDynamicRealizationBodyPlan, CheckedDynamicRealizationCallablePlan,
    CheckedDynamicScalarCallOrigin, CheckedDynamicScalarCallPlan, CheckedDynamicScalarHelperPlan,
    CheckedDynamicSelectionPlan, CheckedDynamicUnitCallOrigin, CheckedDynamicUnitCallPlan,
    CheckedDynamicUnitContinuationPlan, CheckedFusedServiceErasureReceipt,
    CheckedFusedServiceParameterReceipt, CheckedGuardedJumpPlan,
    CheckedJoinedDynamicScalarCallBranchPlan, CheckedJoinedDynamicScalarCallPlan,
    CheckedJoinedDynamicUnitCallBranchPlan, CheckedJoinedDynamicUnitCallPlan,
    CheckedNaturalRankMeasure, CheckedNominalAffineUnitCleanupMachinePlan,
    CheckedNominalAffineUnitCleanupPlans, CheckedPartialAffineUnitCleanupMachinePlan,
    CheckedPartialAffineUnitCleanupPlans, CheckedPayloadlessCaseReturnMachinePlan,
    CheckedPayloadlessGuardedCallEvidencePlan, CheckedPayloadlessGuardedCallEvidenceUsePlan,
    CheckedPayloadlessGuardedCallReturnMachinePlan, CheckedPrimitiveStoreDestination,
    CheckedProviderAttachmentRequirementPlan, CheckedReboundDynamicScalarCallPlan,
    CheckedReboundDynamicUnitCallPlan, CheckedReferenceResultSourcePlan, CheckedScalarBinding,
    CheckedScalarBindingDestination, CheckedScalarBindingValue, CheckedScalarBranchDestination,
    CheckedScalarCaseFieldPlan, CheckedScalarGraphPlans, CheckedScalarGuardedExit,
    CheckedScalarGuardedTail, CheckedScalarMachineGraph, CheckedScalarParameterStorage,
    CheckedScalarPrimitiveLocalPlan, CheckedScalarStateGraph, CheckedScalarStateTerminator,
    CheckedScalarSuccessor, CheckedSelectedOperatorStructuralScalarReturnMachinePlan,
    CheckedSemanticDependencies, CheckedSemanticDependency, CheckedSemanticDependencyExposure,
    CheckedSemanticDependencyKind, CheckedStateNaturalRank, CheckedStoredDynamicScalarCallPlan,
    CheckedStructuralAccess, CheckedStructuralBooleanConvergencePlan,
    CheckedStructuralByteSequenceFieldByteStorePlan, CheckedStructuralByteSequenceFieldStorePlan,
    CheckedStructuralCallCustodyPlan, CheckedStructuralCallReturnPlans,
    CheckedStructuralCaseReturnPlan, CheckedStructuralControlCleanupPlans,
    CheckedStructuralControlEdgeCleanupPlan, CheckedStructuralControlProjectedEdgeCleanupPlan,
    CheckedStructuralControlProjectedTransferPlan, CheckedStructuralControlStateCleanupPlan,
    CheckedStructuralControlSuccessorPlan, CheckedStructuralControlTransferPlan,
    CheckedStructuralControlTransferSourcePlan, CheckedStructuralPathQualification,
    CheckedStructuralRankedArgumentPlan, CheckedStructuralRankedGuardPlan,
    CheckedStructuralRankedSccEdgePlan, CheckedStructuralRankedSccPlan,
    CheckedStructuralResultPlan, CheckedStructuralReturnMachinePlan, CheckedStructuralReturnPlans,
    CheckedStructuralReturnedClaimTransferPlan, CheckedStructuralScalarArgumentPlan,
    CheckedStructuralScalarArgumentSourcePlan, CheckedStructuralScalarFieldStoreDestination,
    CheckedStructuralScalarFieldStorePlan, CheckedStructuralScalarFieldStoreValue,
    CheckedStructuralScalarIntegerBoundKind, CheckedStructuralScalarIntegerBoundPlan,
    CheckedStructuralScalarIntegerBoundRequirementPlan, CheckedStructuralScalarParameterPlan,
    CheckedStructuralScalarReturnCleanupAction, CheckedStructuralScalarReturnMachinePlan,
    CheckedStructuralScalarReturnPlans, CheckedStructuralUnitControlMachinePlan,
    CheckedStructuralUnitControlPlans, CheckedStructuralUnitControlStatePlan,
    CheckedStructuralUnitControlTerminatorPlan, CheckedStructuralValueCall,
    CheckedTerminalDebugPlans, CheckedTerminalMachineDebugPlan, CheckedTerminalMachineSelection,
    CheckedTerminalMachineSelections, CheckedTerminalSignatureEligibility,
    CheckedTerminalStateDebugPlan, CheckedTraitOperatorScalarReturnMachinePlan,
    CheckedTrivialAffineStructuralLocalPlan, CheckedUnitCallCoordinate,
    CheckedUnitClaimTransferPlan, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
    CheckedUnitEffectPlans, CheckedUnitEntryClaimPlan,
    CheckedUnitNominalAffineCallerRequirementPlan, CheckedUnitNominalAffineCleanupPlan,
    CheckedUnitNominalAffineCleanupRequirementPlan, CheckedUnitPartialAffineDiscardPlan,
    CheckedUnitPlanOmission, CheckedUnitPlanOmissionStage, CheckedUnitScalarControlPlan,
    CheckedUnitScalarResultBindingPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralCasePlan,
    CheckedUnitStructuralDomainPlan, CheckedUnitStructuralDomainRequirementPlan,
    CheckedUnitStructuralFieldPlan, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment,
    CheckedUnitStructuralResultBindingPlan, CheckedUnitStructuralReturnPlan,
    CheckedUnitStructuralTypePlan, CheckedUnitStructuralTypeShape, FlowBorrowActivationFact,
    FlowBorrowLifetimeFacts, FlowBorrowWeakeningFact, FlowBorrowWeakeningReason,
    FlowBoundaryEdgeFact, FlowBoundaryFacts, FlowCallFact, FlowClaimJoinAlternative,
    FlowClaimJoinAlternativeSource, FlowClaimJoinExit, FlowClaimJoinExitKind, FlowClaimJoinReceipt,
    FlowClaimOutcomeEntryFact, FlowClaimOutcomeMapFact, FlowClaimOutcomeSource, FlowConstraintKind,
    FlowConstraintRef, FlowContextFacts, FlowControlFacts, FlowExitFact, FlowExitParameterOrigin,
    FlowFacts, FlowInvalidationFact, FlowInvalidationFacts, FlowInvalidationSource,
    FlowOperandReferent, FlowOperatorInvocationFact, FlowOperatorOperandFact,
    FlowOwnedSelectionClaim, FlowOwnedSelectionReceipt, FlowOwnedSelectionSource,
    FlowOwnedSelectionTransfer, FlowOwnershipFacts, FlowPermissionEventFact,
    FlowSemanticContextRef, FlowStateFact, FlowStatementFact, RetiredFlowCall,
};
pub use operators::{
    CheckedArithmeticPolicyAdapter, CheckedBoundaryOperatorApplicationArgument,
    CheckedBoundaryOperatorApplicationDemand, CheckedBoundaryOperatorApplicationUseSite,
    CheckedNamedOperatorUseFact, CheckedNamedRequirementUseFact, CheckedOperatorCandidateFact,
    CheckedOperatorContractUse, CheckedOperatorCrashBucket, CheckedOperatorCrashContract,
    CheckedOperatorFacts, CheckedOperatorOccurrence, CheckedOperatorRealizationContract,
    CheckedOperatorResolutionIssue, CheckedOperatorResolutionStatus,
    CheckedOperatorResolutionSummary, CheckedOperatorUseFact, CheckedOperatorUseHandle,
    CheckedProviderPlanCommitment, CheckedSelectedFloatComparisonExecution,
    CheckedSymbolicBoundaryOperatorApplicationArgument,
    CheckedSymbolicBoundaryOperatorApplicationDemand,
};
pub use proof::{
    BoundaryQualificationAuthorization, CheckedContractEntailmentAssumptionDischarge,
    CheckedDirectBlockFloatParameter, CheckedDirectCallFloatResult,
    CheckedDirectMachineFloatParameter, CheckedDirectMachineFloatResult,
    CheckedDirectOperationFloatResult, CheckedDirectStructuralFloatLeaf,
    CheckedEvidenceInterfaceIdentity, CheckedEvidenceProjection,
    CheckedEvidenceRequirementIdentity, CheckedEvidenceTerm,
    CheckedFloatMeaningEqualityProposition, CheckedFloatMeaningProjection,
    CheckedFloatMeaningProjectionError, CheckedFloatMeaningProjectionOccurrence,
    CheckedFloatMeaningProjectionOccurrenceId, CheckedFloatProjectionContractIdentity,
    CheckedFloatProjectionInput, CheckedFloatProjectionInputId, CheckedFloatProjectionSource,
    CheckedFloatSemanticApplication, CheckedFloatSemanticApplicationError,
    CheckedFloatSemanticApplicationOperand, CheckedFloatUseSite, CheckedMathematicalBinder,
    CheckedMathematicalBinderKind, CheckedMathematicalBody, CheckedMathematicalDeclaration,
    CheckedMathematicalParameter, CheckedProofOnlyValueType, CheckedProofPropositionId,
    CheckedProofValueDeclaration, CheckedProofValueId, CheckedPropositionApplication,
    CheckedPropositionBinder, CheckedPropositionBinderArgument,
    CheckedPropositionBinderArgumentKind, CheckedPropositionBinderKind,
    CheckedPropositionDeclaration, CheckedPropositionEvidence, CheckedPropositionVocabulary,
    ContractCallFact, ContractEvidenceArgument, ContractExitFact,
    ContractExpressionEvidenceArgumentFact, ContractExpressionEvidenceCallFact,
    ContractExpressionStaticConformanceApplicationFact, ContractOperatorUseFact, ContractProofFact,
    ContractProofFactKind, ContractProofFactOwner, ContractProofFactRef, EvidenceAssignmentSource,
    EvidenceForwardingFact, InheritedContractScope, LemmaFacts, OutcomeSpecificArmFact,
    OutcomeSpecificArmRowFact, OutcomeSpecificEvidenceInterfaceScopeFact,
    OutcomeSpecificGuaranteeFact, OutcomeSpecificValidityFact, ProofFactKind, ProofFacts,
    ProofLemmaFact, ProofLemmaKind, ProofObligationFact, ProofObligationOwner, ProofOutputCallFact,
    ProofOutputEvidenceArgumentFact, ProofOutputFact, ProofOutputRuntimeCallFact,
    QuantifiedBoundFact, QuantifiedRangeFact, StaticRequirementDispatchFact,
};
pub use service_parameter::{CheckedServiceParameterCarrier, CheckedServiceParameterError};
pub use values::{
    CheckedArrayConstructionSource, CheckedBooleanExpression, CheckedErasedProofParameterPlan,
    CheckedIeeeFloatComparisonKind, CheckedIntegerBinaryKind, CheckedIntegerComparisonKind,
    CheckedIntegerRange, CheckedLocatedProofTerm, CheckedLocatedScalarExpression, CheckedProofTerm,
    CheckedProofTermField, CheckedProofTermRole, CheckedProofTerms,
    CheckedScalarCaseComputationField, CheckedScalarCaseConstruction, CheckedScalarComputation,
    CheckedScalarComputationHandle, CheckedScalarComputationKind, CheckedScalarComputationPlans,
    CheckedScalarComputationRoot, CheckedScalarComputationStructuralArgument,
    CheckedScalarDispatchArm, CheckedScalarDispatchPattern, CheckedScalarExpression,
    CheckedScalarExpressionBindings, CheckedScalarExpressionPlans, CheckedScalarExpressionRole,
    CheckedStructuralDispatchArm, CheckedStructuralParameterField,
    CheckedStructuralPredicatePathSegment, CheckedStructuralRecordField,
    CheckedStructuralRecordFieldValue, CheckedStructuralValue, CheckedStructuralValueHandle,
    CheckedStructuralValueKind, CheckedStructuralValuePlans, CheckedStructuralValueRoot,
    CheckedValueFact, CheckedValueFacts, CheckedValueHandle, CheckedValueOrigin,
    CheckedValueStatementRole,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedTrees {
    pub typed: typed_trees::TypedTrees,
    pub facts: CheckFacts,
}

impl CheckedTrees {
    pub fn with_roots(typed: typed_trees::TypedTrees, facts: CheckFacts) -> Self {
        Self { typed, facts }
    }
}

impl std::ops::Deref for CheckedTrees {
    type Target = typed_trees::TypedTrees;

    fn deref(&self) -> &Self::Target {
        &self.typed
    }
}

impl AsRef<typed_trees::TypedTrees> for CheckedTrees {
    fn as_ref(&self) -> &typed_trees::TypedTrees {
        &self.typed
    }
}

#[cfg(test)]
mod tests;
