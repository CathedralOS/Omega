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
    BorrowLoanLineage, BorrowLoanOwnerSegment, BorrowRetainedOperand, BorrowReturnedLoanFact,
    BorrowRootKind, BorrowStateIndex, BorrowWritableRootFact, CapturedPlace,
    CapturedPlaceCompatibility, CapturedPlaceContainment, CheckedBorrowResourceDispositionTarget,
    CheckedBorrowResourceLifecyclePhase, CheckedCallBorrowLoanResource,
    CheckedCallBorrowLoanSource, CheckedDirectBorrowLoanResource,
    CheckedDirectBorrowParentLifetime, CheckedDirectBorrowRestorationObligation,
    CheckedParentBorrowResource, CheckedReborrowAccessEffect, CheckedReborrowLoanResource,
    CheckedReborrowParentEndStatus, CheckedReborrowParentSuspensionBoundary,
    CheckedReborrowResourceDisposition, CheckedReborrowResourceDispositionEvent,
    CheckedReborrowRestorationObligation, CheckedRetiredParentResourceDispositionStep,
    IncomingReferenceIdentity, ParentLexicalStatusAtChildEnd, StateBorrowFact,
};
pub use facts::CheckedValueQualification;
pub use facts::{
    CheckedMachineRankCorrespondence, CheckedRankArrivalEdge, CheckedRankArrivalPlan,
    CheckedRankBodyJudgment, CheckedRankCoordinate, CheckedRankCoordinateBinding,
    CheckedRankCoordinateHandle, CheckedRankCoordinateTransport, CheckedRankLocalCapture,
    CheckedRankPremiseRegime, CheckedRankReceivingCoordinate, CheckedRankValueDerivation,
    CheckedRuntimeNaturalState, CheckedRuntimeRankComponent, CheckedRuntimeRankEdge,
    CheckedRuntimeRankMember, CheckedStateRankCorrespondence,
};

pub use facts::{
    CheckedStoredIntegerWrite, ConstantRecipeObligation, ConstantRecipeRequirement,
    ContextualCleanupApplication, ContextualCleanupDisposition, ContextualCleanupEntryRequirement,
    ContextualCleanupEntryRequirements, ContextualCleanupFacts, ContextualStorageEstablishment,
    DecidedCallRequires, ProvenAssignmentIndex, StoredIntegerWriteSite,
};

pub use facts::{
    BlockingFacts, BuildBoundProgressDemand, CallServiceReachRows, CarryFacts, CheckFacts,
    CheckedAcceptedTemplateClassification, CheckedAcceptedTemplateIdentities,
    CheckedAcceptedTemplateIdentity, CheckedBoundaryAdapterDispatch,
    CheckedCallbackResourceReceipt, CheckedCrashCallSite, CheckedCrashOperatorSite,
    CheckedCrashSite, CheckedEntryResourceEnvelope, CheckedFactCallProjection,
    CheckedMachineContractEnvelopeIdentity, CheckedMachineContractRefinement,
    CheckedMachineResourceEnvelopes, CheckedMachineSpecializationIdentity,
    CheckedNominalMachineUse, CheckedPlacedViewInput, CheckedProofRankingRelation,
    CheckedProofRecursiveCallSite, CheckedProofRecursiveComponent, CheckedProofRecursiveEdge,
    CheckedProofRecursiveMember, CheckedProofRecursiveTransitionLane,
    CheckedProviderClosedRequirement, CheckedProviderClosedRequirements, CheckedQuotientFormation,
    CheckedQuotientOperation, CheckedRequirementCallMachineSelection,
    CheckedRequirementCallSpecialization, CheckedRequirementCallTypeBinding,
    CheckedResourceAxisAnchor, CheckedResourceDerivationObligation,
    CheckedRuntimeMembershipSubject, CheckedSpecializationIdentities, CheckedStateLocal,
    ClaimCarryPolicyFact, ClosedFloatRangeRequirement, ClosedIntegerRangeRequirement,
    ClosedScalarContractValue, ClosedScalarValueContractPlan, ContentClassFacts, ContentClassRow,
    ContentIdentityReshuffleFact, ContentPartitionCompositionFact,
    ContentPartitionInputClaimBinding, ContentPartitionPlaceSubstitution,
    ContentPartitionResultRewrite, ContentProjectionFacts, CrashCallSiteLocation, CrashCause,
    CrashContractCapsule, CrashInterface, CrashPlan, CrashPredicateExpression,
    CrashPredicateIdentity, CrashRouteBucket, CrashRouteBucketId, CrashRouteGuard,
    CrashSiteLocation, DataCarryFact, DeclaredCallableContract, DeclaredContractClause,
    DeclaredContractFact, DeclaredContractKind, DeclaredContractParameter,
    DeclaredContractStructure, DeclaredProgressPremise, DeclaredTermination, DomainDependencyFact,
    DomainDependencyPathFact, DomainFacts, DynamicConformanceFacts, DynamicConformanceRowFact,
    DynamicConformanceRowSource, DynamicSelectionFact, IndexCompatibilityDischarge,
    IndexCompatibilityFact, IndexCompatibilityFacts, MachineActivationCarryFact,
    MachineBlockingFact, MachineBuildBoundProgressDemands, MachineContractCommitment,
    MachineContractIdentity, MachineContractPlan, MachineContractPlans, MachineMutationFact,
    MachineQualifications, MachineServiceReachRows, MachineSuspensionFact,
    MachineSynchronousInvocationFact, MachineTerminationFact, MutationFacts,
    NominalMachineUseFacts, NominalMachineUseSite, OperatorSiteLocation, PerformedIntegerWrite,
    ProgressDemandCallSite, QualificationFacts, RealizedMachineContractEnvelope,
    ReferenceCustodyFacts, ReferenceCustodyRow, RequirementCallSpecializationFacts,
    ResolvedCarrierType, ResolvedExpressionType, ResolvedTypeFacts, RetainedBorrowCustodyFact,
    ServiceReachFacts, StateLocalFacts, StateServiceReachRows, StateWriteFramePlan,
    SuspensionCrossingBorrowFrontier, SuspensionCrossingCarryFact, SuspensionCrossingLiveValueFact,
    SuspensionCrossingLoanBacking, SuspensionCrossingLoanFact, SuspensionCrossingLoanSource,
    SuspensionCrossingStorage, SuspensionCrossingValueOrigin, SuspensionFacts,
    SuspensionValueBorrowFrontier, SynchronousInvocationFacts, TerminationFacts,
    VacuousQualificationUse, canonical_suspension_crossing_id, contract_declarations_commitment,
    contract_identity, contract_plan_by_machine, declared_termination_commitment,
};
pub use flow::{
    CheckedAffineConstructionElementPlan, CheckedAtomicAccessPlan, CheckedAtomicEvent,
    CheckedAtomicReadModifyWrite, CheckedBindingFieldReceipt, CheckedBindingParameterReceipt,
    CheckedBoundaryMachinePlan, CheckedBoundaryMachineResultPlan, CheckedBoundaryScalarGuarantee,
    CheckedByteSequenceCarrier, CheckedByteSequenceValue, CheckedByteSequenceValuePlan,
    CheckedCallScalarArgument, CheckedCaseLiteralValuePlan, CheckedCleanupCaseCondition,
    CheckedCleanupControlPlans, CheckedClosedSumCaseOutcomePlan, CheckedClosedSumCasePlan,
    CheckedClosedSumPayloadTransferPlan, CheckedClosedSumSubjectKind,
    CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan,
    CheckedComposedUnitControlTerminatorPlan, CheckedConditionalReturnArm,
    CheckedControlResultPlan, CheckedDynamicClaimPath, CheckedDynamicSlotPlan, CheckedExtent,
    CheckedGuardedExitPlans, CheckedGuardedJumpArmPlan, CheckedGuardedJumpFallbackPlan,
    CheckedGuardedJumpPlan, CheckedGuardedJumpReturnPlan, CheckedIndexedWriteDecisions,
    CheckedInitializedLocal, CheckedMemberIdentity, CheckedNaturalRankMeasure,
    CheckedNominalAffineUnitCleanupMachinePlan, CheckedNominalAffineUnitCleanupPlans,
    CheckedNormalCleanupAction, CheckedNormalExitCleanupPlan, CheckedOperandSequence,
    CheckedPartialAffineUnitCleanupMachinePlan, CheckedPartialAffineUnitCleanupPlans,
    CheckedPlaceLoanPlan, CheckedPlaceLoanRoot, CheckedPrimitiveReceivingRange,
    CheckedProviderAttachmentRequirementPlan, CheckedRankEdgeCoordinate,
    CheckedReferenceOriginProjection, CheckedReferenceOriginSourcePlan,
    CheckedReferenceResultSourcePlan, CheckedReferenceStorage, CheckedReturnedReferencePlan,
    CheckedRuntimeIndex, CheckedScalarBranchDestination, CheckedScalarCaseFieldPlan,
    CheckedScalarGuardedExit, CheckedScalarGuardedTail, CheckedScalarReturnPlan,
    CheckedScalarStateTerminator, CheckedScalarStoreValue, CheckedSemanticDependencies,
    CheckedSemanticDependency, CheckedSemanticDependencyExposure, CheckedSemanticDependencyKind,
    CheckedStateContractRequirement, CheckedStateNaturalRank, CheckedStoreDestination,
    CheckedStoreLeaf, CheckedStorePlan, CheckedStoreRoot, CheckedStoreValue,
    CheckedStructuralAccess, CheckedStructuralCallCustodyPlan,
    CheckedStructuralControlCleanupPlans, CheckedStructuralControlEdgeCleanupPlan,
    CheckedStructuralControlStateCleanupPlan, CheckedStructuralControlSuccessorPlan,
    CheckedStructuralControlTransferPlan, CheckedStructuralControlTransferSourcePlan,
    CheckedStructuralPathQualification, CheckedStructuralResultClaimPlan,
    CheckedStructuralResultPlan, CheckedStructuralReturnCompletion,
    CheckedStructuralScalarArgumentPlan, CheckedStructuralScalarArgumentSourcePlan,
    CheckedStructuralScalarParameterPlan, CheckedStructuralValueCall, CheckedTerminalDebugPlans,
    CheckedTerminalMachineDebugPlan, CheckedTerminalMachineSelection,
    CheckedTerminalMachineSelections, CheckedTerminalSignatureEligibility,
    CheckedTerminalStateDebugPlan, CheckedTrivialAffineStructuralLocalPlan,
    CheckedUnitCallCoordinate, CheckedUnitCallParts, CheckedUnitCallPlan, CheckedUnitCallResult,
    CheckedUnitCallRoute, CheckedUnitCallTarget, CheckedUnitCallTransfers,
    CheckedUnitCallableEntry, CheckedUnitClaimTransferPlan, CheckedUnitCleanupBoundary,
    CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, CheckedUnitEffectPlans,
    CheckedUnitEntryClaimPlan, CheckedUnitLocalBindingPlan,
    CheckedUnitNominalAffineCallerRequirementPlan, CheckedUnitNominalAffineCleanupPlan,
    CheckedUnitNominalAffineCleanupRequirementPlan, CheckedUnitParameterStoragePlan,
    CheckedUnitPartialAffineDiscardPlan, CheckedUnitPlan, CheckedUnitPlanGap,
    CheckedUnitPlanGapReason, CheckedUnitRecordStorage, CheckedUnitScalarControlPlan,
    CheckedUnitScalarResultBindingPlan, CheckedUnitSelectedReferenceRetirementPlan,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralCasePlan, CheckedUnitStructuralDomainPlan,
    CheckedUnitStructuralDomainRequirementPlan, CheckedUnitStructuralFieldPlan,
    CheckedUnitStructuralFieldType, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralResultBindingPlan,
    CheckedUnitStructuralReturnPlan, CheckedUnitStructuralTypePlan, CheckedUnitStructuralTypeShape,
    FlowBorrowActivationFact, FlowBorrowLifetimeFacts, FlowBorrowWeakeningFact,
    FlowBorrowWeakeningReason, FlowBoundaryEdgeFact, FlowBoundaryFacts, FlowCallDispatchRole,
    FlowCallFact, FlowCallResult, FlowCallSite, FlowCaseAccessFact, FlowClaimJoinAlternative,
    FlowClaimJoinAlternativeSource, FlowClaimJoinExit, FlowClaimJoinExitKind, FlowClaimJoinReceipt,
    FlowClaimOutcomeEntryFact, FlowClaimOutcomeMapFact, FlowClaimOutcomeSource, FlowConstraintKind,
    FlowConstraintRef, FlowContextFacts, FlowControlFacts, FlowExitFact, FlowExitParameterOrigin,
    FlowFacts, FlowInvalidationFact, FlowInvalidationFacts, FlowInvalidationSource,
    FlowLoanCompletionFact, FlowOperandChild, FlowOperandFact, FlowOperandReferent,
    FlowOperatorInvocationFact, FlowOwnedSelectionClaim, FlowOwnedSelectionReceipt,
    FlowOwnedSelectionSource, FlowOwnedSelectionTransfer, FlowOwnershipFacts,
    FlowPermissionEvaluation, FlowPermissionEventFact, FlowSemanticContextRef, FlowStateFact,
    FlowStateOutgoingFact, FlowStatementFact, FlowStorageTransferFact, FlowStorageTransferSource,
    RetiredFlowCall, checked_structural_type_by_identity, is_borrowed_view,
    is_bounded_nominal_cleanup_record, is_partial_affine_field_type,
};
pub use operators::{
    CheckedArithmeticPolicyAdapter, CheckedBoundaryApplicationOperands,
    CheckedBoundaryBodyCandidate, CheckedBoundaryOperatorApplicationArgument,
    CheckedBoundaryOperatorApplicationDemand, CheckedBoundaryOperatorApplicationUseSite,
    CheckedCrashQualifiedUse, CheckedNamedOperatorUseFact, CheckedNamedRequirementUseFact,
    CheckedOperatorCandidateFact, CheckedOperatorContractUse, CheckedOperatorCrashBucket,
    CheckedOperatorCrashContract, CheckedOperatorFacts, CheckedOperatorOccurrence,
    CheckedOperatorRealizationContract, CheckedOperatorResolutionIssue,
    CheckedOperatorResolutionStatus, CheckedOperatorUseFact, CheckedOperatorUseHandle,
    CheckedProviderPlanCommitment, CheckedSelectedFloatComparisonExecution,
    CheckedSymbolicBoundaryOperatorApplicationArgument,
    CheckedSymbolicBoundaryOperatorApplicationDemand,
};
pub use proof::{
    BoundaryQualificationAuthorization, CheckedDirectBlockFloatParameter,
    CheckedDirectCallFloatResult, CheckedDirectMachineFloatParameter,
    CheckedDirectMachineFloatResult, CheckedDirectOperationFloatResult,
    CheckedDirectStructuralFloatLeaf, CheckedEvidenceInterfaceIdentity, CheckedEvidenceProjection,
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
    EvidenceForwardingFact, InheritedContractScope, LemmaFacts, OutcomeOccurrenceRoot,
    OutcomeReferencedOccurrence, OutcomeSpecificArmFact, OutcomeSpecificArmRowFact,
    OutcomeSpecificEvidenceInterfaceScopeFact, OutcomeSpecificGuaranteeFact,
    OutcomeSpecificValidityFact, ProofFactKind, ProofFacts, ProofLemmaFact, ProofLemmaKind,
    ProofObligationFact, ProofObligationOwner, ProofOutputCallFact,
    ProofOutputEvidenceArgumentFact, ProofOutputFact, ProofOutputRuntimeCallFact,
    QuantifiedBoundFact, QuantifiedRangeFact, StaticRequirementDispatchFact,
};
pub use service_parameter::{CheckedServiceParameterCarrier, CheckedServiceParameterError};
pub use values::{
    CheckedArrayConstructionSource, CheckedBooleanExpression, CheckedByteSequenceOperand,
    CheckedElementViewRegion, CheckedErasedProofParameterPlan, CheckedIeeeFloatComparisonKind,
    CheckedIntegerBinaryKind, CheckedIntegerComparisonKind, CheckedIntegerMathExpression,
    CheckedIntegerMathTerm, CheckedIntegerRange, CheckedLocatedProofTerm,
    CheckedLocatedScalarExpression, CheckedProofTerm, CheckedProofTermField, CheckedProofTermRole,
    CheckedProofTerms, CheckedQuotientAdaptationLicense, CheckedScalarCallee,
    CheckedScalarCaseComputationField, CheckedScalarCaseConstruction, CheckedScalarComputation,
    CheckedScalarComputationHandle, CheckedScalarComputationKind, CheckedScalarComputationPlans,
    CheckedScalarComputationRoot, CheckedScalarComputationStructuralArgument,
    CheckedScalarComputationTemporaryCleanup, CheckedScalarDispatchArm,
    CheckedScalarDispatchPattern, CheckedScalarExpression, CheckedScalarExpressionBindings,
    CheckedScalarExpressionPlans, CheckedScalarExpressionRole, CheckedScalarReadProjection,
    CheckedScalarSelector, CheckedStorageRoot, CheckedStructuralCaseRoot,
    CheckedStructuralCaseSubject, CheckedStructuralDispatchArm, CheckedStructuralParameterField,
    CheckedStructuralPredicatePathSegment, CheckedStructuralRecordField,
    CheckedStructuralRecordFieldValue, CheckedStructuralValue, CheckedStructuralValueHandle,
    CheckedStructuralValueKind, CheckedStructuralValuePlans, CheckedStructuralValueRoot,
    CheckedSubsliceSite, CheckedValueFact, CheckedValueFacts, CheckedValueHandle,
    CheckedValueOrigin, CheckedValueStatementRole,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedTrees {
    pub typed: typed_trees::FrozenTypedTrees,
    pub facts: CheckFacts,
}

impl CheckedTrees {
    pub fn with_roots(typed: impl Into<typed_trees::FrozenTypedTrees>, facts: CheckFacts) -> Self {
        Self {
            typed: typed.into(),
            facts,
        }
    }

    /// The `u64` literal the selected target's fold landed at a retained
    /// constant's root (`CheckedScalarComputationKind::RetainedConstant`).
    /// Stage 04 mints the row while the root is still a `size_of`/`align_of`
    /// query; execution settlement folds every such query into a literal in
    /// the one checked program before lowering runs. This is the checked
    /// product's view of that fold, not an authored expression: `None` means
    /// no fold has closed the constant for this target.
    pub fn landed_integer_constant(
        &self,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<&numerics::literals::IntegerLiteral> {
        match self.typed.expression_table.expression(expression) {
            typed_trees::expression::ExpressionNode::Integer(literal) => Some(literal),
            _ => None,
        }
    }

    /// Checked choices are independent of the pre-check typed resolution ledger.
    pub fn authored_declaration_selections(
        &self,
    ) -> &language_semantics::declaration_selection::AuthoredDeclarationSelections {
        self.facts.authored_declaration_selections()
    }

    /// Walks `NestedExpression` parents to the enclosing non-expression origin,
    /// requiring every value recorded for a parent to agree on that one origin;
    /// `None` when the chain cycles or a parent carries divergent origins.
    pub fn consistent_terminal_origin(
        &self,
        mut origin: CheckedValueOrigin,
    ) -> Option<CheckedValueOrigin> {
        let mut parents = Vec::new();
        while let CheckedValueOrigin::NestedExpression { parent } = origin {
            if parents.contains(&parent) {
                return None;
            }
            parents.push(parent);
            let mut origins = self
                .facts
                .values
                .expression_values(parent)
                .map(|(_, value)| value.origin);
            let Some(parent_origin) = origins.next() else {
                return None;
            };
            if origins.any(|candidate| candidate != parent_origin) {
                return None;
            }
            origin = parent_origin;
        }
        Some(origin)
    }

    /// Exact authored occurrences, including proof views admitted by checking.
    pub fn authored_selection_occurrences(
        &self,
        expression: expression::ExpressionHandle,
    ) -> impl Iterator<
        Item = language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
    > + '_ {
        self.facts
            .authored_selection_occurrences(&self.typed, expression)
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
