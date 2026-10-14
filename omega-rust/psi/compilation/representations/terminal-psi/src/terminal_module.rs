//! The current, self-contained Terminal program.
//!
//! Start with `TerminalModule` below. Its declarations live in `types`,
//! `values`, `control_flow`, and `boundary`; `ownership` and `proof` retain
//! the semantic facts those declarations consume. `observation` defines what
//! consumers compare, while `identity` names the exact published vocabulary.
//! No target layout, selected installation, or transformation history lives here.

pub mod boundary;
pub mod control_flow;
pub mod identity;
pub mod observation;
pub mod ownership;
pub mod proof;
pub mod quotients;
mod task_activation;
pub mod types;
pub mod values;

pub use boundary::{
    BoundaryApplication, BoundaryApplicationArgument, BoundaryApplicationCandidate,
    BoundaryContentGuarantee, BoundaryDeclarationRole, BoundaryMachineDeclaration,
    BoundaryMachineResult, BoundaryOperatorRequirement, BoundaryParameterKind,
    BoundaryStructuralResultDeclaration, BoundaryTypeIdentity, ClosedConformanceApplication,
    ClosedConformanceApplicationCommitment, ClosedConformanceCallableResult,
    ClosedConformanceParameterBinding, ClosedConformanceParameterKind,
    ClosedConformanceRealizationCallable, ClosedConformanceRow, ClosedReachApplication,
    ClosedReachArgument, ClosedReachCall, ClosedReachCallApplication, ClosedReachMachineBinding,
    ClosedReachParameter, ClosedReachSchema, ContentConservationGuarantee,
    InstallationReachDependency, ProgramLocalRootIntroductionSchema, ProviderCandidateConformance,
    ProviderParameterRefinement, ProviderRefinement, ProviderSignature, ProviderSignatureParameter,
    ServiceDeclaration, StructuralDomainRequirement, TerminalBoundaryApplicationDemand,
    TerminalDynamicClaimPath, TerminalDynamicDispatchCatalog, TerminalDynamicInterface,
    TerminalDynamicReferenceRegion, TerminalDynamicSlot, TerminalDynamicStructuralFormal,
    TerminalDynamicTable, TerminalRootServiceReach, closed_conformance_application_commitment,
    closed_conformance_application_report_fingerprint, find_boundary_machine,
    program_local_root_introduction_compatibility_report_identity,
};
pub use control_flow::{
    AtomicAccessEvent, AtomicReadModifyWrite, Block, CrashCause, ElementViewRegion, FenceOrdering,
    FloatResultPolicy, MemoryOrdering, Operation, OperationKind, OperationResult,
    RecordFieldInitializer, RecordFieldValue, ReferenceStorageView, ScalarCaseField,
    StructuralCaseSuccessorEdge, StructuralOperationResult, StructuralResultClaimBinding,
    StructuralStoreDisposition, SuccessorEdge, TargetMachineCallable, TargetMachineFamily,
    TargetMachineFamilyMember, TargetMachineSupply, TerminalBlockNaturalRank, TerminalMachine,
    TerminalNaturalCycle, TerminalNaturalRankComparison, TerminalNaturalRankEdge,
    TerminalRankedScc, Terminator, TrappingIntegerOperation, TrappingIntegerPrimitive,
    structural_parameter_position,
};
pub use identity::{
    CheckedMachineDeclaration, CheckedMachineSpecialization, NominalDeclarationIdentity,
    SemanticFingerprint, TerminalIdentityIndex, TerminalIdentityTable,
    TerminalIdentityTableBuilder, TerminalPsiIdentity, VocabularyMarker,
};
pub use observation::{
    TerminalObservationSchema, TerminalOperationCrashDenotation, TerminalTraceBoundaryCrashSiteRow,
    TerminalTraceCrashSiteRow, TerminalTraceExternalCompletionSiteRow,
    TerminalTraceOperationCrashSiteRow, TerminalTraceOrdinaryEventKind,
    TerminalTraceOrdinaryEventRow, TerminalTraceResultKind, TerminalTraceResultSchema,
    TerminalTraceResultValue, TerminalTraceRootRow, TerminalTraceScalarSchema,
    TerminalTraceScalarValue, TerminalTraceStructuralSchema, TerminalTraceStructuralValue,
    TerminalTraceV1ConstructionError, TerminalTraceV1Event, TerminalTraceV1Outcome,
    TerminalTraceV1Profile, TerminalTraceV1Rows, TerminalTraceV1RuntimeTrace,
    TerminalTraceV1TraceBuilder, TerminalTraceValueComparison,
};
pub use ownership::{
    ClaimTransfer, CompletionReceipt, EntryClaim, LoanCompletion, NominalAffineCleanup,
    RetainedBorrowContentProjection, RetainedBorrowCustody, RetainedBorrowPlace,
    RetainedBorrowPlaceRoot, StructuralAccess, StructuralAffineDiscard, StructuralMultiplicity,
    TerminalAffineCleanupAction, TerminalBorrowBoundarySource, TerminalBorrowOwnerSegment,
    TerminalBorrowPlace, TerminalBorrowPlaceSegment, TerminalPlacedViewInput,
    TerminalReborrowRestorationClass, TerminalReborrowRestoredCallUse,
    TerminalReborrowSharedCohortMember, TerminalSuspensionCallPlan, TerminalSuspensionCallSite,
    TerminalSuspensionCallTarget, TerminalSuspensionLiveValue, TerminalSuspensionLoan,
    TerminalSuspensionLoanBacking, TerminalSuspensionLoanCapture, TerminalSuspensionLoanResource,
    TerminalSuspensionLoans, TerminalSuspensionPlace, TerminalSuspensionStorage,
    TerminalSuspensionValueLoans, TerminalSuspensionValueType, canonical_placed_view_identity,
    suspension_frontier_commitment,
};
pub use proof::{
    ClaimContentProjection, ContentEntryClaim, ContentIdentityReshuffle,
    ContentPartitionComposition, ContentPlaceSubstitution, ContractClause, CrashPredicateTerm,
    CrashRouteBucket, CrashRouteGuard, DirectBlockFloatParameter, DirectCallFloatResult,
    DirectMachineFloatParameter, DirectMachineFloatResult, DirectOperationFloatResult,
    DirectStructuralFloatLeaf, EvidenceContractLane, EvidenceContractLaneKind,
    EvidenceInterfaceIdentity, EvidenceProjectionIdentity, EvidenceRequirementIdentity,
    EvidenceTermDeclaration, FloatMeaningEqualityProposition, FloatMeaningProjection,
    FloatMeaningProjectionOperation, FloatMeaningSource, FloatProjectionContractIdentity,
    FloatProjectionInput, FloatProjectionInputId, FloatSemanticApplication,
    FloatSemanticApplicationOperand, FloatSemanticContractIdentity, MachineContract,
    OutcomeSpecificCallEvidence, OutcomeSpecificCallEvidenceValidity,
    OutcomeSpecificCallResultSubstitution, OutcomeSpecificEnsure, OutcomeSpecificEvidence,
    OutcomeSpecificEvidenceUse, OutcomeSpecificGuard, ProofOnlyValueType, ProofOutput,
    ProofOutputCall, ProofOutputEvidenceArgument, ProofOutputRuntimeCall, ProofOutputRuntimeResult,
    ProofPropositionId, ProofValueDeclaration, ProofValueId, PropositionApplicationIdentity,
    PropositionBinderArgumentIdentity, PropositionBinderArgumentKind, PropositionBinderDeclaration,
    PropositionBinderKind, PropositionDeclaration, PropositionEvidence, ScalarBlockInvariant,
    ScalarBlockInvariantArrival, StaticRequirementDispatch, TerminalOperationCrashContract,
    TerminalProofRankingRelation, TerminalProofRecursiveCallSite, TerminalProofRecursiveComponent,
    TerminalProofRecursiveEdge, TerminalProofRecursiveField, TerminalProofRecursiveMember,
    TerminalProofRecursiveTransitionLane, TerminalProofRecursiveType,
    float_meaning_equality_proposition_id,
};
pub use task_activation::{
    TerminalTaskActivationDemand, TerminalTaskActivationDemands, TerminalTaskCarryObligations,
    TerminalTaskCarryStorage, TerminalTaskFrame, TerminalTaskFrameCall, TerminalTaskLiveCarry,
    TerminalTaskResidentValue, TerminalTaskSignatureValue, TerminalTaskStartOperation,
    TerminalTaskSuspensionCrossing, TerminalTaskUnresolvedCall, TerminalTaskUnresolvedCallKind,
    TerminalTaskValueType,
};
pub use types::{
    ByteSequenceCarrier, ResultQualificationEstablishment, ScalarDomainDeclaration,
    ScalarDomainEstablishmentRoute, ScalarFloatRange, ScalarIntegerRange,
    ScalarQualificationCatalog, ScalarQualificationCoercion, ScalarQualificationSet,
    StructuralCaseDeclaration, StructuralContentProjection, StructuralDomainDeclaration,
    StructuralEstablishmentRoute, StructuralFieldDeclaration, StructuralFieldType,
    StructuralPathQualification, StructuralReferentIdentity, StructuralTypeDeclaration,
    StructuralTypeShape, field_by_identity, find_structural_case, find_structural_type, live_field,
    live_field_by_id, live_field_by_identity, live_field_entry, structural_case_by_identity,
    structural_case_initializer_path, structural_case_position, structural_children,
    structural_type_by_identity, structural_type_contains_reference,
    structural_type_contains_reference_by, structural_type_has_borrowed_storage_by,
    unique_structural_declaration,
};
pub use values::{
    ErasedProofFormal, OperationProjection, ReferenceOriginProjection, ReferenceOriginSource,
    StructuralArgument, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralReferenceResultSource, StructuralResultDeclaration,
    TerminalMachineResult, ValueDeclaration, is_static_structural_path,
    is_static_structural_scalar_store_path, is_structural_scalar_store_path,
};

use semantic_vocabulary::{MachineId, ServiceId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalModule {
    /// Exact whole-value quotient adaptations; ordinary shape carries no observation authority.
    pub quotients: quotients::TerminalQuotientCatalog,
    pub vocabulary_marker: VocabularyMarker,
    /// Every rendered identity the module's rows name, deduplicated once.
    /// `*identity` fields hold `TerminalIdentityIndex` handles into this
    /// table; the codec writes it once and decoding checks its entries once.
    pub identities: TerminalIdentityTable,
    /// Portable callable root. A target-family head is a contract, not an
    /// executable representative: installation selects its exact tagged body
    /// without changing this root or the canonical artifact.
    pub entry: TargetMachineCallable,
    /// Canonical scalar membership vocabulary and explicit successor-edge coercions.
    pub scalar_qualifications: ScalarQualificationCatalog,
    /// Concrete target-neutral instantiated type shapes, ordered by `id`.
    /// Native layout is deliberately absent and is selected by Omega.
    pub structural_types: Vec<StructuralTypeDeclaration>,
    /// Closed identities reached only through reference edges, ordered by id.
    /// Disjoint from storage definitions; actual dereferences demand their
    /// own definition. Growing generic reference chains therefore stay finite.
    pub structural_referents: Vec<types::StructuralReferentIdentity>,
    /// Structural qualification domains, ordered by `id`.
    pub structural_domains: Vec<StructuralDomainDeclaration>,
    /// Boundary-service declarations and their normalized parent closure.
    pub services: Vec<ServiceDeclaration>,
    /// Source-handle-free service-reach closure of the selected entry.
    /// Concrete reach remains distinct from bounded installation dependencies;
    /// final installation substitutes one selected provider row per dependency.
    pub root_service_reach: TerminalRootServiceReach,
    /// Closure-wide direct entry inputs whose opaque placed-view meaning is
    /// bound to one exact source-derived placement interpretation. This is
    /// semantic custody only and grants no runtime storage or access.
    pub placed_view_inputs: Vec<TerminalPlacedViewInput>,
    /// One exact whole-parent mutating call after a one-hop exclusive child
    /// reactivates its direct mutable root. These rows grant use only at the
    /// named call and cannot express cleanup, transfer, or discharge.
    pub reborrow_restored_call_uses: Vec<TerminalReborrowRestoredCallUse>,
    /// Bodyless target-neutral Unit machines callable from terminal Psi.
    pub boundary_machines: Vec<BoundaryMachineDeclaration>,
    /// Every checked, target-neutral provider candidate eligible to realize a
    /// retained Unit boundary requirement. This is a semantic catalog, not a
    /// selection: installation policy remains outside terminal-Psi identity.
    pub provider_candidates: Vec<ProviderCandidateConformance>,
    pub boundary_application_candidates: Vec<crate::BoundaryApplicationCandidate>,
    /// Checked callback applications joined to their emitted registrar operations.
    pub callback_applications: Vec<crate::TerminalCallbackApplication>,
    /// Interfaces and implementation tables runtime selectors name.
    pub dynamic_dispatch: TerminalDynamicDispatchCatalog,
    /// Source-handle-free proof-only float projections. These rows are
    /// semantic-module evidence, never executable operations or runtime values.
    pub float_meaning_projections: Vec<crate::FloatMeaningProjection>,
    /// Proof-only propositions consuming exact float projection results.
    pub float_meaning_equalities: Vec<crate::FloatMeaningEqualityProposition>,
    /// Nominal proof-formula vocabulary, strictly ordered by `id`.
    /// Transparent aliases never receive a declaration row.
    pub proposition_declarations: Vec<PropositionDeclaration>,
    /// Normalized applications retained without frontend arena handles.
    pub proposition_applications: Vec<PropositionApplicationIdentity>,
    /// Canonical erased witness identities. Multiple terms may inhabit the
    /// same proposition application; a forwarding assignment preserves its
    /// source identity and therefore does not add a declaration here.
    pub evidence_terms: Vec<EvidenceTermDeclaration>,
    /// Strictly ordered erased machine-contract lane rows. These reference
    /// term vocabulary identities and have no runtime representation.
    pub evidence_contract_lanes: Vec<EvidenceContractLane>,
    /// Canonical immediate invocations that introduce fresh caller-local
    /// evidence from a proof-output lane. Runtime-value bindings retain
    /// their exact ordinary scalar call operation.
    pub proof_output_calls: Vec<ProofOutputCall>,
    /// Source-free proof-only SCCs reachable from the retained root proof
    /// closure. These are semantic obligation inputs, not producer evidence.
    pub proof_recursive_components: Vec<TerminalProofRecursiveComponent>,
    /// Optional scalar block assertions ordered by machine and header.
    /// Every actual arrival owns an independently checked obligation;
    /// no first-arrival fact or ranking certificate establishes these implicitly.
    pub scalar_block_invariants: Vec<ScalarBlockInvariant>,
    /// Crash contracts of selected operator invocations that lower to one
    /// ordinary operation, ordered by machine and operation. Each row carries
    /// the operator's published routes in its formal namespace beside the
    /// exact continuations the verifier reconstructs from the operation's
    /// operands; calls keep their own carriers.
    pub operation_crash_contracts: Vec<TerminalOperationCrashContract>,
    /// Exact source-handle-free generic conformance applications used by the
    /// retained machine closure. Rows are owned by the concrete terminal
    /// machine whose specialization selected the application.
    pub closed_conformance_applications: Vec<ClosedConformanceApplication>,
    /// Exact source-free live frontier at each possibly-suspending ordinary
    /// call. This is a semantic carry demand only; it adds no control edge,
    /// cleanup action, activation choice, or park/resume behavior.
    pub suspension_call_plan_count: u32,
    /// Independent call-side roster. Every row must have exactly one detailed
    /// plan with the same key/target and committed frontier.
    pub suspension_call_sites: Vec<TerminalSuspensionCallSite>,
    pub suspension_call_plans: Vec<TerminalSuspensionCallPlan>,
    pub machines: Vec<TerminalMachine>,
    /// All target-scoped callable members, including unreachable bodies and
    /// opaque declarations. Selection belongs to subsequent realization.
    pub target_machine_families: Vec<TargetMachineFamily>,
    /// Complete unclosed policy applications; exact deployment selects a result.
    pub target_layout_applications: Vec<crate::TargetLayoutApplication>,
    /// Exact source-free operator requirements and closed applications for
    /// surviving operations. Provider choices belong to Omega, not this roster.
    pub boundary_application_demands: Vec<TerminalBoundaryApplicationDemand>,
    /// Provider-independent task activation evidence: one demand row per
    /// checked `TaskRuntime` start specialization, with the bundle-local
    /// structural and target-layout tables its value types reference.
    pub task_activation_demands: TerminalTaskActivationDemands,
}

impl TerminalModule {
    /// The machine keyed `id`, when the module declares it.
    /// Every machine's blocks, in machine order.
    pub fn blocks(&self) -> impl Iterator<Item = &Block> + '_ {
        self.machines.iter().flat_map(|machine| &machine.blocks)
    }

    /// Every machine's operations, in machine then block order.
    pub fn operations(&self) -> impl Iterator<Item = &Operation> + '_ {
        self.machines
            .iter()
            .flat_map(|machine| machine.operations())
    }

    pub fn machine(&self, id: MachineId) -> Option<&TerminalMachine> {
        self.machines.iter().find(|machine| machine.id == id)
    }

    /// The service keyed `id`, when the module declares it.
    pub fn service(&self, id: ServiceId) -> Option<&ServiceDeclaration> {
        self.services.iter().find(|service| service.id == id)
    }

    /// The structural type keyed `id`, when the module declares it.
    pub fn structural_type(
        &self,
        id: semantic_vocabulary::StructuralTypeId,
    ) -> Option<&StructuralTypeDeclaration> {
        find_structural_type(&self.structural_types, id)
    }

    /// The structural domain keyed `id`, when the module declares it.
    pub fn structural_domain(
        &self,
        id: semantic_vocabulary::StructuralDomainId,
    ) -> Option<&StructuralDomainDeclaration> {
        self.structural_domains
            .iter()
            .find(|declaration| declaration.id == id)
    }

    /// The boundary machine keyed `id`, when the module declares it.
    pub fn boundary_machine(
        &self,
        id: semantic_vocabulary::BoundaryMachineId,
    ) -> Option<&BoundaryMachineDeclaration> {
        self.boundary_machines
            .iter()
            .find(|declaration| declaration.id == id)
    }

    /// A module for `entry` with the current vocabulary and every roster
    /// empty. Producers name only the rosters they fill
    /// (`TerminalModule { machines, structural_types, ..TerminalModule::for_entry(entry) }`),
    /// so a new module field defaults in one place instead of in every
    /// producer's literal.
    pub fn for_entry(entry: MachineId) -> Self {
        Self {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: TargetMachineCallable::Machine(entry),
            identities: Default::default(),
            scalar_qualifications: Default::default(),
            structural_types: Vec::new(),
            structural_referents: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            placed_view_inputs: Vec::new(),
            reborrow_restored_call_uses: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            boundary_application_candidates: Vec::new(),
            callback_applications: Vec::new(),
            dynamic_dispatch: TerminalDynamicDispatchCatalog::default(),
            quotients: Default::default(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            proof_recursive_components: Vec::new(),
            scalar_block_invariants: Vec::new(),
            operation_crash_contracts: Vec::new(),
            closed_conformance_applications: Vec::new(),
            suspension_call_plan_count: 0,
            suspension_call_sites: Vec::new(),
            suspension_call_plans: Vec::new(),
            machines: Vec::new(),
            target_machine_families: Vec::new(),
            target_layout_applications: Vec::new(),
            boundary_application_demands: Vec::new(),
            task_activation_demands: TerminalTaskActivationDemands::default(),
        }
    }
}
