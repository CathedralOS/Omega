#![forbid(unsafe_code)]

//! Self-contained, target-neutral Terminal Psi.
//!
//! Begin at [`terminal_module`]: one current program with concept-owned
//! vocabulary. Optimizations precede publication; target realization follows it.
//! Canonical serialization and independent verification have separate owners.

mod artifacts;
pub(crate) use optimization::digest_encoder;
pub mod terminal_module;
pub use language_semantics::byte_predicates::{ByteSequencePredicate, ByteSequencePredicateSet};

pub use artifacts::{
    AdmissionEvidence, AdmissionKind, CertificateEnvelope,
    CheckedProgramEntryDeterminedEvidenceField, CheckedProgramEntryFusedServiceField,
    CheckedProgramEntryReceiverCleanup, CheckedProgramEntryReceiverEligibility,
    CheckedProgramEntryReceiverProjection, CheckedProgramEntryTerminalReceipt,
    CorrelatedAffineBranchWitness, CorrelatedAffineStepWitness, DebugFileId, DebugSite,
    DebugSourceDigest, DebugSourceFile, DebugSourceOrigin, DebugSourceSpan, DebugSubject,
    EvidenceRoute, IntegerAffineWitness, IntegerCastChainWitness,
    IntegerCorrelatedForbiddenRootWitness, PrimitiveJudgment, ProofNode, ProofRule,
    ProofSystemMarker, TerminalDebugMap, TerminalMachineProvenance,
};

mod callback_application;
pub use callback_application::TerminalCallbackApplication;

#[cfg(test)]
mod source_guards;

pub use language_core::BindingRelevance;
pub use semantic_vocabulary::{
    BoundaryCompletion, BoundaryRequirementKind, ExternalCompletionKind,
};
pub use terminal_module::{
    AtomicAccessEvent, AtomicReadModifyWrite, Block, BoundaryApplication,
    BoundaryApplicationArgument, BoundaryApplicationCandidate, BoundaryContentGuarantee,
    BoundaryDeclarationRole, BoundaryMachineDeclaration, BoundaryMachineResult,
    BoundaryOperatorRequirement, BoundaryParameterKind, BoundaryStructuralResultDeclaration,
    BoundaryTypeIdentity, ByteSequenceCarrier, CheckedMachineDeclaration,
    CheckedMachineSpecialization, ClaimContentProjection, ClaimTransfer,
    ClosedConformanceApplication, ClosedConformanceApplicationCommitment,
    ClosedConformanceCallableResult, ClosedConformanceParameterBinding,
    ClosedConformanceParameterKind, ClosedConformanceRealizationCallable, ClosedConformanceRow,
    ClosedConformanceSchema, ClosedConformanceSchemaKind, ClosedConformanceStaticApplication,
    ClosedReachApplication, ClosedReachArgument, ClosedReachCall, ClosedReachCallApplication,
    ClosedReachMachineBinding, ClosedReachParameter, ClosedReachSchema, CompletionReceipt,
    ContentConservationGuarantee, ContentEntryClaim, ContentIdentityReshuffle,
    ContentPartitionComposition, ContentPlaceSubstitution, ContractClause, CrashCause,
    CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard, DirectBlockFloatParameter,
    DirectCallFloatResult, DirectMachineFloatParameter, DirectMachineFloatResult,
    DirectOperationFloatResult, DirectStructuralFloatLeaf, ElementViewRegion, EntryClaim,
    ErasedProofFormal, EvidenceContractLane, EvidenceContractLaneKind, EvidenceInterfaceIdentity,
    EvidenceProjectionIdentity, EvidenceRequirementIdentity, EvidenceTermDeclaration,
    FenceOrdering, FloatMeaningEqualityProposition, FloatMeaningProjection,
    FloatMeaningProjectionOperation, FloatMeaningSource, FloatProjectionContractIdentity,
    FloatProjectionInput, FloatProjectionInputId, FloatResultPolicy, FloatSemanticApplication,
    FloatSemanticApplicationOperand, FloatSemanticContractIdentity, InstallationReachDependency,
    LoanCompletion, MachineContract, MemoryOrdering, NominalAffineCleanup,
    NominalDeclarationIdentity, Operation, OperationKind, OperationProjection, OperationResult,
    OutcomeSpecificCallEvidence, OutcomeSpecificCallEvidenceValidity,
    OutcomeSpecificCallResultSubstitution, OutcomeSpecificEnsure, OutcomeSpecificEvidence,
    OutcomeSpecificEvidenceUse, OutcomeSpecificGuard, PlacedRamCorrespondence,
    PlacedRamFieldCorrespondence, PlacedScalarBoundaryDemand, ProgramLocalRootIntroductionSchema,
    ProofOnlyValueType, ProofOutput, ProofOutputCall, ProofOutputEvidenceArgument,
    ProofOutputRuntimeCall, ProofOutputRuntimeResult, ProofPropositionId, ProofValueDeclaration,
    ProofValueId, PropositionApplicationIdentity, PropositionBinderArgumentIdentity,
    PropositionBinderArgumentKind, PropositionBinderDeclaration, PropositionBinderKind,
    PropositionDeclaration, PropositionEvidence, ProviderCandidateConformance,
    ProviderParameterRefinement, ProviderRefinement, ProviderSignature, ProviderSignatureParameter,
    RecordFieldInitializer, RecordFieldValue, ReferenceOriginProjection, ReferenceOriginSource,
    ReferenceStorageView, ResultQualificationEstablishment, RetainedBorrowContentProjection,
    RetainedBorrowCustody, RetainedBorrowPlace, RetainedBorrowPlaceRoot, ScalarBlockInvariant,
    ScalarBlockInvariantArrival, ScalarCaseField, ScalarDomainDeclaration,
    ScalarDomainEstablishmentRoute, ScalarFloatRange, ScalarIntegerRange,
    ScalarQualificationCatalog, ScalarQualificationCoercion, ScalarQualificationSet,
    SemanticFingerprint, ServiceDeclaration, StaticRequirementDispatch, StructuralAccess,
    StructuralAffineDiscard, StructuralArgument, StructuralCaseDeclaration,
    StructuralCaseSuccessorEdge, StructuralContentProjection, StructuralDomainDeclaration,
    StructuralDomainRequirement, StructuralEstablishmentRoute, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPathQualification, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralReferenceResultSource, StructuralReferentIdentity,
    StructuralResultClaimBinding, StructuralResultDeclaration, StructuralStoreDisposition,
    StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge, TargetMachineCallable,
    TargetMachineFamily, TargetMachineFamilyMember, TargetMachineSupply,
    TerminalAffineCleanupAction, TerminalBlockNaturalRank, TerminalBorrowBoundarySource,
    TerminalBorrowOwnerSegment, TerminalBorrowPlace, TerminalBorrowPlaceSegment,
    TerminalBoundaryApplicationDemand, TerminalDynamicClaimPath, TerminalDynamicDispatchCatalog,
    TerminalDynamicInterface, TerminalDynamicReferenceRegion, TerminalDynamicSlot,
    TerminalDynamicStructuralFormal, TerminalDynamicTable, TerminalIdentityIndex,
    TerminalIdentityTable, TerminalIdentityTableBuilder, TerminalMachine, TerminalMachineResult,
    TerminalModule, TerminalNaturalCycle, TerminalNaturalRankComparison, TerminalNaturalRankEdge,
    TerminalObservationSchema, TerminalOperationCrashContract, TerminalOperationCrashDenotation,
    TerminalPlacedAccessor, TerminalPlacedAccessorKind, TerminalPlacedFieldPlan,
    TerminalPlacedViewInput, TerminalPlacedViewPlan, TerminalProofRankingRelation,
    TerminalProofRecursiveCallSite, TerminalProofRecursiveComponent, TerminalProofRecursiveEdge,
    TerminalProofRecursiveField, TerminalProofRecursiveMember,
    TerminalProofRecursiveTransitionLane, TerminalProofRecursiveType, TerminalPsiIdentity,
    TerminalRankedScc, TerminalReborrowRestorationClass, TerminalReborrowRestoredCallUse,
    TerminalReborrowSharedCohortMember, TerminalRootServiceReach, TerminalSuspensionCallPlan,
    TerminalSuspensionCallSite, TerminalSuspensionCallTarget, TerminalSuspensionLiveValue,
    TerminalSuspensionLoan, TerminalSuspensionLoanBacking, TerminalSuspensionLoanCapture,
    TerminalSuspensionLoanResource, TerminalSuspensionLoans, TerminalSuspensionPlace,
    TerminalSuspensionStorage, TerminalSuspensionValueLoans, TerminalSuspensionValueType,
    TerminalTaskActivationDemand, TerminalTaskActivationDemands, TerminalTaskCarryObligations,
    TerminalTaskCarryStorage, TerminalTaskFrame, TerminalTaskFrameCall, TerminalTaskLiveCarry,
    TerminalTaskResidentValue, TerminalTaskSignatureValue, TerminalTaskStartOperation,
    TerminalTaskSuspensionCrossing, TerminalTaskUnresolvedCall, TerminalTaskUnresolvedCallKind,
    TerminalTaskValueType, TerminalTraceBoundaryCrashSiteRow, TerminalTraceCrashSiteRow,
    TerminalTraceExternalCompletionSiteRow, TerminalTraceOperationCrashSiteRow,
    TerminalTraceOrdinaryEventKind, TerminalTraceOrdinaryEventRow, TerminalTraceResultKind,
    TerminalTraceResultSchema, TerminalTraceResultValue, TerminalTraceRootRow,
    TerminalTraceScalarSchema, TerminalTraceScalarValue, TerminalTraceStructuralSchema,
    TerminalTraceStructuralValue, TerminalTraceV1ConstructionError, TerminalTraceV1Event,
    TerminalTraceV1Outcome, TerminalTraceV1Profile, TerminalTraceV1Rows,
    TerminalTraceV1RuntimeTrace, TerminalTraceV1TraceBuilder, TerminalTraceValueComparison,
    Terminator, TrappingIntegerOperation, TrappingIntegerPrimitive, ValueDeclaration,
    VocabularyMarker, boundary, canonical_placed_view_identity,
    closed_conformance_application_commitment, closed_conformance_application_report_fingerprint,
    control_flow, field_by_identity, find_boundary_machine, find_structural_case,
    find_structural_type, float_meaning_equality_proposition_id, identity,
    is_static_structural_path, is_static_structural_scalar_store_path,
    is_structural_scalar_store_path, live_field, live_field_by_id, live_field_by_identity,
    live_field_entry, observation, ownership,
    program_local_root_introduction_compatibility_report_identity, proof,
    structural_case_by_identity, structural_case_initializer_path, structural_case_position,
    structural_children, structural_parameter_position, structural_type_by_identity,
    structural_type_contains_reference, structural_type_contains_reference_by,
    structural_type_has_borrowed_storage_by, suspension_frontier_commitment, types,
    unique_structural_declaration, values,
};

mod target_layouts;
pub use target_layouts::{
    LayoutApplicationId, StructuralRecordStorage, TargetLayoutApplication,
    TargetLayoutFieldStorage, TargetLayoutRealization, TargetLayoutSchemaArgument,
    TargetLayoutSchemaField, TargetLayoutSchemaFieldKind,
};

pub use terminal_module::quotients::{
    TerminalQuotientAdaptation, TerminalQuotientAdaptationLicense, TerminalQuotientCatalog,
    TerminalQuotientOperation, TerminalQuotientRepresentation,
};

pub use terminal_module::quotients::{TerminalQuotientTheorem, TerminalQuotientTheoremAdmission};
