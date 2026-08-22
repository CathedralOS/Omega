use psi_core::{
    BlockId, BoundaryMachineId, CanonicalStructuralPathSegment, ClaimId, ContentProjectionIdentity,
    ContentStructuralPlace, ContractId, EdgeId, EvidenceTermId, MachineId, ObligationId,
    OperationId, PlaceId, PropositionError, PropositionId, ScalarType, ServiceId,
    StructuralDomainId, StructuralFieldId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use psi_terminal::{CrashCause, EvidenceContractLaneKind, StructuralMultiplicity};
use psi_terminal_semantics::OperationSemanticError;

use super::foundation::{ServiceCeilingOwner, StructuralSignatureOwner};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractClauseKind {
    Requires,
    Ensures,
    Crash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleError {
    NonDenseFloatMeaningProjection {
        expected: u32,
        result: u32,
        source: u32,
    },
    InvalidFloatMeaningProjection {
        index: u32,
        error: crate::verification::FloatMeaningProjectionVerificationError,
    },
    OperationSemanticSchema(OperationSemanticError),
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
        fingerprint: u64,
    },
    ClosedConformanceFingerprintMismatch {
        owner: MachineId,
        expected: u64,
        actual: u64,
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
    MissingEvidenceOutputField {
        machine: MachineId,
        position: u32,
    },
    InvalidEvidenceOutputField(MachineId),
    ReservedEvidenceOutputField(MachineId),
    DuplicateEvidenceOutputField(MachineId),
    NonCanonicalEvidencePackageInvocation {
        caller: MachineId,
        ordinal: u32,
    },
    InvalidEvidencePackageInvocation {
        caller: MachineId,
        ordinal: u32,
    },
    OrphanEvidenceTerm(EvidenceTermId),
    EmptyPropositionIdentity,
    DuplicateMachine(MachineId),
    DuplicateStructuralType(StructuralTypeId),
    InvalidStructuralTypeIdentity(StructuralTypeId),
    InvalidStructuralFieldIdentity {
        structural_type: StructuralTypeId,
        field: psi_core::StructuralFieldId,
    },
    InvalidStructuralCaseIdentity {
        structural_type: StructuralTypeId,
        case: psi_core::StructuralCaseId,
    },
    EmptyStructuralSum(StructuralTypeId),
    InvalidErasedStructuralField {
        structural_type: StructuralTypeId,
        field: psi_core::StructuralFieldId,
    },
    InvalidStructuralArrayLength(StructuralTypeId),
    UnknownStructuralType(StructuralTypeId),
    RecursiveStructuralType(StructuralTypeId),
    DuplicateStructuralDomain(StructuralDomainId),
    InvalidStructuralDomainIdentity(StructuralDomainId),
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
    InvalidProviderCandidate {
        boundary: BoundaryMachineId,
        candidate: MachineId,
    },
    StructuralParameterPlaceMismatch {
        machine: MachineId,
        place: PlaceId,
    },
    StructuralPlaceHasNoParameter {
        machine: MachineId,
        place: PlaceId,
    },
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
    DuplicateOperation(OperationId),
    ScalarOperationHasUnitResult(OperationId),
    UnitOperationHasScalarResult(OperationId),
    BoundaryCallResultMismatch {
        operation: OperationId,
        expected: Option<ScalarType>,
        actual: Option<ScalarType>,
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
    BoundaryStructuralRequirementsMintObligations(OperationId),
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
    ClaimFrontierJoinMismatch(BlockId),
    OwnedStructuralFrontierJoinMismatch(BlockId),
    LiveLinearClaimAtUnitReturn {
        machine: MachineId,
        block: BlockId,
        claim: ClaimId,
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
        kind: psi_core::StructuralPlaceKind,
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
        scalar_type: psi_core::IntegerType,
    },
    InvalidIeeeFloatFieldTerm {
        machine: MachineId,
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
        format: psi_core::IeeeFloatFormat,
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
        case: psi_core::StructuralCaseId,
    },
    UnsafeStructuralCrashExactDivisor {
        machine: MachineId,
        scalar_type: psi_core::IntegerType,
    },
    UnsafeStructuralCrashPolicyDivisor {
        machine: MachineId,
        scalar_type: psi_core::IntegerType,
    },
    UnsafeStructuralCrashExactShift {
        machine: MachineId,
        value_type: psi_core::IntegerType,
        count_type: psi_core::IntegerType,
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
    BooleanStructuralFieldRequiresBooleanResult(OperationId),
    InvalidBooleanStructuralField {
        operation: OperationId,
        source: PlaceId,
        field: StructuralFieldId,
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
