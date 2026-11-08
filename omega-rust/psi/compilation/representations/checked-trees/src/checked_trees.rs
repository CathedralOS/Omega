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
pub use flow::CheckedCallOrigin;
pub mod keyed_rows;
pub mod operators;
pub mod proof;
pub(crate) mod shifted_rows;
pub mod statement;
pub mod values;
pub mod views;
pub use views::{
    CheckFactsRef, CheckFactsWalk, ContentProjectionFactsRef, ContentProjectionFactsWalk,
    FlowFactsRef, FlowFactsWalk, QualificationFactsRef, QualificationFactsWalk,
};

pub use keyed_rows::{DurableRows, KeyedRows};

pub use admissibility::{
    AcceptanceCheck, AcceptanceCheckProvenance, AcceptanceCheckVerdict, AcceptanceDimension,
    AcceptanceSummary, AcceptanceVerdict, AcceptanceView, CallAcceptance, ExitAcceptance,
    OperatorAcceptance, StateAcceptance, StateOperationAcceptance, StateOperationAcceptanceKind,
    StatementAcceptance,
};
pub use borrow::{
    BorrowAccessKind, BorrowArgumentAccessFact, BorrowCallFact, BorrowCallReferenceSlotEffect,
    BorrowFacts, BorrowIncomingCompatibilityRequirement, BorrowLoanFact, BorrowLoanFormation,
    BorrowLoanLineage, BorrowLoanOwnerSegment, BorrowRetainedOperand, BorrowReturnedLoanFact,
    BorrowRootKind, BorrowWritableRootFact, CapturedPlace, CapturedPlaceCompatibility,
    CapturedPlaceContainment, CheckedBorrowParentEnd, CheckedBorrowResourceDispositionTarget,
    CheckedBorrowResourceLifecyclePhase, CheckedBorrowResourceRetention,
    CheckedCallBorrowLoanResource, CheckedCallBorrowLoanSource, CheckedDirectBorrowLoanResource,
    CheckedDirectBorrowParentLifetime, CheckedDirectBorrowRestorationObligation,
    CheckedOutputBorrowInput, CheckedOutputBorrowInputPermission,
    CheckedOutputLifetimeDependencyResource, CheckedOutputLifetimeDependencySource,
    CheckedParentBorrowResource, CheckedReborrowAccessEffect, CheckedReborrowLoanResource,
    CheckedReborrowParentEndStatus, CheckedReborrowParentSuspensionBoundary,
    CheckedReborrowResourceDisposition, CheckedReborrowResourceDispositionEvent,
    CheckedReborrowRestorationObligation, CheckedRetiredParentResourceDispositionStep,
    CheckedStoredCallBorrowInputs, CheckedSuppliedOutputBorrowResource, IncomingReferenceIdentity,
    IncomingReferenceOrigin, ParentLexicalStatusAtChildEnd, StateBorrowFact,
};
pub use facts::CheckedValueQualification;
pub use facts::{
    CheckedCaptureArgument, CheckedCaptureArgumentValue, CheckedCaptureInputs,
    CheckedDeclarationCall, CheckedDeclarationDispatchArm, CheckedExecutableBindings,
    CheckedExpressionSource, CheckedMachineRankCorrespondence, CheckedRankApplicationFacts,
    CheckedRankApplicationFormation, CheckedRankApplicationFormationHandle,
    CheckedRankApplicationInputHome, CheckedRankArrivalEdge, CheckedRankArrivalPlan,
    CheckedRankBodyJudgment, CheckedRankCoordinate, CheckedRankCoordinateBinding,
    CheckedRankCoordinateHandle, CheckedRankCoordinateTransport, CheckedRankLocalCapture,
    CheckedRankObservation, CheckedRankOperand, CheckedRankPremiseRegime,
    CheckedRankReceivingCoordinate, CheckedRankValueDerivation, CheckedRequirementMachines,
    CheckedRuntimeIntegerRank, CheckedRuntimeNaturalState, CheckedRuntimeRankComponent,
    CheckedRuntimeRankEdge, CheckedRuntimeRankInput, CheckedRuntimeRankMember,
    CheckedSelectedMachine, CheckedSourceCallTarget, CheckedSourceNaturalRankComponent,
    CheckedSourceNaturalRankEdge, CheckedSourceNaturalRankMember, CheckedSourceTransition,
    CheckedStateRankCorrespondence,
};
pub use facts::{
    CheckedCompilerEvaluationCall, CheckedCompilerEvaluationComponent,
    CheckedCompilerEvaluationDependencies, CheckedDeclarationCapture,
    CheckedDeclarationDescription, CheckedDeclarationDescriptions, CheckedDeclarationParameter,
    CheckedDeclarationSpecialization, CheckedMachineDescription, CheckedSelectionRequirement,
};
pub use facts::{
    ProofApplicationMachine, ProofApplicationPlan, ProofApplicationTargetMember, ProofApplications,
    ProofDeclarationDependencies,
};

pub use facts::{
    CheckedStoredIntegerWrite, ConstantRecipeObligation, ConstantRecipeRequirement,
    ContextualCleanupApplication, ContextualCleanupDisposition, ContextualCleanupEntryRequirement,
    ContextualCleanupEntryRequirements, ContextualCleanupExcludedRegion,
    ContextualCleanupExclusionCause, ContextualCleanupFacts, ContextualStorageEstablishment,
    DecidedCallRequires, ProvenAssignmentIndex, StoredIntegerWriteSite,
};

pub use facts::{
    BlockingFacts, CallServiceReachRows, CarryFacts, CheckFacts,
    CheckedAcceptedTemplateClassification, CheckedAcceptedTemplateIdentities,
    CheckedAcceptedTemplateIdentity, CheckedAccessViewInput, CheckedBodyCrashSummary,
    CheckedBoundaryAdapterDispatch, CheckedCallableHeader, CheckedCallbackResourceReceipt,
    CheckedCrashCallSite, CheckedCrashOperatorSite, CheckedCrashSite, CheckedEntryResourceEnvelope,
    CheckedFactCallProjection, CheckedInstalledInterruptRegistration,
    CheckedMachineContractEnvelopeIdentity, CheckedMachineContractRefinement,
    CheckedMachineResourceEnvelopes, CheckedMachineSpecializationIdentity,
    CheckedNominalMachineUse, CheckedProofRankingRelation, CheckedProofRecursiveCallSite,
    CheckedProofRecursiveComponent, CheckedProofRecursiveEdge, CheckedProofRecursiveMember,
    CheckedProofRecursiveTransitionLane, CheckedProviderCandidate,
    CheckedProviderCandidateDeclaration, CheckedProviderClosedRequirement,
    CheckedProviderClosedRequirements, CheckedProviderInstanceDeclaration,
    CheckedProviderInstancePublication, CheckedProviderInstanceRequirement,
    CheckedProviderReceivingApplication, CheckedProviderReceivingDescription,
    CheckedQuotientFormation, CheckedQuotientOperation, CheckedRequirementCallMachineSelection,
    CheckedRequirementCallSpecialization, CheckedRequirementCallTypeBinding,
    CheckedResourceAxisAnchor, CheckedResourceDerivationObligation,
    CheckedRuntimeMembershipSubject, CheckedSpecializationIdentities, CheckedStateLocal,
    CheckedSynchronousBoundaryTarget, CheckedTrappingSite, ClaimCarryPolicyFact,
    ClosedFloatRangeRequirement, ClosedIntegerRangeRequirement, ClosedScalarContractValue,
    ClosedScalarValueContractPlan, ContentClassFacts, ContentClassRow,
    ContentIdentityReshuffleFact, ContentPartitionCompositionFact,
    ContentPartitionInputClaimBinding, ContentPartitionPlaceSubstitution,
    ContentPartitionResultRewrite, ContentPartitionReturnFact, ContentProjectionFacts,
    ContentWalkRows, CrashCallSiteLocation, CrashCause, CrashContractCapsule, CrashInterface,
    CrashPlan, CrashPredicateExpression, CrashPredicateIdentity, CrashRouteBucket,
    CrashRouteBucketId, CrashRouteGuard, CrashSiteLocation, DataCarryFact,
    DeclaredCallableContract, DeclaredContractClause, DeclaredContractFact, DeclaredContractKind,
    DeclaredContractParameter, DeclaredContractStructure, DeclaredProgressPremise,
    DeclaredTermination, DomainDependencyFact, DomainDependencyPathFact, DomainFacts,
    DynamicConformanceFacts, DynamicConformanceRowFact, DynamicConformanceRowSource,
    DynamicSelectionFact, IndexCompatibilityDischarge, IndexCompatibilityFact,
    IndexCompatibilityFacts, MachineActivationCarryFact, MachineBlockingFact,
    MachineContractCommitment, MachineContractIdentity, MachineContractPlan, MachineContractPlans,
    MachineMutationFact, MachineQualifications, MachineServiceReachRows, MachineSuspensionFact,
    MachineSynchronousInvocationFact, MachineTerminationFact, MutationFacts,
    NominalMachineUseFacts, NominalMachineUseSite, OperatorSiteLocation, PerformedIntegerWrite,
    QualificationFacts, RealizedMachineContractEnvelope, ReferenceCustodyFacts,
    ReferenceCustodyRow, RequirementCallSpecializationFacts, ResolvedCarrierType,
    ResolvedExpressionType, ResolvedTypeFacts, RetainedBorrowCustodyFact, ServiceReachFacts,
    StateLocalFacts, StateServiceReachRows, StateWriteFramePlan, SuspensionCrossingBorrowFrontier,
    SuspensionCrossingCarryFact, SuspensionCrossingLiveValueFact, SuspensionCrossingLoanBacking,
    SuspensionCrossingLoanFact, SuspensionCrossingLoanSource, SuspensionCrossingStorage,
    SuspensionCrossingValueOrigin, SuspensionFacts, SuspensionValueBorrowFrontier,
    SynchronousInvocationFacts, TerminationFacts, VacuousQualificationUse,
    canonical_suspension_crossing_id, contract_declarations_commitment, contract_identity,
    contract_plan_by_machine, declared_termination_commitment,
};
pub use flow::*;
pub use operators::{
    CheckedArithmeticPolicyAdapter, CheckedBoundaryApplicationOperands,
    CheckedBoundaryBodyCandidate, CheckedBoundaryOperatorApplicationArgument,
    CheckedBoundaryOperatorApplicationDemand, CheckedBoundaryOperatorApplicationUseSite,
    CheckedBoundaryOperatorDeclaration, CheckedCrashQualifiedUse, CheckedNamedOperatorUseFact,
    CheckedNamedRequirementUseFact, CheckedOperatorCandidateFact, CheckedOperatorContractUse,
    CheckedOperatorCrashBucket, CheckedOperatorCrashContract, CheckedOperatorFacts,
    CheckedOperatorOccurrence, CheckedOperatorRealizationContract, CheckedOperatorResolutionIssue,
    CheckedOperatorResolutionStatus, CheckedOperatorUseFact, CheckedOperatorUseHandle,
    CheckedProviderPlanCommitment, CheckedProviderRequirement, CheckedProviderRequirementKind,
    CheckedSelectedFloatComparisonExecution, CheckedSymbolicBoundaryOperatorApplicationArgument,
    CheckedSymbolicBoundaryOperatorApplicationDemand,
};
pub use proof::{
    BoundaryQualificationAuthorization, CallContractSignature, CheckedDirectBlockFloatParameter,
    CheckedDirectCallFloatResult, CheckedDirectMachineFloatParameter,
    CheckedDirectMachineFloatResult, CheckedDirectOperationFloatResult,
    CheckedDirectStructuralFloatLeaf, CheckedEvidenceInterfaceIdentity, CheckedEvidenceProjection,
    CheckedEvidenceRequirementIdentity, CheckedEvidenceTerm,
    CheckedFloatMeaningEqualityProposition, CheckedFloatMeaningProjection,
    CheckedFloatMeaningProjectionError, CheckedFloatMeaningProjectionOccurrence,
    CheckedFloatMeaningProjectionOccurrenceId, CheckedFloatProjectionContractIdentity,
    CheckedFloatProjectionInput, CheckedFloatProjectionInputId, CheckedFloatProjectionSource,
    CheckedFloatSemanticApplication, CheckedFloatSemanticApplicationError,
    CheckedFloatSemanticApplicationOperand, CheckedFloatUseSite, CheckedIdentityFormation,
    CheckedIdentityFormationPlan, CheckedIdentityOwner, CheckedIdentityPublication,
    CheckedIndexedCitation, CheckedIndexedTheorem, CheckedIntegerAssignmentDecision,
    CheckedMathematicalBinder, CheckedMathematicalBinderKind, CheckedMathematicalBody,
    CheckedMathematicalDeclaration, CheckedMathematicalParameter, CheckedMathematicalSignature,
    CheckedProofOnlyValueType, CheckedProofPropositionId, CheckedProofValueDeclaration,
    CheckedProofValueId, CheckedPropositionApplication, CheckedPropositionBinder,
    CheckedPropositionBinderArgument, CheckedPropositionBinderArgumentKind,
    CheckedPropositionBinderKind, CheckedPropositionDeclaration, CheckedPropositionEvidence,
    CheckedPropositionVocabulary, CheckedSourceEvidenceCarrier, ContractCallFact,
    ContractEvidenceArgument, ContractExitFact, ContractExpressionEvidenceArgumentFact,
    ContractExpressionEvidenceCallFact, ContractExpressionStaticConformanceApplicationFact,
    ContractOperatorUseFact, ContractProofFact, ContractProofFactKind, ContractProofFactOwner,
    ContractProofFactRef, EvidenceAssignmentSource, EvidenceForwardingFact, InheritedContractScope,
    LemmaFacts, OutcomeOccurrenceRoot, OutcomeReferencedOccurrence, OutcomeSpecificArmFact,
    OutcomeSpecificArmRowFact, OutcomeSpecificEvidenceInterfaceScopeFact,
    OutcomeSpecificGuaranteeFact, OutcomeSpecificValidityFact, ProofFactKind, ProofFacts,
    ProofLemmaFact, ProofLemmaKind, ProofObligationFact, ProofObligationOwner, ProofOutputCallFact,
    ProofOutputEvidenceArgumentFact, ProofOutputFact, ProofOutputRuntimeCallFact,
    QuantifiedBoundFact, QuantifiedRangeFact, StaticRequirementDispatchFact,
};
pub use values::{
    CheckedArrayConstructionSource, CheckedBooleanExpression, CheckedByteSequenceOperand,
    CheckedDispatchSubject, CheckedElementViewRegion, CheckedEntryIndexBound,
    CheckedEntryIndexSelector, CheckedErasedProofParameterPlan, CheckedIeeeFloatComparisonKind,
    CheckedIntegerBinaryKind, CheckedIntegerComparisonKind, CheckedIntegerMathExpression,
    CheckedIntegerMathTerm, CheckedIntegerRange, CheckedLocatedProofTerm,
    CheckedLocatedScalarExpression, CheckedMathematicalArgument, CheckedProjectionSelectorRoot,
    CheckedProofTerm, CheckedProofTermField, CheckedProofTermRole, CheckedProofTerms,
    CheckedQuotientAdaptationLicense, CheckedReferenceSourceContract, CheckedScalarCallee,
    CheckedScalarCaseComputationField, CheckedScalarCaseConstruction, CheckedScalarComputation,
    CheckedScalarComputationHandle, CheckedScalarComputationKind, CheckedScalarComputationPlans,
    CheckedScalarComputationRoot, CheckedScalarComputationStructuralArgument,
    CheckedScalarComputationTemporaryCleanup, CheckedScalarDispatchArm,
    CheckedScalarDispatchPattern, CheckedScalarEvaluationOperand, CheckedScalarExpression,
    CheckedScalarExpressionBindings, CheckedScalarExpressionPlans, CheckedScalarExpressionRole,
    CheckedScalarReadProjection, CheckedScalarSelector, CheckedSourceQualification,
    CheckedStorageRoot, CheckedStructuralCaseRoot, CheckedStructuralCaseSubject,
    CheckedStructuralDispatchArm, CheckedStructuralParameterField,
    CheckedStructuralPredicatePathSegment, CheckedStructuralQualificationContract,
    CheckedStructuralRecordField, CheckedStructuralRecordFieldValue, CheckedStructuralValue,
    CheckedStructuralValueHandle, CheckedStructuralValueKind, CheckedStructuralValuePlans,
    CheckedStructuralValueRoot, CheckedSubsliceSite, CheckedValueCompletion, CheckedValueFact,
    CheckedValueFacts, CheckedValueHandle, CheckedValueOrigin, CheckedValueStatementRole,
};

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
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

    /// Return the crashes already checked for this exact body. Missing,
    /// competing or stale body evidence cannot establish invocation safety.
    /// Callers still owe the body's entry contract for their actual arguments.
    pub fn body_crash_summary(
        &self,
        machine: symbols::SymbolHandle,
    ) -> Option<&CheckedBodyCrashSummary> {
        let mut bodies = self
            .typed
            .machines()
            .iter()
            .filter(|body| body.symbol == machine);
        let body = bodies.next()?;
        if bodies.next().is_some() {
            return None;
        }
        let mut plans = self
            .facts
            .contract_plans
            .machines()
            .iter()
            .filter(|plan| plan.machine == machine);
        let plan = plans.next()?;
        if plans.next().is_some() {
            return None;
        }
        plan.crash.checked_body_summary(machine, body.states)
    }

    /// The `u64` literal the selected target's fold landed at a retained
    /// constant's root (`CheckedScalarComputationKind::RetainedConstant`).
    /// Psi 05 mints the row while the root is still a `size_of`/`align_of`
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
            let parent_origin = origins.next()?;
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
