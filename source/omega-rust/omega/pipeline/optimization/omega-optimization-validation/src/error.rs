//! Public failure vocabulary for unit and rewrite validation.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizationUnitValidationError {
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
    OperationServiceContractMismatch {
        machine: MachineId,
        block: BlockId,
        node: u32,
    },
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
        field: psi_core::StructuralFieldId,
    },
    InvalidStructuralCaseIdentity {
        structural_type: StructuralTypeId,
        case: psi_core::StructuralCaseId,
    },
    NonCanonicalStructuralFieldOrder {
        structural_type: StructuralTypeId,
        case: Option<psi_core::StructuralCaseId>,
    },
    NonCanonicalStructuralCaseOrder(StructuralTypeId),
    EmptyStructuralSum(StructuralTypeId),
    InvalidErasedStructuralField {
        structural_type: StructuralTypeId,
        field: psi_core::StructuralFieldId,
    },
    InvalidProviderAttachmentSpecialization(MachineId),
    DuplicateStructuralDomain(StructuralDomainId),
    NonCanonicalStructuralDomainOrder,
    InvalidStructuralDomainIdentity(StructuralDomainId),
    InvalidStructuralDomainContentProjection(StructuralDomainId),
    ContentProjectionOwnerMismatch(psi_core::ContentProjectionIdentity),
    NonCanonicalTrivialAffineLocals(MachineId),
    TrivialAffineLocalDeclarationRequiresEmptyRecord {
        machine: MachineId,
        place: PlaceId,
    },
    TrivialAffineLocalEstablishmentMismatch(MachineId),
    StructuralReturnTrivialAffineLocalsMismatch {
        machine: MachineId,
        block: BlockId,
        node: u32,
    },
    StructuralReturnTrivialAffineShapeMismatch {
        machine: MachineId,
        block: BlockId,
        node: u32,
    },
    StructuralReturnAffineDiscardsMismatch {
        machine: MachineId,
        block: BlockId,
        node: u32,
    },
    StructuralEdgeAffineDiscardsMismatch {
        machine: MachineId,
        edge: EdgeId,
    },
    StructuralReturnHiddenLocalCustodyMismatch {
        machine: MachineId,
        block: BlockId,
        node: u32,
        operation: OperationId,
    },
    StructuralCatalogMismatch {
        machine: Option<MachineId>,
    },
    DuplicateStructuralPlaceRoot {
        machine: MachineId,
        kind: StructuralPlaceKind,
    },
    InvalidBooleanStructuralField {
        machine: MachineId,
        block: BlockId,
        node: u32,
    },
    StructuralReturnSourceContractMismatch {
        machine: MachineId,
        block: BlockId,
        node: u32,
    },
    ScalarOperationContractMismatch {
        machine: MachineId,
        block: BlockId,
        node: u32,
    },
    StructuralCallContractMismatch {
        machine: MachineId,
        block: BlockId,
        node: u32,
    },
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
    ParameterMetadataMismatch {
        machine: MachineId,
        block: Option<BlockId>,
    },
    DuplicateEdge(EdgeId),
    DuplicateProvenance(PsiProvenance),
    CoExecutableProvenanceOccurrences(PsiProvenance),
    IncompleteProvenance {
        machine: MachineId,
        block: BlockId,
        node: u32,
    },
    FuelDoesNotMatchProvenance {
        machine: MachineId,
        block: BlockId,
        node: u32,
    },
    DuplicateFuelSettlement(PsiProvenance),
    OperationMetadataMismatch {
        machine: MachineId,
        block: BlockId,
        node: u32,
    },
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
    StructuralPlaceNotAvailable {
        machine: MachineId,
        block: BlockId,
        node: u32,
        place: PlaceId,
    },
    UnknownClaim {
        machine: MachineId,
        claim: ClaimId,
    },
    CurrentClaimJoinMismatch {
        machine: MachineId,
        block: BlockId,
    },
    CurrentClaimNotLive {
        machine: MachineId,
        block: BlockId,
        node: u32,
        claim: ClaimId,
    },
    CurrentClaimAlreadyLive {
        machine: MachineId,
        block: BlockId,
        node: u32,
        claim: ClaimId,
    },
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
    CurrentOwnedPlaceNotLive {
        machine: MachineId,
        block: BlockId,
        node: u32,
        place: PlaceId,
    },
    CurrentWholePlacePartiallyMoved {
        machine: MachineId,
        block: BlockId,
        node: u32,
        place: PlaceId,
    },
    CurrentProjectedMoveOverlap {
        machine: MachineId,
        block: BlockId,
        node: u32,
        place: PlaceId,
    },
    CurrentCleanupMismatch {
        machine: MachineId,
        block: BlockId,
    },
    CurrentStructuralReturnSourcePartiallyMoved {
        machine: MachineId,
        block: BlockId,
        place: PlaceId,
    },
    TerminalIdentityMismatch,
    ProofFingerprintMismatch,
    AcceptedObligationMismatch(psi_core::ObligationId),
    OperationObligationOwnerMismatch {
        machine: MachineId,
        operation: psi_core::OperationId,
        obligation: psi_core::ObligationId,
    },
    AcceptedObligationFactIndexMismatch,
    ProofQuestionIndexMismatch,
    OwnershipFrontierFactIndexMismatch,
    CurrentValueRangeFactMismatch,
    CurrentValueRangeFactNotApplicable {
        machine: MachineId,
        block: BlockId,
        node: u32,
    },
    CandidateAcceptedObligationFactMismatch,
    MissingStructuralFrontierMachine(MachineId),
    MissingStructuralOperationFrontier {
        machine: MachineId,
        operation: psi_core::OperationId,
    },
    MissingStructuralEdgeFrontier {
        machine: MachineId,
        edge: EdgeId,
    },
    ContextIdentity(psi_terminal_codec::CodecError),
    ContextProofFingerprint(psi_terminal_codec::ProofCodecError),
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
