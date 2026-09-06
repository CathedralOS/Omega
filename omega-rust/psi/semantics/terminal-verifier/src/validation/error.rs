use semantic_vocabulary::{
    BlockId, BoundaryMachineId, CanonicalStructuralPathSegment, ClaimId, ContentProjectionIdentity,
    ContentStructuralPlace, ContractId, EdgeId, EvidenceTermId, MachineId, ObligationId,
    OperationId, PlaceId, PropositionError, PropositionId, ScalarType, ServiceId, StructuralCaseId,
    StructuralDomainId, StructuralFieldId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    BoundaryMachineResult, CrashCause, EvidenceContractLaneKind, StructuralAccess,
    StructuralMultiplicity, StructuralPathSegment,
};
use terminal_semantics::OperationSemanticError;

use super::foundation::{ServiceCeilingOwner, StructuralSignatureOwner};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractClauseKind {
    Requires,
    Ensures,
    Crash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuspensionCallPlanError {
    CountMismatch,
    SiteMismatch,
    NonCanonical,
    DuplicateOperation,
    DuplicateCrossing,
    UnknownOperation,
    RedirectedToNonCall,
    UnderstatedPolicy,
    UnknownPlace,
    TypeMismatch,
    InvalidClaimFrontier,
    InvalidCallArgument,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleError {
    InvalidSuspensionCallPlan {
        operation: Option<OperationId>,
        reason: SuspensionCallPlanError,
    },
    NonCanonicalDynamicDescriptorParameterOrder,
    DuplicateDynamicDescriptorParameter {
        owner: MachineId,
        ordinal: u32,
    },
    NonDenseDynamicDescriptorParameter {
        owner: MachineId,
        expected: u32,
        actual: u32,
    },
    InvalidDynamicDescriptorParameter {
        owner: MachineId,
        ordinal: u32,
    },
    NonCanonicalDynamicDescriptorArgumentOrder,
    DuplicateDynamicDescriptorArgument {
        owner: MachineId,
        operation: OperationId,
        parameter_ordinal: u32,
    },
    InvalidDynamicDescriptorArgument {
        owner: MachineId,
        operation: OperationId,
        parameter_ordinal: u32,
    },
    NonCanonicalParameterDynamicDispatchOrder,
    DuplicateParameterDynamicDispatch {
        owner: MachineId,
        operation: OperationId,
    },
    InvalidParameterDynamicDispatch {
        owner: MachineId,
        operation: OperationId,
    },
    NonCanonicalDynamicConformanceSelectionOrder,
    DuplicateDynamicConformanceSelection {
        owner: MachineId,
        ordinal: u32,
    },
    NonDenseDynamicConformanceSelection {
        owner: MachineId,
        expected: u32,
        actual: u32,
    },
    InvalidDynamicConformanceSelection {
        owner: MachineId,
        ordinal: u32,
    },
    OrphanDynamicConformanceSelection {
        owner: MachineId,
        ordinal: u32,
    },
    NonCanonicalDirectDynamicDispatchOrder,
    DuplicateDirectDynamicDispatch {
        owner: MachineId,
        operation: OperationId,
    },
    InvalidDirectDynamicDispatch {
        owner: MachineId,
        operation: OperationId,
    },
    NonCanonicalReboundDynamicDescriptorOrder,
    DuplicateReboundDynamicDescriptor {
        owner: MachineId,
        ordinal: u32,
    },
    NonDenseReboundDynamicDescriptor {
        owner: MachineId,
        expected: u32,
        actual: u32,
    },
    InvalidReboundDynamicDescriptor {
        owner: MachineId,
        ordinal: u32,
    },
    OrphanReboundDynamicDescriptor {
        owner: MachineId,
        ordinal: u32,
    },
    NonCanonicalStoredDynamicDescriptorOrder,
    DuplicateStoredDynamicDescriptor {
        owner: MachineId,
        ordinal: u32,
    },
    NonDenseStoredDynamicDescriptor {
        owner: MachineId,
        expected: u32,
        actual: u32,
    },
    InvalidStoredDynamicDescriptor {
        owner: MachineId,
        ordinal: u32,
    },
    OrphanStoredDynamicDescriptor {
        owner: MachineId,
        ordinal: u32,
    },
    NonCanonicalIndirectDynamicDispatchOrder,
    DuplicateIndirectDynamicDispatch {
        owner: MachineId,
        operation: OperationId,
    },
    InvalidIndirectDynamicDispatch {
        owner: MachineId,
        operation: OperationId,
    },
    NonCanonicalStoredDynamicDispatchOrder,
    DuplicateStoredDynamicDispatch {
        owner: MachineId,
        operation: OperationId,
    },
    InvalidStoredDynamicDispatch {
        owner: MachineId,
        operation: OperationId,
    },
    NonCanonicalReborrowRestoredCallUseOrder,
    DuplicateReborrowRestoredCallUse,
    DuplicateReborrowRestoredCallLifecycle,
    InvalidReborrowRestoredCallUse {
        machine: MachineId,
        operation: OperationId,
    },
    NonCanonicalReborrowRootHandoffOrder,
    DuplicateReborrowRootHandoff,
    InvalidReborrowRootHandoff {
        machine: MachineId,
    },
    NonCanonicalPlacedViewInputOrder,
    DuplicatePlacedViewInput {
        machine: MachineId,
        source_state_identity: String,
        position: u32,
    },
    InvalidPlacedViewInput {
        machine: MachineId,
        position: u32,
    },
    NonCanonicalQuotientCorrespondenceOrder,
    DuplicateQuotientCorrespondenceIdentity,
    DuplicateQuotientCorrespondenceOwner,
    InvalidQuotientCorrespondence {
        index: usize,
        error: crate::QuotientCorrespondenceReplayError,
    },
    NonExecutableQuotientCorrespondence,
    NonDenseFloatMeaningProjection {
        expected: u32,
        result: u32,
        transitional_source: Option<u32>,
    },
    InvalidFloatMeaningProjection {
        index: u32,
        error: crate::verification::FloatMeaningProjectionVerificationError,
    },
    InconsistentFloatMeaningProjectionSourceFormat {
        source: u32,
    },
    DuplicateFloatMeaningProjection {
        first: u32,
        duplicate: u32,
    },
    NonDenseFloatMeaningEquality {
        expected: u32,
        actual: u32,
    },
    NonCanonicalFloatMeaningEqualityOperands {
        proposition: u32,
        left: u32,
        right: u32,
    },
    UnknownFloatMeaningEqualityOperand {
        proposition: u32,
        operand: u32,
    },
    OperationSemanticSchema(OperationSemanticError),
    OperationPredicateDenotation(Box<proof_admission::PredicateDenotationError>),
    InvalidPartialAffineCleanup {
        machine: MachineId,
        block: BlockId,
    },
    InvalidNominalAffineCleanup {
        machine: MachineId,
        block: BlockId,
    },
    EmptyModule,
    UnknownClosedConformanceOwner(MachineId),
    InvalidClosedConformanceApplication {
        owner: MachineId,
        declaration: String,
    },
    DuplicateClosedConformanceApplication {
        owner: MachineId,
        report_fingerprint: u64,
    },
    ClosedConformanceFingerprintMismatch {
        owner: MachineId,
        expected: u64,
        actual: u64,
    },
    ClosedConformanceCommitmentMismatch {
        owner: MachineId,
    },
    DuplicatePropositionDeclaration(PropositionId),
    DuplicatePropositionApplication(PropositionId),
    NonDensePropositionDeclaration {
        expected: PropositionId,
        actual: PropositionId,
    },
    NonDensePropositionApplication {
        expected: PropositionId,
        actual: PropositionId,
    },
    NonDenseEvidenceTerm {
        expected: EvidenceTermId,
        actual: EvidenceTermId,
    },
    DuplicatePropositionName(String),
    UnknownPropositionDeclaration(PropositionId),
    InvalidPropositionBinder(PropositionId),
    PropositionApplicationArityMismatch(PropositionId),
    PropositionApplicationBinderMismatch(PropositionId),
    InvalidPropositionEvidenceInterface(PropositionId),
    UnknownEvidenceTermProposition(PropositionId),
    FactOnlyEvidenceTerm(PropositionId),
    InvalidEvidenceInterface(EvidenceTermId),
    EvidenceTermInterfaceMismatch(EvidenceTermId),
    UnknownEvidenceProjectionTerm {
        proposition: PropositionId,
        term: EvidenceTermId,
    },
    EvidenceProjectionRequirementMismatch {
        proposition: PropositionId,
        term: EvidenceTermId,
    },
    UnknownEvidenceContractMachine(MachineId),
    UnknownEvidenceContractTerm(EvidenceTermId),
    EvidenceContractTermMismatch(EvidenceTermId),
    NonDenseEvidenceContractLane {
        machine: MachineId,
        kind: EvidenceContractLaneKind,
        expected: u32,
        actual: u32,
    },
    EvidenceContractLaneOverflow {
        machine: MachineId,
        kind: EvidenceContractLaneKind,
    },
    EvidenceRequiresHasOutputField {
        machine: MachineId,
        position: u32,
    },
    InvalidOutcomeSpecificGuard {
        machine: MachineId,
        result_type: semantic_vocabulary::StructuralTypeId,
        result_case: semantic_vocabulary::StructuralCaseId,
    },
    NonCanonicalOutcomeSpecificEnsures(MachineId),
    NonDenseOutcomeSpecificEnsures {
        machine: MachineId,
        guard: terminal_psi::OutcomeSpecificGuard,
        expected: u32,
        actual: u32,
    },
    OutcomeSpecificEnsureOverflow {
        machine: MachineId,
        guard: terminal_psi::OutcomeSpecificGuard,
    },
    InvalidOutcomeSpecificEvidenceField {
        machine: MachineId,
        position: u32,
    },
    OutcomeSpecificEvidenceMismatch {
        machine: MachineId,
        position: u32,
    },
    OutcomeSpecificGuaranteeReplayUnavailable {
        machine: MachineId,
        obligation: ObligationId,
    },
    MissingEvidenceOutputField {
        machine: MachineId,
        position: u32,
    },
    InvalidEvidenceOutputField(MachineId),
    ReservedEvidenceOutputField(MachineId),
    DuplicateEvidenceOutputField(MachineId),
    NonCanonicalProofOutputCall {
        caller: MachineId,
        ordinal: u32,
    },
    InvalidProofOutputCall {
        caller: MachineId,
        ordinal: u32,
    },
    InvalidOutcomeSpecificCallEvidence {
        caller: MachineId,
        operation: OperationId,
    },
    OrphanEvidenceTerm(EvidenceTermId),
    EmptyPropositionIdentity,
    DuplicateMachine(MachineId),
    DuplicateStructuralType(StructuralTypeId),
    InvalidStructuralTypeIdentity(StructuralTypeId),
    InvalidStructuralFieldIdentity {
        structural_type: StructuralTypeId,
        field: semantic_vocabulary::StructuralFieldId,
    },
    InvalidStructuralCaseIdentity {
        structural_type: StructuralTypeId,
        case: semantic_vocabulary::StructuralCaseId,
    },
    EmptyStructuralSum(StructuralTypeId),
    InvalidErasedStructuralField {
        structural_type: StructuralTypeId,
        field: semantic_vocabulary::StructuralFieldId,
    },
    InvalidStructuralArrayLength(StructuralTypeId),
    UnknownStructuralType(StructuralTypeId),
    RecursiveStructuralType(StructuralTypeId),
    DuplicateStructuralDomain(StructuralDomainId),
    InvalidStructuralDomainIdentity(StructuralDomainId),
    InvalidStructuralDomainContentProjection(StructuralDomainId),
    UnknownStructuralDomain(StructuralDomainId),
    StructuralDomainCarrierMismatch {
        domain: StructuralDomainId,
        expected: StructuralTypeId,
        actual: StructuralTypeId,
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
    InvalidInstallationReachDependency(usize),
    NonCanonicalInstallationReachDependencies,
    InstallationReachBoundaryMismatch(BoundaryMachineId),
    RootConcreteServiceReachMismatch {
        declared: Vec<ServiceId>,
        derived: Vec<ServiceId>,
    },
    RootInstallationReachDependenciesMismatch,
    DuplicateBoundaryMachine(BoundaryMachineId),
    InvalidBoundaryMachineIdentity(BoundaryMachineId),
    UnknownMachineAttachment {
        machine: MachineId,
        attachment: StructuralTypeId,
    },
    UnknownBoundaryAttachment {
        boundary: BoundaryMachineId,
        attachment: StructuralTypeId,
    },
    NonDenseStructuralParameter {
        owner: StructuralSignatureOwner,
        expected: u32,
        actual: u32,
    },
    DuplicateStructuralParameterPlace(PlaceId),
    InvalidStructuralSelfParameter {
        owner: StructuralSignatureOwner,
    },
    DuplicateStructuralQualification {
        place: PlaceId,
        domain: StructuralDomainId,
    },
    NonCanonicalStructuralQualifications(PlaceId),
    InvalidProjectedStructuralQualificationPath {
        place: PlaceId,
        path: Vec<StructuralPathSegment>,
    },
    ProjectedStructuralQualificationCarrierMismatch {
        place: PlaceId,
        path: Vec<StructuralPathSegment>,
        domain: StructuralDomainId,
        expected: StructuralTypeId,
        actual: StructuralTypeId,
    },
    NonCanonicalProjectedStructuralQualifications(PlaceId),
    DuplicatePublishedService {
        owner: ServiceCeilingOwner,
        service: ServiceId,
    },
    NonCanonicalPublishedServiceCeiling(ServiceCeilingOwner),
    UnknownPublishedService {
        owner: ServiceCeilingOwner,
        service: ServiceId,
    },
    IncompletePublishedServiceClosure {
        owner: ServiceCeilingOwner,
        service: ServiceId,
    },
    BoundaryRequirementArgumentOutOfRange {
        boundary: BoundaryMachineId,
        argument_index: u32,
    },
    DuplicateBoundaryRequirement {
        boundary: BoundaryMachineId,
        argument_index: u32,
        domain: StructuralDomainId,
    },
    NonCanonicalBoundaryRequirements(BoundaryMachineId),
    InvalidProgramLocalRootIntroduction {
        boundary: BoundaryMachineId,
        argument_index: u32,
    },
    DuplicateProgramLocalRootIntroduction {
        boundary: BoundaryMachineId,
        argument_index: u32,
        domain: StructuralDomainId,
    },
    NonCanonicalProgramLocalRootIntroductions(BoundaryMachineId),
    InvalidProviderCandidate {
        boundary: BoundaryMachineId,
        candidate: MachineId,
    },
    InvalidProviderAttachmentSpecialization(MachineId),
    StructuralParameterPlaceMismatch {
        machine: MachineId,
        place: PlaceId,
    },
    StructuralPlaceHasNoParameter {
        machine: MachineId,
        place: PlaceId,
    },
    UnknownByteSequenceLiteral {
        operation: OperationId,
        place: PlaceId,
    },
    ByteSequenceLiteralRequiresBorrowedView {
        operation: OperationId,
        place: PlaceId,
    },
    ByteSequenceLiteralDeclarationRequiresBorrowedView {
        machine: MachineId,
        place: PlaceId,
    },
    ByteSequenceLiteralEstablishmentMismatch(MachineId),
    NonCanonicalByteSequenceLiterals(MachineId),
    UnknownTrivialAffineLocal {
        operation: OperationId,
        place: PlaceId,
    },
    TrivialAffineLocalRequiresEmptyRecord {
        operation: OperationId,
        place: PlaceId,
    },
    TrivialAffineLocalDeclarationRequiresEmptyRecord {
        machine: MachineId,
        place: PlaceId,
    },
    TrivialAffineLocalEstablishmentMismatch(MachineId),
    NonCanonicalTrivialAffineLocals(MachineId),
    PayloadlessCaseResultMismatch(OperationId),
    PayloadlessCaseRequiresSum {
        operation: OperationId,
        structural_type: semantic_vocabulary::StructuralTypeId,
        result_case: semantic_vocabulary::StructuralCaseId,
    },
    PayloadlessCaseRequiresPayloadlessMember {
        operation: OperationId,
        structural_type: semantic_vocabulary::StructuralTypeId,
        result_case: semantic_vocabulary::StructuralCaseId,
    },
    AffineScalarRecordResultMismatch(OperationId),
    AffineScalarRecordRequiresSingleI64Field {
        operation: OperationId,
        structural_type: StructuralTypeId,
        field: StructuralFieldId,
    },
    AffineScalarRecordValueOutsideI64(OperationId),
    WriteOnlyPrimitiveStoreDestinationMismatch {
        operation: OperationId,
        place: PlaceId,
    },
    WriteOnlyPrimitiveStoreRequiresPrimitiveScalar {
        operation: OperationId,
        structural_type: StructuralTypeId,
    },
    WriteOnlyPrimitiveStoreValueTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual: ScalarType,
    },
    InvalidStructuralScalarFieldStore {
        operation: OperationId,
        destination: PlaceId,
        path: Vec<StructuralPathSegment>,
        field: StructuralFieldId,
    },
    StructuralScalarFieldStoreValueTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual: ScalarType,
    },
    TrivialAffineLocalAlreadyLive {
        operation: OperationId,
        place: PlaceId,
    },
    StructuralResultMustBeOwned(MachineId),
    StructuralResultPlaceMismatch {
        machine: MachineId,
        place: PlaceId,
    },
    DuplicateEntryClaimInput(PlaceId),
    InvalidEntryClaimFieldPath(ClaimId),
    OverlappingEntryClaimInput {
        first: ClaimId,
        second: ClaimId,
    },
    NonCanonicalEntryClaimOrder(MachineId),
    EntryClaimRequiresStructuralParameter(ClaimId),
    EntryClaimRequiresOwnedParameter(ClaimId),
    LinearParameterHasNoEntryClaim {
        machine: MachineId,
        place: PlaceId,
    },
    IncompleteFixedArrayEntryClaims {
        machine: MachineId,
        place: PlaceId,
    },
    DuplicateBlock(BlockId),
    DuplicateContract(ContractId),
    NonCanonicalProofRecursiveComponents,
    InvalidProofRecursiveComponent,
    InvalidProofRecursiveMember(ContractId),
    InvalidProofRecursiveEdge {
        caller: ContractId,
        callee: ContractId,
    },
    DuplicateOperation(OperationId),
    ScalarOperationHasUnitResult(OperationId),
    UnitOperationHasScalarResult(OperationId),
    BoundaryCallResultMismatch {
        operation: OperationId,
        expected: BoundaryMachineResult,
        actual: Option<BoundaryMachineResult>,
    },
    BoundaryCallArgumentArityMismatch {
        operation: OperationId,
        expected: usize,
        actual: usize,
    },
    UnknownBoundaryCallArgument {
        operation: OperationId,
        argument: ValueId,
    },
    BoundaryCallArgumentUsedBeforeDefinition {
        operation: OperationId,
        argument: ValueId,
    },
    BoundaryCallArgumentTypeMismatch {
        operation: OperationId,
        argument: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    UnitCallTargetHasScalarSignature {
        operation: OperationId,
        callee: MachineId,
    },
    StructuralScalarCallTargetMismatch {
        operation: OperationId,
        callee: MachineId,
        expected: Option<ScalarType>,
        actual: Option<ScalarType>,
    },
    StructuralCallTargetMismatch {
        operation: OperationId,
        callee: MachineId,
    },
    StructuralCallResultMismatch(OperationId),
    StructuralCallResultPlaceMismatch(OperationId),
    StructuralCallClaimInterfaceMismatch(OperationId),
    NonCanonicalStructuralOperationResult(OperationId),
    ProjectedUnitCallOutsideBoundedSlice {
        operation: OperationId,
    },
    ProjectedUnitCallContractUsesStructuralParameter {
        operation: OperationId,
        callee: MachineId,
        place: PlaceId,
    },
    UnitCallContractPlaceHasNoArgument {
        operation: OperationId,
        callee: MachineId,
        place: PlaceId,
    },
    UnknownBoundaryCallTarget {
        operation: OperationId,
        boundary: BoundaryMachineId,
    },
    StructuralArgumentArityMismatch {
        operation: OperationId,
        expected: usize,
        actual: usize,
    },
    UnknownStructuralArgument {
        operation: OperationId,
        argument_index: u32,
        place: PlaceId,
    },
    InvalidStructuralArgumentPath {
        operation: OperationId,
        argument_index: u32,
    },
    StructuralArgumentTypeMismatch {
        operation: OperationId,
        argument_index: u32,
        expected: StructuralTypeId,
        actual: StructuralTypeId,
    },
    StructuralArgumentMultiplicityMismatch {
        operation: OperationId,
        argument_index: u32,
        expected: StructuralMultiplicity,
        actual: StructuralMultiplicity,
    },
    StructuralArgumentAccessMismatch {
        operation: OperationId,
        argument_index: u32,
        expected: StructuralAccess,
        actual: StructuralAccess,
    },
    StructuralArgumentAccessExceedsSource {
        operation: OperationId,
        argument_index: u32,
        source: StructuralAccess,
        presented: StructuralAccess,
    },
    OverlappingExclusiveStructuralArguments {
        operation: OperationId,
        first_argument: u32,
        second_argument: u32,
    },
    StructuralArgumentMissingQualification {
        operation: OperationId,
        argument_index: u32,
        domain: StructuralDomainId,
    },
    UnknownOperationService {
        operation: OperationId,
        service: ServiceId,
    },
    OperationServiceOutsidePublishedCeiling {
        operation: OperationId,
        service: ServiceId,
    },
    UnitCallClaimTransferCountMismatch {
        operation: OperationId,
        expected: usize,
        actual: usize,
    },
    UnitCallClaimHasNoStructuralArgument {
        operation: OperationId,
        claim: ClaimId,
    },
    UnitCallClaimPresenceMismatch {
        operation: OperationId,
        argument_index: u32,
    },
    UnitCallContentClaimMismatch {
        operation: OperationId,
        argument_index: u32,
    },
    DuplicateUnitCallClaimTransfer(OperationId),
    NonCanonicalUnitCallClaimTransfers(OperationId),
    MissingUnitCallClaimTransfer {
        operation: OperationId,
        argument_index: u32,
    },
    ClaimActionArgumentOutOfRange {
        operation: OperationId,
        argument_index: u32,
    },
    UnknownClaimAtOperation {
        operation: OperationId,
        claim: ClaimId,
    },
    ClaimActionPlaceMismatch {
        operation: OperationId,
        claim: ClaimId,
        argument_index: u32,
    },
    BoundaryArgumentMissingQualification {
        operation: OperationId,
        argument_index: u32,
        domain: StructuralDomainId,
    },
    DuplicateBoundaryCompletionReceipt(OperationId),
    NonCanonicalBoundaryCompletionReceipts(OperationId),
    BoundaryCompletionReceiptMismatch(OperationId),
    ClaimNotLiveAtOperation {
        operation: OperationId,
        claim: ClaimId,
    },
    OwnedStructuralPlaceNotLiveAtOperation {
        operation: OperationId,
        place: PlaceId,
    },
    OverlappingProjectedStructuralMove {
        operation: OperationId,
        place: PlaceId,
    },
    PartiallyMovedStructuralPlaceUsedWholeAtOperation {
        operation: OperationId,
        place: PlaceId,
    },
    ClaimFrontierJoinMismatch(BlockId),
    OwnedStructuralFrontierJoinMismatch(BlockId),
    LiveLinearClaimAtUnitReturn {
        machine: MachineId,
        block: BlockId,
        claim: ClaimId,
    },
    PartialStructuralCustodyAtUnitReturn {
        machine: MachineId,
        block: BlockId,
        place: PlaceId,
    },
    UnitReturnAffineDiscardsMismatch {
        machine: MachineId,
        block: BlockId,
    },
    ScalarReturnAffineDiscardsMismatch {
        machine: MachineId,
        block: BlockId,
    },
    EdgeAffineDiscardsInvalid {
        edge: EdgeId,
    },
    LiveLinearClaimAtScalarReturn {
        machine: MachineId,
        block: BlockId,
        claim: ClaimId,
    },
    StructuralReturnFromNonStructuralMachine {
        machine: MachineId,
        block: BlockId,
    },
    StructuralReturnRequiresParameterSource {
        machine: MachineId,
        block: BlockId,
        place: PlaceId,
    },
    StructuralReturnSourceNotLive {
        machine: MachineId,
        block: BlockId,
        place: PlaceId,
    },
    StructuralReturnSourcePartiallyMoved {
        machine: MachineId,
        block: BlockId,
        place: PlaceId,
    },
    StructuralReturnSignatureMismatch {
        machine: MachineId,
        block: BlockId,
    },
    NonCanonicalStructuralReturnClaims {
        machine: MachineId,
        block: BlockId,
    },
    StructuralReturnClaimSetMismatch {
        machine: MachineId,
        block: BlockId,
    },
    StructuralReturnAffineDiscardsMismatch {
        machine: MachineId,
        block: BlockId,
    },
    LiveClaimAtStructuralReturn {
        machine: MachineId,
        block: BlockId,
        claim: ClaimId,
    },
    DuplicateEdge(EdgeId),
    DuplicateObligation(ObligationId),
    NonCanonicalContractEnsures(ContractId),
    DuplicateValue(ValueId),
    DuplicatePlace(PlaceId),
    DuplicateClaim(ClaimId),
    NonDenseStructuralEntryClaim {
        machine: MachineId,
        expected: ClaimId,
        actual: ClaimId,
    },
    DuplicateStructuralPlaceRoot {
        machine: MachineId,
        kind: semantic_vocabulary::StructuralPlaceKind,
    },
    ContentPartitionSourceLocalUnsupported(PlaceId),
    UnitMachineHasResultStructuralPlace {
        machine: MachineId,
        place: PlaceId,
    },
    ScalarMachineHasResultStructuralPlace {
        machine: MachineId,
        place: PlaceId,
    },
    UnknownEntryMachine(MachineId),
    MachineHasNoBlocks(MachineId),
    UnknownEntryBlock {
        machine: MachineId,
        block: BlockId,
    },
    EntryBlockCannotHaveParameters(BlockId),
    ContractValueOutsideScope {
        contract: ContractId,
        clause: ContractClauseKind,
        value: ValueId,
    },
    InvalidBooleanFieldTerm {
        machine: MachineId,
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
    },
    InvalidIntegerFieldTerm {
        machine: MachineId,
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
        scalar_type: semantic_vocabulary::IntegerType,
    },
    InvalidIeeeFloatFieldTerm {
        machine: MachineId,
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
        format: semantic_vocabulary::IeeeFloatFormat,
    },
    InvalidByteSequenceFieldTerm {
        machine: MachineId,
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
    },
    InvalidStructuralCaseMembership {
        machine: MachineId,
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
        case: semantic_vocabulary::StructuralCaseId,
    },
    UnsafeStructuralCrashExactDivisor {
        machine: MachineId,
        scalar_type: semantic_vocabulary::IntegerType,
    },
    UnsafeStructuralCrashPolicyDivisor {
        machine: MachineId,
        scalar_type: semantic_vocabulary::IntegerType,
    },
    UnsafeStructuralCrashExactShift {
        machine: MachineId,
        value_type: semantic_vocabulary::IntegerType,
        count_type: semantic_vocabulary::IntegerType,
        left_shift: bool,
    },
    NonCanonicalCrashRoutes(MachineId),
    EmptyCrashRouteBucket {
        machine: MachineId,
        cause: CrashCause,
    },
    NonCanonicalCrashRouteAlternatives {
        machine: MachineId,
        cause: CrashCause,
    },
    NonCanonicalCrashSiteGuard(BlockId),
    CrashSiteReconstructionLimitExceeded(MachineId),
    CrashSiteGuardUnproved {
        block: BlockId,
        edge: EdgeId,
        predicate: usize,
    },
    CrashRouteUncovered {
        block: BlockId,
        cause: CrashCause,
    },
    NonCanonicalCrashFrontier(BlockId),
    CrashFrontierMismatch {
        block: BlockId,
    },
    NonDenseContentEntryClaim {
        expected: ClaimId,
        actual: ClaimId,
    },
    ContentEntryClaimHasNoProjections(ClaimId),
    NonCanonicalContentEntryProjectionOrder(ClaimId),
    ContentEntryClaimRequiresEntryParameter(ClaimId),
    ContentEntryClaimStructuralBindingMismatch(ClaimId),
    DuplicateContentEntryClaimInput(ContentStructuralPlace),
    OverlappingContentEntryClaimInput {
        first: ContentStructuralPlace,
        second: ContentStructuralPlace,
    },
    ContentIdentityReshuffleHasNoProjections(ClaimId),
    ContentIdentityClaimHasNoEntryBinding(ClaimId),
    ContentIdentityEntryBindingMismatch(ClaimId),
    NonCanonicalContentIdentityProjectionOrder(ClaimId),
    NonCanonicalContentIdentityReshuffles(MachineId),
    ContentIdentityReshuffleRequiresEntryParameter(ClaimId),
    ContentIdentityReshuffleRequiresCurrentResult(ClaimId),
    ContentIdentityReshuffleRequiresStructuralResult(MachineId),
    DuplicateContentIdentityInput(ContentStructuralPlace),
    DuplicateContentIdentityOutput(ContentStructuralPlace),
    OverlappingContentIdentityInput {
        first: ContentStructuralPlace,
        second: ContentStructuralPlace,
    },
    OverlappingContentIdentityOutput {
        first: ContentStructuralPlace,
        second: ContentStructuralPlace,
    },
    ContentProjectionAlgebraMismatch(ContentProjectionIdentity),
    ContentProjectionOwnerMismatch(ContentProjectionIdentity),
    DuplicateContentPartitionComposition,
    ContentPartitionCompositionHasNoInputClaims,
    NonCanonicalContentPartitionInputClaims,
    NonCanonicalContentPartitionSubstitutions,
    DuplicateContentPartitionSubstitutionTarget,
    ContentPartitionAlgebraMismatch,
    ContentPartitionSourceHasNoSeparation,
    ContentPartitionSourceFingerprintMismatch {
        recorded: u64,
        reconstructed: Option<u64>,
    },
    DuplicateContentPartitionSourcePlace(PlaceId),
    DuplicateContentPartitionSourceRoot(StructuralPlaceKind),
    InvalidContentPartitionSubstitutionShape,
    ContentPartitionSubstitutionCoverageMismatch,
    ContentPartitionReplayMismatch,
    ContentPartitionInputProjectionNotClaimBound(ContentStructuralPlace),
    ContentPartitionInputClaimNotListed(ClaimId),
    ContentPartitionInputClaimUnused,
    ContentPartitionProducerOperationMissing(OperationId),
    ContentPartitionProducerNotCall(OperationId),
    ContentPartitionProducerGuaranteeMissing(OperationId),
    ContentPartitionProducerArgumentMismatch(OperationId),
    NonCanonicalBoundaryContentGuarantees(BoundaryMachineId),
    InvalidBoundaryContentGuarantee(BoundaryMachineId),
    RetainedBorrowBoundaryIsNotExecutable {
        operation: OperationId,
        boundary: BoundaryMachineId,
    },
    ContentConservationRequiresEnsures {
        contract: ContractId,
    },
    UnknownTargetBlock(BlockId),
    UnknownValue(ValueId),
    ValueUsedBeforeDefinition(ValueId),
    UnknownCallTarget {
        operation: OperationId,
        callee: MachineId,
    },
    NonCanonicalCallCrashContinuations(OperationId),
    CallCrashContinuationsMismatch {
        operation: OperationId,
        callee: MachineId,
    },
    CallCrashContinuationUncovered {
        operation: OperationId,
        cause: CrashCause,
    },
    CallTargetHasStructuralContract {
        operation: OperationId,
        callee: MachineId,
    },
    CallTargetReturnsUnit {
        operation: OperationId,
        callee: MachineId,
    },
    CallResultTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual: ScalarType,
    },
    CallArgumentArityMismatch {
        operation: OperationId,
        expected: usize,
        actual: usize,
    },
    CallArgumentTypeMismatch {
        operation: OperationId,
        argument: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    CallRequirementArityMismatch {
        operation: OperationId,
        expected: usize,
        actual: usize,
    },
    RecursiveCallSliceNotYetSupported(MachineId),
    IntegerConstantRequiresIntegerResult(OperationId),
    IntegerConstantOutsideResultType(OperationId),
    BooleanConstantRequiresBooleanResult(OperationId),
    IeeeFloatConstantResultTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual: ScalarType,
    },
    IeeeFloatFusedMultiplyAddRequiresFloatResult(OperationId),
    IeeeFloatFusedMultiplyAddOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    BooleanStructuralFieldRequiresBooleanResult(OperationId),
    InvalidBooleanStructuralField {
        operation: OperationId,
        source: PlaceId,
        field: StructuralFieldId,
    },
    IntegerStructuralFieldRequiresIntegerResult(OperationId),
    InvalidIntegerStructuralField {
        operation: OperationId,
        source: PlaceId,
        field: StructuralFieldId,
    },
    StructuralObservationRequiresReadableAccess {
        operation: OperationId,
        source: PlaceId,
    },
    BooleanNotRequiresBooleanResult(OperationId),
    BooleanNotOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        actual: ScalarType,
    },
    BooleanEqualRequiresBooleanResult(OperationId),
    BooleanEqualOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        actual: ScalarType,
    },
    IntegerEqualRequiresBooleanResult(OperationId),
    IntegerEqualOperandTypeMismatch {
        operation: OperationId,
        left: ScalarType,
        right: ScalarType,
    },
    IntegerOrderingRequiresBooleanResult(OperationId),
    IntegerOrderingOperandTypeMismatch {
        operation: OperationId,
        left: ScalarType,
        right: ScalarType,
    },
    IntegerBitwiseRequiresIntegerResult(OperationId),
    IntegerWidenRequiresIntegerResult(OperationId),
    IntegerWidenOperandTypeMismatch {
        operation: OperationId,
        source: ScalarType,
        target: ScalarType,
    },
    IntegerExactCastRequiresIntegerResult(OperationId),
    IntegerExactCastOperandTypeMismatch {
        operation: OperationId,
        source: ScalarType,
        target: ScalarType,
    },
    IntegerBitwiseNotOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual: ScalarType,
    },
    IntegerBitwiseOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        left: ScalarType,
        right: ScalarType,
    },
    WrappingIntegerShiftRequiresIntegerResult(OperationId),
    WrappingIntegerShiftOperandTypeMismatch {
        operation: OperationId,
        expected_value: ScalarType,
        actual_value: ScalarType,
        actual_count: ScalarType,
    },
    ExactIntegerShiftRequiresIntegerResult(OperationId),
    ExactIntegerShiftOperandTypeMismatch {
        operation: OperationId,
        expected_value: ScalarType,
        actual_value: ScalarType,
        actual_count: ScalarType,
    },
    ExactIntegerAddRequiresIntegerResult(OperationId),
    ExactIntegerAddOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual_left: ScalarType,
        actual_right: ScalarType,
    },
    ExactIntegerSubtractRequiresIntegerResult(OperationId),
    ExactIntegerSubtractOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual_left: ScalarType,
        actual_right: ScalarType,
    },
    ExactIntegerMultiplyRequiresIntegerResult(OperationId),
    ExactIntegerMultiplyOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual_left: ScalarType,
        actual_right: ScalarType,
    },
    ExactIntegerDivideRequiresIntegerResult(OperationId),
    ExactIntegerDivideOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual_left: ScalarType,
        actual_right: ScalarType,
    },
    ExactIntegerRemainderRequiresIntegerResult(OperationId),
    ExactIntegerRemainderOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual_left: ScalarType,
        actual_right: ScalarType,
    },
    WrappingIntegerDivideRequiresIntegerResult(OperationId),
    WrappingIntegerDivideOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual_left: ScalarType,
        actual_right: ScalarType,
    },
    WrappingIntegerRemainderRequiresIntegerResult(OperationId),
    WrappingIntegerRemainderOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual_left: ScalarType,
        actual_right: ScalarType,
    },
    SaturatingIntegerDivideRequiresIntegerResult(OperationId),
    SaturatingIntegerDivideOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual_left: ScalarType,
        actual_right: ScalarType,
    },
    SaturatingIntegerRemainderRequiresIntegerResult(OperationId),
    SaturatingIntegerRemainderOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual_left: ScalarType,
        actual_right: ScalarType,
    },
    WrappingIntegerAddRequiresIntegerResult(OperationId),
    SaturatingIntegerAddRequiresIntegerResult(OperationId),
    WrappingIntegerSubtractRequiresIntegerResult(OperationId),
    SaturatingIntegerSubtractRequiresIntegerResult(OperationId),
    WrappingIntegerMultiplyRequiresIntegerResult(OperationId),
    SaturatingIntegerMultiplyRequiresIntegerResult(OperationId),
    WrappingIntegerAddOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    SaturatingIntegerAddOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    WrappingIntegerSubtractOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    SaturatingIntegerSubtractOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    WrappingIntegerMultiplyOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    SaturatingIntegerMultiplyOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    JumpArityMismatch {
        edge: EdgeId,
        expected: usize,
        actual: usize,
    },
    JumpTypeMismatch {
        edge: EdgeId,
        argument: ScalarType,
        parameter: ScalarType,
    },
    ConditionalConditionTypeMismatch {
        block: BlockId,
        condition: ValueId,
        actual: ScalarType,
    },
    StructuralCaseSourceUnknown {
        machine: MachineId,
        block: BlockId,
        place: PlaceId,
    },
    StructuralCaseRequiresClosedSum {
        machine: MachineId,
        block: BlockId,
        structural_type: StructuralTypeId,
    },
    StructuralCaseRosterMismatch {
        machine: MachineId,
        block: BlockId,
    },
    StructuralCasePayloadMismatch {
        edge: EdgeId,
        case: StructuralCaseId,
        field: Option<StructuralFieldId>,
    },
    ReturnTypeMismatch {
        machine: MachineId,
        value: ScalarType,
        result: ScalarType,
    },
    ScalarReturnFromUnitMachine {
        machine: MachineId,
        block: BlockId,
    },
    UnitReturnFromScalarMachine {
        machine: MachineId,
        block: BlockId,
    },
    InvalidRankedScc(MachineId),
    NonExecutableRankedScc(MachineId),
    ControlCycle(BlockId),
    UnreachableBlock(BlockId),
    MalformedProposition(PropositionError),
}

impl std::fmt::Display for ModuleError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ModuleError {}
