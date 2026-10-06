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
    BorrowAccessKind, BorrowArgumentAccessFact, BorrowCallFact, BorrowFacts, BorrowLoanFact,
    BorrowLoanLineage, BorrowLoanOwnerSegment, BorrowRootKind, BorrowStateIndex,
    BorrowWritableRootFact, CapturedPlace, CapturedPlaceCompatibility, CapturedPlaceContainment,
    CheckedBorrowResourceDispositionTarget, CheckedBorrowResourceLifecyclePhase,
    CheckedDirectBorrowLoanResource, CheckedDirectBorrowParentLifetime,
    CheckedDirectBorrowRestorationObligation, CheckedParentBorrowResource,
    CheckedReborrowAccessEffect, CheckedReborrowContainmentCertificate,
    CheckedReborrowContainmentKind, CheckedReborrowLoanResource, CheckedReborrowParentEndStatus,
    CheckedReborrowParentSuspensionBoundary, CheckedReborrowResourceDisposition,
    CheckedReborrowResourceDispositionEvent, CheckedReborrowRestorationObligation,
    CheckedReborrowRestoredCallUseCertificate, CheckedRetiredParentResourceDispositionStep,
    ParentLexicalStatusAtChildEnd, StateBorrowFact,
};
pub use facts::{
    CheckedStoredIntegerWrite, ContextualCleanupApplication, ContextualCleanupDisposition,
    ContextualCleanupEntryRequirement, ContextualCleanupEntryRequirements, ContextualCleanupFacts,
    DecidedCallRequires, StoredIntegerWriteSite,
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
    CheckedResourceAxisAnchor, CheckedResourceDerivationObligation,
    CheckedRuntimeMembershipSubject, ClaimCarryPolicyFact, ClosedFloatRangeRequirement,
    ClosedIntegerRangeRequirement, ClosedScalarContractValue, ClosedScalarValueContractPlan,
    ContentClassFacts, ContentClassRow, ContentIdentityReshuffleFact,
    ContentPartitionCompositionFact, ContentPartitionInputClaimBinding,
    ContentPartitionPlaceSubstitution, ContentPartitionResultRewrite, ContentProjectionFacts,
    CrashCallSiteLocation, CrashCause, CrashContractCapsule, CrashInterface, CrashPlan,
    CrashPredicateExpression, CrashPredicateIdentity, CrashRouteBucket, CrashRouteBucketId,
    CrashRouteGuard, CrashSiteLocation, DataCarryFact, DomainDependencyFact,
    DomainDependencyPathFact, DomainFacts, DynamicConformanceBindingFact,
    DynamicConformanceBindingFacts, DynamicConformanceCandidateFact, DynamicConformanceFacts,
    DynamicConformanceRowFact, DynamicConformanceRowSource, DynamicConformanceSelectionFact,
    DynamicDescriptorStorageFact, IndexCompatibilityDischarge, IndexCompatibilityFact,
    IndexCompatibilityFacts, MachineActivationCarryFact, MachineBlockingFact,
    MachineBuildBoundProgressDemands, MachineContractCommitment, MachineContractIdentity,
    MachineContractPlan, MachineContractPlans, MachineMutationFact, MachineQualifications,
    MachineServiceReachRows, MachineSuspensionFact, MachineSynchronousInvocationFact,
    MachineTerminationFact, MutationFacts, NominalMachineUseFacts, NominalMachineUseSite,
    OperatorSiteLocation, PerformedIntegerWrite, ProgressDemandCallSite, QualificationFacts,
    RealizedMachineContractEnvelope, ReferenceCustodyFacts, ReferenceCustodyRow,
    RequirementCallSpecializationFacts, RetainedBorrowCustodyFact, ServiceReachFacts,
    StateReferenceCustodyFacts, StateReferenceCustodyRow, StateServiceReachRows,
    StateWriteFramePlan, SuspensionCrossingCarryFact, SuspensionCrossingLiveValueFact,
    SuspensionCrossingStorage, SuspensionCrossingValueOrigin, SuspensionFacts,
    SynchronousInvocationFacts, TerminationFacts, VacuousQualificationUse,
    canonical_suspension_crossing_id, contract_identity, contract_report_fingerprint,
};
pub use flow::{
    CheckedAffineConstructionElementPlan, CheckedAtomicAccessPlan, CheckedAtomicEvent,
    CheckedAtomicReadModifyWrite, CheckedBoundaryMachinePlan, CheckedBoundaryMachineResultPlan,
    CheckedBoundaryScalarGuarantee, CheckedBoundaryScalarReturnMachinePlan,
    CheckedBoundaryScalarReturnPlans, CheckedByteSequenceCarrier,
    CheckedByteSequenceStoreDestination, CheckedByteSequenceStorePlan, CheckedByteSequenceValue,
    CheckedByteSequenceValuePlan, CheckedByteSequenceWritePlan, CheckedCallScalarArgument,
    CheckedClaimFreeAffineStructuralReturnMachinePlan, CheckedClosedSumCaseOutcomePlan,
    CheckedClosedSumCasePlan, CheckedClosedSumPayloadTransferPlan, CheckedClosedSumSubjectKind,
    CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan,
    CheckedComposedUnitControlTerminatorPlan, CheckedConditionalReturnArm,
    CheckedControlResultPlan, CheckedDynamicBinding, CheckedDynamicBindingKind,
    CheckedDynamicDescriptorTransferEdge, CheckedDynamicDescriptorTransferPath,
    CheckedDynamicDescriptorTransferPlan, CheckedDynamicDescriptorTransferSource,
    CheckedDynamicDispatchPlan, CheckedDynamicDispatchPlans, CheckedDynamicJoinBranchPlan,
    CheckedDynamicJoinControlPlan, CheckedDynamicRealizationBodyPlan,
    CheckedDynamicRealizationCallablePlan, CheckedDynamicScalarCallOrigin,
    CheckedDynamicScalarCallPlan, CheckedDynamicScalarHelperPlan, CheckedDynamicSelectionPlan,
    CheckedDynamicStoredDescriptorPlan, CheckedDynamicUnitCallOrigin, CheckedDynamicUnitCallPlan,
    CheckedDynamicUnitContinuationPlan, CheckedDynamicUnitHelperPlan, CheckedElementViewWritePlan,
    CheckedFusedServiceErasureReceipt, CheckedFusedServiceParameterReceipt,
    CheckedGuardedJumpArmPlan, CheckedGuardedJumpFallbackPlan, CheckedGuardedJumpPlan,
    CheckedGuardedJumpReturnPlan, CheckedNaturalRankMeasure,
    CheckedNominalAffineUnitCleanupMachinePlan, CheckedNominalAffineUnitCleanupPlans,
    CheckedNormalCleanupAction, CheckedNormalExitCleanupPlan, CheckedOperandSequence,
    CheckedPartialAffineUnitCleanupMachinePlan, CheckedPartialAffineUnitCleanupPlans,
    CheckedPayloadlessGuardedCallEvidencePlan, CheckedPayloadlessGuardedCallEvidenceUsePlan,
    CheckedPayloadlessGuardedCallReturnMachinePlan, CheckedProviderAttachmentRequirementPlan,
    CheckedReferenceResultSourcePlan, CheckedRepresentationViewStorePlan, CheckedReturnPlan,
    CheckedRuntimeIndex, CheckedScalarBinding, CheckedScalarBindingDestination,
    CheckedScalarBindingValue, CheckedScalarBranchDestination, CheckedScalarCaseFieldPlan,
    CheckedScalarGraphPlans, CheckedScalarGuardedExit, CheckedScalarGuardedTail,
    CheckedScalarMachineGraph, CheckedScalarParameterStorage, CheckedScalarPrimitiveLocalPlan,
    CheckedScalarReturnPlan, CheckedScalarStateGraph, CheckedScalarStateTerminator,
    CheckedScalarSuccessor, CheckedSemanticDependencies, CheckedSemanticDependency,
    CheckedSemanticDependencyExposure, CheckedSemanticDependencyKind, CheckedStateNaturalRank,
    CheckedStoreRoot, CheckedStoreValue, CheckedStructuralAccess,
    CheckedStructuralBooleanConvergencePlan, CheckedStructuralByteSequenceFieldByteStorePlan,
    CheckedStructuralCallCustodyPlan, CheckedStructuralCallReturnPlans,
    CheckedStructuralCaseFieldStorePlan, CheckedStructuralCaseLiteralFieldStorePlan,
    CheckedStructuralCaseReturnPlan, CheckedStructuralControlCleanupPlans,
    CheckedStructuralControlEdgeCleanupPlan, CheckedStructuralControlStateCleanupPlan,
    CheckedStructuralControlSuccessorPlan, CheckedStructuralControlTransferPlan,
    CheckedStructuralControlTransferSourcePlan, CheckedStructuralPathQualification,
    CheckedStructuralRankedArgumentPlan, CheckedStructuralRankedGuardPlan,
    CheckedStructuralRankedSccEdgePlan, CheckedStructuralRankedSccPlan,
    CheckedStructuralResultPlan, CheckedStructuralReturnMachinePlan, CheckedStructuralReturnPlans,
    CheckedStructuralReturnedClaimTransferPlan, CheckedStructuralScalarArgumentPlan,
    CheckedStructuralScalarArgumentSourcePlan, CheckedStructuralScalarFieldStorePlan,
    CheckedStructuralScalarIntegerBoundKind, CheckedStructuralScalarIntegerBoundPlan,
    CheckedStructuralScalarIntegerBoundRequirementPlan, CheckedStructuralScalarParameterPlan,
    CheckedStructuralScalarReturnMachinePlan, CheckedStructuralScalarReturnPlans,
    CheckedStructuralUnitControlMachinePlan, CheckedStructuralUnitControlPlans,
    CheckedStructuralUnitControlStatePlan, CheckedStructuralUnitControlTerminatorPlan,
    CheckedStructuralValueCall, CheckedTerminalDebugPlans, CheckedTerminalMachineDebugPlan,
    CheckedTerminalMachineSelection, CheckedTerminalMachineSelections,
    CheckedTerminalSignatureEligibility, CheckedTerminalStateDebugPlan,
    CheckedTraitOperatorScalarReturnMachinePlan, CheckedTrivialAffineStructuralLocalPlan,
    CheckedUnitCallCoordinate, CheckedUnitCallParts, CheckedUnitCallPlan, CheckedUnitCallResult,
    CheckedUnitCallRoute, CheckedUnitCallTarget, CheckedUnitCallTransfers,
    CheckedUnitCallableEntry, CheckedUnitClaimTransferPlan, CheckedUnitCleanupBoundary,
    CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, CheckedUnitEffectPlans,
    CheckedUnitEntryClaimPlan, CheckedUnitNominalAffineCallerRequirementPlan,
    CheckedUnitNominalAffineCleanupPlan, CheckedUnitNominalAffineCleanupRequirementPlan,
    CheckedUnitPartialAffineDiscardPlan, CheckedUnitPlan, CheckedUnitPlanOmission,
    CheckedUnitPlanOmissionStage, CheckedUnitScalarControlPlan, CheckedUnitScalarResultBindingPlan,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralCasePlan, CheckedUnitStructuralDomainPlan,
    CheckedUnitStructuralDomainRequirementPlan, CheckedUnitStructuralFieldPlan,
    CheckedUnitStructuralFieldType, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralResultBindingPlan,
    CheckedUnitStructuralReturnPlan, CheckedUnitStructuralTypePlan, CheckedUnitStructuralTypeShape,
    FlowBorrowActivationFact, FlowBorrowLifetimeFacts, FlowBorrowWeakeningFact,
    FlowBorrowWeakeningReason, FlowBoundaryEdgeFact, FlowBoundaryFacts, FlowCallDispatchRole,
    FlowCallFact, FlowCallResult, FlowCallSite, FlowClaimJoinAlternative,
    FlowClaimJoinAlternativeSource, FlowClaimJoinExit, FlowClaimJoinExitKind, FlowClaimJoinReceipt,
    FlowClaimOutcomeEntryFact, FlowClaimOutcomeMapFact, FlowClaimOutcomeSource, FlowConstraintKind,
    FlowConstraintRef, FlowContextFacts, FlowControlFacts, FlowExitFact, FlowExitParameterOrigin,
    FlowFacts, FlowInvalidationFact, FlowInvalidationFacts, FlowInvalidationSource,
    FlowOperandChild, FlowOperandFact, FlowOperandReferent, FlowOperatorInvocationFact,
    FlowOwnedSelectionClaim, FlowOwnedSelectionReceipt, FlowOwnedSelectionSource,
    FlowOwnedSelectionTransfer, FlowOwnershipFacts, FlowPermissionEvaluation,
    FlowPermissionEventFact, FlowSemanticContextRef, FlowStateFact, FlowStateOutgoingFact,
    FlowStatementFact, RetiredFlowCall, is_borrowed_view,
};
pub use operators::{
    CheckedArithmeticPolicyAdapter, CheckedBoundaryApplicationOperands,
    CheckedBoundaryOperatorApplicationArgument, CheckedBoundaryOperatorApplicationDemand,
    CheckedBoundaryOperatorApplicationUseSite, CheckedCrashQualifiedUse,
    CheckedNamedOperatorUseFact, CheckedNamedRequirementUseFact, CheckedOperatorCandidateFact,
    CheckedOperatorContractUse, CheckedOperatorCrashBucket, CheckedOperatorCrashContract,
    CheckedOperatorFacts, CheckedOperatorOccurrence, CheckedOperatorRealizationContract,
    CheckedOperatorResolutionIssue, CheckedOperatorResolutionStatus, CheckedOperatorUseFact,
    CheckedOperatorUseHandle, CheckedProviderPlanCommitment,
    CheckedSelectedFloatComparisonExecution, CheckedSymbolicBoundaryOperatorApplicationArgument,
    CheckedSymbolicBoundaryOperatorApplicationDemand,
};
pub use proof::{
    BoundaryQualificationAuthorization, CheckedContractEntailmentAssumptionDischarge,
    CheckedContractEntailmentExitMembershipDischarge, CheckedContractMembershipExit,
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
    CheckedFloatSemanticApplicationOperand, CheckedFloatUseSite, CheckedIntegerAssignmentDecision,
    CheckedMathematicalBinder, CheckedMathematicalBinderKind, CheckedMathematicalBody,
    CheckedMathematicalDeclaration, CheckedMathematicalParameter, CheckedMathematicalSignature,
    CheckedProofOnlyValueType, CheckedProofPropositionId, CheckedProofValueDeclaration,
    CheckedProofValueId, CheckedPropositionApplication, CheckedPropositionBinder,
    CheckedPropositionBinderArgument, CheckedPropositionBinderArgumentKind,
    CheckedPropositionBinderKind, CheckedPropositionDeclaration, CheckedPropositionEvidence,
    CheckedPropositionVocabulary, ContractCallFact, ContractEvidenceArgument, ContractExitFact,
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
    CheckedArrayConstructionSource, CheckedBooleanExpression, CheckedElementViewRegion,
    CheckedErasedProofParameterPlan, CheckedIeeeFloatComparisonKind, CheckedIntegerBinaryKind,
    CheckedIntegerComparisonKind, CheckedIntegerMathExpression, CheckedIntegerMathTerm,
    CheckedIntegerRange, CheckedLocatedProofTerm, CheckedLocatedScalarExpression, CheckedProofTerm,
    CheckedProofTermField, CheckedProofTermRole, CheckedProofTerms,
    CheckedScalarCaseComputationField, CheckedScalarCaseConstruction, CheckedScalarComputation,
    CheckedScalarComputationHandle, CheckedScalarComputationKind, CheckedScalarComputationPlans,
    CheckedScalarComputationRoot, CheckedScalarComputationStructuralArgument,
    CheckedScalarComputationTemporaryCleanup, CheckedScalarDispatchArm,
    CheckedScalarDispatchPattern, CheckedScalarExpression, CheckedScalarExpressionBindings,
    CheckedScalarExpressionPlans, CheckedScalarExpressionRole, CheckedScalarReadProjection,
    CheckedScalarSelector, CheckedStorageRoot, CheckedStructuralDispatchArm,
    CheckedStructuralParameterField, CheckedStructuralPredicatePathSegment,
    CheckedStructuralRecordField, CheckedStructuralRecordFieldValue, CheckedStructuralValue,
    CheckedStructuralValueHandle, CheckedStructuralValueKind, CheckedStructuralValuePlans,
    CheckedStructuralValueRoot, CheckedSubsliceSite, CheckedValueFact, CheckedValueFacts,
    CheckedValueHandle, CheckedValueOrigin, CheckedValueStatementRole,
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
