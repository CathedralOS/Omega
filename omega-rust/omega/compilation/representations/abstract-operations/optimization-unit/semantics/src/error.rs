//! Public failure vocabulary for unit and rewrite validation.
use abstract_operations::AtomicCoherenceViolation;

use optimization_core::OptimizationUnitIdentity;
use optimization_unit::PsiProvenance;
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ClaimId, EdgeId, MachineId, OperationId, PlaceId, ServiceId,
    StructuralDomainId, StructuralPlaceKind, StructuralTypeId, ValueId,
};

/// The operation row a validation error is about: the machine whose function
/// was rejected, the block inside it, and the index of the node in that block.
/// One payload row owner keeps the site spelled once for every error family
/// whose subject is an operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationSite {
    pub machine: MachineId,
    pub block: BlockId,
    pub node: u32,
}

impl OperationSite {
    pub const fn new(machine: MachineId, block: BlockId, node: u32) -> Self {
        Self {
            machine,
            block,
            node,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizationUnitValidationError {
    InvalidRuntimeEvidence {
        reason: &'static str,
    },
    InvalidActivationStorage {
        machine: MachineId,
        reason: &'static str,
    },
    InvalidOwnedValueTransfer {
        machine: MachineId,
        operation: OperationId,
        reason: &'static str,
    },
    InvalidQuotientCatalog(&'static str),
    ContentIdentityMismatch {
        stored: OptimizationUnitIdentity,
        recomputed: OptimizationUnitIdentity,
    },
    WrongFuelSchedule,
    EntryClaimIndexMismatch(MachineId),
    FunctionResultMismatch(MachineId),
    MissingEntryMachine(MachineId),
    DuplicateMachine(MachineId),
    NonCanonicalPrunedMachineRoster,
    ActivePrunedMachineOverlap(MachineId),
    PrunedEntryMachine(MachineId),
    PrunedProviderMachine(MachineId),
    DuplicateBoundaryMachine(BoundaryMachineId),
    DynamicDispatchCatalogMismatch,
    DynamicReferenceContract {
        interface: u32,
        slot: u32,
        reason: &'static str,
    },
    DuplicateService(ServiceId),
    InvalidServiceIdentity(ServiceId),
    InvalidServiceParent {
        service: ServiceId,
        parent: ServiceId,
    },
    NonCanonicalServiceParents(ServiceId),
    RecursiveServiceHierarchy(ServiceId),
    IncompleteServiceParentClosure {
        service: ServiceId,
        ancestor: ServiceId,
    },
    InvalidFunctionServiceCeiling(MachineId),
    InvalidBoundaryServiceCeiling(BoundaryMachineId),
    InvalidProviderServiceRefinement {
        boundary: BoundaryMachineId,
        candidate: MachineId,
    },
    OperationServiceContractMismatch(OperationSite),
    InvalidRootConcreteServiceReach,
    InvalidRootInstallationReachDependency(usize),
    NonCanonicalRootInstallationReachDependencies,
    RootInstallationReachBoundaryMismatch(BoundaryMachineId),
    RootConcreteServiceReachMismatch {
        declared: Vec<ServiceId>,
        derived: Vec<ServiceId>,
    },
    RootInstallationReachDependenciesMismatch,
    DuplicateStructuralType(StructuralTypeId),
    NonCanonicalStructuralTypeOrder,
    InvalidStructuralTypeIdentity(StructuralTypeId),
    InvalidStructuralArrayLength(StructuralTypeId),
    UnknownStructuralType(StructuralTypeId),
    RecursiveStructuralType(StructuralTypeId),
    InvalidStructuralFieldIdentity {
        structural_type: StructuralTypeId,
        field: semantic_vocabulary::StructuralFieldId,
    },
    InvalidStructuralCaseIdentity {
        structural_type: StructuralTypeId,
        case: semantic_vocabulary::StructuralCaseId,
    },
    NonCanonicalStructuralFieldOrder {
        structural_type: StructuralTypeId,
        case: Option<semantic_vocabulary::StructuralCaseId>,
    },
    NonCanonicalStructuralCaseOrder(StructuralTypeId),
    EmptyStructuralSum(StructuralTypeId),
    InvalidStructuralCaseDispatch {
        machine: MachineId,
        source: PlaceId,
    },
    InvalidErasedStructuralField {
        structural_type: StructuralTypeId,
        field: semantic_vocabulary::StructuralFieldId,
    },
    InvalidProviderAttachmentSpecialization(MachineId),
    DuplicateStructuralDomain(StructuralDomainId),
    NonCanonicalStructuralDomainOrder,
    InvalidStructuralDomainIdentity(StructuralDomainId),
    InvalidStructuralDomainContentProjection(StructuralDomainId),
    ContentProjectionOwnerMismatch(semantic_vocabulary::ContentProjectionIdentity),
    NonCanonicalTrivialAffineLocals(MachineId),
    TrivialAffineLocalDeclarationRequiresEmptyRecord {
        machine: MachineId,
        place: PlaceId,
    },
    TrivialAffineLocalEstablishmentMismatch(MachineId),
    StructuralReturnTrivialAffineLocalsMismatch(OperationSite),
    StructuralReturnTrivialAffineShapeMismatch(OperationSite),
    StructuralReturnAffineDiscardsMismatch(OperationSite),
    StructuralEdgeAffineDiscardsMismatch {
        machine: MachineId,
        edge: EdgeId,
    },
    StructuralReturnHiddenLocalCustodyMismatch(OperationSite, OperationId),
    StructuralCatalogMismatch {
        machine: Option<MachineId>,
    },
    DuplicateStructuralPlaceRoot {
        machine: MachineId,
        kind: StructuralPlaceKind,
    },
    InvalidStructuralScalarFieldRead(OperationSite),
    InvalidPrimitiveLocal(OperationSite),
    InvalidWriteOnlyPrimitiveStore(OperationSite),
    InvalidStructuralScalarFieldStore(OperationSite),
    InvalidByteSequenceRead(OperationSite),
    InvalidByteSequenceWrite(OperationSite),
    InvalidByteSequenceSubslice(OperationSite),
    InvalidByteSequenceLength(OperationSite),
    StructuralReturnSourceContractMismatch(OperationSite),
    ScalarOperationContractMismatch(OperationSite),
    AtomicEventCoherenceMismatch(OperationSite, AtomicCoherenceViolation),
    StructuralCallContractMismatch(OperationSite),
    MissingEntryBlock {
        machine: MachineId,
        block: BlockId,
    },
    EntryBlockHasParameters {
        machine: MachineId,
        block: BlockId,
    },
    DuplicateBlock {
        machine: MachineId,
        block: BlockId,
    },
    EmptyBlock {
        machine: MachineId,
        block: BlockId,
    },
    TerminatorNotLast {
        machine: MachineId,
        block: BlockId,
    },
    MissingTerminator {
        machine: MachineId,
        block: BlockId,
    },
    UnknownSuccessor {
        machine: MachineId,
        block: BlockId,
        target: BlockId,
    },
    UnreachableBlock {
        machine: MachineId,
        block: BlockId,
    },
    ControlCycle {
        machine: MachineId,
        block: BlockId,
    },
    RankedCycleFunctionMissing(MachineId),
    RankedCycleTopologyMismatch {
        machine: MachineId,
    },
    RankedCycleFrozenBlockMismatch {
        machine: MachineId,
        block: BlockId,
    },
    RankedCycleComponentSnapshotMismatch,
    RankedCycleRankingEvidenceMismatch {
        machine: MachineId,
    },
    RankedCycleRankingCertificateSnapshotMismatch,
    ParameterMetadataMismatch {
        machine: MachineId,
        block: Option<BlockId>,
    },
    DuplicateEdge(EdgeId),
    DuplicateProvenance(PsiProvenance),
    CoExecutableProvenanceOccurrences(PsiProvenance),
    IncompleteProvenance(OperationSite),
    FuelDoesNotMatchProvenance(OperationSite),
    DuplicateFuelSettlement(PsiProvenance),
    OperationMetadataMismatch(OperationSite),
    FactIndexMismatch(MachineId),
    BrokenEffectChain {
        machine: MachineId,
        expected: u64,
        actual: u64,
    },
    DuplicateValue(ValueId),
    UndefinedValue {
        machine: MachineId,
        block: BlockId,
        value: ValueId,
    },
    NondominatingValue {
        machine: MachineId,
        block: BlockId,
        value: ValueId,
    },
    UseBeforeDefinition {
        machine: MachineId,
        block: BlockId,
        value: ValueId,
    },
    BindingArityMismatch {
        machine: MachineId,
        edge: EdgeId,
    },
    BindingTypeMismatch {
        machine: MachineId,
        edge: EdgeId,
        value: ValueId,
    },
    UnknownPlace {
        machine: MachineId,
        place: PlaceId,
    },
    NonCanonicalByteSequenceLiterals(MachineId),
    ByteSequenceLiteralDeclarationRequiresBorrowedView {
        machine: MachineId,
        place: PlaceId,
    },
    ByteSequenceLiteralEstablishmentMismatch(MachineId),
    StructuralPlaceNotAvailable(OperationSite, PlaceId),
    UnknownClaim {
        machine: MachineId,
        claim: ClaimId,
    },
    CurrentClaimJoinMismatch {
        machine: MachineId,
        block: BlockId,
    },
    CurrentClaimNotLive(OperationSite, ClaimId),
    CurrentClaimAlreadyLive(OperationSite, ClaimId),
    CurrentLinearClaimAtReturn {
        machine: MachineId,
        block: BlockId,
        claim: ClaimId,
    },
    CurrentStructuralReturnClaimSetMismatch {
        machine: MachineId,
        block: BlockId,
    },
    CurrentClaimLiveAfterStructuralReturn {
        machine: MachineId,
        block: BlockId,
        claim: ClaimId,
    },
    CurrentCrashClaimFrontierMismatch {
        machine: MachineId,
        block: BlockId,
    },
    CurrentOwnedPlaceJoinMismatch {
        machine: MachineId,
        block: BlockId,
    },
    CurrentOwnedPlaceNotLive(OperationSite, PlaceId),
    CurrentWholePlacePartiallyMoved(OperationSite, PlaceId),
    CurrentProjectedMoveOverlap(OperationSite, PlaceId),
    /// A root lent to a shared address join was moved, released, written,
    /// exclusively borrowed or discarded while the join could observe it.
    CurrentSharedJoinOriginDisturbed(OperationSite, PlaceId),
    CurrentCleanupMismatch {
        machine: MachineId,
        block: BlockId,
    },
    CurrentStructuralReturnSourcePartiallyMoved {
        machine: MachineId,
        block: BlockId,
        place: PlaceId,
    },
    /// The replayed loan set rejected an establishment, release, transfer,
    /// discard, or completed frontier. `node` is present when the violation
    /// belongs to one operation row rather than an edge or block exit.
    CurrentReferenceCustodyViolation {
        machine: MachineId,
        block: BlockId,
        node: Option<u32>,
        reason: &'static str,
    },
    TerminalIdentityMismatch,
    ProofFingerprintMismatch,
    AcceptedObligationMismatch(semantic_vocabulary::ObligationId),
    OperationObligationOwnerMismatch {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
        obligation: semantic_vocabulary::ObligationId,
    },
    AcceptedObligationFactIndexMismatch,
    ProofQuestionIndexMismatch,
    OwnershipFrontierFactIndexMismatch,
    CurrentValueRangeFactMismatch,
    CurrentValueRangeFactNotApplicable(OperationSite),
    CandidateAcceptedObligationFactMismatch,
    MissingStructuralFrontierMachine(MachineId),
    MissingStructuralOperationFrontier {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    MissingStructuralEdgeFrontier {
        machine: MachineId,
        edge: EdgeId,
    },
    ContextIdentity(terminal_codec::CodecError),
    VerifiedOptimizationUnitProjectionMismatch,
    CandidateInputMismatch,
    CandidateAnalysisContractMismatch,
    CandidateSafetyClassMismatch,
    CandidateLocationMissing,
    CandidatePatchMismatch,
    CandidateProvenanceMismatch,
    CandidateFuelMismatch,
    CandidateOperandFactMismatch,
    CandidateEvaluationMismatch,
    CandidateObservationMismatch,
    CandidateLiveBoundaryMismatch,
    CandidateRegionObservationUnavailable,
    CandidateRegionObservationMismatch,
    CandidateReachabilityMismatch,
    CandidateOutsideRegionMismatch,
    CandidateBlockParameterMismatch,
    CandidateIncomingBindingMismatch,
    CandidateSubstitutionMismatch,
}

impl std::fmt::Display for OptimizationUnitValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid Psi optimization unit: {self:?}")
    }
}

impl std::error::Error for OptimizationUnitValidationError {}
