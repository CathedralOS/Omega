#![forbid(unsafe_code)]

//! Independent structural validation for [`PsiOptimizationUnit`].
//!
//! Pass implementations do not participate in this validator. Publication
//! must call it after applying a candidate and before committing the candidate
//! to the durable transformation ledger.

use std::collections::{BTreeMap, BTreeSet};

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationCandidateIdentity,
    OptimizationRuleIdentity, OptimizationSafetyClass, OptimizationUnitIdentity,
    OptimizationValidatorIdentity,
};
use omega_optimization_unit::{
    AdjacentBlockMergeRewrite, BlockParameterIncomingBinding, BooleanConstantRewrite,
    ConstantConditionalRewrite, DeadScalarNodeRewrite, IntegerConstantRewrite,
    IntegerEvaluationWitness, LocalScalarCommonSubexpressionRewrite, NodeLocation,
    NonAdjacentBlockMergeRewrite, ObservationKnowledge, OptimizationEdge, OptimizationFact,
    OwnershipEvent, OwnershipFrontierFact, OwnershipFrontierLiveClaim, OwnershipFrontierOwnedPlace,
    OwnershipFrontierPartialCustody, OwnershipFrontierSite, OwnershipFrontierSnapshot,
    OwnershipFrontierWitness, OwnershipFrontierWitnessRow, PhiTranslatedScalarGvnRewrite,
    PhiTranslatedScalarIncoming, ProofCertifiedScalarIdentityKind,
    ProofCertifiedScalarIdentityRewrite, ProofQuestion, ProofQuestionAdmissionKind,
    ProofQuestionClass, ProofQuestionOwner, ProvenanceDisposition, ProvenanceRewrite,
    PsiNodeObservation, PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance,
    PsiRealizationSite, PsiRewriteCandidate, PsiRewritePatch, RedundantBlockParameterRewrite,
    ScalarConstantValue, ScalarSubstitution, SccpBlockRow, SccpEdgeRow, SccpEdgeState,
    SccpMachineSnapshot, SccpValueRow, SccpValueState, SharedJumpFusionRewrite, ValueDefinition,
    ValueDefinitionSite, ValueUse, canonical_ownership_frontier_snapshot,
    derived_sccp_scalar_constant_fact_identity, literal_scalar_constant_fact_identity,
    recompute_psi_optimization_unit_identity, reconstruct_psi_closed_region_observation,
    reconstruct_psi_observation_model, structural_domain_catalog_identity,
};
use psi_core::{
    BlockId, BoundaryMachineId, ClaimId, ContentProjectionExpression, ContentProjectionScalar,
    ContentTerm, EdgeId, IntegerCarrier, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, PlaceId, Proposition, ScalarTerm, ScalarType, ServiceId, StructuralDomainId,
    StructuralPlaceKind, StructuralTypeId, ValueId,
};
use psi_terminal_fuel::TerminalFuelSchedule;

mod current_ownership;
mod current_value_ranges;
mod prephysical_manifest;
mod projection;

pub use current_value_ranges::{
    validate_current_value_range_fact, validate_current_value_range_fact_at,
};

pub use prephysical_manifest::{
    OptimizationManifestStage, OptimizationStructuralStatistics, PhysicalOptimizationDataStatus,
    PrePhysicalOptimizationManifest, PrePhysicalOptimizationManifestDecodeError,
    PrePhysicalOptimizationManifestError, ValidatedPrePhysicalOptimizationManifest,
    project_pre_physical_optimization_manifest, validate_pre_physical_optimization_manifest,
};
pub use projection::{
    OptimizedAbstractPlanProjectionError, ValidatedOptimizedAbstractPlanProjection,
    validate_optimized_abstract_plan_projection,
};

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

#[derive(Debug, Clone, Copy)]
struct IndependentProofCertifiedScalarIdentity {
    source_operation: OperationId,
    obligation: psi_core::ObligationId,
    result: ValueId,
    replacement: ValueId,
    identity_operand: ValueId,
    result_type: IntegerType,
    identity_type: IntegerType,
    identity_constant: IntegerValue,
}

fn independent_proof_certified_scalar_identity(
    operation: &O,
    identity: ProofCertifiedScalarIdentityKind,
) -> Option<IndependentProofCertifiedScalarIdentity> {
    let row = match (operation, identity) {
        (
            O::ExactIntegerAdd {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroLeft,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *right,
            *left,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::ExactIntegerAdd {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroRight,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *right,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::ExactIntegerSubtract {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerSubtractZeroRight,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *right,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::ExactIntegerMultiply {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneLeft,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *right,
            *left,
            *scalar_type,
            *scalar_type,
            independent_integer_one(*scalar_type),
        ),
        (
            O::ExactIntegerMultiply {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneRight,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *right,
            *scalar_type,
            *scalar_type,
            independent_integer_one(*scalar_type),
        ),
        (
            O::ExactIntegerShiftLeft {
                psi_operation,
                obligation,
                result,
                value_type,
                count_type,
                value,
                count,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroCount,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *value,
            *count,
            *value_type,
            *count_type,
            independent_integer_zero(*count_type),
        ),
        (
            O::ExactIntegerShiftRight {
                psi_operation,
                obligation,
                result,
                value_type,
                count_type,
                value,
                count,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroCount,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *value,
            *count,
            *value_type,
            *count_type,
            independent_integer_zero(*count_type),
        ),
        (
            O::ExactIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerDivideOneRight,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *right,
            *scalar_type,
            *scalar_type,
            independent_integer_one(*scalar_type),
        ),
        (
            O::WrappingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::WrappingIntegerDivideOneRight,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *right,
            *scalar_type,
            *scalar_type,
            independent_integer_one(*scalar_type),
        ),
        (
            O::SaturatingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideOneRight,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *right,
            *scalar_type,
            *scalar_type,
            independent_integer_one(*scalar_type),
        ),
        (
            O::ExactIntegerMultiply {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                ..
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroLeft,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *left,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::ExactIntegerMultiply {
                psi_operation,
                obligation,
                result,
                scalar_type,
                right,
                ..
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroRight,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *right,
            *right,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::ExactIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                ..
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerDivideZeroLeft,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *left,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::WrappingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                ..
            },
            ProofCertifiedScalarIdentityKind::WrappingIntegerDivideZeroLeft,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *left,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::SaturatingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                ..
            },
            ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideZeroLeft,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *left,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::ExactIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                ..
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerRemainderZeroLeft,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *left,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::WrappingIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                ..
            },
            ProofCertifiedScalarIdentityKind::WrappingIntegerRemainderZeroLeft,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *left,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::SaturatingIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                ..
            },
            ProofCertifiedScalarIdentityKind::SaturatingIntegerRemainderZeroLeft,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *left,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::ExactIntegerShiftLeft {
                psi_operation,
                obligation,
                result,
                value_type,
                value,
                ..
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroValue,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *value,
            *value,
            *value_type,
            *value_type,
            independent_integer_zero(*value_type),
        ),
        (
            O::ExactIntegerShiftRight {
                psi_operation,
                obligation,
                result,
                value_type,
                value,
                ..
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroValue,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *value,
            *value,
            *value_type,
            *value_type,
            independent_integer_zero(*value_type),
        ),
        _ => return None,
    };
    Some(IndependentProofCertifiedScalarIdentity {
        source_operation: row.0,
        obligation: row.1,
        result: row.2,
        replacement: row.3,
        identity_operand: row.4,
        result_type: row.5,
        identity_type: row.6,
        identity_constant: row.7,
    })
}

fn independent_integer_zero(scalar_type: IntegerType) -> IntegerValue {
    match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(0),
        IntegerSign::Unsigned => IntegerValue::Unsigned(0),
    }
}

fn independent_integer_one(scalar_type: IntegerType) -> IntegerValue {
    match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(1),
        IntegerSign::Unsigned => IntegerValue::Unsigned(1),
    }
}

/// Independently remove one live proof-certified integer identity.
/// Accepted proof and literal evidence are reconstructed from immutable input
/// custody; the declared operation is deleted rather than reclassified.
pub fn validate_proof_certified_scalar_identity_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    let exact_identity_rule = OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.live-proof-certified-integer-identity-elimination.v1",
    );
    let divide_by_one_rule = OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.live-proof-certified-integer-divide-by-one-elimination.v1",
    );
    let multiply_by_zero_rule = OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.live-proof-certified-exact-integer-multiply-by-zero-elimination.v1",
    );
    let zero_dividend_rule = OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.live-proof-certified-integer-zero-dividend-elimination.v1",
    );
    let zero_value_shift_rule = OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.live-proof-certified-exact-integer-zero-value-shift-elimination.v1",
    );
    if ![
        exact_identity_rule,
        divide_by_one_rule,
        multiply_by_zero_rule,
        zero_dividend_rule,
        zero_value_shift_rule,
    ]
    .contains(&candidate.rule())
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::ScalarConstants)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || candidate.safety_class() != OptimizationSafetyClass::ProofCertified
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    let validator = match candidate.rule() {
        rule if rule == exact_identity_rule
            && matches!(
                patch.identity,
                ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroLeft
                    | ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroRight
                    | ProofCertifiedScalarIdentityKind::ExactIntegerSubtractZeroRight
                    | ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneLeft
                    | ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneRight
                    | ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroCount
                    | ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroCount
            ) =>
        {
            b"omega.validator.live-proof-certified-integer-identity-elimination.v1".as_slice()
        }
        rule if rule == divide_by_one_rule
            && matches!(
                patch.identity,
                ProofCertifiedScalarIdentityKind::ExactIntegerDivideOneRight
                    | ProofCertifiedScalarIdentityKind::WrappingIntegerDivideOneRight
                    | ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideOneRight
            ) =>
        {
            b"omega.validator.live-proof-certified-integer-divide-by-one-elimination.v1".as_slice()
        }
        rule if rule == multiply_by_zero_rule
            && matches!(
                patch.identity,
                ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroLeft
                    | ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroRight
            ) =>
        {
            b"omega.validator.live-proof-certified-exact-integer-multiply-by-zero-elimination.v1"
                .as_slice()
        }
        rule if rule == zero_dividend_rule
            && matches!(
                patch.identity,
                ProofCertifiedScalarIdentityKind::ExactIntegerDivideZeroLeft
                    | ProofCertifiedScalarIdentityKind::WrappingIntegerDivideZeroLeft
                    | ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideZeroLeft
                    | ProofCertifiedScalarIdentityKind::ExactIntegerRemainderZeroLeft
                    | ProofCertifiedScalarIdentityKind::WrappingIntegerRemainderZeroLeft
                    | ProofCertifiedScalarIdentityKind::SaturatingIntegerRemainderZeroLeft
            ) =>
        {
            b"omega.validator.live-proof-certified-integer-zero-dividend-elimination.v1".as_slice()
        }
        rule if rule == zero_value_shift_rule
            && matches!(
                patch.identity,
                ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroValue
                    | ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroValue
            ) =>
        {
            b"omega.validator.live-proof-certified-exact-integer-zero-value-shift-elimination.v1"
                .as_slice()
        }
        _ => return Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    };
    if candidate.node_decision_point() != Some(patch.location)
        || candidate.substitutions()
            != [ScalarSubstitution {
                from: patch.result,
                to: patch.replacement,
                scalar_type: ScalarType::Integer(patch.scalar_type),
            }]
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.location.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == patch.location.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let node_index = usize::try_from(patch.location.node)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    let node = block
        .nodes
        .get(node_index)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let shape = independent_proof_certified_scalar_identity(&node.operation, patch.identity)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    if shape.source_operation != patch.source_operation
        || shape.result != patch.result
        || shape.replacement != patch.replacement
        || shape.result_type != patch.scalar_type
        || node.definitions
            != [ValueDefinition {
                value: patch.result,
                scalar_type: ScalarType::Integer(patch.scalar_type),
                site: ValueDefinitionSite::Node {
                    block: block.id,
                    node: patch.location.node,
                },
            }]
        || !node.successors.is_empty()
        || !node.ownership.is_empty()
        || block.nodes.get(node_index + 1).is_none()
        || !function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .flat_map(|node| &node.uses)
            .any(|use_site| use_site.value == patch.result)
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    if scalar_value_definition(function, shape.replacement)
        .is_none_or(|definition| definition.scalar_type != ScalarType::Integer(shape.result_type))
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let identity_definition = scalar_value_definition(function, shape.identity_operand)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    if identity_definition.scalar_type != ScalarType::Integer(shape.identity_type) {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    }
    let ValueDefinitionSite::Node {
        block: literal_block,
        node: literal_node,
    } = identity_definition.site
    else {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    };
    let literal = function
        .blocks
        .iter()
        .find(|block| block.id == literal_block)
        .and_then(|block| {
            usize::try_from(literal_node)
                .ok()
                .and_then(|node| block.nodes.get(node))
        })
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let O::IntegerConstant {
        psi_operation: literal_support,
        result: literal_result,
        scalar_type: literal_type,
        value: literal_value,
    } = literal.operation
    else {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    };
    if literal_result != shape.identity_operand
        || literal_type != ScalarType::Integer(shape.identity_type)
        || literal_value != shape.identity_constant
        || !function.facts.iter().any(|fact| {
            matches!(fact, OptimizationFact::IntegerConstant { value, constant, support }
                if *value == shape.identity_operand
                    && *constant == shape.identity_constant
                    && *support == literal_support)
        })
    {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    }
    let expected_constant_fact = literal_scalar_constant_fact_identity(
        input.identity,
        function.machine,
        identity_definition,
        ScalarConstantValue::Integer(shape.identity_constant),
        literal_support,
    )
    .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let Some((constant_fact, obligation_fact)) =
        candidate.proof_certified_scalar_identity_witness()
    else {
        return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
    };
    if constant_fact != expected_constant_fact {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    }
    if !function.facts.iter().any(|fact| {
        matches!(fact, OptimizationFact::OperationObligationReference { obligation, support }
            if *obligation == shape.obligation && *support == shape.source_operation)
    }) || !input.accepted_obligation_facts.iter().any(|fact| {
        fact.identity == obligation_fact
            && fact.machine == function.machine
            && fact.operation == shape.source_operation
            && fact.obligation == shape.obligation
    }) {
        return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
    }
    let receiver = &block.nodes[node_index + 1];
    if receiver
        .provenance
        .iter()
        .any(|source| node.provenance.contains(source))
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }
    let (expected_blocks, accepted_provenance) =
        reconstruct_proof_certified_scalar_identity_accounting(function, patch)
            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.affected_blocks() != expected_blocks
        || candidate.provenance() != accepted_provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.location.machine)
        .expect("candidate function exists");
    let output_block = output_function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.location.block)
        .expect("candidate block exists");
    let removed = output_block.nodes.remove(node_index);
    let receiver = output_block
        .nodes
        .get_mut(node_index)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    receiver.provenance.extend_from_slice(&removed.provenance);
    receiver.fuel.extend_from_slice(&removed.fuel);
    let mut effect = 0u64;
    for block in &mut output_function.blocks {
        for (node_index, node) in block.nodes.iter_mut().enumerate() {
            rewrite_scalar_value_uses(&mut node.operation, patch.result, patch.replacement);
            let node_index = u32::try_from(node_index)
                .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
            node.definitions = expected_definitions(&node.operation, block.id, node_index);
            node.uses = expected_uses(&node.operation, block.id, node_index);
            node.successors = preserve_edge_custody(node);
            node.ownership = expected_ownership(&node.operation);
            node.effect = omega_optimization_unit::EffectLink {
                input: effect,
                output: effect
                    .checked_add(1)
                    .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?,
            };
            effect = effect
                .checked_add(1)
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
        }
    }
    output_function.facts = reconstruct_fact_index(output_function);
    output_function.declared_places = reconstruct_declared_places(output_function)?;
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
    if output.accepted_obligation_facts != input.accepted_obligation_facts {
        return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
    }
    let output_function = output
        .functions
        .iter()
        .find(|output_function| output_function.machine == function.machine)
        .expect("output function exists");
    for input_block in &function.blocks {
        if !expected_blocks.contains(&input_block.id)
            && output_function
                .blocks
                .iter()
                .find(|block| block.id == input_block.id)
                != Some(input_block)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(validator),
        provenance: accepted_provenance,
    })
}

impl std::fmt::Display for OptimizationUnitValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid Psi optimization unit: {self:?}")
    }
}

impl std::error::Error for OptimizationUnitValidationError {}

/// Independently reconstructed scalar interface of one closed node region.
/// Canonical ordering is by `ValueId`; block-parameter bindings remain uses of
/// the predecessor terminator and therefore participate naturally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedScalarObservationBoundary {
    pub location: NodeLocation,
    pub live_in: Vec<ValueId>,
    pub live_out: Vec<ValueId>,
}

pub fn reconstruct_closed_scalar_node_boundary(
    unit: &PsiOptimizationUnit,
    location: NodeLocation,
) -> Option<ClosedScalarObservationBoundary> {
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == location.machine)?;
    let mut live_entry = function
        .blocks
        .iter()
        .map(|block| (block.id, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut live_exit = live_entry.clone();
    loop {
        let mut changed = false;
        for block in function.blocks.iter().rev() {
            let next_exit = block
                .nodes
                .last()
                .into_iter()
                .flat_map(|node| &node.successors)
                .filter_map(|edge| live_entry.get(&edge.target))
                .flat_map(|values| values.iter().copied())
                .collect::<BTreeSet<_>>();
            let mut next_entry = next_exit.clone();
            for node in block.nodes.iter().rev() {
                for definition in &node.definitions {
                    next_entry.remove(&definition.value);
                }
                next_entry.extend(node.uses.iter().map(|use_site| use_site.value));
            }
            for parameter in &block.parameters {
                next_entry.remove(&parameter.value);
            }
            if live_exit[&block.id] != next_exit {
                live_exit.insert(block.id, next_exit);
                changed = true;
            }
            if live_entry[&block.id] != next_entry {
                live_entry.insert(block.id, next_entry);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let block = function
        .blocks
        .iter()
        .find(|block| block.id == location.block)?;
    let target = usize::try_from(location.node).ok()?;
    if target >= block.nodes.len() {
        return None;
    }
    let mut live = live_exit[&block.id].clone();
    for (node_index, node) in block.nodes.iter().enumerate().rev() {
        let live_out = live.clone();
        for definition in &node.definitions {
            live.remove(&definition.value);
        }
        live.extend(node.uses.iter().map(|use_site| use_site.value));
        if node_index == target {
            return Some(ClosedScalarObservationBoundary {
                location,
                live_in: live.iter().copied().collect(),
                live_out: live_out.iter().copied().collect(),
            });
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPsiRewrite {
    unit: PsiOptimizationUnit,
    candidate: OptimizationCandidateIdentity,
    validator: OptimizationValidatorIdentity,
    provenance: Vec<omega_optimization_unit::ProvenanceRewrite>,
}

impl ValidatedPsiRewrite {
    pub const fn unit(&self) -> &PsiOptimizationUnit {
        &self.unit
    }

    pub const fn candidate(&self) -> OptimizationCandidateIdentity {
        self.candidate
    }

    pub const fn validator(&self) -> OptimizationValidatorIdentity {
        self.validator
    }

    /// Validator-accepted source disposition and fuel accounting. Consumers
    /// must ledger this value rather than re-reading the proposal.
    pub fn provenance(&self) -> &[omega_optimization_unit::ProvenanceRewrite] {
        &self.provenance
    }

    pub fn into_unit(self) -> PsiOptimizationUnit {
        self.unit
    }
}

pub fn validate_psi_optimization_unit(
    unit: &PsiOptimizationUnit,
) -> Result<(), OptimizationUnitValidationError> {
    let recomputed = recompute_psi_optimization_unit_identity(unit);
    if unit.identity != recomputed {
        return Err(OptimizationUnitValidationError::ContentIdentityMismatch {
            stored: unit.identity,
            recomputed,
        });
    }
    if unit.fuel_schedule != TerminalFuelSchedule::CURRENT.identity() {
        return Err(OptimizationUnitValidationError::WrongFuelSchedule);
    }
    if unit
        .accepted_obligation_facts
        .iter()
        .any(|fact| fact.psi != unit.psi || !fact.has_canonical_identity())
        || unit.accepted_obligation_facts.windows(2).any(|pair| {
            (pair[0].machine, pair[0].operation, pair[0].obligation)
                >= (pair[1].machine, pair[1].operation, pair[1].obligation)
        })
    {
        return Err(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch);
    }
    let mut proof_question_identities = BTreeSet::new();
    let mut proof_question_owners = BTreeSet::new();
    if unit.proof_questions.iter().any(|question| {
        question.terminal_psi != unit.psi
            || !question.has_canonical_identity()
            || !proof_question_identities.insert(question.identity)
            || !proof_question_owners.insert((question.owner, question.obligation))
    }) {
        return Err(OptimizationUnitValidationError::ProofQuestionIndexMismatch);
    }
    if unit.ownership_frontier_facts.iter().any(|fact| {
        fact.psi != unit.psi
            || !fact.has_canonical_identity()
            || !canonical_ownership_frontier_snapshot(&fact.snapshot)
    }) || unit
        .ownership_frontier_facts
        .windows(2)
        .any(|pair| (pair[0].machine, pair[0].site) >= (pair[1].machine, pair[1].site))
    {
        return Err(OptimizationUnitValidationError::OwnershipFrontierFactIndexMismatch);
    }
    let mut machines = BTreeMap::new();
    for function in &unit.functions {
        if machines.insert(function.machine, function).is_some() {
            return Err(OptimizationUnitValidationError::DuplicateMachine(
                function.machine,
            ));
        }
    }
    if unit
        .pruned_machines
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(OptimizationUnitValidationError::NonCanonicalPrunedMachineRoster);
    }
    let pruned = unit
        .pruned_machines
        .iter()
        .map(|custody| custody.machine)
        .collect::<BTreeSet<_>>();
    if pruned.len() != unit.pruned_machines.len() {
        return Err(OptimizationUnitValidationError::NonCanonicalPrunedMachineRoster);
    }
    if let Some(machine) = machines
        .keys()
        .find(|machine| pruned.contains(machine))
        .copied()
    {
        return Err(OptimizationUnitValidationError::ActivePrunedMachineOverlap(
            machine,
        ));
    }
    if pruned.contains(&unit.entry) {
        return Err(OptimizationUnitValidationError::PrunedEntryMachine(
            unit.entry,
        ));
    }
    if let Some(machine) = unit
        .provider_candidates
        .iter()
        .map(|candidate| candidate.candidate)
        .find(|machine| pruned.contains(machine))
    {
        return Err(OptimizationUnitValidationError::PrunedProviderMachine(
            machine,
        ));
    }
    if unit
        .accepted_obligation_facts
        .iter()
        .any(|fact| !machines.contains_key(&fact.machine) && !pruned.contains(&fact.machine))
    {
        return Err(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch);
    }
    if unit.proof_questions.iter().any(|question| {
        let machine = question.owner.machine();
        !machines.contains_key(&machine) && !pruned.contains(&machine)
    }) {
        return Err(OptimizationUnitValidationError::ProofQuestionIndexMismatch);
    }
    let mut boundary_machines = BTreeMap::new();
    for boundary in &unit.boundary_machines {
        if boundary_machines.insert(boundary.id, boundary).is_some() {
            return Err(OptimizationUnitValidationError::DuplicateBoundaryMachine(
                boundary.id,
            ));
        }
    }
    let services = index_service_catalog(unit)?;
    let (structural_types, structural_domains) = index_structural_catalogs(unit)?;
    for boundary in &unit.boundary_machines {
        if !valid_service_ceiling(&boundary.published_service_ceiling, &services) {
            return Err(
                OptimizationUnitValidationError::InvalidBoundaryServiceCeiling(boundary.id),
            );
        }
        if !boundary_structural_signature_matches(boundary, &structural_types, &structural_domains)
        {
            return Err(OptimizationUnitValidationError::StructuralCatalogMismatch {
                machine: None,
            });
        }
    }
    validate_provider_service_refinements(unit, &machines, &boundary_machines)?;
    for function in &unit.functions {
        validate_function(
            function,
            unit.entry,
            &machines,
            &boundary_machines,
            &services,
            &structural_types,
            &structural_domains,
        )?;
    }
    validate_retained_ownership_authority(unit)?;
    for fact in &unit.ownership_frontier_facts {
        if unit
            .functions
            .iter()
            .find(|function| function.machine == fact.machine)
            .is_none()
            && !pruned.contains(&fact.machine)
        {
            return Err(OptimizationUnitValidationError::OwnershipFrontierFactIndexMismatch);
        }
    }
    if !machines.contains_key(&unit.entry) {
        return Err(OptimizationUnitValidationError::MissingEntryMachine(
            unit.entry,
        ));
    }
    validate_root_service_reach(unit, &machines, &boundary_machines, &services)?;
    Ok(())
}

/// Validate the bounded ownership authority retained by the current unit.
///
/// This intentionally does not replay the current CFG ownership automaton.
/// It binds authored edge cleanup and compressed hidden establishments to the
/// immutable source-site entry/exit transitions that remain in the unit.
fn validate_retained_ownership_authority(
    unit: &PsiOptimizationUnit,
) -> Result<(), OptimizationUnitValidationError> {
    if unit.ownership_frontier_facts.is_empty() {
        // Bare reconstruction seeds have no verifier authority to replay.
        return Ok(());
    }
    let frontiers = unit
        .ownership_frontier_facts
        .iter()
        .map(|fact| ((fact.machine, fact.site), &fact.snapshot))
        .collect::<BTreeMap<_, _>>();

    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let node_index = u32::try_from(node_index).expect("unit node index fits u32");
                for edge in &node.successors {
                    for (source_index, source) in edge.provenance.iter().enumerate() {
                        let PsiProvenance::Edge(source) = source else {
                            return Err(
                                OptimizationUnitValidationError::StructuralEdgeAffineDiscardsMismatch {
                                    machine: function.machine,
                                    edge: edge.psi_edge,
                                },
                            );
                        };
                        let Some(entry) = frontiers
                            .get(&(function.machine, OwnershipFrontierSite::EdgeEntry(*source)))
                        else {
                            return Err(
                                OptimizationUnitValidationError::MissingStructuralEdgeFrontier {
                                    machine: function.machine,
                                    edge: *source,
                                },
                            );
                        };
                        let Some(exit) = frontiers
                            .get(&(function.machine, OwnershipFrontierSite::EdgeExit(*source)))
                        else {
                            return Err(
                                OptimizationUnitValidationError::MissingStructuralEdgeFrontier {
                                    machine: function.machine,
                                    edge: *source,
                                },
                            );
                        };
                        let discards = if source_index == 0 {
                            edge.trivial_affine_discards.as_slice()
                        } else {
                            // Every implemented edge-combining rewrite fences
                            // nonempty inherited cleanup work.
                            &[]
                        };
                        if !valid_edge_affine_transition(function, entry, exit, discards) {
                            return Err(
                                OptimizationUnitValidationError::StructuralEdgeAffineDiscardsMismatch {
                                    machine: function.machine,
                                    edge: edge.psi_edge,
                                },
                            );
                        }
                    }
                }

                let O::ReturnStructural {
                    trivial_affine_locals,
                    ..
                } = &node.operation
                else {
                    continue;
                };
                for (operation, place, _) in trivial_affine_locals {
                    let mismatch = || {
                        OptimizationUnitValidationError::StructuralReturnHiddenLocalCustodyMismatch {
                            machine: function.machine,
                            block: block.id,
                            node: node_index,
                            operation: *operation,
                        }
                    };
                    let entry = frontiers
                        .get(&(
                            function.machine,
                            OwnershipFrontierSite::OperationEntry(*operation),
                        ))
                        .ok_or_else(mismatch)?;
                    let exit = frontiers
                        .get(&(
                            function.machine,
                            OwnershipFrontierSite::OperationExit(*operation),
                        ))
                        .ok_or_else(mismatch)?;
                    if !valid_hidden_affine_establishment(entry, exit, place.id) {
                        return Err(mismatch());
                    }
                }
            }
        }
    }
    Ok(())
}

fn valid_edge_affine_transition(
    function: &PsiOptimizationFunction,
    entry: &OwnershipFrontierSnapshot,
    exit: &OwnershipFrontierSnapshot,
    discards: &[PlaceId],
) -> bool {
    if entry.claims != exit.claims || entry.partial_custody != exit.partial_custody {
        return false;
    }
    let live = entry
        .owned_places
        .iter()
        .map(|owned| owned.place)
        .collect::<BTreeSet<_>>();
    let mut eligible = function
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                ..
            } if live.contains(&place.id) => Some((declaration_ordinal, place.id)),
            _ => None,
        })
        .collect::<Vec<_>>();
    eligible.sort_by_key(|(ordinal, _)| std::cmp::Reverse(*ordinal));
    let mut eligible = eligible
        .into_iter()
        .map(|(_, place)| place)
        .collect::<Vec<_>>();
    eligible.extend(
        function
            .structural_parameters
            .iter()
            .rev()
            .filter_map(|parameter| {
                (parameter.multiplicity == psi_terminal::StructuralMultiplicity::Affine
                    && live.contains(&parameter.place)
                    && !entry
                        .claims
                        .iter()
                        .any(|claim| claim.input == Some(parameter.place))
                    && !function
                        .content_entry_claims
                        .iter()
                        .any(|claim| claim.input.root == parameter.place))
                .then_some(parameter.place)
            }),
    );
    let mut next = 0;
    for eligible_place in eligible {
        if discards.get(next) == Some(&eligible_place) {
            next += 1;
        }
    }
    if next != discards.len() {
        return false;
    }
    let discard_set = discards.iter().copied().collect::<BTreeSet<_>>();
    if discard_set.len() != discards.len() {
        return false;
    }
    let expected_exit = entry
        .owned_places
        .iter()
        .filter(|owned| !discard_set.contains(&owned.place))
        .copied()
        .collect::<Vec<_>>();
    expected_exit == exit.owned_places
}

fn valid_hidden_affine_establishment(
    entry: &OwnershipFrontierSnapshot,
    exit: &OwnershipFrontierSnapshot,
    place: PlaceId,
) -> bool {
    let mut expected_owned = entry.owned_places.clone();
    if expected_owned.iter().any(|owned| owned.place == place) {
        return false;
    }
    expected_owned.push(OwnershipFrontierOwnedPlace {
        place,
        multiplicity: psi_terminal::StructuralMultiplicity::Affine,
    });
    expected_owned.sort_by_key(|owned| owned.place);
    entry.claims == exit.claims
        && entry.partial_custody == exit.partial_custody
        && expected_owned == exit.owned_places
}

fn index_service_catalog(
    unit: &PsiOptimizationUnit,
) -> Result<BTreeMap<ServiceId, &psi_terminal::ServiceDeclaration>, OptimizationUnitValidationError>
{
    let mut services = BTreeMap::new();
    let mut identities = BTreeSet::new();
    for declaration in unit.services.iter() {
        if services.insert(declaration.id, declaration).is_some() {
            return Err(OptimizationUnitValidationError::DuplicateService(
                declaration.id,
            ));
        }
        if declaration.identity.is_empty() || !identities.insert(declaration.identity.as_str()) {
            return Err(OptimizationUnitValidationError::InvalidServiceIdentity(
                declaration.id,
            ));
        }
    }
    for declaration in unit.services.iter() {
        let mut parents = BTreeSet::new();
        for parent in &declaration.parents {
            if *parent == declaration.id
                || !parents.insert(*parent)
                || !services.contains_key(parent)
            {
                return Err(OptimizationUnitValidationError::InvalidServiceParent {
                    service: declaration.id,
                    parent: *parent,
                });
            }
        }
        if declaration
            .parents
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(OptimizationUnitValidationError::NonCanonicalServiceParents(
                declaration.id,
            ));
        }
    }

    fn visit(
        id: ServiceId,
        services: &BTreeMap<ServiceId, &psi_terminal::ServiceDeclaration>,
        active: &mut BTreeSet<ServiceId>,
        complete: &mut BTreeSet<ServiceId>,
    ) -> Result<(), OptimizationUnitValidationError> {
        if complete.contains(&id) {
            return Ok(());
        }
        if !active.insert(id) {
            return Err(OptimizationUnitValidationError::RecursiveServiceHierarchy(
                id,
            ));
        }
        for parent in &services[&id].parents {
            visit(*parent, services, active, complete)?;
        }
        active.remove(&id);
        complete.insert(id);
        Ok(())
    }

    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    for id in services.keys().copied() {
        visit(id, &services, &mut active, &mut complete)?;
    }
    for declaration in services.values() {
        for parent in &declaration.parents {
            if let Some(ancestor) = services[parent]
                .parents
                .iter()
                .find(|ancestor| !declaration.parents.contains(ancestor))
            {
                return Err(
                    OptimizationUnitValidationError::IncompleteServiceParentClosure {
                        service: declaration.id,
                        ancestor: *ancestor,
                    },
                );
            }
        }
    }
    Ok(services)
}

fn valid_service_ceiling(
    ceiling: &[ServiceId],
    services: &BTreeMap<ServiceId, &psi_terminal::ServiceDeclaration>,
) -> bool {
    let mut seen = BTreeSet::new();
    ceiling.iter().all(|service| {
        seen.insert(*service)
            && services.get(service).is_some_and(|declaration| {
                declaration
                    .parents
                    .iter()
                    .all(|parent| ceiling.contains(parent))
            })
    }) && ceiling.windows(2).all(|pair| pair[0] < pair[1])
}

fn validate_root_service_reach(
    unit: &PsiOptimizationUnit,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundaries: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    services: &BTreeMap<ServiceId, &psi_terminal::ServiceDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    if !valid_service_ceiling(&unit.root_service_reach.concrete, services) {
        return Err(OptimizationUnitValidationError::InvalidRootConcreteServiceReach);
    }
    let mut requirement_identities = BTreeSet::new();
    for (index, dependency) in unit
        .root_service_reach
        .installation_dependencies
        .iter()
        .enumerate()
    {
        if dependency.requirement_identity.is_empty()
            || !requirement_identities.insert(dependency.requirement_identity.as_str())
            || !valid_service_ceiling(&dependency.upper_bound, services)
        {
            return Err(
                OptimizationUnitValidationError::InvalidRootInstallationReachDependency(index),
            );
        }
    }
    if unit
        .root_service_reach
        .installation_dependencies
        .windows(2)
        .any(|pair| pair[0].requirement_identity >= pair[1].requirement_identity)
    {
        return Err(OptimizationUnitValidationError::NonCanonicalRootInstallationReachDependencies);
    }
    let derived = derive_root_service_reach(unit, functions, boundaries, services)?;
    if derived.concrete != unit.root_service_reach.concrete {
        return Err(
            OptimizationUnitValidationError::RootConcreteServiceReachMismatch {
                declared: unit.root_service_reach.concrete.clone(),
                derived: derived.concrete,
            },
        );
    }
    if derived.installation_dependencies != unit.root_service_reach.installation_dependencies {
        return Err(OptimizationUnitValidationError::RootInstallationReachDependenciesMismatch);
    }
    Ok(())
}

fn derive_root_service_reach(
    unit: &PsiOptimizationUnit,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundaries: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    services: &BTreeMap<ServiceId, &psi_terminal::ServiceDeclaration>,
) -> Result<psi_terminal::TerminalRootServiceReach, OptimizationUnitValidationError> {
    let dependencies = unit
        .root_service_reach
        .installation_dependencies
        .iter()
        .map(|dependency| (dependency.requirement_identity.as_str(), dependency))
        .collect::<BTreeMap<_, _>>();
    let mut pending = vec![unit.entry];
    let mut visited = BTreeSet::new();
    let mut concrete = BTreeSet::new();
    let mut used_dependencies = BTreeSet::new();
    while let Some(machine) = pending.pop() {
        if !visited.insert(machine) {
            continue;
        }
        let function = functions.get(&machine).copied().ok_or(
            OptimizationUnitValidationError::MissingEntryMachine(machine),
        )?;
        for operation in function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .map(|node| &node.operation)
        {
            match operation {
                O::Call { callee, .. }
                | O::CallUnit { callee, .. }
                | O::CallStructuralScalar { callee, .. }
                | O::CallStructural { callee, .. } => pending.push(*callee),
                O::BoundaryCall { boundary, .. } => {
                    let declaration = boundaries.get(boundary).copied().ok_or(
                        OptimizationUnitValidationError::OperationServiceContractMismatch {
                            machine: function.machine,
                            block: function.entry,
                            node: 0,
                        },
                    )?;
                    if let Some(dependency) = dependencies.get(declaration.identity.as_str()) {
                        if declaration.published_service_ceiling != dependency.upper_bound {
                            return Err(
                                OptimizationUnitValidationError::RootInstallationReachBoundaryMismatch(
                                    *boundary,
                                ),
                            );
                        }
                        used_dependencies.insert(declaration.identity.as_str());
                    } else {
                        concrete.extend(declaration.published_service_ceiling.iter().copied());
                    }
                }
                O::PortWrite { service, .. } => {
                    concrete.insert(*service);
                    if let Some(declaration) = services.get(service) {
                        concrete.extend(declaration.parents.iter().copied());
                    }
                }
                _ => {}
            }
        }
    }
    let installation_dependencies = unit
        .root_service_reach
        .installation_dependencies
        .iter()
        .filter(|dependency| used_dependencies.contains(dependency.requirement_identity.as_str()))
        .cloned()
        .collect();
    Ok(psi_terminal::TerminalRootServiceReach {
        concrete: concrete.into_iter().collect(),
        installation_dependencies,
    })
}

fn refresh_root_service_reach(
    unit: &mut PsiOptimizationUnit,
) -> Result<(), OptimizationUnitValidationError> {
    let reach = {
        let functions = unit
            .functions
            .iter()
            .map(|function| (function.machine, function))
            .collect::<BTreeMap<_, _>>();
        let boundaries = unit
            .boundary_machines
            .iter()
            .map(|boundary| (boundary.id, boundary))
            .collect::<BTreeMap<_, _>>();
        let services = unit
            .services
            .iter()
            .map(|service| (service.id, service))
            .collect::<BTreeMap<_, _>>();
        derive_root_service_reach(unit, &functions, &boundaries, &services)?
    };
    unit.root_service_reach = reach;
    Ok(())
}

fn validate_provider_service_refinements(
    unit: &PsiOptimizationUnit,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundaries: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    for provider in &unit.provider_candidates {
        let invalid = || OptimizationUnitValidationError::InvalidProviderServiceRefinement {
            boundary: provider.boundary,
            candidate: provider.candidate,
        };
        let candidate = functions.get(&provider.candidate).ok_or_else(invalid)?;
        let boundary = boundaries.get(&provider.boundary).ok_or_else(invalid)?;
        if provider.refinement.realized_service_ceiling != candidate.published_service_ceiling
            || provider
                .refinement
                .realized_service_ceiling
                .iter()
                .any(|service| !boundary.published_service_ceiling.contains(service))
        {
            return Err(invalid());
        }
    }
    Ok(())
}

fn boundary_structural_signature_matches(
    boundary: &psi_terminal::BoundaryMachineDeclaration,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
    domains: &BTreeMap<StructuralDomainId, &psi_terminal::StructuralDomainDeclaration>,
) -> bool {
    structural_signature_matches(
        &boundary.structural_parameters,
        boundary.attachment,
        types,
        domains,
    ) && boundary.requires.windows(2).all(|pair| pair[0] < pair[1])
        && boundary.requires.iter().all(|requirement| {
            boundary
                .structural_parameters
                .get(requirement.argument_index as usize)
                .is_some_and(|parameter| {
                    domains
                        .get(&requirement.domain)
                        .is_some_and(|domain| domain.carrier == parameter.structural_type)
                })
        })
}

/// Replay Terminal's exact attachment/self half of a structural signature.
/// An attachment need not have a runtime `self` parameter (provider-backed
/// specializations deliberately do not), but every retained `self` must be the
/// unique parameter whose type is that attachment.
fn structural_signature_matches(
    parameters: &[psi_terminal::StructuralParameterDeclaration],
    attachment: Option<StructuralTypeId>,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
    domains: &BTreeMap<StructuralDomainId, &psi_terminal::StructuralDomainDeclaration>,
) -> bool {
    if attachment.is_some_and(|attachment| !types.contains_key(&attachment)) {
        return false;
    }
    let mut places = BTreeSet::new();
    let mut saw_self = false;
    parameters.iter().enumerate().all(|(position, parameter)| {
        let self_matches = if parameter.is_self {
            let matches = !saw_self && attachment == Some(parameter.structural_type);
            saw_self = true;
            matches
        } else {
            true
        };
        u32::try_from(position).ok() == Some(parameter.position)
            && places.insert(parameter.place)
            && types.contains_key(&parameter.structural_type)
            && self_matches
            && structural_qualifications_match(
                parameter.structural_type,
                &parameter.qualifications,
                domains,
            )
    })
}

/// Independently check and construct one integer-evaluation rewrite.
/// The proposing rule never receives a mutable unit and cannot construct the
/// accepted output itself.
pub fn validate_integer_evaluation_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::ScalarConstants)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate.substitutions().is_empty()
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.location) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.location.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == patch.location.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let node = block
        .nodes
        .get(usize::try_from(patch.location.node).expect("u32 fits usize"))
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let input_observation = observation_at(input, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let input_live = reconstruct_closed_scalar_node_boundary(input, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let [provenance] = candidate.provenance() else {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    };
    let site = PsiRealizationSite::Node(patch.location);
    if provenance.input != site
        || provenance.disposition != ProvenanceDisposition::RealizedAt(site)
        || provenance.sources != node.provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }
    if provenance.fuel != node.fuel {
        return Err(OptimizationUnitValidationError::CandidateFuelMismatch);
    }
    let accepted_provenance = vec![omega_optimization_unit::ProvenanceRewrite {
        input: site,
        disposition: ProvenanceDisposition::RealizedAt(site),
        sources: node.provenance.clone(),
        fuel: node.fuel.clone(),
    }];

    let (source_operation, result, scalar_type, evaluated, safety_class) =
        evaluate_integer_operation(function, node, candidate)?;
    if candidate.safety_class() != safety_class {
        return Err(OptimizationUnitValidationError::CandidateSafetyClassMismatch);
    }
    match (
        safety_class,
        candidate
            .scalar_evaluation_witness()
            .and_then(IntegerEvaluationWitness::obligation_fact),
    ) {
        (OptimizationSafetyClass::ProofCertified, Some(identity)) => {
            let fact = input
                .accepted_obligation_facts
                .iter()
                .find(|fact| {
                    fact.identity == identity
                        && fact.machine == function.machine
                        && fact.operation == source_operation
                })
                .ok_or(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)?;
            if !function.facts.iter().any(|reference| {
                matches!(
                    reference,
                    OptimizationFact::OperationObligationReference { obligation, support }
                        if *support == source_operation && *obligation == fact.obligation
                )
            }) {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
        }
        (OptimizationSafetyClass::ProofCertified, None) | (_, Some(_)) => {
            return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
        }
        (_, None) => {}
    }
    if patch
        != (IntegerConstantRewrite {
            location: patch.location,
            source_operation,
            result,
            scalar_type,
            constant: evaluated,
        })
    {
        return Err(OptimizationUnitValidationError::CandidateEvaluationMismatch);
    }
    let mut output = input.clone();
    let function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.location.machine)
        .expect("candidate source function exists");
    let block = function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.location.block)
        .expect("candidate source block exists");
    let node = &mut block.nodes[usize::try_from(patch.location.node).expect("u32 fits usize")];
    node.operation = omega_abstract_operations::AbstractOperation::IntegerConstant {
        psi_operation: patch.source_operation,
        result: patch.result,
        scalar_type: ScalarType::Integer(patch.scalar_type),
        value: patch.constant,
    };
    node.definitions = vec![ValueDefinition {
        value: patch.result,
        scalar_type: ScalarType::Integer(patch.scalar_type),
        site: ValueDefinitionSite::Node {
            block: patch.location.block,
            node: patch.location.node,
        },
    }];
    node.uses.clear();
    node.successors.clear();
    node.ownership.clear();
    function.facts = reconstruct_fact_index(function);
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
    let output_observation = observation_at(&output, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if !same_closed_scalar_observation(&input_observation, &output_observation) {
        return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
    }
    let output_live = reconstruct_closed_scalar_node_boundary(&output, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if input_live.live_out != output_live.live_out
        || output_live
            .live_in
            .iter()
            .any(|value| !input_live.live_in.contains(value))
    {
        return Err(OptimizationUnitValidationError::CandidateLiveBoundaryMismatch);
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.exact-integer-evaluation.v2",
        ),
        provenance: accepted_provenance,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProofCertifiedSameOperandIntegerConstantLaw {
    ExactSubtractZero,
    SelfRemainderZero,
    SelfDivideOne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IndependentSameOperandIntegerConstant {
    psi_operation: OperationId,
    obligation: psi_core::ObligationId,
    result: ValueId,
    scalar_type: IntegerType,
    operand: ValueId,
}

fn independent_same_operand_integer_constant(
    operation: &O,
    law: ProofCertifiedSameOperandIntegerConstantLaw,
) -> Option<IndependentSameOperandIntegerConstant> {
    let (psi_operation, obligation, result, scalar_type, left, right) = match (law, operation) {
        (
            ProofCertifiedSameOperandIntegerConstantLaw::ExactSubtractZero,
            O::ExactIntegerSubtract {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        (
            ProofCertifiedSameOperandIntegerConstantLaw::SelfRemainderZero,
            O::ExactIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            }
            | O::WrappingIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            }
            | O::SaturatingIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        (
            ProofCertifiedSameOperandIntegerConstantLaw::SelfDivideOne,
            O::ExactIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            }
            | O::WrappingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            }
            | O::SaturatingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
        ) if scalar_type.carrier() == IntegerCarrier::Fixed
            && !(scalar_type.sign() == IntegerSign::Signed && scalar_type.bits() == 1) =>
        {
            (
                *psi_operation,
                *obligation,
                *result,
                *scalar_type,
                *left,
                *right,
            )
        }
        _ => return None,
    };
    (left == right).then_some(IndependentSameOperandIntegerConstant {
        psi_operation,
        obligation,
        result,
        scalar_type,
        operand: left,
    })
}

/// Independently validate and materialize the exact symbolic law `x - x = 0`.
pub fn validate_proof_certified_exact_integer_self_subtract_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_proof_certified_same_operand_integer_constant_candidate(
        input,
        candidate,
        ProofCertifiedSameOperandIntegerConstantLaw::ExactSubtractZero,
        b"omega.psi-rule.live-proof-certified-exact-integer-self-subtract-elimination.v1",
        b"omega.validator.live-proof-certified-exact-integer-self-subtract-elimination.v1",
    )
}

/// Independently validate the defined remainder laws `x % x = 0` for exact,
/// wrapping, and saturating fixed-width integers. The accepted obligation is
/// required because it is the capability proving the authored divisor is
/// legal; no operand constant or range fact is inferred.
pub fn validate_proof_certified_integer_self_remainder_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_proof_certified_same_operand_integer_constant_candidate(
        input,
        candidate,
        ProofCertifiedSameOperandIntegerConstantLaw::SelfRemainderZero,
        b"omega.psi-rule.live-proof-certified-integer-self-remainder-elimination.v1",
        b"omega.validator.live-proof-certified-integer-self-remainder-elimination.v1",
    )
}

/// Independently validate the defined division laws `x / x = 1` for exact,
/// wrapping, and saturating fixed-width integers. The accepted obligation is
/// the capability proving that the authored divisor is nonzero (and that the
/// signed overflow case is absent). Signed one-bit integers are excluded
/// because typed positive one is not representable.
pub fn validate_proof_certified_integer_self_divide_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_proof_certified_same_operand_integer_constant_candidate(
        input,
        candidate,
        ProofCertifiedSameOperandIntegerConstantLaw::SelfDivideOne,
        b"omega.psi-rule.live-proof-certified-integer-self-divide-elimination.v1",
        b"omega.validator.live-proof-certified-integer-self-divide-elimination.v1",
    )
}

/// Validate only the shared in-place constant custody. The operation-law
/// selector remains validation-local and closed, so adding one producer rule
/// cannot broaden another rule's accepted policy vocabulary.
fn validate_proof_certified_same_operand_integer_constant_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
    law: ProofCertifiedSameOperandIntegerConstantLaw,
    expected_rule_domain: &[u8],
    validator_domain: &[u8],
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    let expected_rule = OptimizationRuleIdentity::from_canonical_bytes(expected_rule_domain);
    if candidate.rule() != expected_rule
        || candidate.required_analyses()
            != AnalysisSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries])
        || candidate.invalidated_analyses()
            != AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ])
        || candidate.safety_class() != OptimizationSafetyClass::ProofCertified
        || !candidate.substitutions().is_empty()
        || candidate.predicted_cost_delta() != -1
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.location)
        || candidate.affected_blocks() != [patch.location.block]
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.location.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == patch.location.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let node = block
        .nodes
        .get(
            usize::try_from(patch.location.node)
                .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?,
        )
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let shape = independent_same_operand_integer_constant(&node.operation, law)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    if patch.source_operation != shape.psi_operation
        || patch.result != shape.result
        || patch.scalar_type != shape.scalar_type
        || patch.constant
            != match law {
                ProofCertifiedSameOperandIntegerConstantLaw::ExactSubtractZero
                | ProofCertifiedSameOperandIntegerConstantLaw::SelfRemainderZero => {
                    independent_integer_zero(shape.scalar_type)
                }
                ProofCertifiedSameOperandIntegerConstantLaw::SelfDivideOne => {
                    independent_integer_one(shape.scalar_type)
                }
            }
        || node.definitions
            != [ValueDefinition {
                value: shape.result,
                scalar_type: ScalarType::Integer(shape.scalar_type),
                site: ValueDefinitionSite::Node {
                    block: patch.location.block,
                    node: patch.location.node,
                },
            }]
    {
        return Err(OptimizationUnitValidationError::CandidateEvaluationMismatch);
    }
    let input_observation = observation_at(input, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if !input_observation.events.is_empty()
        || !input_observation.successors.is_empty()
        || !input_observation.ownership.is_empty()
        || input_observation.crash != ObservationKnowledge::No
        || input_observation.suspension != ObservationKnowledge::No
    {
        return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
    }
    let input_live = reconstruct_closed_scalar_node_boundary(input, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if !input_live.live_in.contains(&shape.operand) || !input_live.live_out.contains(&shape.result)
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let expected_fact = independently_accepted_operation_fact(
        input,
        function,
        shape.psi_operation,
        shape.obligation,
    )
    .ok_or(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)?;
    if candidate.accepted_obligation_witness() != Some(expected_fact) {
        return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
    }
    let site = PsiRealizationSite::Node(patch.location);
    let expected_provenance = [ProvenanceRewrite {
        input: site,
        disposition: ProvenanceDisposition::RealizedAt(site),
        sources: node.provenance.clone(),
        fuel: node.fuel.clone(),
    }];
    if candidate.provenance() != expected_provenance {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let accepted_catalog = input.accepted_obligation_facts.clone();
    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.location.machine)
        .expect("candidate source function exists");
    let output_block = output_function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.location.block)
        .expect("candidate source block exists");
    let output_node = &mut output_block.nodes[patch.location.node as usize];
    output_node.operation = O::IntegerConstant {
        psi_operation: shape.psi_operation,
        result: shape.result,
        scalar_type: ScalarType::Integer(shape.scalar_type),
        value: patch.constant,
    };
    output_node.definitions = vec![ValueDefinition {
        value: shape.result,
        scalar_type: ScalarType::Integer(shape.scalar_type),
        site: ValueDefinitionSite::Node {
            block: patch.location.block,
            node: patch.location.node,
        },
    }];
    output_node.uses.clear();
    output_node.successors.clear();
    output_node.ownership.clear();
    output_function.facts = reconstruct_fact_index(output_function);
    output.identity = recompute_psi_optimization_unit_identity(&output);
    if output.accepted_obligation_facts != accepted_catalog {
        return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
    }
    validate_psi_optimization_unit(&output)?;
    let output_observation = observation_at(&output, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if !same_closed_scalar_observation(&input_observation, &output_observation) {
        return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
    }
    let output_live = reconstruct_closed_scalar_node_boundary(&output, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if input_live.live_out != output_live.live_out
        || output_live
            .live_in
            .iter()
            .any(|value| !input_live.live_in.contains(value))
    {
        return Err(OptimizationUnitValidationError::CandidateLiveBoundaryMismatch);
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(validator_domain),
        provenance: expected_provenance.into(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IndependentRemainderUnitConstant {
    psi_operation: OperationId,
    obligation: psi_core::ObligationId,
    result: ValueId,
    scalar_type: IntegerType,
    left: ValueId,
    right: ValueId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IndependentRemainderUnitDivisor {
    PositiveOne,
    SignedNegativeOne,
}

impl IndependentRemainderUnitDivisor {
    fn value(self, scalar_type: IntegerType) -> Option<IntegerValue> {
        match self {
            Self::PositiveOne => Some(independent_integer_one(scalar_type)),
            Self::SignedNegativeOne if scalar_type.sign() == IntegerSign::Signed => {
                Some(IntegerValue::Signed(-1))
            }
            Self::SignedNegativeOne => None,
        }
    }
}

fn independent_remainder_unit_constant(
    operation: &O,
    divisor: IndependentRemainderUnitDivisor,
) -> Option<IndependentRemainderUnitConstant> {
    let (psi_operation, obligation, result, scalar_type, left, right) = match operation {
        O::ExactIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        }
        | O::WrappingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        }
        | O::SaturatingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } if scalar_type.carrier() == IntegerCarrier::Fixed && left != right => (
            *psi_operation,
            *obligation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        _ => return None,
    };
    divisor.value(scalar_type)?;
    Some(IndependentRemainderUnitConstant {
        psi_operation,
        obligation,
        result,
        scalar_type,
        left,
        right,
    })
}

/// Independently validate the defined integer laws `x % 1 = 0` for exact,
/// wrapping, and saturating fixed-width integers. The right operand must be a
/// direct typed literal, and the authored operation must retain its exact
/// verifier-accepted obligation even though the literal also establishes that
/// the divisor is nonzero.
pub fn validate_proof_certified_integer_remainder_by_one_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_proof_certified_integer_remainder_by_unit_candidate(
        input,
        candidate,
        IndependentRemainderUnitDivisor::PositiveOne,
        b"omega.psi-rule.live-proof-certified-integer-remainder-by-one-elimination.v1",
        b"omega.validator.live-proof-certified-integer-remainder-by-one-elimination.v1",
    )
}

/// Independently validate the defined signed integer laws `x % -1 = 0` for
/// exact, wrapping, and saturating fixed-width integers. The right operand must
/// be a direct typed literal, and the authored operation must retain its exact
/// verifier-accepted obligation. For exact arithmetic that accepted obligation
/// proves the otherwise exceptional signed-minimum input is unreachable.
pub fn validate_proof_certified_signed_integer_remainder_by_negative_one_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_proof_certified_integer_remainder_by_unit_candidate(
        input,
        candidate,
        IndependentRemainderUnitDivisor::SignedNegativeOne,
        b"omega.psi-rule.live-proof-certified-signed-integer-remainder-by-negative-one-elimination.v1",
        b"omega.validator.live-proof-certified-signed-integer-remainder-by-negative-one-elimination.v1",
    )
}

fn validate_proof_certified_integer_remainder_by_unit_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
    divisor: IndependentRemainderUnitDivisor,
    rule_domain: &[u8],
    validator_domain: &[u8],
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    let expected_rule = OptimizationRuleIdentity::from_canonical_bytes(rule_domain);
    if candidate.rule() != expected_rule
        || candidate.required_analyses()
            != AnalysisSet::new([
                AnalysisKind::ScalarConstants,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ])
        || candidate.invalidated_analyses()
            != AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ])
        || candidate.safety_class() != OptimizationSafetyClass::ProofCertified
        || !candidate.substitutions().is_empty()
        || candidate.predicted_cost_delta() != -1
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.location)
        || candidate.affected_blocks() != [patch.location.block]
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.location.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == patch.location.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let node = block
        .nodes
        .get(
            usize::try_from(patch.location.node)
                .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?,
        )
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let shape = independent_remainder_unit_constant(&node.operation, divisor)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    if patch.source_operation != shape.psi_operation
        || patch.result != shape.result
        || patch.scalar_type != shape.scalar_type
        || patch.constant != independent_integer_zero(shape.scalar_type)
        || node.definitions
            != [ValueDefinition {
                value: shape.result,
                scalar_type: ScalarType::Integer(shape.scalar_type),
                site: ValueDefinitionSite::Node {
                    block: patch.location.block,
                    node: patch.location.node,
                },
            }]
    {
        return Err(OptimizationUnitValidationError::CandidateEvaluationMismatch);
    }

    let right_definition = scalar_value_definition(function, shape.right)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    if right_definition.scalar_type != ScalarType::Integer(shape.scalar_type) {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    }
    let ValueDefinitionSite::Node {
        block: literal_block,
        node: literal_node,
    } = right_definition.site
    else {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    };
    let literal = function
        .blocks
        .iter()
        .find(|block| block.id == literal_block)
        .and_then(|block| {
            usize::try_from(literal_node)
                .ok()
                .and_then(|node| block.nodes.get(node))
        })
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let O::IntegerConstant {
        psi_operation: literal_support,
        result: literal_result,
        scalar_type: literal_type,
        value: literal_value,
    } = literal.operation
    else {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    };
    let expected_one = divisor
        .value(shape.scalar_type)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    if literal_result != shape.right
        || literal_type != ScalarType::Integer(shape.scalar_type)
        || literal_value != expected_one
        || !shape.scalar_type.admits(expected_one)
        || !function.facts.iter().any(|fact| {
            matches!(fact, OptimizationFact::IntegerConstant { value, constant, support }
                if *value == shape.right
                    && *constant == expected_one
                    && *support == literal_support)
        })
    {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    }
    let expected_constant_fact = literal_scalar_constant_fact_identity(
        input.identity,
        function.machine,
        right_definition,
        ScalarConstantValue::Integer(expected_one),
        literal_support,
    )
    .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let expected_obligation_fact = independently_accepted_operation_fact(
        input,
        function,
        shape.psi_operation,
        shape.obligation,
    )
    .ok_or(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)?;
    let Some((constant_fact, obligation_fact)) =
        candidate.proof_certified_scalar_identity_witness()
    else {
        return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
    };
    if constant_fact != expected_constant_fact {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    }
    if obligation_fact != expected_obligation_fact {
        return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
    }

    let input_observation = observation_at(input, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if !input_observation.events.is_empty()
        || !input_observation.successors.is_empty()
        || !input_observation.ownership.is_empty()
        || input_observation.crash != ObservationKnowledge::No
        || input_observation.suspension != ObservationKnowledge::No
    {
        return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
    }
    let input_live = reconstruct_closed_scalar_node_boundary(input, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if !input_live.live_in.contains(&shape.left)
        || !input_live.live_in.contains(&shape.right)
        || !input_live.live_out.contains(&shape.result)
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let site = PsiRealizationSite::Node(patch.location);
    let expected_provenance = [ProvenanceRewrite {
        input: site,
        disposition: ProvenanceDisposition::RealizedAt(site),
        sources: node.provenance.clone(),
        fuel: node.fuel.clone(),
    }];
    if candidate.provenance() != expected_provenance {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let accepted_catalog = input.accepted_obligation_facts.clone();
    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.location.machine)
        .expect("candidate source function exists");
    let output_block = output_function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.location.block)
        .expect("candidate source block exists");
    let output_node = &mut output_block.nodes[patch.location.node as usize];
    output_node.operation = O::IntegerConstant {
        psi_operation: shape.psi_operation,
        result: shape.result,
        scalar_type: ScalarType::Integer(shape.scalar_type),
        value: patch.constant,
    };
    output_node.definitions = vec![ValueDefinition {
        value: shape.result,
        scalar_type: ScalarType::Integer(shape.scalar_type),
        site: ValueDefinitionSite::Node {
            block: patch.location.block,
            node: patch.location.node,
        },
    }];
    output_node.uses.clear();
    output_node.successors.clear();
    output_node.ownership.clear();
    output_function.facts = reconstruct_fact_index(output_function);
    output.identity = recompute_psi_optimization_unit_identity(&output);
    if output.accepted_obligation_facts != accepted_catalog {
        return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
    }
    validate_psi_optimization_unit(&output)?;
    let output_observation = observation_at(&output, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if !same_closed_scalar_observation(&input_observation, &output_observation) {
        return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
    }
    let output_live = reconstruct_closed_scalar_node_boundary(&output, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if input_live.live_out != output_live.live_out
        || output_live
            .live_in
            .iter()
            .any(|value| !input_live.live_in.contains(value))
    {
        return Err(OptimizationUnitValidationError::CandidateLiveBoundaryMismatch);
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(validator_domain),
        provenance: expected_provenance.into(),
    })
}

/// Dispatch one typed scalar-constant candidate to its independent validator.
pub fn validate_scalar_evaluation_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    if candidate.rule()
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.live-proof-certified-exact-integer-self-subtract-elimination.v1",
        )
    {
        return validate_proof_certified_exact_integer_self_subtract_candidate(input, candidate);
    }
    if candidate.rule()
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.live-proof-certified-integer-self-remainder-elimination.v1",
        )
    {
        return validate_proof_certified_integer_self_remainder_candidate(input, candidate);
    }
    if candidate.rule()
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.live-proof-certified-integer-self-divide-elimination.v1",
        )
    {
        return validate_proof_certified_integer_self_divide_candidate(input, candidate);
    }
    if candidate.rule()
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.live-proof-certified-integer-remainder-by-one-elimination.v1",
        )
    {
        return validate_proof_certified_integer_remainder_by_one_candidate(input, candidate);
    }
    if candidate.rule()
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.live-proof-certified-signed-integer-remainder-by-negative-one-elimination.v1",
        )
    {
        return validate_proof_certified_signed_integer_remainder_by_negative_one_candidate(
            input, candidate,
        );
    }
    match candidate.patch() {
        PsiRewritePatch::ReplaceIntegerOperationWithConstant(_) => {
            validate_integer_evaluation_candidate(input, candidate)
        }
        PsiRewritePatch::ReplaceBooleanOperationWithConstant(_) => {
            validate_boolean_evaluation_candidate(input, candidate)
        }
        PsiRewritePatch::RemoveRedundantBlockParameter(_) => {
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        }
        PsiRewritePatch::FoldConstantConditional(_) => {
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        }
        PsiRewritePatch::ThreadLinearEmptyBlock(_) => {
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        }
        PsiRewritePatch::ThreadPathQualifiedEmptyBlock(_) => {
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        }
        PsiRewritePatch::MergeAdjacentBlock(_)
        | PsiRewritePatch::MergeNonAdjacentBlock(_)
        | PsiRewritePatch::FuseSharedTerminalJump(_)
        | PsiRewritePatch::RemoveDeadScalarNode(_)
        | PsiRewritePatch::EliminateLocalScalarCommonSubexpression(_)
        | PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(_)
        | PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(_)
        | PsiRewritePatch::EliminateProofCertifiedScalarIdentity(_)
        | PsiRewritePatch::PruneUnreachablePrivateMachines(_) => {
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        }
    }
}

/// Dispatch one closed Psi rewrite candidate to a patch-specific independent
/// validator. Rules cannot construct accepted outputs themselves.
pub fn validate_psi_rewrite_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    match candidate.patch() {
        PsiRewritePatch::ReplaceIntegerOperationWithConstant(_)
        | PsiRewritePatch::ReplaceBooleanOperationWithConstant(_) => {
            validate_scalar_evaluation_candidate(input, candidate)
        }
        PsiRewritePatch::RemoveRedundantBlockParameter(_) => {
            validate_redundant_block_parameter_candidate(input, candidate)
        }
        PsiRewritePatch::FoldConstantConditional(_) => {
            validate_constant_conditional_candidate(input, candidate)
        }
        PsiRewritePatch::ThreadLinearEmptyBlock(_) => {
            validate_linear_empty_block_candidate(input, candidate)
        }
        PsiRewritePatch::ThreadPathQualifiedEmptyBlock(_) => {
            validate_path_qualified_empty_block_candidate(input, candidate)
        }
        PsiRewritePatch::MergeAdjacentBlock(_) => {
            validate_adjacent_block_merge_candidate(input, candidate)
        }
        PsiRewritePatch::MergeNonAdjacentBlock(_) => {
            validate_non_adjacent_block_merge_candidate(input, candidate)
        }
        PsiRewritePatch::FuseSharedTerminalJump(_) => {
            validate_shared_jump_fusion_candidate(input, candidate)
        }
        PsiRewritePatch::RemoveDeadScalarNode(_) => {
            validate_dead_scalar_node_candidate(input, candidate)
        }
        PsiRewritePatch::EliminateLocalScalarCommonSubexpression(_) => {
            validate_local_scalar_common_subexpression_candidate(input, candidate)
        }
        PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(_) => {
            validate_dominating_scalar_common_subexpression_candidate(input, candidate)
        }
        PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(_) => {
            validate_phi_translated_scalar_common_subexpression_candidate(input, candidate)
        }
        PsiRewritePatch::EliminateProofCertifiedScalarIdentity(_) => {
            validate_proof_certified_scalar_identity_candidate(input, candidate)
        }
        PsiRewritePatch::PruneUnreachablePrivateMachines(_) => {
            validate_unreachable_private_machines_candidate(input, candidate)
        }
    }
}

/// Independently reconstruct the complete executable-machine root closure and
/// remove its exact active complement. Proof/frontier catalogs remain immutable
/// historical custody; only executable function bodies leave the active roster.
pub fn validate_unreachable_private_machines_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::CallGraph)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::CallGraph)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || candidate.safety_class() != OptimizationSafetyClass::StructuralIdentity
        || !candidate.affected_blocks().is_empty()
        || !candidate.substitutions().is_empty()
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let omega_optimization_unit::PsiRewriteDecisionPoint::MachineSet(decision_machines) =
        candidate.decision_point()
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    let PsiRewritePatch::PruneUnreachablePrivateMachines(patch) = candidate.patch_ref() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    let expected_machines = unreachable_private_machine_complement(input);
    let patch_machines = patch
        .machines
        .iter()
        .map(|row| row.machine)
        .collect::<Vec<_>>();
    if expected_machines.is_empty()
        || *decision_machines != expected_machines
        || patch_machines != expected_machines
        || candidate.affected_machines() != expected_machines
    {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    let source_ordinals = validator_active_source_ordinals(input);
    let expected_custody = expected_machines
        .iter()
        .map(|machine| omega_optimization_unit::PrunedMachineCustody {
            machine: *machine,
            source_ordinal: source_ordinals[machine],
        })
        .collect::<Vec<_>>();
    if patch.machines != expected_custody {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    let expected_provenance = pruned_machine_provenance(input, &expected_machines)
        .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.provenance() != expected_provenance {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }
    let removed = expected_machines.iter().copied().collect::<BTreeSet<_>>();
    let mut output = input.clone();
    output
        .functions
        .retain(|function| !removed.contains(&function.machine));
    output.pruned_machines.extend(expected_custody);
    output.pruned_machines.sort_unstable();
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.unreachable-private-machine-pruning.v1",
        ),
        provenance: expected_provenance,
    })
}

fn validator_active_source_ordinals(unit: &PsiOptimizationUnit) -> BTreeMap<MachineId, u32> {
    let pruned = unit
        .pruned_machines
        .iter()
        .map(|row| (row.source_ordinal, row.machine))
        .collect::<BTreeMap<_, _>>();
    let mut active = unit.functions.iter();
    let mut result = BTreeMap::new();
    for ordinal in 0..(unit.functions.len() + unit.pruned_machines.len()) {
        let ordinal = u32::try_from(ordinal).expect("function ordinal fits u32");
        if !pruned.contains_key(&ordinal)
            && let Some(function) = active.next()
        {
            result.insert(function.machine, ordinal);
        }
    }
    result
}

fn unreachable_private_machine_complement(unit: &PsiOptimizationUnit) -> Vec<MachineId> {
    let active = unit
        .functions
        .iter()
        .map(|function| function.machine)
        .collect::<BTreeSet<_>>();
    let mut reachable = BTreeSet::from([unit.entry]);
    reachable.extend(
        unit.provider_candidates
            .iter()
            .map(|candidate| candidate.candidate),
    );
    reachable.extend(
        unit.functions
            .iter()
            .filter(|function| function.attachment.is_some())
            .map(|function| function.machine),
    );
    let references = unit
        .functions
        .iter()
        .map(|function| (function.machine, validator_machine_references(function)))
        .collect::<BTreeMap<_, _>>();
    let mut work = reachable.iter().copied().collect::<Vec<_>>();
    while let Some(machine) = work.pop() {
        for callee in references.get(&machine).into_iter().flatten().copied() {
            if active.contains(&callee) && reachable.insert(callee) {
                work.push(callee);
            }
        }
    }
    active.difference(&reachable).copied().collect()
}

fn validator_machine_references(function: &PsiOptimizationFunction) -> BTreeSet<MachineId> {
    let mut references = BTreeSet::new();
    for operation in function
        .blocks
        .iter()
        .flat_map(|block| block.nodes.iter().map(|node| &node.operation))
    {
        match operation {
            O::CallUnit { callee, .. }
            | O::CallStructuralScalar { callee, .. }
            | O::CallStructural { callee, .. }
            | O::Call { callee, .. } => {
                references.insert(*callee);
            }
            O::Return {
                cleanup_actions, ..
            }
            | O::ReturnUnit {
                cleanup_actions, ..
            } => {
                references.extend(cleanup_actions.iter().filter_map(|action| match action {
                    psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                        Some(cleanup.cleanup_machine)
                    }
                    psi_terminal::TerminalAffineCleanupAction::DiscardRoot(_)
                    | psi_terminal::TerminalAffineCleanupAction::DiscardResidual(_) => None,
                }));
            }
            _ => {}
        }
    }
    references
}

fn pruned_machine_provenance(
    unit: &PsiOptimizationUnit,
    machines: &[MachineId],
) -> Option<Vec<omega_optimization_unit::ProvenanceRewrite>> {
    let machines = machines.iter().copied().collect::<BTreeSet<_>>();
    let mut rows = Vec::new();
    for function in unit
        .functions
        .iter()
        .filter(|function| machines.contains(&function.machine))
    {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let input = PsiRealizationSite::Node(NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: u32::try_from(node_index).ok()?,
                });
                if !node.provenance.is_empty() {
                    rows.push(omega_optimization_unit::ProvenanceRewrite {
                        input,
                        disposition: ProvenanceDisposition::ProvenUnreachableAt(input),
                        sources: node.provenance.clone(),
                        fuel: node.fuel.clone(),
                    });
                }
                for edge in &node.successors {
                    let input = PsiRealizationSite::Edge {
                        machine: function.machine,
                        edge: edge.psi_edge,
                    };
                    if !edge.provenance.is_empty() {
                        rows.push(omega_optimization_unit::ProvenanceRewrite {
                            input,
                            disposition: ProvenanceDisposition::ProvenUnreachableAt(input),
                            sources: edge.provenance.clone(),
                            fuel: edge.fuel.clone(),
                        });
                    }
                }
            }
        }
    }
    rows.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some(rows)
}

/// Independently replay one Boolean-proven conditional fold and atomically
/// remove the exact block complement made unreachable by the rejected edge.
pub fn validate_constant_conditional_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::ScalarConstants)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::CallGraph)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || candidate.safety_class() != OptimizationSafetyClass::ExactOperationSemantics
        || !candidate.substitutions().is_empty()
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::FoldConstantConditional(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.location) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.location.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == patch.location.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let node = block
        .nodes
        .get(usize::try_from(patch.location.node).expect("u32 fits usize"))
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let O::Conditional {
        condition,
        when_true,
        when_false,
    } = &node.operation
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    let condition_fact = candidate
        .scalar_evaluation_witness()
        .and_then(IntegerEvaluationWitness::unary_operand)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let constant = literal_boolean_fact(function, input.identity, *condition, condition_fact)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let (selected, rejected) = if constant {
        (when_true, when_false)
    } else {
        (when_false, when_true)
    };
    if patch
        != (ConstantConditionalRewrite {
            location: patch.location,
            condition: *condition,
            constant,
            selected_edge: selected.psi_edge,
            rejected_edge: rejected.psi_edge,
        })
    {
        return Err(OptimizationUnitValidationError::CandidateEvaluationMismatch);
    }
    let reachable =
        reachable_blocks_after_conditional_fold(function, patch.location.block, selected.psi_edge)
            .ok_or(OptimizationUnitValidationError::CandidateReachabilityMismatch)?;
    let (expected_blocks, accepted_provenance) = reconstruct_conditional_fold_accounting(
        function,
        patch.location,
        selected.psi_edge,
        rejected.psi_edge,
        &reachable,
    )
    .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.affected_blocks() != expected_blocks {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    if candidate.provenance().len() != accepted_provenance.len()
        || candidate
            .provenance()
            .iter()
            .zip(&accepted_provenance)
            .any(|(actual, expected)| {
                actual.input != expected.input
                    || actual.disposition != expected.disposition
                    || actual.sources != expected.sources
            })
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }
    if candidate
        .provenance()
        .iter()
        .zip(&accepted_provenance)
        .any(|(actual, expected)| actual.fuel != expected.fuel)
    {
        return Err(OptimizationUnitValidationError::CandidateFuelMismatch);
    }
    let selected_site = PsiRealizationSite::Edge {
        machine: patch.location.machine,
        edge: selected.psi_edge,
    };
    let selected_fuel = accepted_provenance
        .iter()
        .find(|row| row.disposition == ProvenanceDisposition::RealizedAt(selected_site))
        .expect("independent accounting includes the selected edge")
        .fuel
        .clone();

    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.location.machine)
        .expect("candidate function exists");
    let output_block = output_function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.location.block)
        .expect("candidate block exists");
    let output_node =
        &mut output_block.nodes[usize::try_from(patch.location.node).expect("u32 fits usize")];
    output_node.operation = O::Jump {
        psi_edge: selected.psi_edge,
        target: selected.target,
        bindings: selected.bindings.clone(),
        trivial_affine_discards: selected.trivial_affine_discards.clone(),
    };
    output_node.definitions.clear();
    output_node.uses = selected
        .bindings
        .iter()
        .map(|binding| ValueUse {
            value: binding.argument,
            block: patch.location.block,
            node: patch.location.node,
        })
        .collect();
    output_node.successors = vec![OptimizationEdge {
        psi_edge: selected.psi_edge,
        target: selected.target,
        bindings: selected.bindings.clone(),
        trivial_affine_discards: selected.trivial_affine_discards.clone(),
        provenance: vec![PsiProvenance::Edge(selected.psi_edge)],
        fuel: selected_fuel,
    }];
    output_node.ownership.clear();
    output_node.provenance.clear();
    output_node.fuel.clear();
    output_function
        .blocks
        .retain(|block| reachable.contains(&block.id));
    let mut effect = 0u64;
    for block in &mut output_function.blocks {
        for node in &mut block.nodes {
            node.effect = omega_optimization_unit::EffectLink {
                input: effect,
                output: effect
                    .checked_add(1)
                    .expect("validated function effect count fits u64"),
            };
            effect = effect
                .checked_add(1)
                .expect("validated function effect count fits u64");
        }
    }
    output_function.facts = reconstruct_fact_index(output_function);
    output_function.declared_places = reconstruct_declared_places(output_function)?;
    refresh_root_service_reach(&mut output)?;
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;

    let output_function = output
        .functions
        .iter()
        .find(|function| function.machine == patch.location.machine)
        .expect("output function exists");
    for input_block in function
        .blocks
        .iter()
        .filter(|block| reachable.contains(&block.id))
    {
        if !expected_blocks.contains(&input_block.id)
            && output_function
                .blocks
                .iter()
                .find(|block| block.id == input_block.id)
                != Some(input_block)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.constant-conditional-fold.v4",
        ),
        provenance: accepted_provenance,
    })
}

fn reachable_blocks_after_conditional_fold(
    function: &PsiOptimizationFunction,
    source: BlockId,
    selected_edge: EdgeId,
) -> Option<BTreeSet<BlockId>> {
    let mut reachable = BTreeSet::new();
    let mut pending = vec![function.entry];
    while let Some(block_id) = pending.pop() {
        if !reachable.insert(block_id) {
            continue;
        }
        let block = function.blocks.iter().find(|block| block.id == block_id)?;
        for edge in block.nodes.iter().flat_map(|node| &node.successors) {
            if block_id != source || edge.psi_edge == selected_edge {
                pending.push(edge.target);
            }
        }
    }
    Some(reachable)
}

fn reconstruct_conditional_fold_accounting(
    function: &PsiOptimizationFunction,
    decision: NodeLocation,
    selected_edge: EdgeId,
    rejected_edge: EdgeId,
    reachable: &BTreeSet<BlockId>,
) -> Option<(
    Vec<BlockId>,
    Vec<omega_optimization_unit::ProvenanceRewrite>,
)> {
    let decision_node = function
        .blocks
        .iter()
        .find(|block| block.id == decision.block)?
        .nodes
        .get(usize::try_from(decision.node).ok()?)?;
    let selected = decision_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == selected_edge)?;
    let rejected = decision_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == rejected_edge)?;
    let selected_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: selected_edge,
    };
    let rejected_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: rejected_edge,
    };
    let removed = function
        .blocks
        .iter()
        .map(|block| block.id)
        .filter(|block| !reachable.contains(block))
        .collect::<BTreeSet<_>>();
    let mut affected = BTreeSet::from([decision.block]);
    affected.extend(removed.iter().copied());
    let mut realized = vec![omega_optimization_unit::ProvenanceRewrite {
        input: selected_site,
        disposition: ProvenanceDisposition::RealizedAt(selected_site),
        sources: selected.provenance.clone(),
        fuel: selected.fuel.clone(),
    }];
    let mut unreachable = vec![omega_optimization_unit::ProvenanceRewrite {
        input: rejected_site,
        disposition: ProvenanceDisposition::ProvenUnreachableAt(rejected_site),
        sources: rejected.provenance.clone(),
        fuel: rejected.fuel.clone(),
    }];
    let mut expected_effect = 0u64;
    for block in &function.blocks {
        if removed.contains(&block.id) {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let location = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: u32::try_from(node_index).ok()?,
                };
                if !node.provenance.is_empty() {
                    let site = PsiRealizationSite::Node(location);
                    unreachable.push(omega_optimization_unit::ProvenanceRewrite {
                        input: site,
                        disposition: ProvenanceDisposition::ProvenUnreachableAt(site),
                        sources: node.provenance.clone(),
                        fuel: node.fuel.clone(),
                    });
                }
                for edge in &node.successors {
                    let site = PsiRealizationSite::Edge {
                        machine: function.machine,
                        edge: edge.psi_edge,
                    };
                    unreachable.push(omega_optimization_unit::ProvenanceRewrite {
                        input: site,
                        disposition: ProvenanceDisposition::ProvenUnreachableAt(site),
                        sources: edge.provenance.clone(),
                        fuel: edge.fuel.clone(),
                    });
                }
            }
            continue;
        }
        for (node_index, node) in block.nodes.iter().enumerate() {
            let location = NodeLocation {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).ok()?,
            };
            let effect_changes = node.effect.input != expected_effect
                || node.effect.output != expected_effect.checked_add(1)?;
            if effect_changes && location != decision {
                affected.insert(block.id);
                if !node.provenance.is_empty() {
                    let site = PsiRealizationSite::Node(location);
                    realized.push(omega_optimization_unit::ProvenanceRewrite {
                        input: site,
                        disposition: ProvenanceDisposition::RealizedAt(site),
                        sources: node.provenance.clone(),
                        fuel: node.fuel.clone(),
                    });
                }
            }
            expected_effect = expected_effect.checked_add(1)?;
        }
    }
    realized.extend(unreachable);
    realized.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((affected.into_iter().collect(), realized))
}

/// Independently replay one linear empty-jump thread. This deliberately
/// excludes conditional or multiple predecessors: only two edge occurrences
/// that always execute together may be fused into one output edge occurrence.
pub fn validate_linear_empty_block_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::OwnershipFrontiers)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || candidate.safety_class() != OptimizationSafetyClass::StructuralIdentity
        || !candidate.substitutions().is_empty()
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::ThreadLinearEmptyBlock(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.predecessor)
        || patch.empty.node != 0
        || patch.empty.machine != patch.predecessor.machine
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.predecessor.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if function.entry == patch.empty.block {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    let empty_block = function
        .blocks
        .iter()
        .find(|block| block.id == patch.empty.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let [empty_node] = empty_block.nodes.as_slice() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    let O::Jump {
        psi_edge: outgoing_edge,
        target,
        bindings: outgoing_bindings,
        trivial_affine_discards: outgoing_discards,
    } = &empty_node.operation
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if *outgoing_edge != patch.outgoing_edge || *target != patch.target {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    if empty_block.parameters.iter().any(|parameter| {
        function.blocks.iter().any(|block| {
            block.nodes.iter().any(|node| {
                node.uses.iter().any(|use_site| {
                    use_site.value == parameter.value
                        && (use_site.block != empty_block.id || use_site.node != 0)
                })
            })
        })
    }) {
        return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
    }

    let incoming = function
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .nodes
                .iter()
                .enumerate()
                .filter_map(move |(node_index, node)| {
                    node.successors
                        .iter()
                        .any(|edge| edge.target == patch.empty.block)
                        .then_some((block, node_index, node))
                })
        })
        .collect::<Vec<_>>();
    let [(predecessor_block, predecessor_index, predecessor_node)] = incoming.as_slice() else {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    };
    let predecessor_location = NodeLocation {
        machine: function.machine,
        block: predecessor_block.id,
        node: u32::try_from(*predecessor_index)
            .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?,
    };
    let O::Jump {
        psi_edge: incoming_edge,
        target: predecessor_target,
        bindings: incoming_bindings,
        trivial_affine_discards: incoming_discards,
    } = &predecessor_node.operation
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if !incoming_discards.is_empty()
        || !outgoing_discards.is_empty()
        || predecessor_location != patch.predecessor
        || *incoming_edge != patch.incoming_edge
        || *predecessor_target != patch.empty.block
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let composed_bindings = reconstruct_linear_thread_bindings(
        &empty_block.parameters,
        incoming_bindings,
        outgoing_bindings,
    )
    .ok_or(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)?;
    if !reconstruct_linear_thread_ownership_is_identity(
        input,
        function,
        patch.incoming_edge,
        patch.empty.block,
        patch.outgoing_edge,
        patch.target,
    ) {
        return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
    }
    let (expected_blocks, accepted_provenance) =
        reconstruct_linear_thread_accounting(function, patch.predecessor, patch.empty)
            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.affected_blocks() != expected_blocks {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    if candidate.provenance().len() != accepted_provenance.len()
        || candidate
            .provenance()
            .iter()
            .zip(&accepted_provenance)
            .any(|(actual, expected)| {
                actual.input != expected.input
                    || actual.disposition != expected.disposition
                    || actual.sources != expected.sources
            })
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }
    if candidate
        .provenance()
        .iter()
        .zip(&accepted_provenance)
        .any(|(actual, expected)| actual.fuel != expected.fuel)
    {
        return Err(OptimizationUnitValidationError::CandidateFuelMismatch);
    }

    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.predecessor.machine)
        .expect("candidate function exists");
    let output_predecessor = output_function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.predecessor.block)
        .and_then(|block| {
            block
                .nodes
                .get_mut(usize::try_from(patch.predecessor.node).ok()?)
        })
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let predecessor_edge = predecessor_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == patch.incoming_edge)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    let empty_edge = empty_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == patch.outgoing_edge)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    let mut combined_sources = predecessor_edge.provenance.clone();
    combined_sources.extend_from_slice(&empty_edge.provenance);
    let mut combined_fuel = predecessor_edge.fuel.clone();
    combined_fuel.extend_from_slice(&empty_edge.fuel);
    output_predecessor.operation = O::Jump {
        psi_edge: patch.incoming_edge,
        target: patch.target,
        bindings: composed_bindings,
        trivial_affine_discards: Vec::new(),
    };
    output_predecessor.definitions = expected_definitions(
        &output_predecessor.operation,
        patch.predecessor.block,
        patch.predecessor.node,
    );
    output_predecessor.uses = expected_uses(
        &output_predecessor.operation,
        patch.predecessor.block,
        patch.predecessor.node,
    );
    output_predecessor.successors = expected_edges(&output_predecessor.operation);
    output_predecessor.successors[0].provenance = combined_sources;
    output_predecessor.successors[0].fuel = combined_fuel;
    output_predecessor.ownership = expected_ownership(&output_predecessor.operation);
    output_predecessor.provenance.clear();
    output_predecessor.fuel.clear();
    output_function
        .blocks
        .retain(|block| block.id != patch.empty.block);
    let mut effect = 0u64;
    for block in &mut output_function.blocks {
        for node in &mut block.nodes {
            node.effect = omega_optimization_unit::EffectLink {
                input: effect,
                output: effect
                    .checked_add(1)
                    .expect("validated function effect count fits u64"),
            };
            effect = effect
                .checked_add(1)
                .expect("validated function effect count fits u64");
        }
    }
    output_function.facts = reconstruct_fact_index(output_function);
    output_function.declared_places = reconstruct_declared_places(output_function)?;
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;

    let output_function = output
        .functions
        .iter()
        .find(|function| function.machine == patch.predecessor.machine)
        .expect("output function exists");
    for input_block in &function.blocks {
        if input_block.id != patch.empty.block
            && !expected_blocks.contains(&input_block.id)
            && output_function
                .blocks
                .iter()
                .find(|block| block.id == input_block.id)
                != Some(input_block)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.linear-empty-block-thread.v2",
        ),
        provenance: accepted_provenance,
    })
}

/// Independently replay an all-predecessor empty-block bypass. Every incoming
/// edge remains its own output occurrence; the removed outgoing occurrence is
/// copied only onto that mutually exclusive edge antichain.
pub fn validate_path_qualified_empty_block_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::OwnershipFrontiers)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || candidate.safety_class() != OptimizationSafetyClass::StructuralIdentity
        || !candidate.substitutions().is_empty()
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::ThreadPathQualifiedEmptyBlock(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.empty) || patch.empty.node != 0 {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.empty.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if function.entry == patch.empty.block {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    let empty_block = function
        .blocks
        .iter()
        .find(|block| block.id == patch.empty.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let [empty_node] = empty_block.nodes.as_slice() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    let O::Jump {
        psi_edge: outgoing_edge,
        target,
        bindings: outgoing_bindings,
        trivial_affine_discards: outgoing_discards,
    } = &empty_node.operation
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if !outgoing_discards.is_empty()
        || *outgoing_edge != patch.outgoing_edge
        || *target != patch.target
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    if empty_block.parameters.iter().any(|parameter| {
        function.blocks.iter().any(|block| {
            block.nodes.iter().any(|node| {
                node.uses.iter().any(|use_site| {
                    use_site.value == parameter.value
                        && (use_site.block != empty_block.id || use_site.node != 0)
                })
            })
        })
    }) {
        return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
    }
    let mut incoming = Vec::new();
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            for edge in node
                .successors
                .iter()
                .filter(|edge| edge.target == patch.empty.block)
            {
                if !edge.trivial_affine_discards.is_empty() {
                    return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                }
                let composed = reconstruct_linear_thread_bindings(
                    &empty_block.parameters,
                    &edge.bindings,
                    outgoing_bindings,
                )
                .ok_or(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)?;
                if !reconstruct_linear_thread_ownership_is_identity(
                    input,
                    function,
                    edge.psi_edge,
                    patch.empty.block,
                    patch.outgoing_edge,
                    patch.target,
                ) {
                    return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
                }
                incoming.push((
                    NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: u32::try_from(node_index).map_err(|_| {
                            OptimizationUnitValidationError::CandidateLocationMissing
                        })?,
                    },
                    edge.psi_edge,
                    composed,
                ));
            }
        }
    }
    if incoming.is_empty()
        || (incoming.len() == 1
            && matches!(
                function
                    .blocks
                    .iter()
                    .find(|block| block.id == incoming[0].0.block)
                    .and_then(|block| block.nodes.get(usize::try_from(incoming[0].0.node).ok()?))
                    .map(|node| &node.operation),
                Some(O::Jump { .. })
            ))
    {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    let incoming_edges = incoming
        .iter()
        .map(|(_, edge, _)| *edge)
        .collect::<Vec<_>>();
    let (expected_blocks, accepted_provenance) =
        reconstruct_path_thread_accounting(function, patch.empty, &incoming_edges)
            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.affected_blocks() != expected_blocks
        || candidate.provenance() != accepted_provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let outgoing_edge = empty_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == patch.outgoing_edge)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.empty.machine)
        .expect("candidate function exists");
    for (location, incoming_edge, composed) in &incoming {
        let node = output_function
            .blocks
            .iter_mut()
            .find(|block| block.id == location.block)
            .and_then(|block| block.nodes.get_mut(usize::try_from(location.node).ok()?))
            .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
        if !rewrite_successor_operation(&mut node.operation, *incoming_edge, patch.target, composed)
        {
            return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
        }
        let edge = node
            .successors
            .iter_mut()
            .find(|edge| edge.psi_edge == *incoming_edge)
            .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
        edge.target = patch.target;
        edge.bindings = composed.clone();
        edge.provenance.extend_from_slice(&outgoing_edge.provenance);
        edge.fuel.extend_from_slice(&outgoing_edge.fuel);
        node.definitions = expected_definitions(&node.operation, location.block, location.node);
        node.uses = expected_uses(&node.operation, location.block, location.node);
        node.ownership = expected_ownership(&node.operation);
    }
    output_function
        .blocks
        .retain(|block| block.id != patch.empty.block);
    let mut effect = 0u64;
    for block in &mut output_function.blocks {
        for node in &mut block.nodes {
            node.effect = omega_optimization_unit::EffectLink {
                input: effect,
                output: effect
                    .checked_add(1)
                    .expect("validated function effect count fits u64"),
            };
            effect = effect
                .checked_add(1)
                .expect("validated function effect count fits u64");
        }
    }
    output_function.facts = reconstruct_fact_index(output_function);
    output_function.declared_places = reconstruct_declared_places(output_function)?;
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;

    let output_function = output
        .functions
        .iter()
        .find(|function| function.machine == patch.empty.machine)
        .expect("output function exists");
    for input_block in &function.blocks {
        if input_block.id != patch.empty.block
            && !expected_blocks.contains(&input_block.id)
            && output_function
                .blocks
                .iter()
                .find(|block| block.id == input_block.id)
                != Some(input_block)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.path-qualified-empty-block-thread.v1",
        ),
        provenance: accepted_provenance,
    })
}

/// Independently replay one adjacent single-predecessor block merge. The
/// validator rederives adjacency, unique incoming custody, typed parameter
/// substitutions, ownership-frontier identity, and every moved occurrence.
pub fn validate_adjacent_block_merge_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::Dominators)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::OwnershipFrontiers)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || candidate.safety_class() != OptimizationSafetyClass::StructuralIdentity
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::MergeAdjacentBlock(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.predecessor) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.predecessor.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let predecessor_position = function
        .blocks
        .iter()
        .position(|block| block.id == patch.predecessor.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let target_position = function
        .blocks
        .iter()
        .position(|block| block.id == patch.target)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if target_position != predecessor_position + 1 || function.entry == patch.target {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    let predecessor = &function.blocks[predecessor_position];
    let target = &function.blocks[target_position];
    let predecessor_index = usize::try_from(patch.predecessor.node)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    let eligible_first = target.nodes.first().is_some_and(|node| {
        (node.successors.is_empty()
            && (matches!(node.provenance.first(), Some(PsiProvenance::Operation(_)))
                || (matches!(node.provenance.first(), Some(PsiProvenance::Edge(_)))
                    && matches!(
                        node.operation,
                        O::Return { .. }
                            | O::ReturnUnit { .. }
                            | O::ReturnStructural { .. }
                            | O::Crash { .. }
                    ))))
            || (matches!(node.operation, O::Conditional { .. })
                && node.successors.len() == 2
                && node.provenance.is_empty())
    });
    if predecessor_index + 1 != predecessor.nodes.len() || !eligible_first {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let predecessor_node = &predecessor.nodes[predecessor_index];
    let O::Jump {
        psi_edge,
        target: jump_target,
        bindings,
        trivial_affine_discards,
    } = &predecessor_node.operation
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if !trivial_affine_discards.is_empty()
        || *psi_edge != patch.incoming_edge
        || *jump_target != patch.target
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let incoming = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .filter(|edge| edge.target == patch.target)
        .collect::<Vec<_>>();
    if incoming.len() != 1 || incoming[0].psi_edge != patch.incoming_edge {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    if target.parameters.len() != bindings.len() {
        return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
    }
    let dominators = independent_reachable_dominators(function);
    let mut substitutions = target
        .parameters
        .iter()
        .zip(bindings)
        .map(|(parameter, binding)| {
            (binding.parameter == parameter.value
                && binding.scalar_type == parameter.scalar_type
                && independently_replacement_dominates_uses(
                    function,
                    &dominators,
                    binding.argument,
                    parameter.value,
                    parameter.scalar_type,
                ))
            .then_some(ScalarSubstitution {
                from: parameter.value,
                to: binding.argument,
                scalar_type: parameter.scalar_type,
            })
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)?;
    substitutions.sort();
    if candidate.substitutions() != substitutions {
        return Err(OptimizationUnitValidationError::CandidateSubstitutionMismatch);
    }
    let ownership_witness = reconstruct_adjacent_merge_ownership_witness(
        input,
        function,
        patch.incoming_edge,
        patch.target,
    )
    .ok_or(OptimizationUnitValidationError::CandidateObservationMismatch)?;
    if candidate.ownership_frontier_witness() != Some(&ownership_witness) {
        return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
    }
    let (expected_blocks, accepted_provenance) =
        reconstruct_adjacent_merge_accounting(function, patch, &substitutions)
            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.affected_blocks() != expected_blocks
        || candidate.provenance() != accepted_provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.predecessor.machine)
        .expect("candidate function exists");
    let mut moved = output_function.blocks.remove(target_position);
    let output_predecessor = &mut output_function.blocks[predecessor_position];
    let removed = output_predecessor
        .nodes
        .pop()
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let removed_edge = removed
        .successors
        .iter()
        .find(|edge| edge.psi_edge == patch.incoming_edge)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    let first = moved
        .nodes
        .first_mut()
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    if first.successors.is_empty() {
        first.provenance.extend_from_slice(&removed_edge.provenance);
        first.fuel.extend_from_slice(&removed_edge.fuel);
    } else {
        for successor in &mut first.successors {
            successor
                .provenance
                .extend_from_slice(&removed_edge.provenance);
            successor.fuel.extend_from_slice(&removed_edge.fuel);
        }
    }
    output_predecessor.nodes.append(&mut moved.nodes);
    for block in &mut output_function.blocks {
        for (node_index, node) in block.nodes.iter_mut().enumerate() {
            for substitution in &substitutions {
                rewrite_scalar_value_uses(&mut node.operation, substitution.from, substitution.to);
            }
            let node_index = u32::try_from(node_index)
                .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
            node.definitions = expected_definitions(&node.operation, block.id, node_index);
            node.uses = expected_uses(&node.operation, block.id, node_index);
            node.successors = preserve_edge_custody(node);
            node.ownership = expected_ownership(&node.operation);
        }
    }
    let mut effect = 0u64;
    for block in &mut output_function.blocks {
        for node in &mut block.nodes {
            node.effect = omega_optimization_unit::EffectLink {
                input: effect,
                output: effect
                    .checked_add(1)
                    .expect("validated function effect count fits u64"),
            };
            effect = effect
                .checked_add(1)
                .expect("validated function effect count fits u64");
        }
    }
    output_function.facts = reconstruct_fact_index(output_function);
    output_function.declared_places = reconstruct_declared_places(output_function)?;
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
    let output_function = output
        .functions
        .iter()
        .find(|function| function.machine == patch.predecessor.machine)
        .expect("output function exists");
    for input_block in &function.blocks {
        if !expected_blocks.contains(&input_block.id)
            && output_function
                .blocks
                .iter()
                .find(|block| block.id == input_block.id)
                != Some(input_block)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.adjacent-single-predecessor-block-merge.v5",
        ),
        provenance: accepted_provenance,
    })
}

/// Independently replay one non-adjacent unique-predecessor block merge. The
/// validator treats source-roster order as serialization only: it reconstructs
/// execution dominance, every global parameter substitution, all moved value
/// definitions, and every dense-effect relocation before total validation.
pub fn validate_non_adjacent_block_merge_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::Dominators)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::OwnershipFrontiers)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || candidate.safety_class() != OptimizationSafetyClass::StructuralIdentity
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::MergeNonAdjacentBlock(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.predecessor) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.predecessor.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let predecessor_position = function
        .blocks
        .iter()
        .position(|block| block.id == patch.predecessor.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let target_position = function
        .blocks
        .iter()
        .position(|block| block.id == patch.target)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if function.entry == patch.target
        || target_position == predecessor_position.saturating_add(1)
        || function.blocks[target_position].nodes.is_empty()
        || matches!(function.blocks[target_position].nodes.as_slice(), [node] if matches!(node.operation, O::Jump { .. }))
    {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    let predecessor = &function.blocks[predecessor_position];
    let target = &function.blocks[target_position];
    let predecessor_index = usize::try_from(patch.predecessor.node)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    if predecessor_index + 1 != predecessor.nodes.len() {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let predecessor_node = &predecessor.nodes[predecessor_index];
    let O::Jump {
        psi_edge,
        target: jump_target,
        bindings,
        trivial_affine_discards,
    } = &predecessor_node.operation
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if !trivial_affine_discards.is_empty()
        || *psi_edge != patch.incoming_edge
        || *jump_target != patch.target
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let incoming = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .filter(|edge| edge.target == patch.target)
        .collect::<Vec<_>>();
    if incoming.len() != 1 || incoming[0].psi_edge != patch.incoming_edge {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    let dominators = independent_reachable_dominators(function);
    if !dominators
        .get(&patch.target)
        .is_some_and(|rows| rows.contains(&patch.predecessor.block))
    {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    if target.parameters.len() != bindings.len() {
        return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
    }
    let mut substitutions = target
        .parameters
        .iter()
        .zip(bindings)
        .map(|(parameter, binding)| {
            (binding.parameter == parameter.value
                && binding.scalar_type == parameter.scalar_type
                && independently_replacement_dominates_uses(
                    function,
                    &dominators,
                    binding.argument,
                    parameter.value,
                    parameter.scalar_type,
                ))
            .then_some(ScalarSubstitution {
                from: parameter.value,
                to: binding.argument,
                scalar_type: parameter.scalar_type,
            })
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)?;
    substitutions.sort();
    if candidate.substitutions() != substitutions {
        return Err(OptimizationUnitValidationError::CandidateSubstitutionMismatch);
    }
    if !reconstruct_adjacent_merge_ownership_is_identity(
        input,
        function,
        patch.incoming_edge,
        patch.target,
    ) {
        return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
    }
    let (expected_blocks, accepted_provenance) =
        reconstruct_non_adjacent_merge_accounting(function, patch, &substitutions)
            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.affected_blocks() != expected_blocks
        || candidate.provenance() != accepted_provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.predecessor.machine)
        .expect("candidate function exists");
    let output_target_position = output_function
        .blocks
        .iter()
        .position(|block| block.id == patch.target)
        .expect("candidate target exists");
    let mut moved = output_function.blocks.remove(output_target_position);
    let output_predecessor_position = output_function
        .blocks
        .iter()
        .position(|block| block.id == patch.predecessor.block)
        .expect("candidate predecessor exists");
    let removed = output_function.blocks[output_predecessor_position]
        .nodes
        .pop()
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let removed_edge = removed
        .successors
        .iter()
        .find(|edge| edge.psi_edge == patch.incoming_edge)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    let first = moved
        .nodes
        .first_mut()
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    if !first.provenance.is_empty() {
        first.provenance.extend_from_slice(&removed_edge.provenance);
        first.fuel.extend_from_slice(&removed_edge.fuel);
    } else if !first.successors.is_empty() {
        for successor in &mut first.successors {
            successor
                .provenance
                .extend_from_slice(&removed_edge.provenance);
            successor.fuel.extend_from_slice(&removed_edge.fuel);
        }
    } else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    output_function.blocks[output_predecessor_position]
        .nodes
        .append(&mut moved.nodes);
    for block in &mut output_function.blocks {
        for (node_index, node) in block.nodes.iter_mut().enumerate() {
            for substitution in &substitutions {
                rewrite_scalar_value_uses(&mut node.operation, substitution.from, substitution.to);
            }
            let node_index = u32::try_from(node_index)
                .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
            node.definitions = expected_definitions(&node.operation, block.id, node_index);
            node.uses = expected_uses(&node.operation, block.id, node_index);
            node.successors = preserve_edge_custody(node);
            node.ownership = expected_ownership(&node.operation);
        }
    }
    let mut effect = 0u64;
    for block in &mut output_function.blocks {
        for node in &mut block.nodes {
            node.effect = omega_optimization_unit::EffectLink {
                input: effect,
                output: effect
                    .checked_add(1)
                    .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?,
            };
            effect = effect
                .checked_add(1)
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
        }
    }
    output_function.facts = reconstruct_fact_index(output_function);
    output_function.declared_places = reconstruct_declared_places(output_function)?;
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
    let output_function = output
        .functions
        .iter()
        .find(|function| function.machine == patch.predecessor.machine)
        .expect("output function exists");
    for input_block in &function.blocks {
        if input_block.id != patch.target
            && !expected_blocks.contains(&input_block.id)
            && output_function
                .blocks
                .iter()
                .find(|block| block.id == input_block.id)
                != Some(input_block)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.non-adjacent-unique-predecessor-block-merge.v1",
        ),
        provenance: accepted_provenance,
    })
}

/// Independently replay one selected incoming path into a shared terminal.
/// The target remains intact; only the chosen jump is replaced by a typed
/// terminal clone, with exact fanout and fused incoming-edge custody.
pub fn validate_shared_jump_fusion_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::OwnershipFrontiers)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || candidate.safety_class() != OptimizationSafetyClass::StructuralIdentity
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::FuseSharedTerminalJump(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.predecessor) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.predecessor.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let predecessor = function
        .blocks
        .iter()
        .find(|block| block.id == patch.predecessor.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let predecessor_index = usize::try_from(patch.predecessor.node)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    if predecessor_index + 1 != predecessor.nodes.len() {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let predecessor_node = predecessor
        .nodes
        .get(predecessor_index)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let O::Jump {
        psi_edge,
        target: jump_target,
        bindings,
        trivial_affine_discards,
    } = &predecessor_node.operation
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if !trivial_affine_discards.is_empty()
        || *psi_edge != patch.incoming_edge
        || *jump_target != patch.target
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let target = function
        .blocks
        .iter()
        .find(|block| block.id == patch.target)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let [terminal] = target.nodes.as_slice() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if target.id == function.entry
        || predecessor.id == target.id
        || !terminal.successors.is_empty()
        || !matches!(terminal.provenance.first(), Some(PsiProvenance::Edge(_)))
        || !matches!(
            terminal.operation,
            O::Return { .. } | O::ReturnUnit { .. } | O::ReturnStructural { .. } | O::Crash { .. }
        )
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let incoming = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .filter(|edge| edge.target == patch.target)
        .collect::<Vec<_>>();
    if incoming.len() < 2
        || incoming
            .iter()
            .filter(|edge| edge.psi_edge == patch.incoming_edge)
            .count()
            != 1
    {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    if target.parameters.len() != bindings.len() {
        return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
    }
    let mut substitutions = target
        .parameters
        .iter()
        .zip(bindings)
        .map(|(parameter, binding)| {
            (binding.parameter == parameter.value && binding.scalar_type == parameter.scalar_type)
                .then_some(ScalarSubstitution {
                    from: parameter.value,
                    to: binding.argument,
                    scalar_type: parameter.scalar_type,
                })
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)?;
    substitutions.sort();
    if candidate.substitutions() != substitutions {
        return Err(OptimizationUnitValidationError::CandidateSubstitutionMismatch);
    }
    if !reconstruct_adjacent_merge_ownership_is_identity(
        input,
        function,
        patch.incoming_edge,
        patch.target,
    ) {
        return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
    }
    let (expected_blocks, accepted_provenance) =
        reconstruct_shared_terminal_fusion_accounting(function, patch)
            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.affected_blocks() != expected_blocks
        || candidate.provenance() != accepted_provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let incoming_edge = predecessor_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == patch.incoming_edge)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?
        .clone();
    let removed_effect = predecessor_node.effect;
    let mut clone = terminal.clone();
    rewrite_scalar_substitutions(
        &mut clone.operation,
        &substitutions,
        patch.predecessor.machine,
        patch.target,
    );
    clone
        .provenance
        .extend_from_slice(&incoming_edge.provenance);
    clone.fuel.extend_from_slice(&incoming_edge.fuel);
    clone.effect = removed_effect;
    clone.definitions = expected_definitions(
        &clone.operation,
        patch.predecessor.block,
        patch.predecessor.node,
    );
    clone.uses = expected_uses(
        &clone.operation,
        patch.predecessor.block,
        patch.predecessor.node,
    );
    clone.successors = expected_edges(&clone.operation);
    clone.ownership = expected_ownership(&clone.operation);

    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.predecessor.machine)
        .expect("candidate function exists");
    let output_predecessor = output_function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.predecessor.block)
        .expect("candidate predecessor exists");
    output_predecessor.nodes[predecessor_index] = clone;
    output_function.facts = reconstruct_fact_index(output_function);
    output_function.declared_places = reconstruct_declared_places(output_function)?;
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
    let output_function = output
        .functions
        .iter()
        .find(|function| function.machine == patch.predecessor.machine)
        .expect("output function exists");
    for input_block in &function.blocks {
        if !expected_blocks.contains(&input_block.id)
            && output_function
                .blocks
                .iter()
                .find(|block| block.id == input_block.id)
                != Some(input_block)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.shared-terminal-jump-fusion.v1",
        ),
        provenance: accepted_provenance,
    })
}

/// Independently remove one unused, unconditionally total scalar operation.
/// Execution custody remains realized at the immediately following,
/// necessarily co-executed node; it is never represented as unreachable work.
pub fn validate_dead_scalar_node_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    let proof_rule = OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.dead-unused-proof-certified-scalar-elimination.v1",
    );
    let expected_safety = if candidate.rule() == proof_rule {
        OptimizationSafetyClass::ProofCertified
    } else {
        OptimizationSafetyClass::ExactOperationSemantics
    };
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::ValueLiveness)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || candidate.safety_class() != expected_safety
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::RemoveDeadScalarNode(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.location)
        || !candidate.substitutions().is_empty()
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.location.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == patch.location.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let node_index = usize::try_from(patch.location.node)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    let node = block
        .nodes
        .get(node_index)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let (source_operation, result, scalar_type, obligation) =
        independently_validated_dead_scalar_shape(candidate.rule(), &node.operation)
            .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    if source_operation != patch.source_operation
        || result != patch.result
        || scalar_type != patch.scalar_type
        || node.definitions
            != [ValueDefinition {
                value: result,
                scalar_type,
                site: ValueDefinitionSite::Node {
                    block: block.id,
                    node: patch.location.node,
                },
            }]
        || !node.successors.is_empty()
        || !node.ownership.is_empty()
        || block.nodes.get(node_index + 1).is_none()
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let live = reconstruct_closed_scalar_node_boundary(input, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if live.live_out.contains(&result)
        || function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .flat_map(|node| &node.uses)
            .any(|use_site| use_site.value == result)
    {
        return Err(OptimizationUnitValidationError::CandidateLiveBoundaryMismatch);
    }
    match (obligation, candidate.accepted_obligation_witness()) {
        (Some(obligation), Some(identity)) => {
            if !function.facts.iter().any(|fact| {
                matches!(
                    fact,
                    OptimizationFact::OperationObligationReference {
                        obligation: reference,
                        support,
                    } if *support == source_operation && *reference == obligation
                )
            }) || !input.accepted_obligation_facts.iter().any(|fact| {
                fact.identity == identity
                    && fact.machine == function.machine
                    && fact.operation == source_operation
                    && fact.obligation == obligation
            }) {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
        }
        (None, None) => {
            if function.facts.iter().any(|fact| {
                matches!(
                    fact,
                    OptimizationFact::OperationObligationReference { support, .. }
                        if *support == source_operation
                )
            }) {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
        }
        _ => {
            return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
        }
    }
    let receiver = &block.nodes[node_index + 1];
    if receiver
        .provenance
        .iter()
        .any(|source| node.provenance.contains(source))
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }
    let (expected_blocks, accepted_provenance) =
        reconstruct_dead_scalar_node_accounting(function, patch)
            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.affected_blocks() != expected_blocks
        || candidate.provenance() != accepted_provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.location.machine)
        .expect("candidate function exists");
    let output_block = output_function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.location.block)
        .expect("candidate block exists");
    let removed = output_block.nodes.remove(node_index);
    let receiver = output_block
        .nodes
        .get_mut(node_index)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    receiver.provenance.extend_from_slice(&removed.provenance);
    receiver.fuel.extend_from_slice(&removed.fuel);
    let mut effect = 0u64;
    for block in &mut output_function.blocks {
        for (node_index, node) in block.nodes.iter_mut().enumerate() {
            let node_index = u32::try_from(node_index)
                .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
            node.definitions = expected_definitions(&node.operation, block.id, node_index);
            node.uses = expected_uses(&node.operation, block.id, node_index);
            node.successors = preserve_edge_custody(node);
            node.ownership = expected_ownership(&node.operation);
            node.effect = omega_optimization_unit::EffectLink {
                input: effect,
                output: effect
                    .checked_add(1)
                    .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?,
            };
            effect = effect
                .checked_add(1)
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
        }
    }
    output_function.facts = reconstruct_fact_index(output_function);
    output_function.declared_places = reconstruct_declared_places(output_function)?;
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
    let output_function = output
        .functions
        .iter()
        .find(|function| function.machine == patch.location.machine)
        .expect("output function exists");
    for input_block in &function.blocks {
        if !expected_blocks.contains(&input_block.id)
            && output_function
                .blocks
                .iter()
                .find(|block| block.id == input_block.id)
                != Some(input_block)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(if obligation.is_some() {
            b"omega.validator.dead-unused-proof-certified-scalar-node.v1"
        } else {
            b"omega.validator.dead-unused-total-scalar-node.v1"
        }),
        provenance: accepted_provenance,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum IndependentTotalScalarExpressionKey {
    BooleanConstant(bool),
    IntegerConstant(ScalarType, psi_core::IntegerValue),
    BooleanNot(ValueId),
    BooleanEqual(ValueId, ValueId),
    IntegerEqual(IntegerType, ValueId, ValueId),
    IntegerLessThan(IntegerType, ValueId, ValueId),
    IntegerLessOrEqual(IntegerType, ValueId, ValueId),
    IntegerBitwiseNot(IntegerType, ValueId),
    IntegerWiden(IntegerType, IntegerType, ValueId),
    IntegerBitwiseAnd(IntegerType, ValueId, ValueId),
    IntegerBitwiseOr(IntegerType, ValueId, ValueId),
    IntegerBitwiseXor(IntegerType, ValueId, ValueId),
    WrappingShiftLeft(IntegerType, IntegerType, ValueId, ValueId),
    WrappingShiftRight(IntegerType, IntegerType, ValueId, ValueId),
    WrappingAdd(IntegerType, ValueId, ValueId),
    WrappingSubtract(IntegerType, ValueId, ValueId),
    WrappingMultiply(IntegerType, ValueId, ValueId),
    SaturatingAdd(IntegerType, ValueId, ValueId),
    SaturatingSubtract(IntegerType, ValueId, ValueId),
    SaturatingMultiply(IntegerType, ValueId, ValueId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum IndependentProofScalarExpressionKey {
    ExactCast(IntegerType, IntegerType, ValueId),
    ExactShiftLeft(IntegerType, IntegerType, ValueId, ValueId),
    ExactShiftRight(IntegerType, IntegerType, ValueId, ValueId),
    ExactAdd(IntegerType, ValueId, ValueId),
    ExactSubtract(IntegerType, ValueId, ValueId),
    ExactMultiply(IntegerType, ValueId, ValueId),
    ExactDivide(IntegerType, ValueId, ValueId),
    ExactRemainder(IntegerType, ValueId, ValueId),
    WrappingDivide(IntegerType, ValueId, ValueId),
    WrappingRemainder(IntegerType, ValueId, ValueId),
    SaturatingDivide(IntegerType, ValueId, ValueId),
    SaturatingRemainder(IntegerType, ValueId, ValueId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum IndependentCompatiblePolicyScalarExpressionKey {
    ShiftLeft(IntegerType, IntegerType, ValueId, ValueId),
    ShiftRight(IntegerType, IntegerType, ValueId, ValueId),
    Add(IntegerType, ValueId, ValueId),
    Subtract(IntegerType, ValueId, ValueId),
    Multiply(IntegerType, ValueId, ValueId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum IndependentScalarExpressionKey {
    ObligationFree(IndependentTotalScalarExpressionKey),
    ProofCertified(IndependentProofScalarExpressionKey),
    CompatiblePolicy(IndependentCompatiblePolicyScalarExpressionKey),
}

fn independent_pair(left: ValueId, right: ValueId) -> (ValueId, ValueId) {
    if left <= right {
        (left, right)
    } else {
        (right, left)
    }
}

fn independent_total_scalar_expression(
    operation: &O,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Option<(
    IndependentTotalScalarExpressionKey,
    OperationId,
    ValueId,
    ScalarType,
)> {
    let operand_integer = |value: ValueId| match value_types.get(&value) {
        Some(ScalarType::Integer(row)) => Some(*row),
        _ => None,
    };
    Some(match operation {
        O::BooleanConstant {
            psi_operation,
            result,
            value,
        } => (
            IndependentTotalScalarExpressionKey::BooleanConstant(*value),
            *psi_operation,
            *result,
            ScalarType::Boolean,
        ),
        O::IntegerConstant {
            psi_operation,
            result,
            scalar_type,
            value,
        } => (
            IndependentTotalScalarExpressionKey::IntegerConstant(*scalar_type, *value),
            *psi_operation,
            *result,
            *scalar_type,
        ),
        O::BooleanNot {
            psi_operation,
            result,
            operand,
        } => (
            IndependentTotalScalarExpressionKey::BooleanNot(*operand),
            *psi_operation,
            *result,
            ScalarType::Boolean,
        ),
        O::BooleanEqual {
            psi_operation,
            result,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentTotalScalarExpressionKey::BooleanEqual(left, right),
                *psi_operation,
                *result,
                ScalarType::Boolean,
            )
        }
        O::IntegerEqual {
            psi_operation,
            result,
            left,
            right,
        } => {
            let scalar_type = operand_integer(*left)?;
            if operand_integer(*right)? != scalar_type {
                return None;
            }
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentTotalScalarExpressionKey::IntegerEqual(scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Boolean,
            )
        }
        O::IntegerLessThan {
            psi_operation,
            result,
            left,
            right,
        } => {
            let scalar_type = operand_integer(*left)?;
            if operand_integer(*right)? != scalar_type {
                return None;
            }
            (
                IndependentTotalScalarExpressionKey::IntegerLessThan(scalar_type, *left, *right),
                *psi_operation,
                *result,
                ScalarType::Boolean,
            )
        }
        O::IntegerLessOrEqual {
            psi_operation,
            result,
            left,
            right,
        } => {
            let scalar_type = operand_integer(*left)?;
            if operand_integer(*right)? != scalar_type {
                return None;
            }
            (
                IndependentTotalScalarExpressionKey::IntegerLessOrEqual(scalar_type, *left, *right),
                *psi_operation,
                *result,
                ScalarType::Boolean,
            )
        }
        O::IntegerBitwiseNot {
            psi_operation,
            result,
            scalar_type,
            operand,
        } => (
            IndependentTotalScalarExpressionKey::IntegerBitwiseNot(*scalar_type, *operand),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
        ),
        O::IntegerWiden {
            psi_operation,
            result,
            source_type,
            target_type,
            operand,
        } => (
            IndependentTotalScalarExpressionKey::IntegerWiden(*source_type, *target_type, *operand),
            *psi_operation,
            *result,
            ScalarType::Integer(*target_type),
        ),
        O::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentTotalScalarExpressionKey::IntegerBitwiseAnd(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        O::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentTotalScalarExpressionKey::IntegerBitwiseOr(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        O::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentTotalScalarExpressionKey::IntegerBitwiseXor(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        O::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            IndependentTotalScalarExpressionKey::WrappingShiftLeft(
                *value_type,
                *count_type,
                *value,
                *count,
            ),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
        ),
        O::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            IndependentTotalScalarExpressionKey::WrappingShiftRight(
                *value_type,
                *count_type,
                *value,
                *count,
            ),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
        ),
        O::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentTotalScalarExpressionKey::WrappingAdd(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        O::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IndependentTotalScalarExpressionKey::WrappingSubtract(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
        ),
        O::WrappingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentTotalScalarExpressionKey::WrappingMultiply(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        O::SaturatingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentTotalScalarExpressionKey::SaturatingAdd(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        O::SaturatingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IndependentTotalScalarExpressionKey::SaturatingSubtract(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
        ),
        O::SaturatingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentTotalScalarExpressionKey::SaturatingMultiply(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        _ => return None,
    })
}

fn independent_proof_scalar_expression(
    operation: &O,
) -> Option<(
    IndependentProofScalarExpressionKey,
    OperationId,
    ValueId,
    ScalarType,
    psi_core::ObligationId,
)> {
    Some(match operation {
        O::IntegerExactCast {
            psi_operation,
            obligation,
            result,
            source_type,
            target_type,
            operand,
        } => (
            IndependentProofScalarExpressionKey::ExactCast(*source_type, *target_type, *operand),
            *psi_operation,
            *result,
            ScalarType::Integer(*target_type),
            *obligation,
        ),
        O::ExactIntegerShiftLeft {
            psi_operation,
            obligation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            IndependentProofScalarExpressionKey::ExactShiftLeft(
                *value_type,
                *count_type,
                *value,
                *count,
            ),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
            *obligation,
        ),
        O::ExactIntegerShiftRight {
            psi_operation,
            obligation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            IndependentProofScalarExpressionKey::ExactShiftRight(
                *value_type,
                *count_type,
                *value,
                *count,
            ),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
            *obligation,
        ),
        O::ExactIntegerAdd {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentProofScalarExpressionKey::ExactAdd(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
                *obligation,
            )
        }
        O::ExactIntegerSubtract {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IndependentProofScalarExpressionKey::ExactSubtract(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
            *obligation,
        ),
        O::ExactIntegerMultiply {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentProofScalarExpressionKey::ExactMultiply(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
                *obligation,
            )
        }
        O::ExactIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IndependentProofScalarExpressionKey::ExactDivide(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
            *obligation,
        ),
        O::ExactIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IndependentProofScalarExpressionKey::ExactRemainder(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
            *obligation,
        ),
        O::WrappingIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IndependentProofScalarExpressionKey::WrappingDivide(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
            *obligation,
        ),
        O::WrappingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IndependentProofScalarExpressionKey::WrappingRemainder(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
            *obligation,
        ),
        O::SaturatingIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IndependentProofScalarExpressionKey::SaturatingDivide(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
            *obligation,
        ),
        O::SaturatingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IndependentProofScalarExpressionKey::SaturatingRemainder(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
            *obligation,
        ),
        _ => return None,
    })
}

fn independent_compatible_policy_scalar_leader(
    operation: &O,
) -> Option<(
    IndependentScalarExpressionKey,
    OperationId,
    ValueId,
    ScalarType,
    Option<psi_core::ObligationId>,
)> {
    let row = match operation {
        O::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            IndependentCompatiblePolicyScalarExpressionKey::ShiftLeft(
                *value_type,
                *count_type,
                *value,
                *count,
            ),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
        ),
        O::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            IndependentCompatiblePolicyScalarExpressionKey::ShiftRight(
                *value_type,
                *count_type,
                *value,
                *count,
            ),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
        ),
        O::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | O::SaturatingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentCompatiblePolicyScalarExpressionKey::Add(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        O::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | O::SaturatingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IndependentCompatiblePolicyScalarExpressionKey::Subtract(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
        ),
        O::WrappingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | O::SaturatingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentCompatiblePolicyScalarExpressionKey::Multiply(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        _ => return None,
    };
    Some((
        IndependentScalarExpressionKey::CompatiblePolicy(row.0),
        row.1,
        row.2,
        row.3,
        None,
    ))
}

fn independent_compatible_policy_scalar_redundant(
    operation: &O,
) -> Option<(
    IndependentScalarExpressionKey,
    OperationId,
    ValueId,
    ScalarType,
    Option<psi_core::ObligationId>,
)> {
    let row = match operation {
        O::ExactIntegerShiftLeft {
            psi_operation,
            obligation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            IndependentCompatiblePolicyScalarExpressionKey::ShiftLeft(
                *value_type,
                *count_type,
                *value,
                *count,
            ),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
            *obligation,
        ),
        O::ExactIntegerShiftRight {
            psi_operation,
            obligation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            IndependentCompatiblePolicyScalarExpressionKey::ShiftRight(
                *value_type,
                *count_type,
                *value,
                *count,
            ),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
            *obligation,
        ),
        O::ExactIntegerAdd {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentCompatiblePolicyScalarExpressionKey::Add(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
                *obligation,
            )
        }
        O::ExactIntegerSubtract {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IndependentCompatiblePolicyScalarExpressionKey::Subtract(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
            *obligation,
        ),
        O::ExactIntegerMultiply {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentCompatiblePolicyScalarExpressionKey::Multiply(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
                *obligation,
            )
        }
        _ => return None,
    };
    Some((
        IndependentScalarExpressionKey::CompatiblePolicy(row.0),
        row.1,
        row.2,
        row.3,
        Some(row.4),
    ))
}

fn independent_cse_expression(
    operation: &O,
    value_types: &BTreeMap<ValueId, ScalarType>,
    proof_class: ScalarCseProofClass,
) -> Option<(
    IndependentScalarExpressionKey,
    OperationId,
    ValueId,
    ScalarType,
    Option<psi_core::ObligationId>,
)> {
    match proof_class {
        ScalarCseProofClass::ObligationFree => {
            let (key, operation, result, scalar_type) =
                independent_total_scalar_expression(operation, value_types)?;
            Some((
                IndependentScalarExpressionKey::ObligationFree(key),
                operation,
                result,
                scalar_type,
                None,
            ))
        }
        ScalarCseProofClass::ProofCertified => {
            let (key, operation, result, scalar_type, obligation) =
                independent_proof_scalar_expression(operation)?;
            Some((
                IndependentScalarExpressionKey::ProofCertified(key),
                operation,
                result,
                scalar_type,
                Some(obligation),
            ))
        }
        ScalarCseProofClass::CompatiblePolicy => None,
    }
}

fn independently_accepted_operation_fact(
    input: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    operation: OperationId,
    obligation: psi_core::ObligationId,
) -> Option<omega_optimization_core::AcceptedObligationFactIdentity> {
    function
        .facts
        .iter()
        .any(|fact| {
            matches!(
                fact,
                OptimizationFact::OperationObligationReference {
                    obligation: reference,
                    support,
                } if *support == operation && *reference == obligation
            )
        })
        .then(|| {
            input
                .accepted_obligation_facts
                .iter()
                .find(|fact| {
                    fact.machine == function.machine
                        && fact.operation == operation
                        && fact.obligation == obligation
                })
                .map(|fact| fact.identity)
        })
        .flatten()
}

/// Independently validate and apply one same-block common-subexpression elimination.
pub fn validate_local_scalar_common_subexpression_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_scalar_common_subexpression_candidate(input, candidate, ScalarCseScope::SameBlock)
}

/// Independently validate and apply one cross-block dominating
/// common-subexpression elimination.
pub fn validate_dominating_scalar_common_subexpression_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_scalar_common_subexpression_candidate(input, candidate, ScalarCseScope::Dominating)
}

/// Independently validate one obligation-free, proof-certified, or
/// proof-certified compatible-policy scalar
/// expression translated through every incoming binding of an acyclic join.
/// The redundant result identity becomes a new join parameter; every incoming
/// edge supplies the canonical available leader for its translated expression.
pub fn validate_phi_translated_scalar_common_subexpression_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    let proof_class = if candidate.rule()
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.phi-translated-obligation-free-total-scalar-gvn.v1",
        ) {
        ScalarCseProofClass::ObligationFree
    } else if candidate.rule()
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.phi-translated-proof-certified-total-scalar-gvn.v1",
        )
    {
        ScalarCseProofClass::ProofCertified
    } else if candidate.rule()
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.phi-translated-proof-certified-compatible-policy-scalar-gvn.v1",
        )
    {
        ScalarCseProofClass::CompatiblePolicy
    } else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    let expected_safety = match proof_class {
        ScalarCseProofClass::ObligationFree => OptimizationSafetyClass::ExactOperationSemantics,
        ScalarCseProofClass::ProofCertified | ScalarCseProofClass::CompatiblePolicy => {
            OptimizationSafetyClass::ProofCertified
        }
    };
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::Dominators)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || candidate.safety_class() != expected_safety
        || !candidate.substitutions().is_empty()
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(patch) = candidate.patch()
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.redundant) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|row| row.machine == patch.redundant.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let join = function
        .blocks
        .iter()
        .find(|row| row.id == patch.redundant.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let redundant_index = usize::try_from(patch.redundant.node)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    let redundant = join
        .nodes
        .get(redundant_index)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if function.entry == join.id
        || join.nodes.get(redundant_index + 1).is_none()
        || usize::try_from(patch.parameter_position).ok() != Some(join.parameters.len())
        || join
            .parameters
            .iter()
            .any(|row| row.value == patch.redundant_result)
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let value_types = function
        .parameters
        .iter()
        .map(|row| (row.value, row.scalar_type))
        .chain(function.blocks.iter().flat_map(|block| {
            block
                .parameters
                .iter()
                .map(|row| (row.value, row.scalar_type))
        }))
        .chain(function.blocks.iter().flat_map(|block| {
            block.nodes.iter().flat_map(|node| {
                node.definitions
                    .iter()
                    .map(|row| (row.value, row.scalar_type))
            })
        }))
        .collect::<BTreeMap<_, _>>();
    let (_, redundant_operation, redundant_result, redundant_type, redundant_obligation) =
        match proof_class {
            ScalarCseProofClass::CompatiblePolicy => {
                independent_compatible_policy_scalar_redundant(&redundant.operation)
            }
            _ => independent_cse_expression(&redundant.operation, &value_types, proof_class),
        }
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    if redundant_operation != patch.redundant_operation
        || redundant_result != patch.redundant_result
        || redundant_type != patch.scalar_type
        || redundant.definitions
            != [ValueDefinition {
                value: redundant_result,
                scalar_type: redundant_type,
                site: ValueDefinitionSite::Node {
                    block: join.id,
                    node: patch.redundant.node,
                },
            }]
        || !redundant.successors.is_empty()
        || !redundant.ownership.is_empty()
        || !function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .flat_map(|node| &node.uses)
            .any(|use_site| use_site.value == redundant_result)
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let _redundant_fact = match (proof_class, redundant_obligation) {
        (ScalarCseProofClass::ObligationFree, None) => {
            if candidate.accepted_obligation_witness().is_some()
                || function.facts.iter().any(|fact| {
                    matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                        if *support == redundant_operation)
                })
            {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
            None
        }
        (
            ScalarCseProofClass::ProofCertified | ScalarCseProofClass::CompatiblePolicy,
            Some(obligation),
        ) => {
            let fact = independently_accepted_operation_fact(
                input,
                function,
                redundant_operation,
                obligation,
            )
            .ok_or(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)?;
            if candidate.accepted_obligation_witness() != Some(fact) {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
            Some(fact)
        }
        _ => {
            return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
        }
    };

    let dominators = independent_reachable_dominators(function);
    let mut expected_incoming = Vec::new();
    for source in &function.blocks {
        for edge in source
            .nodes
            .iter()
            .flat_map(|node| &node.successors)
            .filter(|edge| edge.target == join.id)
        {
            if edge.bindings.len() != join.parameters.len() {
                return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
            }
            let mut translated = redundant.operation.clone();
            for (parameter, binding) in join.parameters.iter().zip(&edge.bindings) {
                if binding.parameter != parameter.value
                    || binding.scalar_type != parameter.scalar_type
                    || value_types.get(&binding.argument) != Some(&binding.scalar_type)
                {
                    return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
                }
                rewrite_scalar_value_uses(&mut translated, parameter.value, binding.argument);
            }
            let (translated_key, _, _, translated_type, _) = match proof_class {
                ScalarCseProofClass::CompatiblePolicy => {
                    independent_compatible_policy_scalar_redundant(&translated)
                }
                _ => independent_cse_expression(&translated, &value_types, proof_class),
            }
            .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
            let mut available_leaders = Vec::new();
            let mut missing_leader_evidence = false;
            for leader_block in &function.blocks {
                for (node_index, node) in leader_block.nodes.iter().enumerate() {
                    let available = if leader_block.id == source.id {
                        node_index + 1 < source.nodes.len()
                    } else {
                        dominators
                            .get(&source.id)
                            .is_some_and(|rows| rows.contains(&leader_block.id))
                    };
                    if !available {
                        continue;
                    }
                    let Some((key, operation, result, scalar_type, obligation)) = (match proof_class
                    {
                        ScalarCseProofClass::CompatiblePolicy => {
                            independent_compatible_policy_scalar_leader(&node.operation)
                        }
                        _ => independent_cse_expression(&node.operation, &value_types, proof_class),
                    }) else {
                        continue;
                    };
                    let admitted = match (proof_class, obligation) {
                        (ScalarCseProofClass::ObligationFree, None) => !function
                            .facts
                            .iter()
                            .any(|fact| matches!(fact, OptimizationFact::OperationObligationReference { support, .. } if *support == operation)),
                        (ScalarCseProofClass::ProofCertified, Some(obligation)) => {
                            independently_accepted_operation_fact(
                                input,
                                function,
                                operation,
                                obligation,
                            )
                            .is_some()
                        }
                        (ScalarCseProofClass::CompatiblePolicy, None) => !function
                            .facts
                            .iter()
                            .any(|fact| matches!(fact, OptimizationFact::OperationObligationReference { support, .. } if *support == operation)),
                        _ => false,
                    };
                    if !admitted
                        && ((proof_class == ScalarCseProofClass::ProofCertified
                            && obligation.is_some())
                            || proof_class == ScalarCseProofClass::CompatiblePolicy)
                        && key == translated_key
                        && scalar_type == translated_type
                    {
                        missing_leader_evidence = true;
                    }
                    if admitted && key == translated_key && scalar_type == translated_type {
                        available_leaders.push((
                            NodeLocation {
                                machine: function.machine,
                                block: leader_block.id,
                                node: u32::try_from(node_index).map_err(|_| {
                                    OptimizationUnitValidationError::CandidateLocationMissing
                                })?,
                            },
                            operation,
                            result,
                            obligation,
                        ));
                    }
                }
            }
            let canonical = available_leaders
                .into_iter()
                .min_by_key(|(location, _, _, _)| {
                    (
                        dominators
                            .get(&location.block)
                            .map_or(usize::MAX, BTreeSet::len),
                        *location,
                    )
                })
                .ok_or(if missing_leader_evidence {
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch
                } else {
                    OptimizationUnitValidationError::CandidatePatchMismatch
                })?;
            expected_incoming.push(PhiTranslatedScalarIncoming {
                source: source.id,
                edge: edge.psi_edge,
                leader: canonical.0,
                leader_operation: canonical.1,
                leader_result: canonical.2,
            });
        }
    }
    expected_incoming.sort_by_key(|row| (row.edge, row.source));
    if expected_incoming.len() < 2 || patch.incoming != expected_incoming {
        return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
    }
    let (expected_blocks, accepted_provenance) =
        reconstruct_phi_translated_cse_accounting(function, &patch)
            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.affected_blocks() != expected_blocks
        || candidate.provenance() != accepted_provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|row| row.machine == patch.redundant.machine)
        .expect("candidate function exists");
    let output_join = output_function
        .blocks
        .iter_mut()
        .find(|row| row.id == patch.redundant.block)
        .expect("candidate join exists");
    output_join.parameters.push(ValueDefinition {
        value: patch.redundant_result,
        scalar_type: patch.scalar_type,
        site: ValueDefinitionSite::BlockParameter {
            block: patch.redundant.block,
            position: patch.parameter_position,
        },
    });
    let removed = output_join.nodes.remove(redundant_index);
    let receiver = output_join
        .nodes
        .get_mut(redundant_index)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    receiver.provenance.extend_from_slice(&removed.provenance);
    receiver.fuel.extend_from_slice(&removed.fuel);
    for incoming in &patch.incoming {
        let source = output_function
            .blocks
            .iter_mut()
            .find(|row| row.id == incoming.source)
            .expect("incoming source exists");
        let node = source
            .nodes
            .iter_mut()
            .find(|node| {
                node.successors
                    .iter()
                    .any(|edge| edge.psi_edge == incoming.edge)
            })
            .expect("incoming edge exists");
        let edge = node
            .successors
            .iter()
            .find(|edge| edge.psi_edge == incoming.edge)
            .expect("incoming edge exists");
        let mut bindings = edge.bindings.clone();
        bindings.push(omega_abstract_operations::ValueBinding {
            parameter: patch.redundant_result,
            argument: incoming.leader_result,
            scalar_type: patch.scalar_type,
        });
        if !rewrite_successor_operation(
            &mut node.operation,
            incoming.edge,
            patch.redundant.block,
            &bindings,
        ) {
            return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
        }
    }
    let mut effect = 0u64;
    for block in &mut output_function.blocks {
        for (node_index, node) in block.nodes.iter_mut().enumerate() {
            let node_index = u32::try_from(node_index)
                .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
            node.definitions = expected_definitions(&node.operation, block.id, node_index);
            node.uses = expected_uses(&node.operation, block.id, node_index);
            node.successors = preserve_edge_custody(node);
            node.ownership = expected_ownership(&node.operation);
            node.effect = omega_optimization_unit::EffectLink {
                input: effect,
                output: effect
                    .checked_add(1)
                    .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?,
            };
            effect = effect
                .checked_add(1)
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
        }
    }
    output_function.facts = reconstruct_fact_index(output_function);
    output_function.declared_places = reconstruct_declared_places(output_function)?;
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
    let output_function = output
        .functions
        .iter()
        .find(|row| row.machine == function.machine)
        .expect("output function exists");
    for input_block in &function.blocks {
        if !expected_blocks.contains(&input_block.id)
            && output_function
                .blocks
                .iter()
                .find(|row| row.id == input_block.id)
                != Some(input_block)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(match proof_class {
            ScalarCseProofClass::ObligationFree => {
                b"omega.validator.phi-translated-obligation-free-total-scalar-gvn.v1"
            }
            ScalarCseProofClass::ProofCertified => {
                b"omega.validator.phi-translated-proof-certified-total-scalar-gvn.v1"
            }
            ScalarCseProofClass::CompatiblePolicy => {
                b"omega.validator.phi-translated-proof-certified-compatible-policy-scalar-gvn.v1"
            }
        }),
        provenance: accepted_provenance,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScalarCseScope {
    SameBlock,
    Dominating,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScalarCseProofClass {
    ObligationFree,
    ProofCertified,
    CompatiblePolicy,
}

fn validate_scalar_common_subexpression_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
    scope: ScalarCseScope,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    let proof_class = match (scope, candidate.rule()) {
        (ScalarCseScope::SameBlock, rule)
            if rule
                == OptimizationRuleIdentity::from_canonical_bytes(
                    b"omega.psi-rule.same-block-obligation-free-total-scalar-cse.v1",
                ) =>
        {
            ScalarCseProofClass::ObligationFree
        }
        (ScalarCseScope::Dominating, rule)
            if rule
                == OptimizationRuleIdentity::from_canonical_bytes(
                    b"omega.psi-rule.dominator-obligation-free-total-scalar-gvn.v1",
                ) =>
        {
            ScalarCseProofClass::ObligationFree
        }
        (ScalarCseScope::SameBlock, rule)
            if rule
                == OptimizationRuleIdentity::from_canonical_bytes(
                    b"omega.psi-rule.same-block-proof-certified-total-scalar-cse.v1",
                ) =>
        {
            ScalarCseProofClass::ProofCertified
        }
        (ScalarCseScope::Dominating, rule)
            if rule
                == OptimizationRuleIdentity::from_canonical_bytes(
                    b"omega.psi-rule.dominator-proof-certified-total-scalar-gvn.v1",
                ) =>
        {
            ScalarCseProofClass::ProofCertified
        }
        (ScalarCseScope::SameBlock, rule)
            if rule
                == OptimizationRuleIdentity::from_canonical_bytes(
                    b"omega.psi-rule.same-block-proof-certified-compatible-policy-scalar-cse.v1",
                ) =>
        {
            ScalarCseProofClass::CompatiblePolicy
        }
        (ScalarCseScope::Dominating, rule)
            if rule
                == OptimizationRuleIdentity::from_canonical_bytes(
                    b"omega.psi-rule.dominator-proof-certified-compatible-policy-scalar-gvn.v1",
                ) =>
        {
            ScalarCseProofClass::CompatiblePolicy
        }
        _ => return Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    };
    let expected_safety = match proof_class {
        ScalarCseProofClass::ObligationFree => OptimizationSafetyClass::ExactOperationSemantics,
        ScalarCseProofClass::ProofCertified | ScalarCseProofClass::CompatiblePolicy => {
            OptimizationSafetyClass::ProofCertified
        }
    };
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::UseDefinition)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || candidate.safety_class() != expected_safety
        || (scope == ScalarCseScope::Dominating
            && (!candidate
                .required_analyses()
                .contains(AnalysisKind::ControlFlowGraph)
                || !candidate
                    .required_analyses()
                    .contains(AnalysisKind::Dominators)))
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let patch = match (scope, candidate.patch()) {
        (
            ScalarCseScope::SameBlock,
            PsiRewritePatch::EliminateLocalScalarCommonSubexpression(patch),
        ) => patch,
        (
            ScalarCseScope::Dominating,
            PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(patch),
        ) => LocalScalarCommonSubexpressionRewrite {
            leader: patch.leader,
            redundant: patch.redundant,
            leader_operation: patch.leader_operation,
            redundant_operation: patch.redundant_operation,
            leader_result: patch.leader_result,
            redundant_result: patch.redundant_result,
            scalar_type: patch.scalar_type,
        },
        _ => return Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    };
    if candidate.node_decision_point() != Some(patch.redundant) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let expected_substitution = [ScalarSubstitution {
        from: patch.redundant_result,
        to: patch.leader_result,
        scalar_type: patch.scalar_type,
    }];
    if candidate.substitutions() != expected_substitution {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    if patch.leader.machine != patch.redundant.machine {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|row| row.machine == patch.leader.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let leader_block = function
        .blocks
        .iter()
        .find(|row| row.id == patch.leader.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let redundant_block = function
        .blocks
        .iter()
        .find(|row| row.id == patch.redundant.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let leader = leader_block
        .nodes
        .get(
            usize::try_from(patch.leader.node)
                .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?,
        )
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let redundant_index = usize::try_from(patch.redundant.node)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    let redundant = redundant_block
        .nodes
        .get(redundant_index)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if redundant_block.nodes.get(redundant_index + 1).is_none() {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    match scope {
        ScalarCseScope::SameBlock
            if patch.leader.block != patch.redundant.block
                || patch.leader.node >= patch.redundant.node =>
        {
            return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
        }
        ScalarCseScope::Dominating if patch.leader.block == patch.redundant.block => {
            return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
        }
        _ => {}
    }
    let value_types = function
        .parameters
        .iter()
        .map(|row| (row.value, row.scalar_type))
        .chain(function.blocks.iter().flat_map(|block| {
            block
                .parameters
                .iter()
                .map(|row| (row.value, row.scalar_type))
        }))
        .chain(function.blocks.iter().flat_map(|block| {
            block.nodes.iter().flat_map(|node| {
                node.definitions
                    .iter()
                    .map(|row| (row.value, row.scalar_type))
            })
        }))
        .collect::<BTreeMap<_, _>>();
    let admitted_expression = |operation: &O| {
        let row = match proof_class {
            ScalarCseProofClass::CompatiblePolicy => {
                independent_compatible_policy_scalar_leader(operation)?
            }
            _ => independent_cse_expression(operation, &value_types, proof_class)?,
        };
        match (proof_class, row.4) {
            (ScalarCseProofClass::ObligationFree, None) => Some(row),
            (ScalarCseProofClass::ProofCertified, Some(obligation))
                if independently_accepted_operation_fact(input, function, row.1, obligation)
                    .is_some() =>
            {
                Some(row)
            }
            (ScalarCseProofClass::CompatiblePolicy, None)
                if !function.facts.iter().any(|fact| {
                    matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                        if *support == row.1)
                }) =>
            {
                Some(row)
            }
            _ => None,
        }
    };
    let (leader_key, leader_operation, leader_result, leader_type, leader_obligation) =
        match proof_class {
            ScalarCseProofClass::CompatiblePolicy => {
                independent_compatible_policy_scalar_leader(&leader.operation)
            }
            _ => independent_cse_expression(&leader.operation, &value_types, proof_class),
        }
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    let (
        redundant_key,
        redundant_operation,
        redundant_result,
        redundant_type,
        redundant_obligation,
    ) = match proof_class {
        ScalarCseProofClass::CompatiblePolicy => {
            independent_compatible_policy_scalar_redundant(&redundant.operation)
        }
        _ => independent_cse_expression(&redundant.operation, &value_types, proof_class),
    }
    .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    if leader_key != redundant_key
        || leader_operation != patch.leader_operation
        || redundant_operation != patch.redundant_operation
        || leader_result != patch.leader_result
        || redundant_result != patch.redundant_result
        || leader_type != patch.scalar_type
        || redundant_type != patch.scalar_type
        || leader_result == redundant_result
        || leader_operation == redundant_operation
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let _proof_facts = match (proof_class, leader_obligation, redundant_obligation) {
        (ScalarCseProofClass::ObligationFree, None, None) => {
            if candidate.accepted_obligation_witness().is_some()
                || function.facts.iter().any(|fact| {
                    matches!(
                        fact,
                        OptimizationFact::OperationObligationReference { support, .. }
                            if *support == leader_operation || *support == redundant_operation
                    )
                })
            {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
            None
        }
        (ScalarCseProofClass::ProofCertified, Some(leader), Some(redundant)) => {
            let leader_fact =
                independently_accepted_operation_fact(input, function, leader_operation, leader)
                    .ok_or(
                        OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                    )?;
            let redundant_fact = independently_accepted_operation_fact(
                input,
                function,
                redundant_operation,
                redundant,
            )
            .ok_or(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)?;
            if candidate.accepted_obligation_witness() != Some(redundant_fact) {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
            Some((leader_fact, redundant_fact))
        }
        (ScalarCseProofClass::CompatiblePolicy, None, Some(redundant)) => {
            if function.facts.iter().any(|fact| {
                matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                    if *support == leader_operation)
            }) {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
            let redundant_fact = independently_accepted_operation_fact(
                input,
                function,
                redundant_operation,
                redundant,
            )
            .ok_or(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)?;
            if candidate.accepted_obligation_witness() != Some(redundant_fact) {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
            None
        }
        _ => {
            return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
        }
    };
    if scope == ScalarCseScope::SameBlock {
        let canonical_leader = leader_block
            .nodes
            .iter()
            .take(redundant_index)
            .enumerate()
            .filter_map(|(node, candidate)| {
                let (key, _, _, scalar_type, _) = admitted_expression(&candidate.operation)?;
                (key == redundant_key && scalar_type == patch.scalar_type).then_some(NodeLocation {
                    machine: function.machine,
                    block: leader_block.id,
                    node: u32::try_from(node).ok()?,
                })
            })
            .next();
        if canonical_leader != Some(patch.leader) {
            return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
        }
    }
    if scope == ScalarCseScope::Dominating {
        let dominators = independent_reachable_dominators(function);
        if !dominators
            .get(&patch.redundant.block)
            .is_some_and(|rows| rows.contains(&patch.leader.block))
        {
            return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
        }
        let canonical_leader = function
            .blocks
            .iter()
            .filter(|block| block.id != patch.redundant.block)
            .filter(|block| {
                dominators
                    .get(&patch.redundant.block)
                    .is_some_and(|rows| rows.contains(&block.id))
            })
            .flat_map(|block| {
                block
                    .nodes
                    .iter()
                    .enumerate()
                    .filter_map(|(node, candidate)| {
                        let (key, _, _, scalar_type, _) =
                            admitted_expression(&candidate.operation)?;
                        (key == redundant_key && scalar_type == patch.scalar_type).then_some(
                            NodeLocation {
                                machine: function.machine,
                                block: block.id,
                                node: u32::try_from(node).ok()?,
                            },
                        )
                    })
            })
            .min_by_key(|location| {
                (
                    dominators
                        .get(&location.block)
                        .map_or(usize::MAX, BTreeSet::len),
                    *location,
                )
            });
        if canonical_leader != Some(patch.leader)
            || function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .flat_map(|node| &node.uses)
                .filter(|use_site| use_site.value == redundant_result)
                .any(|use_site| {
                    if use_site.block == patch.leader.block {
                        patch.leader.node >= use_site.node
                    } else {
                        !dominators
                            .get(&use_site.block)
                            .is_some_and(|rows| rows.contains(&patch.leader.block))
                    }
                })
        {
            return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
        }
    }
    if leader.definitions
        != [ValueDefinition {
            value: leader_result,
            scalar_type: leader_type,
            site: ValueDefinitionSite::Node {
                block: leader_block.id,
                node: patch.leader.node,
            },
        }]
        || redundant.definitions
            != [ValueDefinition {
                value: redundant_result,
                scalar_type: redundant_type,
                site: ValueDefinitionSite::Node {
                    block: redundant_block.id,
                    node: patch.redundant.node,
                },
            }]
        || !leader.successors.is_empty()
        || !redundant.successors.is_empty()
        || !leader.ownership.is_empty()
        || !redundant.ownership.is_empty()
        || !function
            .blocks
            .iter()
            .flat_map(|row| &row.nodes)
            .flat_map(|row| &row.uses)
            .any(|row| row.value == redundant_result)
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let (expected_blocks, accepted_provenance) = reconstruct_local_cse_accounting(function, patch)
        .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.affected_blocks() != expected_blocks
        || candidate.provenance() != accepted_provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }
    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|row| row.machine == patch.leader.machine)
        .expect("candidate function exists");
    let output_block = output_function
        .blocks
        .iter_mut()
        .find(|row| row.id == patch.redundant.block)
        .expect("candidate block exists");
    let removed = output_block.nodes.remove(redundant_index);
    let receiver = output_block
        .nodes
        .get_mut(redundant_index)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    receiver.provenance.extend_from_slice(&removed.provenance);
    receiver.fuel.extend_from_slice(&removed.fuel);
    let mut effect = 0u64;
    for block in &mut output_function.blocks {
        for (node_index, node) in block.nodes.iter_mut().enumerate() {
            rewrite_scalar_value_uses(&mut node.operation, redundant_result, leader_result);
            let node_index = u32::try_from(node_index)
                .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
            node.definitions = expected_definitions(&node.operation, block.id, node_index);
            node.uses = expected_uses(&node.operation, block.id, node_index);
            node.successors = preserve_edge_custody(node);
            node.ownership = expected_ownership(&node.operation);
            node.effect = omega_optimization_unit::EffectLink {
                input: effect,
                output: effect
                    .checked_add(1)
                    .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?,
            };
            effect = effect
                .checked_add(1)
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
        }
    }
    output_function.facts = reconstruct_fact_index(output_function);
    output_function.declared_places = reconstruct_declared_places(output_function)?;
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
    let output_function = output
        .functions
        .iter()
        .find(|row| row.machine == function.machine)
        .expect("output function exists");
    for input_block in &function.blocks {
        if !expected_blocks.contains(&input_block.id)
            && output_function
                .blocks
                .iter()
                .find(|row| row.id == input_block.id)
                != Some(input_block)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            match (scope, proof_class) {
                (ScalarCseScope::SameBlock, ScalarCseProofClass::ObligationFree) => {
                    b"omega.validator.same-block-obligation-free-total-scalar-cse.v1"
                }
                (ScalarCseScope::Dominating, ScalarCseProofClass::ObligationFree) => {
                    b"omega.validator.dominator-total-scalar-cse.v1"
                }
                (ScalarCseScope::SameBlock, ScalarCseProofClass::ProofCertified) => {
                    b"omega.validator.same-block-proof-certified-total-scalar-cse.v1"
                }
                (ScalarCseScope::Dominating, ScalarCseProofClass::ProofCertified) => {
                    b"omega.validator.dominator-proof-certified-total-scalar-gvn.v1"
                }
                (ScalarCseScope::SameBlock, ScalarCseProofClass::CompatiblePolicy) => {
                    b"omega.validator.same-block-proof-certified-compatible-policy-scalar-cse.v1"
                }
                (ScalarCseScope::Dominating, ScalarCseProofClass::CompatiblePolicy) => {
                    b"omega.validator.dominator-proof-certified-compatible-policy-scalar-gvn.v1"
                }
            },
        ),
        provenance: accepted_provenance,
    })
}

fn independent_reachable_dominators(
    function: &PsiOptimizationFunction,
) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
    let successors = function
        .blocks
        .iter()
        .map(|block| {
            (
                block.id,
                block
                    .nodes
                    .last()
                    .map(|node| node.successors.iter().map(|edge| edge.target).collect())
                    .unwrap_or_default(),
            )
        })
        .collect::<BTreeMap<BlockId, Vec<BlockId>>>();
    let mut reachable = BTreeSet::from([function.entry]);
    let mut frontier = vec![function.entry];
    while let Some(block) = frontier.pop() {
        for successor in successors.get(&block).into_iter().flatten() {
            if reachable.insert(*successor) {
                frontier.push(*successor);
            }
        }
    }
    let mut predecessors = reachable
        .iter()
        .copied()
        .map(|block| (block, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for (source, targets) in &successors {
        if !reachable.contains(source) {
            continue;
        }
        for target in targets.iter().filter(|target| reachable.contains(target)) {
            predecessors.get_mut(target).unwrap().insert(*source);
        }
    }
    let mut result = reachable
        .iter()
        .copied()
        .map(|block| {
            (
                block,
                if block == function.entry {
                    BTreeSet::from([block])
                } else {
                    reachable.clone()
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for block in reachable
            .iter()
            .copied()
            .filter(|block| *block != function.entry)
        {
            let mut incoming = predecessors[&block].iter();
            let mut next = incoming
                .next()
                .map(|predecessor| result[predecessor].clone())
                .unwrap_or_default();
            for predecessor in incoming {
                next = next.intersection(&result[predecessor]).copied().collect();
            }
            next.insert(block);
            if result[&block] != next {
                result.insert(block, next);
                changed = true;
            }
        }
        if !changed {
            return result;
        }
    }
}

fn independently_replacement_dominates_uses(
    function: &PsiOptimizationFunction,
    dominators: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    replacement: ValueId,
    parameter: ValueId,
    scalar_type: ScalarType,
) -> bool {
    if replacement == parameter {
        return false;
    }
    let Some(definition) = function
        .parameters
        .iter()
        .chain(function.blocks.iter().flat_map(|block| {
            block
                .parameters
                .iter()
                .chain(block.nodes.iter().flat_map(|node| &node.definitions))
        }))
        .find(|definition| definition.value == replacement)
    else {
        return false;
    };
    if definition.scalar_type != scalar_type {
        return false;
    }
    function
        .blocks
        .iter()
        .flat_map(|block| block.nodes.iter().flat_map(|node| &node.uses))
        .filter(|use_site| use_site.value == parameter)
        .all(|use_site| match definition.site {
            ValueDefinitionSite::FunctionParameter(_) => true,
            ValueDefinitionSite::BlockParameter {
                block: defining, ..
            } => dominators
                .get(&use_site.block)
                .is_some_and(|rows| rows.contains(&defining)),
            ValueDefinitionSite::Node {
                block: defining,
                node,
            } if defining == use_site.block => node < use_site.node,
            ValueDefinitionSite::Node {
                block: defining, ..
            } => dominators
                .get(&use_site.block)
                .is_some_and(|rows| rows.contains(&defining)),
        })
}

fn independently_validated_dead_scalar_shape(
    rule: OptimizationRuleIdentity,
    operation: &O,
) -> Option<(
    psi_core::OperationId,
    ValueId,
    ScalarType,
    Option<psi_core::ObligationId>,
)> {
    let literal_rule = OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.dead-unused-scalar-literal-elimination.v1",
    );
    let total_rule = OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.dead-unused-unconditionally-total-scalar-elimination.v1",
    );
    let proof_rule = OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.dead-unused-proof-certified-scalar-elimination.v1",
    );
    match (rule, operation) {
        (
            rule,
            O::IntegerConstant {
                psi_operation,
                result,
                scalar_type,
                ..
            },
        ) if rule == literal_rule => Some((*psi_operation, *result, *scalar_type, None)),
        (
            rule,
            O::BooleanConstant {
                psi_operation,
                result,
                ..
            },
        ) if rule == literal_rule => Some((*psi_operation, *result, ScalarType::Boolean, None)),
        (
            rule,
            O::BooleanNot {
                psi_operation,
                result,
                ..
            }
            | O::BooleanEqual {
                psi_operation,
                result,
                ..
            }
            | O::IntegerEqual {
                psi_operation,
                result,
                ..
            }
            | O::IntegerLessThan {
                psi_operation,
                result,
                ..
            }
            | O::IntegerLessOrEqual {
                psi_operation,
                result,
                ..
            },
        ) if rule == total_rule => Some((*psi_operation, *result, ScalarType::Boolean, None)),
        (
            rule,
            O::IntegerBitwiseNot {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::IntegerBitwiseAnd {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::IntegerBitwiseOr {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::IntegerBitwiseXor {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::WrappingIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::SaturatingIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::WrappingIntegerSubtract {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::SaturatingIntegerSubtract {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::WrappingIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::SaturatingIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                ..
            },
        ) if rule == total_rule => Some((
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
            None,
        )),
        (
            rule,
            O::IntegerWiden {
                psi_operation,
                result,
                target_type,
                ..
            },
        ) if rule == total_rule => Some((
            *psi_operation,
            *result,
            ScalarType::Integer(*target_type),
            None,
        )),
        (
            rule,
            O::WrappingIntegerShiftLeft {
                psi_operation,
                result,
                value_type,
                ..
            }
            | O::WrappingIntegerShiftRight {
                psi_operation,
                result,
                value_type,
                ..
            },
        ) if rule == total_rule => Some((
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
            None,
        )),
        (
            rule,
            O::IntegerExactCast {
                psi_operation,
                obligation,
                result,
                target_type,
                ..
            },
        ) if rule == proof_rule => Some((
            *psi_operation,
            *result,
            ScalarType::Integer(*target_type),
            Some(*obligation),
        )),
        (
            rule,
            O::ExactIntegerShiftLeft {
                psi_operation,
                obligation,
                result,
                value_type,
                ..
            }
            | O::ExactIntegerShiftRight {
                psi_operation,
                obligation,
                result,
                value_type,
                ..
            },
        ) if rule == proof_rule => Some((
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
            Some(*obligation),
        )),
        (
            rule,
            O::ExactIntegerAdd {
                psi_operation,
                obligation,
                result,
                scalar_type,
                ..
            }
            | O::ExactIntegerSubtract {
                psi_operation,
                obligation,
                result,
                scalar_type,
                ..
            }
            | O::ExactIntegerMultiply {
                psi_operation,
                obligation,
                result,
                scalar_type,
                ..
            }
            | O::ExactIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                ..
            }
            | O::ExactIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                ..
            }
            | O::WrappingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                ..
            }
            | O::WrappingIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                ..
            }
            | O::SaturatingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                ..
            }
            | O::SaturatingIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                ..
            },
        ) if rule == proof_rule => Some((
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
            Some(*obligation),
        )),
        _ => None,
    }
}

fn preserve_edge_custody(
    node: &omega_optimization_unit::OptimizationNode,
) -> Vec<OptimizationEdge> {
    let expected = expected_edges(&node.operation);
    expected
        .into_iter()
        .map(|mut edge| {
            if let Some(existing) = node
                .successors
                .iter()
                .find(|existing| existing.psi_edge == edge.psi_edge)
            {
                edge.provenance = existing.provenance.clone();
                edge.fuel = existing.fuel.clone();
            }
            edge
        })
        .collect()
}

fn rewrite_scalar_substitutions(
    operation: &mut O,
    substitutions: &[ScalarSubstitution],
    machine: MachineId,
    removed_block: BlockId,
) {
    for substitution in substitutions {
        rewrite_block_parameter_operation(
            operation,
            RedundantBlockParameterRewrite {
                machine,
                block: removed_block,
                position: 0,
                parameter: substitution.from,
                replacement: substitution.to,
                scalar_type: substitution.scalar_type,
            },
        );
    }
}

fn reconstruct_adjacent_merge_ownership_is_identity(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    incoming: EdgeId,
    target: BlockId,
) -> bool {
    reconstruct_adjacent_merge_ownership_witness(unit, function, incoming, target).is_some()
}

fn reconstruct_adjacent_merge_ownership_witness(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    incoming: EdgeId,
    target: BlockId,
) -> Option<OwnershipFrontierWitness> {
    let sites = [
        OwnershipFrontierSite::EdgeEntry(incoming),
        OwnershipFrontierSite::EdgeExit(incoming),
        OwnershipFrontierSite::BlockEntry(target),
    ];
    let facts = sites.map(|site| {
        unit.ownership_frontier_facts
            .iter()
            .find(|fact| fact.machine == function.machine && fact.site == site)
    });
    if facts.iter().all(Option::is_none) {
        return (function.structural_parameters.is_empty()
            && function.entry_claim_declarations.is_empty()
            && function.declared_places.is_empty())
        .then_some(OwnershipFrontierWitness { rows: Vec::new() });
    }
    if !facts.iter().all(Option::is_some)
        || !facts
            .windows(2)
            .all(|pair| pair[0].unwrap().snapshot == pair[1].unwrap().snapshot)
    {
        return None;
    }
    let mut rows = facts
        .into_iter()
        .map(|fact| {
            let fact = fact.expect("complete ownership frontier fact set");
            OwnershipFrontierWitnessRow {
                site: fact.site,
                fact: fact.identity,
            }
        })
        .collect::<Vec<_>>();
    rows.sort_unstable_by_key(|row| row.site);
    Some(OwnershipFrontierWitness { rows })
}

fn reconstruct_adjacent_merge_accounting(
    function: &PsiOptimizationFunction,
    patch: AdjacentBlockMergeRewrite,
    substitutions: &[ScalarSubstitution],
) -> Option<(
    Vec<BlockId>,
    Vec<omega_optimization_unit::ProvenanceRewrite>,
)> {
    let predecessor_position = function
        .blocks
        .iter()
        .position(|block| block.id == patch.predecessor.block)?;
    let target_position = function
        .blocks
        .iter()
        .position(|block| block.id == patch.target)?;
    if target_position != predecessor_position.checked_add(1)? {
        return None;
    }
    let predecessor = &function.blocks[predecessor_position];
    let predecessor_node = predecessor
        .nodes
        .get(usize::try_from(patch.predecessor.node).ok()?)?;
    let incoming = predecessor_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == patch.incoming_edge)?;
    let target = &function.blocks[target_position];
    let incoming_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: patch.incoming_edge,
    };
    let mut affected = BTreeSet::from([predecessor.id, target.id]);
    let first = target.nodes.first()?;
    let mut realized = if first.successors.is_empty() {
        vec![omega_optimization_unit::ProvenanceRewrite {
            input: incoming_site,
            disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(
                NodeLocation {
                    machine: function.machine,
                    block: predecessor.id,
                    node: patch.predecessor.node,
                },
            )),
            sources: incoming.provenance.clone(),
            fuel: incoming.fuel.clone(),
        }]
    } else {
        first
            .successors
            .iter()
            .map(|successor| omega_optimization_unit::ProvenanceRewrite {
                input: incoming_site,
                disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Edge {
                    machine: function.machine,
                    edge: successor.psi_edge,
                }),
                sources: incoming.provenance.clone(),
                fuel: incoming.fuel.clone(),
            })
            .collect()
    };
    for (node_index, node) in target.nodes.iter().enumerate() {
        if node.provenance.is_empty() {
            continue;
        }
        let input = PsiRealizationSite::Node(NodeLocation {
            machine: function.machine,
            block: target.id,
            node: u32::try_from(node_index).ok()?,
        });
        let output = PsiRealizationSite::Node(NodeLocation {
            machine: function.machine,
            block: predecessor.id,
            node: patch
                .predecessor
                .node
                .checked_add(u32::try_from(node_index).ok()?)?,
        });
        realized.push(omega_optimization_unit::ProvenanceRewrite {
            input,
            disposition: ProvenanceDisposition::RealizedAt(output),
            sources: node.provenance.clone(),
            fuel: node.fuel.clone(),
        });
    }
    for block in function.blocks.iter().skip(target_position + 1) {
        affected.insert(block.id);
        for (node_index, node) in block.nodes.iter().enumerate() {
            if node.provenance.is_empty() {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).ok()?,
            });
            realized.push(omega_optimization_unit::ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
    }
    let substituted_values = substitutions
        .iter()
        .map(|row| row.from)
        .collect::<BTreeSet<_>>();
    for block in &function.blocks {
        if affected.contains(&block.id) {
            continue;
        }
        let changed_nodes = block
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| {
                node.uses
                    .iter()
                    .any(|row| substituted_values.contains(&row.value))
            })
            .collect::<Vec<_>>();
        if changed_nodes.is_empty() {
            continue;
        }
        affected.insert(block.id);
        for (node_index, node) in changed_nodes {
            if node.provenance.is_empty() {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).ok()?,
            });
            realized.push(omega_optimization_unit::ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
    }
    realized.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((affected.into_iter().collect(), realized))
}

fn reconstruct_non_adjacent_merge_accounting(
    function: &PsiOptimizationFunction,
    patch: NonAdjacentBlockMergeRewrite,
    substitutions: &[ScalarSubstitution],
) -> Option<(
    Vec<BlockId>,
    Vec<omega_optimization_unit::ProvenanceRewrite>,
)> {
    let predecessor_position = function
        .blocks
        .iter()
        .position(|block| block.id == patch.predecessor.block)?;
    let target_position = function
        .blocks
        .iter()
        .position(|block| block.id == patch.target)?;
    if target_position == predecessor_position.checked_add(1)? {
        return None;
    }
    let predecessor = &function.blocks[predecessor_position];
    let predecessor_node = predecessor
        .nodes
        .get(usize::try_from(patch.predecessor.node).ok()?)?;
    let incoming = predecessor_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == patch.incoming_edge)?;
    let target = &function.blocks[target_position];
    let first = target.nodes.first()?;
    let incoming_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: patch.incoming_edge,
    };
    let mut realized = if !first.provenance.is_empty() {
        vec![omega_optimization_unit::ProvenanceRewrite {
            input: incoming_site,
            disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(
                patch.predecessor,
            )),
            sources: incoming.provenance.clone(),
            fuel: incoming.fuel.clone(),
        }]
    } else if !first.successors.is_empty() {
        first
            .successors
            .iter()
            .map(|successor| omega_optimization_unit::ProvenanceRewrite {
                input: incoming_site,
                disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Edge {
                    machine: function.machine,
                    edge: successor.psi_edge,
                }),
                sources: incoming.provenance.clone(),
                fuel: incoming.fuel.clone(),
            })
            .collect()
    } else {
        return None;
    };

    for (node_index, node) in target.nodes.iter().enumerate() {
        if node.provenance.is_empty() {
            continue;
        }
        let input = PsiRealizationSite::Node(NodeLocation {
            machine: function.machine,
            block: target.id,
            node: u32::try_from(node_index).ok()?,
        });
        let output = PsiRealizationSite::Node(NodeLocation {
            machine: function.machine,
            block: predecessor.id,
            node: patch
                .predecessor
                .node
                .checked_add(u32::try_from(node_index).ok()?)?,
        });
        realized.push(omega_optimization_unit::ProvenanceRewrite {
            input,
            disposition: ProvenanceDisposition::RealizedAt(output),
            sources: node.provenance.clone(),
            fuel: node.fuel.clone(),
        });
    }

    let mut input_effect = 0u64;
    let mut input_starts = BTreeMap::new();
    for block in &function.blocks {
        input_starts.insert(block.id, input_effect);
        input_effect = input_effect.checked_add(u64::try_from(block.nodes.len()).ok()?)?;
    }
    let mut output_effect = 0u64;
    let mut effect_shifted = BTreeSet::new();
    for block in &function.blocks {
        if block.id == patch.target {
            continue;
        }
        if input_starts.get(&block.id).copied()? != output_effect {
            effect_shifted.insert(block.id);
        }
        let output_nodes = if block.id == patch.predecessor.block {
            block
                .nodes
                .len()
                .checked_sub(1)?
                .checked_add(target.nodes.len())?
        } else {
            block.nodes.len()
        };
        output_effect = output_effect.checked_add(u64::try_from(output_nodes).ok()?)?;
    }

    let substituted_values = substitutions
        .iter()
        .map(|row| row.from)
        .collect::<BTreeSet<_>>();
    let mut affected = BTreeSet::from([patch.predecessor.block, patch.target]);
    affected.extend(effect_shifted.iter().copied());
    for block in &function.blocks {
        if block.id == patch.target {
            continue;
        }
        let mut changed_uses = BTreeSet::new();
        for (node_index, node) in block.nodes.iter().enumerate() {
            if node
                .uses
                .iter()
                .any(|row| substituted_values.contains(&row.value))
            {
                changed_uses.insert(node_index);
                affected.insert(block.id);
            }
        }
        for (node_index, node) in block.nodes.iter().enumerate() {
            if node.provenance.is_empty()
                || (!effect_shifted.contains(&block.id) && !changed_uses.contains(&node_index))
            {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).ok()?,
            });
            realized.push(omega_optimization_unit::ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
    }
    realized.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((affected.into_iter().collect(), realized))
}

fn reconstruct_shared_terminal_fusion_accounting(
    function: &PsiOptimizationFunction,
    patch: SharedJumpFusionRewrite,
) -> Option<(
    Vec<BlockId>,
    Vec<omega_optimization_unit::ProvenanceRewrite>,
)> {
    let predecessor = function
        .blocks
        .iter()
        .find(|block| block.id == patch.predecessor.block)?;
    let predecessor_node = predecessor
        .nodes
        .get(usize::try_from(patch.predecessor.node).ok()?)?;
    let incoming = predecessor_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == patch.incoming_edge)?;
    let target = function
        .blocks
        .iter()
        .find(|block| block.id == patch.target)?;
    let [terminal] = target.nodes.as_slice() else {
        return None;
    };
    let input_edge = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: patch.incoming_edge,
    };
    let input_terminal = PsiRealizationSite::Node(NodeLocation {
        machine: function.machine,
        block: patch.target,
        node: 0,
    });
    let output_clone = PsiRealizationSite::Node(patch.predecessor);
    let mut provenance = vec![
        omega_optimization_unit::ProvenanceRewrite {
            input: input_edge,
            disposition: ProvenanceDisposition::RealizedAt(output_clone),
            sources: incoming.provenance.clone(),
            fuel: incoming.fuel.clone(),
        },
        omega_optimization_unit::ProvenanceRewrite {
            input: input_terminal,
            disposition: ProvenanceDisposition::RealizedAt(output_clone),
            sources: terminal.provenance.clone(),
            fuel: terminal.fuel.clone(),
        },
        omega_optimization_unit::ProvenanceRewrite {
            input: input_terminal,
            disposition: ProvenanceDisposition::RealizedAt(input_terminal),
            sources: terminal.provenance.clone(),
            fuel: terminal.fuel.clone(),
        },
    ];
    provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    let mut blocks = vec![patch.predecessor.block, patch.target];
    blocks.sort();
    blocks.dedup();
    Some((blocks, provenance))
}

fn reconstruct_dead_scalar_node_accounting(
    function: &PsiOptimizationFunction,
    patch: DeadScalarNodeRewrite,
) -> Option<(
    Vec<BlockId>,
    Vec<omega_optimization_unit::ProvenanceRewrite>,
)> {
    let block_position = function
        .blocks
        .iter()
        .position(|block| block.id == patch.location.block)?;
    let node_position = usize::try_from(patch.location.node).ok()?;
    let block = &function.blocks[block_position];
    let removed = block.nodes.get(node_position)?;
    block.nodes.get(node_position.checked_add(1)?)?;
    let mut provenance = vec![omega_optimization_unit::ProvenanceRewrite {
        input: PsiRealizationSite::Node(patch.location),
        disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(patch.location)),
        sources: removed.provenance.clone(),
        fuel: removed.fuel.clone(),
    }];
    for (index, node) in block.nodes.iter().enumerate().skip(node_position + 1) {
        if node.provenance.is_empty() {
            continue;
        }
        let old = NodeLocation {
            machine: function.machine,
            block: block.id,
            node: u32::try_from(index).ok()?,
        };
        let new = NodeLocation {
            node: old.node.checked_sub(1)?,
            ..old
        };
        provenance.push(omega_optimization_unit::ProvenanceRewrite {
            input: PsiRealizationSite::Node(old),
            disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(new)),
            sources: node.provenance.clone(),
            fuel: node.fuel.clone(),
        });
    }
    let mut blocks = vec![block.id];
    for later in function.blocks.iter().skip(block_position + 1) {
        blocks.push(later.id);
        for (index, node) in later.nodes.iter().enumerate() {
            if node.provenance.is_empty() {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: later.id,
                node: u32::try_from(index).ok()?,
            });
            provenance.push(omega_optimization_unit::ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
    }
    blocks.sort();
    provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((blocks, provenance))
}

fn reconstruct_proof_certified_scalar_identity_accounting(
    function: &PsiOptimizationFunction,
    patch: ProofCertifiedScalarIdentityRewrite,
) -> Option<(
    Vec<BlockId>,
    Vec<omega_optimization_unit::ProvenanceRewrite>,
)> {
    let dead = DeadScalarNodeRewrite {
        location: patch.location,
        source_operation: patch.source_operation,
        result: patch.result,
        scalar_type: ScalarType::Integer(patch.scalar_type),
    };
    let (mut blocks, mut provenance) = reconstruct_dead_scalar_node_accounting(function, dead)?;
    for use_block in &function.blocks {
        if blocks.contains(&use_block.id)
            || !use_block
                .nodes
                .iter()
                .flat_map(|node| &node.uses)
                .any(|row| row.value == patch.result)
        {
            continue;
        }
        blocks.push(use_block.id);
        for (index, node) in use_block.nodes.iter().enumerate() {
            if node.provenance.is_empty() {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: use_block.id,
                node: u32::try_from(index).ok()?,
            });
            provenance.push(omega_optimization_unit::ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
    }
    blocks.sort();
    blocks.dedup();
    provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((blocks, provenance))
}

fn reconstruct_local_cse_accounting(
    function: &PsiOptimizationFunction,
    patch: LocalScalarCommonSubexpressionRewrite,
) -> Option<(
    Vec<BlockId>,
    Vec<omega_optimization_unit::ProvenanceRewrite>,
)> {
    let dead = DeadScalarNodeRewrite {
        location: patch.redundant,
        source_operation: patch.redundant_operation,
        result: patch.redundant_result,
        scalar_type: patch.scalar_type,
    };
    let (mut blocks, mut provenance) = reconstruct_dead_scalar_node_accounting(function, dead)?;
    for use_block in &function.blocks {
        if blocks.contains(&use_block.id)
            || !use_block
                .nodes
                .iter()
                .flat_map(|node| &node.uses)
                .any(|row| row.value == patch.redundant_result)
        {
            continue;
        }
        blocks.push(use_block.id);
        for (index, node) in use_block.nodes.iter().enumerate() {
            if node.provenance.is_empty() {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: use_block.id,
                node: u32::try_from(index).ok()?,
            });
            provenance.push(omega_optimization_unit::ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
    }
    blocks.sort();
    provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((blocks, provenance))
}

fn reconstruct_phi_translated_cse_accounting(
    function: &PsiOptimizationFunction,
    patch: &PhiTranslatedScalarGvnRewrite,
) -> Option<(
    Vec<BlockId>,
    Vec<omega_optimization_unit::ProvenanceRewrite>,
)> {
    let dead = DeadScalarNodeRewrite {
        location: patch.redundant,
        source_operation: patch.redundant_operation,
        result: patch.redundant_result,
        scalar_type: patch.scalar_type,
    };
    let (mut blocks, mut provenance) = reconstruct_dead_scalar_node_accounting(function, dead)?;
    for incoming in &patch.incoming {
        let edge = function
            .blocks
            .iter()
            .find(|block| block.id == incoming.source)?
            .nodes
            .iter()
            .flat_map(|node| &node.successors)
            .find(|edge| edge.psi_edge == incoming.edge && edge.target == patch.redundant.block)?;
        blocks.push(incoming.source);
        let site = PsiRealizationSite::Edge {
            machine: function.machine,
            edge: incoming.edge,
        };
        provenance.push(omega_optimization_unit::ProvenanceRewrite {
            input: site,
            disposition: ProvenanceDisposition::RealizedAt(site),
            sources: edge.provenance.clone(),
            fuel: edge.fuel.clone(),
        });
    }
    blocks.sort();
    blocks.dedup();
    provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((blocks, provenance))
}

fn rewrite_scalar_value_uses(operation: &mut O, from: ValueId, to: ValueId) {
    let replace = |value: &mut ValueId| {
        if *value == from {
            *value = to;
        }
    };
    let rewrite_bindings = |bindings: &mut Vec<omega_abstract_operations::ValueBinding>| {
        for binding in bindings {
            replace(&mut binding.argument);
        }
    };
    match operation {
        O::Call { arguments, .. } | O::BoundaryCall { arguments, .. } => {
            for argument in arguments {
                replace(argument);
            }
        }
        O::BooleanNot { operand, .. }
        | O::IntegerBitwiseNot { operand, .. }
        | O::IntegerWiden { operand, .. }
        | O::IntegerExactCast { operand, .. } => replace(operand),
        O::BooleanEqual { left, right, .. }
        | O::IntegerEqual { left, right, .. }
        | O::IntegerLessThan { left, right, .. }
        | O::IntegerLessOrEqual { left, right, .. }
        | O::IntegerBitwiseAnd { left, right, .. }
        | O::IntegerBitwiseOr { left, right, .. }
        | O::IntegerBitwiseXor { left, right, .. }
        | O::WrappingIntegerAdd { left, right, .. }
        | O::ExactIntegerAdd { left, right, .. }
        | O::SaturatingIntegerAdd { left, right, .. }
        | O::WrappingIntegerSubtract { left, right, .. }
        | O::ExactIntegerSubtract { left, right, .. }
        | O::SaturatingIntegerSubtract { left, right, .. }
        | O::WrappingIntegerMultiply { left, right, .. }
        | O::ExactIntegerMultiply { left, right, .. }
        | O::SaturatingIntegerMultiply { left, right, .. }
        | O::ExactIntegerDivide { left, right, .. }
        | O::ExactIntegerRemainder { left, right, .. }
        | O::WrappingIntegerDivide { left, right, .. }
        | O::WrappingIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerDivide { left, right, .. }
        | O::SaturatingIntegerRemainder { left, right, .. } => {
            replace(left);
            replace(right);
        }
        O::WrappingIntegerShiftLeft { value, count, .. }
        | O::WrappingIntegerShiftRight { value, count, .. }
        | O::ExactIntegerShiftLeft { value, count, .. }
        | O::ExactIntegerShiftRight { value, count, .. } => {
            replace(value);
            replace(count);
        }
        O::Jump { bindings, .. } => rewrite_bindings(bindings),
        O::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            replace(condition);
            rewrite_bindings(&mut when_true.bindings);
            rewrite_bindings(&mut when_false.bindings);
        }
        O::Return { value, .. } => replace(value),
        O::EstablishPayloadlessCase { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::CallUnit { .. }
        | O::CallStructuralScalar { .. }
        | O::CallStructural { .. }
        | O::PortWrite { .. }
        | O::IntegerConstant { .. }
        | O::BooleanConstant { .. }
        | O::BooleanStructuralField { .. }
        | O::ReturnUnit { .. }
        | O::ReturnStructural { .. }
        | O::Crash { .. } => {}
    }
}

fn rewrite_successor_operation(
    operation: &mut O,
    edge: EdgeId,
    target: BlockId,
    bindings: &[omega_abstract_operations::ValueBinding],
) -> bool {
    match operation {
        O::Jump {
            psi_edge,
            target: operation_target,
            bindings: operation_bindings,
            ..
        } if *psi_edge == edge => {
            *operation_target = target;
            *operation_bindings = bindings.to_vec();
            true
        }
        O::Conditional {
            when_true,
            when_false,
            ..
        } => {
            let successor = if when_true.psi_edge == edge {
                when_true
            } else if when_false.psi_edge == edge {
                when_false
            } else {
                return false;
            };
            successor.target = target;
            successor.bindings = bindings.to_vec();
            true
        }
        _ => false,
    }
}

fn reconstruct_linear_thread_bindings(
    parameters: &[ValueDefinition],
    incoming: &[omega_abstract_operations::ValueBinding],
    outgoing: &[omega_abstract_operations::ValueBinding],
) -> Option<Vec<omega_abstract_operations::ValueBinding>> {
    if parameters.len() != incoming.len() {
        return None;
    }
    let replacements = parameters
        .iter()
        .zip(incoming)
        .map(|(parameter, binding)| {
            (binding.parameter == parameter.value && binding.scalar_type == parameter.scalar_type)
                .then_some((parameter.value, (binding.argument, binding.scalar_type)))
        })
        .collect::<Option<BTreeMap<_, _>>>()?;
    Some(
        outgoing
            .iter()
            .map(|binding| {
                replacements
                    .get(&binding.argument)
                    .map_or(*binding, |(argument, scalar_type)| {
                        omega_abstract_operations::ValueBinding {
                            parameter: binding.parameter,
                            argument: *argument,
                            scalar_type: *scalar_type,
                        }
                    })
            })
            .collect(),
    )
}

fn reconstruct_linear_thread_ownership_is_identity(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    incoming: EdgeId,
    empty: BlockId,
    outgoing: EdgeId,
    target: BlockId,
) -> bool {
    let sites = [
        OwnershipFrontierSite::EdgeEntry(incoming),
        OwnershipFrontierSite::EdgeExit(incoming),
        OwnershipFrontierSite::BlockEntry(empty),
        OwnershipFrontierSite::EdgeEntry(outgoing),
        OwnershipFrontierSite::EdgeExit(outgoing),
        OwnershipFrontierSite::BlockEntry(target),
    ];
    let facts = sites.map(|site| {
        unit.ownership_frontier_facts
            .iter()
            .find(|fact| fact.machine == function.machine && fact.site == site)
    });
    if facts.iter().all(Option::is_none) {
        return function.structural_parameters.is_empty()
            && function.entry_claim_declarations.is_empty()
            && function.declared_places.is_empty();
    }
    facts.iter().all(Option::is_some)
        && facts
            .windows(2)
            .all(|pair| pair[0].unwrap().snapshot == pair[1].unwrap().snapshot)
}

fn reconstruct_linear_thread_accounting(
    function: &PsiOptimizationFunction,
    predecessor: NodeLocation,
    empty: NodeLocation,
) -> Option<(
    Vec<BlockId>,
    Vec<omega_optimization_unit::ProvenanceRewrite>,
)> {
    let predecessor_node = function
        .blocks
        .iter()
        .find(|block| block.id == predecessor.block)?
        .nodes
        .get(usize::try_from(predecessor.node).ok()?)?;
    let empty_node = function
        .blocks
        .iter()
        .find(|block| block.id == empty.block)?
        .nodes
        .get(usize::try_from(empty.node).ok()?)?;
    let predecessor_edge = predecessor_node.successors.first()?;
    let empty_edge = empty_node.successors.first()?;
    let output_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: predecessor_edge.psi_edge,
    };
    let predecessor_site = output_site;
    let empty_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: empty_edge.psi_edge,
    };
    let mut affected = BTreeSet::from([predecessor.block, empty.block]);
    let mut realized = vec![
        omega_optimization_unit::ProvenanceRewrite {
            input: predecessor_site,
            disposition: ProvenanceDisposition::RealizedAt(output_site),
            sources: predecessor_edge.provenance.clone(),
            fuel: predecessor_edge.fuel.clone(),
        },
        omega_optimization_unit::ProvenanceRewrite {
            input: empty_site,
            disposition: ProvenanceDisposition::RealizedAt(output_site),
            sources: empty_edge.provenance.clone(),
            fuel: empty_edge.fuel.clone(),
        },
    ];
    let mut expected_effect = 0u64;
    for block in &function.blocks {
        if block.id == empty.block {
            continue;
        }
        for (node_index, node) in block.nodes.iter().enumerate() {
            let location = NodeLocation {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).ok()?,
            };
            let effect_changes = node.effect.input != expected_effect
                || node.effect.output != expected_effect.checked_add(1)?;
            if effect_changes && location != predecessor {
                affected.insert(block.id);
                if !node.provenance.is_empty() {
                    let site = PsiRealizationSite::Node(location);
                    realized.push(omega_optimization_unit::ProvenanceRewrite {
                        input: site,
                        disposition: ProvenanceDisposition::RealizedAt(site),
                        sources: node.provenance.clone(),
                        fuel: node.fuel.clone(),
                    });
                }
            }
            expected_effect = expected_effect.checked_add(1)?;
        }
    }
    realized.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((affected.into_iter().collect(), realized))
}

fn reconstruct_path_thread_accounting(
    function: &PsiOptimizationFunction,
    empty: NodeLocation,
    incoming_edges: &[EdgeId],
) -> Option<(
    Vec<BlockId>,
    Vec<omega_optimization_unit::ProvenanceRewrite>,
)> {
    let empty_node = function
        .blocks
        .iter()
        .find(|block| block.id == empty.block)?
        .nodes
        .get(usize::try_from(empty.node).ok()?)?;
    let outgoing = empty_node.successors.first()?;
    let outgoing_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: outgoing.psi_edge,
    };
    let incoming_set = incoming_edges.iter().copied().collect::<BTreeSet<_>>();
    if incoming_set.len() != incoming_edges.len() || incoming_set.is_empty() {
        return None;
    }
    let mut affected = BTreeSet::from([empty.block]);
    let mut realized = Vec::new();
    for block in &function.blocks {
        for node in &block.nodes {
            for edge in &node.successors {
                if !incoming_set.contains(&edge.psi_edge) || edge.target != empty.block {
                    continue;
                }
                affected.insert(block.id);
                let site = PsiRealizationSite::Edge {
                    machine: function.machine,
                    edge: edge.psi_edge,
                };
                realized.push(omega_optimization_unit::ProvenanceRewrite {
                    input: site,
                    disposition: ProvenanceDisposition::RealizedAt(site),
                    sources: edge.provenance.clone(),
                    fuel: edge.fuel.clone(),
                });
                realized.push(omega_optimization_unit::ProvenanceRewrite {
                    input: outgoing_site,
                    disposition: ProvenanceDisposition::RealizedAt(site),
                    sources: outgoing.provenance.clone(),
                    fuel: outgoing.fuel.clone(),
                });
            }
        }
    }
    if realized.len() != incoming_edges.len().checked_mul(2)? {
        return None;
    }
    let mut expected_effect = 0u64;
    for block in &function.blocks {
        if block.id == empty.block {
            continue;
        }
        for (node_index, node) in block.nodes.iter().enumerate() {
            let location = NodeLocation {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).ok()?,
            };
            let effect_changes = node.effect.input != expected_effect
                || node.effect.output != expected_effect.checked_add(1)?;
            if effect_changes {
                affected.insert(block.id);
                if !node.provenance.is_empty() {
                    let site = PsiRealizationSite::Node(location);
                    realized.push(omega_optimization_unit::ProvenanceRewrite {
                        input: site,
                        disposition: ProvenanceDisposition::RealizedAt(site),
                        sources: node.provenance.clone(),
                        fuel: node.fuel.clone(),
                    });
                }
            }
            expected_effect = expected_effect.checked_add(1)?;
        }
    }
    realized.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((affected.into_iter().collect(), realized))
}

/// Independently replay one redundant block-parameter elimination. The rule's
/// incoming-edge witness is treated only as a claim: this validator enumerates
/// every exact incoming edge again before applying the substitution.
pub fn validate_redundant_block_parameter_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::Dominators)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || candidate.safety_class() != OptimizationSafetyClass::StructuralIdentity
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::RemoveRedundantBlockParameter(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    let witness = candidate
        .redundant_block_parameter_witness()
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if function.entry == patch.block || patch.parameter == patch.replacement {
        return Err(OptimizationUnitValidationError::CandidateBlockParameterMismatch);
    }
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == patch.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let position = usize::try_from(patch.position).expect("u32 fits usize");
    let Some(parameter) = block.parameters.get(position) else {
        return Err(OptimizationUnitValidationError::CandidateBlockParameterMismatch);
    };
    if parameter.value != patch.parameter
        || parameter.scalar_type != patch.scalar_type
        || parameter.site
            != (ValueDefinitionSite::BlockParameter {
                block: patch.block,
                position: patch.position,
            })
    {
        return Err(OptimizationUnitValidationError::CandidateBlockParameterMismatch);
    }
    let replacement_type = function
        .parameters
        .iter()
        .chain(function.blocks.iter().flat_map(|block| {
            block
                .parameters
                .iter()
                .chain(block.nodes.iter().flat_map(|node| &node.definitions))
        }))
        .find(|definition| definition.value == patch.replacement)
        .map(|definition| definition.scalar_type);
    if replacement_type != Some(patch.scalar_type) {
        return Err(OptimizationUnitValidationError::CandidateSubstitutionMismatch);
    }

    let mut incoming = Vec::new();
    let mut expected_provenance = Vec::new();
    let mut affected_blocks = BTreeSet::from([patch.block]);
    for source in &function.blocks {
        for (node_index, node) in source.nodes.iter().enumerate() {
            let location = NodeLocation {
                machine: patch.machine,
                block: source.id,
                node: u32::try_from(node_index).expect("unit node index fits u32"),
            };
            let changes_use = node
                .uses
                .iter()
                .any(|use_site| use_site.value == patch.parameter);
            for edge in &node.successors {
                if edge.target != patch.block {
                    continue;
                }
                let Some(binding) = edge.bindings.get(position) else {
                    return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
                };
                incoming.push(BlockParameterIncomingBinding {
                    source: source.id,
                    edge: edge.psi_edge,
                    argument: binding.argument,
                });
                let site = PsiRealizationSite::Edge {
                    machine: patch.machine,
                    edge: edge.psi_edge,
                };
                expected_provenance.push(omega_optimization_unit::ProvenanceRewrite {
                    input: site,
                    disposition: ProvenanceDisposition::RealizedAt(site),
                    sources: edge.provenance.clone(),
                    fuel: edge.fuel.clone(),
                });
            }
            if changes_use {
                affected_blocks.insert(source.id);
                if !node.provenance.is_empty() {
                    let site = PsiRealizationSite::Node(location);
                    expected_provenance.push(omega_optimization_unit::ProvenanceRewrite {
                        input: site,
                        disposition: ProvenanceDisposition::RealizedAt(site),
                        sources: node.provenance.clone(),
                        fuel: node.fuel.clone(),
                    });
                }
            }
            if node
                .successors
                .iter()
                .any(|edge| edge.target == patch.block)
            {
                affected_blocks.insert(source.id);
            }
        }
    }
    incoming.sort_by_key(|row| (row.edge, row.source));
    expected_provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    if incoming != witness.incoming
        || incoming
            .iter()
            .any(|binding| binding.argument != patch.replacement)
    {
        return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
    }
    if candidate.substitutions()
        != [omega_optimization_unit::ScalarSubstitution {
            from: patch.parameter,
            to: patch.replacement,
            scalar_type: patch.scalar_type,
        }]
    {
        return Err(OptimizationUnitValidationError::CandidateSubstitutionMismatch);
    }
    if candidate.affected_blocks() != affected_blocks.into_iter().collect::<Vec<_>>()
        || candidate.provenance() != expected_provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let normalized_input =
        normalize_redundant_parameter_observation_input(input, patch, candidate.affected_blocks())?;
    let input_region = reconstruct_psi_closed_region_observation(
        &normalized_input,
        patch.machine,
        candidate.affected_blocks(),
    )
    .ok_or(OptimizationUnitValidationError::CandidateRegionObservationUnavailable)?;

    let mut output = input.clone();
    let function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.machine)
        .expect("candidate function exists");
    let block = function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.block)
        .expect("candidate block exists");
    block.parameters.remove(position);
    for (new_position, parameter) in block.parameters.iter_mut().enumerate().skip(position) {
        parameter.site = ValueDefinitionSite::BlockParameter {
            block: patch.block,
            position: u32::try_from(new_position).expect("parameter index fits u32"),
        };
    }
    for block in &mut function.blocks {
        for (node_index, node) in block.nodes.iter_mut().enumerate() {
            rewrite_block_parameter_operation(&mut node.operation, patch);
            let node_index = u32::try_from(node_index).expect("unit node index fits u32");
            node.definitions = expected_definitions(&node.operation, block.id, node_index);
            node.uses = expected_uses(&node.operation, block.id, node_index);
            node.successors = expected_edges(&node.operation);
            node.ownership = expected_ownership(&node.operation);
        }
    }
    function.facts = reconstruct_fact_index(function);
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
    if !unchanged_outside_redundant_parameter_region(
        input,
        &output,
        patch.machine,
        candidate.affected_blocks(),
    ) {
        return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
    }
    let output_region = reconstruct_psi_closed_region_observation(
        &output,
        patch.machine,
        candidate.affected_blocks(),
    )
    .ok_or(OptimizationUnitValidationError::CandidateRegionObservationUnavailable)?;
    if input_region.semantics != output_region.semantics {
        return Err(OptimizationUnitValidationError::CandidateRegionObservationMismatch);
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.redundant-block-parameter.v2",
        ),
        provenance: expected_provenance,
    })
}

/// Construct the validator's normalized pre-rewrite question independently of
/// the output constructor below. Only the exact scalar substitution and the
/// one proved incoming binding slot may change.
fn normalize_redundant_parameter_observation_input(
    input: &PsiOptimizationUnit,
    patch: RedundantBlockParameterRewrite,
    affected_blocks: &[BlockId],
) -> Result<PsiOptimizationUnit, OptimizationUnitValidationError> {
    let affected = affected_blocks.iter().copied().collect::<BTreeSet<_>>();
    let mut normalized = input.clone();
    let function = normalized
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let target = function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let position = usize::try_from(patch.position).expect("u32 fits usize");
    let removed = target
        .parameters
        .get(position)
        .copied()
        .ok_or(OptimizationUnitValidationError::CandidateBlockParameterMismatch)?;
    if removed.value != patch.parameter
        || removed.scalar_type != patch.scalar_type
        || removed.site
            != (ValueDefinitionSite::BlockParameter {
                block: patch.block,
                position: patch.position,
            })
    {
        return Err(OptimizationUnitValidationError::CandidateBlockParameterMismatch);
    }
    target.parameters.remove(position);
    for (new_position, parameter) in target.parameters.iter_mut().enumerate().skip(position) {
        parameter.site = ValueDefinitionSite::BlockParameter {
            block: patch.block,
            position: u32::try_from(new_position).expect("parameter index fits u32"),
        };
    }

    for block in function
        .blocks
        .iter_mut()
        .filter(|block| affected.contains(&block.id))
    {
        for (node_index, node) in block.nodes.iter_mut().enumerate() {
            node.operation =
                normalize_redundant_parameter_observation_operation(&node.operation, patch)?;
            let node_index = u32::try_from(node_index).expect("unit node index fits u32");
            node.definitions = expected_definitions(&node.operation, block.id, node_index);
            node.uses = expected_uses(&node.operation, block.id, node_index);
            node.successors = expected_edges(&node.operation);
            node.ownership = expected_ownership(&node.operation);
        }
    }
    normalized.identity = recompute_psi_optimization_unit_identity(&normalized);
    Ok(normalized)
}

fn normalize_redundant_parameter_observation_operation(
    operation: &omega_abstract_operations::AbstractOperation,
    patch: RedundantBlockParameterRewrite,
) -> Result<omega_abstract_operations::AbstractOperation, OptimizationUnitValidationError> {
    use omega_abstract_operations::AbstractOperation as O;

    let mut normalized = operation.clone();
    let replace = |value: &mut ValueId| {
        if *value == patch.parameter {
            *value = patch.replacement;
        }
    };
    let normalize_bindings = |target: BlockId,
                              bindings: &mut Vec<omega_abstract_operations::ValueBinding>|
     -> Result<(), OptimizationUnitValidationError> {
        for binding in bindings.iter_mut() {
            replace(&mut binding.argument);
        }
        if target == patch.block {
            let position = usize::try_from(patch.position).expect("u32 fits usize");
            let binding = bindings
                .get(position)
                .ok_or(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)?;
            if binding.parameter != patch.parameter
                || binding.argument != patch.replacement
                || binding.scalar_type != patch.scalar_type
            {
                return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
            }
            bindings.remove(position);
        }
        Ok(())
    };

    match &mut normalized {
        O::Call { arguments, .. } | O::BoundaryCall { arguments, .. } => {
            for argument in arguments {
                replace(argument);
            }
        }
        O::BooleanNot { operand, .. }
        | O::IntegerBitwiseNot { operand, .. }
        | O::IntegerWiden { operand, .. }
        | O::IntegerExactCast { operand, .. } => replace(operand),
        O::BooleanEqual { left, right, .. }
        | O::IntegerEqual { left, right, .. }
        | O::IntegerLessThan { left, right, .. }
        | O::IntegerLessOrEqual { left, right, .. }
        | O::IntegerBitwiseAnd { left, right, .. }
        | O::IntegerBitwiseOr { left, right, .. }
        | O::IntegerBitwiseXor { left, right, .. }
        | O::WrappingIntegerAdd { left, right, .. }
        | O::ExactIntegerAdd { left, right, .. }
        | O::SaturatingIntegerAdd { left, right, .. }
        | O::WrappingIntegerSubtract { left, right, .. }
        | O::ExactIntegerSubtract { left, right, .. }
        | O::SaturatingIntegerSubtract { left, right, .. }
        | O::WrappingIntegerMultiply { left, right, .. }
        | O::ExactIntegerMultiply { left, right, .. }
        | O::ExactIntegerDivide { left, right, .. }
        | O::ExactIntegerRemainder { left, right, .. }
        | O::WrappingIntegerDivide { left, right, .. }
        | O::WrappingIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerDivide { left, right, .. }
        | O::SaturatingIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerMultiply { left, right, .. } => {
            replace(left);
            replace(right);
        }
        O::WrappingIntegerShiftLeft { value, count, .. }
        | O::WrappingIntegerShiftRight { value, count, .. }
        | O::ExactIntegerShiftLeft { value, count, .. }
        | O::ExactIntegerShiftRight { value, count, .. } => {
            replace(value);
            replace(count);
        }
        O::Jump {
            target, bindings, ..
        } => normalize_bindings(*target, bindings)?,
        O::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            replace(condition);
            normalize_bindings(when_true.target, &mut when_true.bindings)?;
            normalize_bindings(when_false.target, &mut when_false.bindings)?;
        }
        O::Return { value, .. } => replace(value),
        O::EstablishPayloadlessCase { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::CallUnit { .. }
        | O::CallStructuralScalar { .. }
        | O::CallStructural { .. }
        | O::PortWrite { .. }
        | O::IntegerConstant { .. }
        | O::BooleanConstant { .. }
        | O::BooleanStructuralField { .. }
        | O::ReturnUnit { .. }
        | O::ReturnStructural { .. }
        | O::Crash { .. } => {}
    }
    Ok(normalized)
}

fn unchanged_outside_redundant_parameter_region(
    input: &PsiOptimizationUnit,
    output: &PsiOptimizationUnit,
    machine: MachineId,
    affected_blocks: &[BlockId],
) -> bool {
    let mut expected = input.clone();
    let Some(expected_function) = expected
        .functions
        .iter_mut()
        .find(|function| function.machine == machine)
    else {
        return false;
    };
    let Some(output_function) = output
        .functions
        .iter()
        .find(|function| function.machine == machine)
    else {
        return false;
    };
    for block_id in affected_blocks {
        let Some(expected_block) = expected_function
            .blocks
            .iter_mut()
            .find(|block| block.id == *block_id)
        else {
            return false;
        };
        let Some(output_block) = output_function
            .blocks
            .iter()
            .find(|block| block.id == *block_id)
        else {
            return false;
        };
        *expected_block = output_block.clone();
    }
    expected.identity = output.identity;
    expected == *output
}

fn rewrite_block_parameter_operation(
    operation: &mut omega_abstract_operations::AbstractOperation,
    patch: RedundantBlockParameterRewrite,
) {
    use omega_abstract_operations::AbstractOperation as O;

    let replace = |value: &mut ValueId| {
        if *value == patch.parameter {
            *value = patch.replacement;
        }
    };
    let rewrite_bindings = |bindings: &mut Vec<omega_abstract_operations::ValueBinding>| {
        for binding in bindings.iter_mut() {
            if binding.argument == patch.parameter {
                binding.argument = patch.replacement;
            }
        }
    };
    match operation {
        O::Call { arguments, .. } | O::BoundaryCall { arguments, .. } => {
            for argument in arguments {
                replace(argument);
            }
        }
        O::BooleanNot { operand, .. }
        | O::IntegerBitwiseNot { operand, .. }
        | O::IntegerWiden { operand, .. }
        | O::IntegerExactCast { operand, .. } => replace(operand),
        O::BooleanEqual { left, right, .. }
        | O::IntegerEqual { left, right, .. }
        | O::IntegerLessThan { left, right, .. }
        | O::IntegerLessOrEqual { left, right, .. }
        | O::IntegerBitwiseAnd { left, right, .. }
        | O::IntegerBitwiseOr { left, right, .. }
        | O::IntegerBitwiseXor { left, right, .. }
        | O::WrappingIntegerAdd { left, right, .. }
        | O::ExactIntegerAdd { left, right, .. }
        | O::SaturatingIntegerAdd { left, right, .. }
        | O::WrappingIntegerSubtract { left, right, .. }
        | O::ExactIntegerSubtract { left, right, .. }
        | O::SaturatingIntegerSubtract { left, right, .. }
        | O::WrappingIntegerMultiply { left, right, .. }
        | O::ExactIntegerMultiply { left, right, .. }
        | O::ExactIntegerDivide { left, right, .. }
        | O::ExactIntegerRemainder { left, right, .. }
        | O::WrappingIntegerDivide { left, right, .. }
        | O::WrappingIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerDivide { left, right, .. }
        | O::SaturatingIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerMultiply { left, right, .. } => {
            replace(left);
            replace(right);
        }
        O::WrappingIntegerShiftLeft { value, count, .. }
        | O::WrappingIntegerShiftRight { value, count, .. }
        | O::ExactIntegerShiftLeft { value, count, .. }
        | O::ExactIntegerShiftRight { value, count, .. } => {
            replace(value);
            replace(count);
        }
        O::Jump {
            target, bindings, ..
        } => {
            rewrite_bindings(bindings);
            if *target == patch.block {
                bindings.remove(usize::try_from(patch.position).expect("u32 fits usize"));
            }
        }
        O::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            replace(condition);
            for successor in [when_true, when_false] {
                rewrite_bindings(&mut successor.bindings);
                if successor.target == patch.block {
                    successor
                        .bindings
                        .remove(usize::try_from(patch.position).expect("u32 fits usize"));
                }
            }
        }
        O::Return { value, .. } => replace(value),
        O::EstablishPayloadlessCase { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::CallUnit { .. }
        | O::CallStructuralScalar { .. }
        | O::CallStructural { .. }
        | O::PortWrite { .. }
        | O::IntegerConstant { .. }
        | O::BooleanConstant { .. }
        | O::BooleanStructuralField { .. }
        | O::ReturnUnit { .. }
        | O::ReturnStructural { .. }
        | O::Crash { .. } => {}
    }
}

/// Independently check and construct one Boolean-evaluation rewrite.
pub fn validate_boolean_evaluation_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::ScalarConstants)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate.substitutions().is_empty()
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::ReplaceBooleanOperationWithConstant(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.location) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.location.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == patch.location.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let node = block
        .nodes
        .get(usize::try_from(patch.location.node).expect("u32 fits usize"))
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let input_observation = observation_at(input, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let input_live = reconstruct_closed_scalar_node_boundary(input, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let [provenance] = candidate.provenance() else {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    };
    let site = PsiRealizationSite::Node(patch.location);
    if provenance.input != site
        || provenance.disposition != ProvenanceDisposition::RealizedAt(site)
        || provenance.sources != node.provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }
    if provenance.fuel != node.fuel {
        return Err(OptimizationUnitValidationError::CandidateFuelMismatch);
    }
    let accepted_provenance = vec![omega_optimization_unit::ProvenanceRewrite {
        input: site,
        disposition: ProvenanceDisposition::RealizedAt(site),
        sources: node.provenance.clone(),
        fuel: node.fuel.clone(),
    }];
    let (source_operation, result, evaluated, safety_class) =
        evaluate_boolean_operation(input, function, node, candidate, patch.location)?;
    if candidate.safety_class() != safety_class {
        return Err(OptimizationUnitValidationError::CandidateSafetyClassMismatch);
    }
    if patch
        != (BooleanConstantRewrite {
            location: patch.location,
            source_operation,
            result,
            constant: evaluated,
        })
    {
        return Err(OptimizationUnitValidationError::CandidateEvaluationMismatch);
    }
    let mut output = input.clone();
    let function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.location.machine)
        .expect("candidate source function exists");
    let block = function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.location.block)
        .expect("candidate source block exists");
    let node = &mut block.nodes[usize::try_from(patch.location.node).expect("u32 fits usize")];
    node.operation = omega_abstract_operations::AbstractOperation::BooleanConstant {
        psi_operation: patch.source_operation,
        result: patch.result,
        value: patch.constant,
    };
    node.definitions = vec![ValueDefinition {
        value: patch.result,
        scalar_type: ScalarType::Boolean,
        site: ValueDefinitionSite::Node {
            block: patch.location.block,
            node: patch.location.node,
        },
    }];
    node.uses.clear();
    node.successors.clear();
    node.ownership.clear();
    function.facts = reconstruct_fact_index(function);
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
    let output_observation = observation_at(&output, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if !same_closed_scalar_observation(&input_observation, &output_observation) {
        return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
    }
    let output_live = reconstruct_closed_scalar_node_boundary(&output, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if input_live.live_out != output_live.live_out
        || output_live
            .live_in
            .iter()
            .any(|value| !input_live.live_in.contains(value))
    {
        return Err(OptimizationUnitValidationError::CandidateLiveBoundaryMismatch);
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.boolean-evaluation.v3",
        ),
        provenance: accepted_provenance,
    })
}

fn evaluate_boolean_operation(
    input: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    node: &omega_optimization_unit::OptimizationNode,
    candidate: &PsiRewriteCandidate,
    location: NodeLocation,
) -> Result<
    (
        psi_core::OperationId,
        ValueId,
        bool,
        OptimizationSafetyClass,
    ),
    OptimizationUnitValidationError,
> {
    use omega_abstract_operations::AbstractOperation as O;
    match node.operation {
        O::BooleanNot {
            psi_operation,
            result,
            operand,
        } => {
            let Some(operand_fact) = candidate
                .scalar_evaluation_witness()
                .and_then(IntegerEvaluationWitness::unary_operand)
            else {
                return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
            };
            let operand = literal_boolean_fact(function, candidate.input(), operand, operand_fact)
                .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
            Ok((
                psi_operation,
                result,
                !operand,
                OptimizationSafetyClass::ExactOperationSemantics,
            ))
        }
        O::BooleanEqual {
            psi_operation,
            result,
            left,
            right,
        } => {
            let Some((left_fact, right_fact)) = candidate
                .scalar_evaluation_witness()
                .and_then(IntegerEvaluationWitness::binary_operands)
            else {
                return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
            };
            let left = literal_boolean_fact(function, candidate.input(), left, left_fact)
                .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
            let right = literal_boolean_fact(function, candidate.input(), right, right_fact)
                .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
            Ok((
                psi_operation,
                result,
                left == right,
                OptimizationSafetyClass::ExactOperationSemantics,
            ))
        }
        O::IntegerEqual {
            psi_operation,
            result,
            left,
            right,
        }
        | O::IntegerLessThan {
            psi_operation,
            result,
            left,
            right,
        }
        | O::IntegerLessOrEqual {
            psi_operation,
            result,
            left,
            right,
        } => {
            if let Some((range_fact, constant_fact)) = candidate
                .scalar_evaluation_witness()
                .and_then(IntegerEvaluationWitness::range_against_constant)
            {
                if !candidate
                    .required_analyses()
                    .contains(AnalysisKind::ValueRanges)
                {
                    return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
                }
                let kind = independently_validated_integer_range_comparison_kind(
                    candidate.rule(),
                    &node.operation,
                )
                .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
                let (range_operand, constant_operand) = match kind {
                    ValidatedIntegerRangeComparisonKind::RangeLessThanConstant
                    | ValidatedIntegerRangeComparisonKind::RangeLessOrEqualConstant => {
                        (left, right)
                    }
                    ValidatedIntegerRangeComparisonKind::ConstantLessThanRange
                    | ValidatedIntegerRangeComparisonKind::ConstantLessOrEqualRange => {
                        (right, left)
                    }
                };
                let constant_value = direct_literal_integer_fact(
                    function,
                    candidate.input(),
                    constant_operand,
                    constant_fact,
                )
                .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
                let range = current_value_ranges::independently_reconstruct_value_range_fact_at(
                    input,
                    range_fact,
                    function.machine,
                    range_operand,
                    location.block,
                    location.node,
                )
                .ok_or(OptimizationUnitValidationError::CurrentValueRangeFactMismatch)?;
                if validator_integer_value_type(function, constant_operand)
                    != Some(range.scalar_type)
                {
                    return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
                }
                let constant = independently_evaluate_integer_range_comparison(
                    kind,
                    range.scalar_type,
                    range.minimum,
                    range.maximum,
                    constant_value,
                )
                .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
                return Ok((
                    psi_operation,
                    result,
                    constant,
                    OptimizationSafetyClass::ProofCertified,
                ));
            }
            let Some((left_fact, right_fact)) = candidate
                .scalar_evaluation_witness()
                .and_then(IntegerEvaluationWitness::binary_operands)
            else {
                return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
            };
            let left_value = literal_integer_fact(function, candidate.input(), left, left_fact)
                .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
            let right_value = literal_integer_fact(function, candidate.input(), right, right_fact)
                .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
            let left_type = validator_integer_value_type(function, left)
                .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
            if validator_integer_value_type(function, right) != Some(left_type) {
                return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
            }
            let ordering = left_type
                .compare(left_value, right_value)
                .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
            let constant = match node.operation {
                O::IntegerEqual { .. } => ordering.is_eq(),
                O::IntegerLessThan { .. } => ordering.is_lt(),
                O::IntegerLessOrEqual { .. } => !ordering.is_gt(),
                _ => unreachable!(),
            };
            Ok((
                psi_operation,
                result,
                constant,
                OptimizationSafetyClass::ExactOperationSemantics,
            ))
        }
        _ => Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValidatedIntegerRangeComparisonKind {
    RangeLessThanConstant,
    ConstantLessThanRange,
    RangeLessOrEqualConstant,
    ConstantLessOrEqualRange,
}

fn independently_validated_integer_range_comparison_kind(
    rule: OptimizationRuleIdentity,
    operation: &O,
) -> Option<ValidatedIntegerRangeComparisonKind> {
    let kind = if rule
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.integer-less-than-range-constant.v1",
        ) {
        ValidatedIntegerRangeComparisonKind::RangeLessThanConstant
    } else if rule
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.integer-less-than-constant-range.v1",
        )
    {
        ValidatedIntegerRangeComparisonKind::ConstantLessThanRange
    } else if rule
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.integer-less-or-equal-range-constant.v1",
        )
    {
        ValidatedIntegerRangeComparisonKind::RangeLessOrEqualConstant
    } else if rule
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.integer-less-or-equal-constant-range.v1",
        )
    {
        ValidatedIntegerRangeComparisonKind::ConstantLessOrEqualRange
    } else {
        return None;
    };
    match (kind, operation) {
        (
            ValidatedIntegerRangeComparisonKind::RangeLessThanConstant
            | ValidatedIntegerRangeComparisonKind::ConstantLessThanRange,
            O::IntegerLessThan { .. },
        )
        | (
            ValidatedIntegerRangeComparisonKind::RangeLessOrEqualConstant
            | ValidatedIntegerRangeComparisonKind::ConstantLessOrEqualRange,
            O::IntegerLessOrEqual { .. },
        ) => Some(kind),
        _ => None,
    }
}

fn independently_evaluate_integer_range_comparison(
    kind: ValidatedIntegerRangeComparisonKind,
    scalar_type: psi_core::IntegerType,
    minimum: psi_core::IntegerValue,
    maximum: psi_core::IntegerValue,
    constant: psi_core::IntegerValue,
) -> Option<bool> {
    let minimum_to_constant = scalar_type.compare(minimum, constant)?;
    let maximum_to_constant = scalar_type.compare(maximum, constant)?;
    match kind {
        ValidatedIntegerRangeComparisonKind::RangeLessThanConstant => maximum_to_constant
            .is_lt()
            .then_some(true)
            .or_else(|| (!minimum_to_constant.is_lt()).then_some(false)),
        ValidatedIntegerRangeComparisonKind::ConstantLessThanRange => minimum_to_constant
            .is_gt()
            .then_some(true)
            .or_else(|| (!maximum_to_constant.is_gt()).then_some(false)),
        ValidatedIntegerRangeComparisonKind::RangeLessOrEqualConstant => (!maximum_to_constant
            .is_gt())
        .then_some(true)
        .or_else(|| minimum_to_constant.is_gt().then_some(false)),
        ValidatedIntegerRangeComparisonKind::ConstantLessOrEqualRange => (!minimum_to_constant
            .is_lt())
        .then_some(true)
        .or_else(|| maximum_to_constant.is_lt().then_some(false)),
    }
}

fn observation_at(
    unit: &PsiOptimizationUnit,
    location: omega_optimization_unit::NodeLocation,
) -> Option<PsiNodeObservation> {
    reconstruct_psi_observation_model(unit)
        .nodes
        .into_iter()
        .find(|row| {
            row.machine == location.machine
                && row.block == location.block
                && row.node == location.node
        })
}

fn same_closed_scalar_observation(input: &PsiNodeObservation, output: &PsiNodeObservation) -> bool {
    input.machine == output.machine
        && input.block == output.block
        && input.node == output.node
        && input.definitions == output.definitions
        && input.effect == output.effect
        && input.ownership == output.ownership
        && input.provenance == output.provenance
        && input.fuel == output.fuel
        && input.crash == output.crash
        && input.suspension == output.suspension
        && input.events == output.events
}

fn evaluate_integer_operation(
    function: &PsiOptimizationFunction,
    node: &omega_optimization_unit::OptimizationNode,
    candidate: &PsiRewriteCandidate,
) -> Result<
    (
        psi_core::OperationId,
        ValueId,
        psi_core::IntegerType,
        psi_core::IntegerValue,
        OptimizationSafetyClass,
    ),
    OptimizationUnitValidationError,
> {
    use omega_abstract_operations::AbstractOperation as O;
    if let O::IntegerExactCast {
        psi_operation,
        result,
        source_type,
        target_type,
        operand,
        ..
    } = node.operation
    {
        let operand_value = unary_integer_operand(function, candidate, operand)?;
        let evaluated = source_type
            .exact_cast_value_to(target_type, operand_value)
            .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
        return Ok((
            psi_operation,
            result,
            target_type,
            evaluated,
            OptimizationSafetyClass::ProofCertified,
        ));
    }
    if let O::IntegerWiden {
        psi_operation,
        result,
        source_type,
        target_type,
        operand,
    } = node.operation
    {
        let operand_value = unary_integer_operand(function, candidate, operand)?;
        let evaluated = source_type
            .widen_value_to(target_type, operand_value)
            .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
        return Ok((
            psi_operation,
            result,
            target_type,
            evaluated,
            OptimizationSafetyClass::ExactOperationSemantics,
        ));
    }
    if let O::IntegerBitwiseNot {
        psi_operation,
        result,
        scalar_type,
        operand,
    } = node.operation
    {
        let operand_value = unary_integer_operand(function, candidate, operand)?;
        let evaluated = scalar_type
            .bitwise_not(operand_value)
            .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
        return Ok((
            psi_operation,
            result,
            scalar_type,
            evaluated,
            OptimizationSafetyClass::ExactOperationSemantics,
        ));
    }
    enum IntegerOperation {
        ExactAdd,
        ExactSubtract,
        ExactMultiply,
        WrappingAdd,
        WrappingSubtract,
        WrappingMultiply,
        SaturatingAdd,
        SaturatingSubtract,
        SaturatingMultiply,
        ExactDivide,
        ExactRemainder,
        WrappingDivide,
        WrappingRemainder,
        SaturatingDivide,
        SaturatingRemainder,
        ExactShiftLeft(psi_core::IntegerType),
        ExactShiftRight(psi_core::IntegerType),
        WrappingShiftLeft(psi_core::IntegerType),
        WrappingShiftRight(psi_core::IntegerType),
        BitwiseAnd,
        BitwiseOr,
        BitwiseXor,
    }
    let (kind, source, result, scalar_type, left, right) = match &node.operation {
        O::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::ExactAdd,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::ExactSubtract,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::ExactMultiply,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::WrappingAdd,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::WrappingSubtract,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::WrappingMultiply,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::SaturatingAdd,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::SaturatingSubtract,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::SaturatingMultiply,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::ExactDivide,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::ExactRemainder,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::WrappingDivide,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::WrappingRemainder,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::SaturatingDivide,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::SaturatingRemainder,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
            ..
        } => (
            IntegerOperation::ExactShiftLeft(*count_type),
            *psi_operation,
            *result,
            *value_type,
            *value,
            *count,
        ),
        O::ExactIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
            ..
        } => (
            IntegerOperation::ExactShiftRight(*count_type),
            *psi_operation,
            *result,
            *value_type,
            *value,
            *count,
        ),
        O::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            IntegerOperation::WrappingShiftLeft(*count_type),
            *psi_operation,
            *result,
            *value_type,
            *value,
            *count,
        ),
        O::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            IntegerOperation::WrappingShiftRight(*count_type),
            *psi_operation,
            *result,
            *value_type,
            *value,
            *count,
        ),
        O::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::BitwiseAnd,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::BitwiseOr,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::BitwiseXor,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        _ => return Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    };
    let Some((left_fact, right_fact)) = candidate
        .scalar_evaluation_witness()
        .and_then(IntegerEvaluationWitness::binary_operands)
    else {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    };
    let left_value = literal_integer_fact(function, candidate.input(), left, left_fact)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let right_value = literal_integer_fact(function, candidate.input(), right, right_fact)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let (evaluated, safety_class) = match kind {
        IntegerOperation::ExactAdd => (
            scalar_type.exact_add(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::ExactSubtract => (
            scalar_type.exact_sub(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::ExactMultiply => (
            scalar_type.exact_mul(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::WrappingAdd => (
            scalar_type.wrapping_add(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::WrappingSubtract => (
            scalar_type.wrapping_sub(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::WrappingMultiply => (
            scalar_type.wrapping_mul(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::SaturatingAdd => (
            scalar_type.saturating_add(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::SaturatingSubtract => (
            scalar_type.saturating_sub(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::SaturatingMultiply => (
            scalar_type.saturating_mul(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::ExactDivide => (
            scalar_type.exact_div(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::ExactRemainder => (
            scalar_type.exact_rem(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::WrappingDivide => (
            scalar_type.wrapping_div(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::WrappingRemainder => (
            scalar_type.wrapping_rem(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::SaturatingDivide => (
            scalar_type.saturating_div(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::SaturatingRemainder => (
            scalar_type.saturating_rem(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::ExactShiftLeft(count_type) => (
            scalar_type.exact_shift_left(left_value, count_type, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::ExactShiftRight(count_type) => (
            scalar_type.exact_shift_right(left_value, count_type, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::WrappingShiftLeft(count_type) => (
            scalar_type.wrapping_shift_left(left_value, count_type, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::WrappingShiftRight(count_type) => (
            scalar_type.wrapping_shift_right(left_value, count_type, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::BitwiseAnd => (
            scalar_type.bitwise_and(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::BitwiseOr => (
            scalar_type.bitwise_or(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::BitwiseXor => (
            scalar_type.bitwise_xor(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
    };
    let evaluated =
        evaluated.ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
    Ok((source, result, scalar_type, evaluated, safety_class))
}

fn unary_integer_operand(
    function: &PsiOptimizationFunction,
    candidate: &PsiRewriteCandidate,
    operand: ValueId,
) -> Result<psi_core::IntegerValue, OptimizationUnitValidationError> {
    let Some(operand_fact) = candidate
        .scalar_evaluation_witness()
        .and_then(IntegerEvaluationWitness::unary_operand)
    else {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    };
    literal_integer_fact(function, candidate.input(), operand, operand_fact)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)
}

fn literal_integer_fact(
    function: &PsiOptimizationFunction,
    input: omega_optimization_core::OptimizationUnitIdentity,
    value: ValueId,
    identity: omega_optimization_core::ScalarConstantFactIdentity,
) -> Option<psi_core::IntegerValue> {
    validator_scalar_constant_facts(input, function)
        .into_iter()
        .find_map(|(fact_value, constant, fact_identity)| {
            (fact_value == value && fact_identity == identity)
                .then_some(constant)
                .and_then(|constant| match constant {
                    ScalarConstantValue::Integer(value) => Some(value),
                    ScalarConstantValue::Boolean(_) => None,
                })
        })
}

fn direct_literal_integer_fact(
    function: &PsiOptimizationFunction,
    input: omega_optimization_core::OptimizationUnitIdentity,
    value: ValueId,
    identity: omega_optimization_core::ScalarConstantFactIdentity,
) -> Option<psi_core::IntegerValue> {
    let definition = scalar_value_definition(function, value)?;
    let ValueDefinitionSite::Node { block, node } = definition.site else {
        return None;
    };
    let operation = &function
        .blocks
        .iter()
        .find(|candidate| candidate.id == block)?
        .nodes
        .get(usize::try_from(node).ok()?)?
        .operation;
    let O::IntegerConstant {
        psi_operation,
        result,
        scalar_type,
        value: constant,
    } = operation
    else {
        return None;
    };
    if *result != value || *scalar_type != definition.scalar_type {
        return None;
    }
    let expected = literal_scalar_constant_fact_identity(
        input,
        function.machine,
        definition,
        ScalarConstantValue::Integer(*constant),
        *psi_operation,
    )?;
    (identity == expected).then_some(*constant)
}

fn literal_boolean_fact(
    function: &PsiOptimizationFunction,
    input: omega_optimization_core::OptimizationUnitIdentity,
    value: ValueId,
    identity: omega_optimization_core::ScalarConstantFactIdentity,
) -> Option<bool> {
    validator_scalar_constant_facts(input, function)
        .into_iter()
        .find_map(|(fact_value, constant, fact_identity)| {
            (fact_value == value && fact_identity == identity)
                .then_some(constant)
                .and_then(|constant| match constant {
                    ScalarConstantValue::Boolean(value) => Some(value),
                    ScalarConstantValue::Integer(_) => None,
                })
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValidatorSccpValue {
    Unknown,
    Constant(ScalarConstantValue),
    Overdefined,
}

fn validator_scalar_constant_facts(
    input: omega_optimization_core::OptimizationUnitIdentity,
    function: &PsiOptimizationFunction,
) -> Vec<(
    ValueId,
    ScalarConstantValue,
    omega_optimization_core::ScalarConstantFactIdentity,
)> {
    fn merge(target: &mut ValidatorSccpValue, incoming: ValidatorSccpValue) -> bool {
        let next = match (*target, incoming) {
            (ValidatorSccpValue::Unknown, incoming) => incoming,
            (_, ValidatorSccpValue::Unknown) | (ValidatorSccpValue::Overdefined, _) => {
                return false;
            }
            (_, ValidatorSccpValue::Overdefined) => ValidatorSccpValue::Overdefined,
            (ValidatorSccpValue::Constant(current), ValidatorSccpValue::Constant(incoming))
                if current == incoming =>
            {
                return false;
            }
            (ValidatorSccpValue::Constant(_), ValidatorSccpValue::Constant(_)) => {
                ValidatorSccpValue::Overdefined
            }
        };
        if *target == next {
            false
        } else {
            *target = next;
            true
        }
    }

    let mut values = BTreeMap::<ValueId, ValidatorSccpValue>::new();
    for parameter in &function.parameters {
        values.insert(parameter.value, ValidatorSccpValue::Overdefined);
    }
    for block in &function.blocks {
        for parameter in &block.parameters {
            values.insert(parameter.value, ValidatorSccpValue::Unknown);
        }
        for definition in block.nodes.iter().flat_map(|node| &node.definitions) {
            values.insert(definition.value, ValidatorSccpValue::Overdefined);
        }
    }
    let support_blocks = function
        .blocks
        .iter()
        .flat_map(|block| {
            block.nodes.iter().flat_map(move |node| {
                node.provenance
                    .iter()
                    .filter_map(move |source| match source {
                        PsiProvenance::Operation(operation) => Some((*operation, block.id)),
                        PsiProvenance::Edge(_) => None,
                    })
            })
        })
        .collect::<BTreeMap<_, _>>();
    let mut literal_rows = Vec::new();
    let mut literal_support = BTreeMap::new();
    for fact in &function.facts {
        let (value, constant, support) = match fact {
            OptimizationFact::BooleanConstant {
                value,
                constant,
                support,
            } => (*value, ScalarConstantValue::Boolean(*constant), *support),
            OptimizationFact::IntegerConstant {
                value,
                constant,
                support,
            } => (*value, ScalarConstantValue::Integer(*constant), *support),
            OptimizationFact::OperationObligationReference { .. } => continue,
        };
        let block = support_blocks.get(&support).copied();
        literal_rows.push((value, constant, block));
        literal_support.insert(value, support);
        values.insert(
            value,
            if block.is_some() {
                ValidatorSccpValue::Unknown
            } else {
                ValidatorSccpValue::Constant(constant)
            },
        );
    }

    let mut reachable = BTreeSet::from([function.entry]);
    let mut feasible_edges = BTreeSet::<EdgeId>::new();
    loop {
        let mut changed = false;
        for block in &function.blocks {
            if !reachable.contains(&block.id) {
                continue;
            }
            for (value, constant, site) in &literal_rows {
                if *site == Some(block.id)
                    && matches!(values.get(value), Some(ValidatorSccpValue::Unknown))
                {
                    values.insert(*value, ValidatorSccpValue::Constant(*constant));
                    changed = true;
                }
            }
            let Some(node) = block.nodes.last() else {
                continue;
            };
            let operation_successors = validator_scalar_operation_successors(&node.operation);
            let successors = match &node.operation {
                omega_abstract_operations::AbstractOperation::Jump { .. } => {
                    operation_successors.iter().collect::<Vec<_>>()
                }
                omega_abstract_operations::AbstractOperation::Conditional {
                    condition,
                    when_true,
                    when_false,
                } => match values.get(condition) {
                    Some(ValidatorSccpValue::Constant(ScalarConstantValue::Boolean(value))) => {
                        let selected = if *value {
                            when_true.psi_edge
                        } else {
                            when_false.psi_edge
                        };
                        operation_successors
                            .iter()
                            .filter(|successor| successor.psi_edge == selected)
                            .collect()
                    }
                    Some(ValidatorSccpValue::Overdefined) => {
                        operation_successors.iter().collect::<Vec<_>>()
                    }
                    _ => Vec::new(),
                },
                _ => Vec::new(),
            };
            for successor in successors {
                changed |= feasible_edges.insert(successor.psi_edge);
                changed |= reachable.insert(successor.target);
                for binding in &successor.bindings {
                    let incoming = values
                        .get(&binding.argument)
                        .copied()
                        .unwrap_or(ValidatorSccpValue::Overdefined);
                    let target = values
                        .entry(binding.parameter)
                        .or_insert(ValidatorSccpValue::Unknown);
                    changed |= merge(target, incoming);
                }
            }
        }
        if !changed {
            break;
        }
    }

    let snapshot = validator_sccp_snapshot(function, &values, &reachable, &feasible_edges);
    values
        .into_iter()
        .filter_map(|(value, state)| {
            let ValidatorSccpValue::Constant(constant) = state else {
                return None;
            };
            let definition = scalar_value_definition(function, value)?;
            let identity = literal_support
                .get(&value)
                .and_then(|support| {
                    literal_scalar_constant_fact_identity(
                        input,
                        function.machine,
                        definition,
                        constant,
                        *support,
                    )
                })
                .or_else(|| {
                    derived_sccp_scalar_constant_fact_identity(
                        input,
                        function.machine,
                        definition,
                        constant,
                        &snapshot,
                    )
                })?;
            Some((value, constant, identity))
        })
        .collect()
}

fn validator_scalar_operation_successors(
    operation: &omega_abstract_operations::AbstractOperation,
) -> Vec<OptimizationEdge> {
    use omega_abstract_operations::AbstractOperation as O;
    match operation {
        O::Jump {
            psi_edge,
            target,
            bindings,
            trivial_affine_discards,
        } => vec![OptimizationEdge {
            psi_edge: *psi_edge,
            target: *target,
            bindings: bindings.clone(),
            trivial_affine_discards: trivial_affine_discards.clone(),
            provenance: vec![PsiProvenance::Edge(*psi_edge)],
            fuel: vec![omega_optimization_unit::FuelSettlement {
                site: PsiProvenance::Edge(*psi_edge),
                units: 1,
            }],
        }],
        O::Conditional {
            when_true,
            when_false,
            ..
        } => [when_true, when_false]
            .into_iter()
            .map(|successor| OptimizationEdge {
                psi_edge: successor.psi_edge,
                target: successor.target,
                bindings: successor.bindings.clone(),
                trivial_affine_discards: successor.trivial_affine_discards.clone(),
                provenance: vec![PsiProvenance::Edge(successor.psi_edge)],
                fuel: vec![omega_optimization_unit::FuelSettlement {
                    site: PsiProvenance::Edge(successor.psi_edge),
                    units: 1,
                }],
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn validator_sccp_snapshot(
    function: &PsiOptimizationFunction,
    values: &BTreeMap<ValueId, ValidatorSccpValue>,
    reachable: &BTreeSet<BlockId>,
    feasible_edges: &BTreeSet<EdgeId>,
) -> SccpMachineSnapshot {
    use omega_abstract_operations::AbstractOperation as O;
    let mut blocks = function
        .blocks
        .iter()
        .map(|block| SccpBlockRow {
            block: block.id,
            executable: reachable.contains(&block.id),
        })
        .collect::<Vec<_>>();
    blocks.sort_by_key(|row| row.block);
    let mut edges = function
        .blocks
        .iter()
        .flat_map(|block| {
            let reachable_source = reachable.contains(&block.id);
            block.nodes.last().into_iter().flat_map(move |node| {
                validator_scalar_operation_successors(&node.operation)
                    .into_iter()
                    .map(move |successor| {
                        let state = if feasible_edges.contains(&successor.psi_edge) {
                            SccpEdgeState::Executable
                        } else if !reachable_source {
                            SccpEdgeState::Inexecutable
                        } else if let O::Conditional { condition, .. } = &node.operation {
                            match values.get(condition) {
                                Some(ValidatorSccpValue::Constant(
                                    ScalarConstantValue::Boolean(_),
                                )) => SccpEdgeState::Inexecutable,
                                _ => SccpEdgeState::Unknown,
                            }
                        } else {
                            SccpEdgeState::Inexecutable
                        };
                        SccpEdgeRow {
                            source: block.id,
                            edge: successor.psi_edge,
                            target: successor.target,
                            state,
                        }
                    })
            })
        })
        .collect::<Vec<_>>();
    edges.sort_by_key(|row| (row.source, row.edge));
    let mut snapshot_values = values
        .iter()
        .filter_map(|(value, state)| {
            let definition = scalar_value_definition(function, *value)?;
            Some(SccpValueRow {
                definition,
                state: match state {
                    ValidatorSccpValue::Unknown => SccpValueState::Unknown,
                    ValidatorSccpValue::Constant(ScalarConstantValue::Boolean(value)) => {
                        SccpValueState::Boolean(*value)
                    }
                    ValidatorSccpValue::Constant(ScalarConstantValue::Integer(value)) => {
                        SccpValueState::Integer(*value)
                    }
                    ValidatorSccpValue::Overdefined => SccpValueState::Overdefined,
                },
            })
        })
        .collect::<Vec<_>>();
    snapshot_values.sort_by_key(|row| row.definition.value);
    SccpMachineSnapshot {
        blocks,
        edges,
        values: snapshot_values,
    }
}

fn scalar_value_definition(
    function: &PsiOptimizationFunction,
    value: ValueId,
) -> Option<ValueDefinition> {
    function
        .parameters
        .iter()
        .chain(function.blocks.iter().flat_map(|block| &block.parameters))
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .flat_map(|node| &node.definitions),
        )
        .copied()
        .find(|definition| definition.value == value)
}

fn validator_integer_value_type(
    function: &PsiOptimizationFunction,
    value: ValueId,
) -> Option<psi_core::IntegerType> {
    scalar_value_definition(function, value).and_then(|definition| match definition.scalar_type {
        ScalarType::Integer(integer) => Some(integer),
        ScalarType::Boolean => None,
    })
}

/// Independently validate both the reconstructible unit and the required
/// verifier context retained by the optimizer-facing constructor.
pub fn validate_verified_psi_optimization_unit(
    verified: &omega_psi_to_abstract_operations::VerifiedPsiOptimizationUnit,
) -> Result<(), OptimizationUnitValidationError> {
    validate_psi_optimization_unit_with_context(verified.input(), verified.unit(), true)
}

/// Validate a committed optimization revision while retaining the immutable
/// verifier context that authorized its proof and ownership facts.
///
/// Unlike [`validate_verified_psi_optimization_unit`], this permits the unit's
/// revision identity and executable shape to differ from the initial verified
/// seed. The admitted-fact projection and every surviving provenance frontier
/// must still match the original artifact exactly.
pub fn validate_transformed_psi_optimization_unit(
    input: &omega_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
    unit: &PsiOptimizationUnit,
) -> Result<(), OptimizationUnitValidationError> {
    validate_psi_optimization_unit_with_context(input, unit, false)
}

fn validate_psi_optimization_unit_with_context(
    input: &omega_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
    unit: &PsiOptimizationUnit,
    require_initial_revision: bool,
) -> Result<(), OptimizationUnitValidationError> {
    validate_psi_optimization_unit(unit)?;
    let context = input.context();
    let terminal_identity = psi_terminal_codec::terminal_psi_identity(context.module())
        .map_err(OptimizationUnitValidationError::ContextIdentity)?;
    if input.plan().psi != terminal_identity || unit.psi != terminal_identity {
        return Err(OptimizationUnitValidationError::TerminalIdentityMismatch);
    }
    let proof_fingerprint = psi_terminal_codec::proof_bundle_fingerprint(context.proof_bundle())
        .map_err(OptimizationUnitValidationError::ContextProofFingerprint)?;
    if proof_fingerprint != context.proof_bundle_fingerprint() {
        return Err(OptimizationUnitValidationError::ProofFingerprintMismatch);
    }
    let proof_questions = independently_project_proof_questions(input)
        .map_err(OptimizationUnitValidationError::ContextIdentity)?;
    if proof_questions != unit.proof_questions {
        return Err(OptimizationUnitValidationError::ProofQuestionIndexMismatch);
    }
    let ownership_frontiers = independently_project_ownership_frontiers(input)
        .ok_or(OptimizationUnitValidationError::OwnershipFrontierFactIndexMismatch)?;
    if ownership_frontiers != unit.ownership_frontier_facts {
        return Err(OptimizationUnitValidationError::OwnershipFrontierFactIndexMismatch);
    }

    let reconstructed = context
        .reconstructed_obligations()
        .obligations()
        .iter()
        .map(|row| (row.obligation.id, row))
        .collect::<BTreeMap<_, _>>();
    let accepted = context
        .accepted_facts()
        .iter()
        .map(|fact| (fact.obligation, fact))
        .collect::<BTreeMap<_, _>>();
    if reconstructed.len() != accepted.len() {
        let obligation = reconstructed
            .keys()
            .find(|id| !accepted.contains_key(id))
            .or_else(|| accepted.keys().find(|id| !reconstructed.contains_key(id)))
            .copied()
            .expect("different finite obligation maps have a differing key");
        return Err(OptimizationUnitValidationError::AcceptedObligationMismatch(
            obligation,
        ));
    }
    for (obligation, row) in &reconstructed {
        if accepted
            .get(obligation)
            .is_none_or(|fact| fact.proposition != row.obligation.proposition)
        {
            return Err(OptimizationUnitValidationError::AcceptedObligationMismatch(
                *obligation,
            ));
        }
    }

    let mut seed = omega_optimization_unit::reconstruct_psi_optimization_unit_seed(
        input.plan(),
        unit.fuel_schedule,
    )
    .map_err(|_| OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch)?;
    attach_verified_structural_context(&mut seed, context.module())?;
    if !same_immutable_signature_custody(&seed, unit) {
        return Err(OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch);
    }
    let mut projected_facts = Vec::new();
    for function in &seed.functions {
        for reference in &function.facts {
            let OptimizationFact::OperationObligationReference {
                obligation,
                support,
            } = reference
            else {
                continue;
            };
            let row = reconstructed.get(obligation).filter(|row| {
                row.owner
                    == psi_terminal_verifier::ReconstructedTerminalObligationOwner::Operation {
                        machine: function.machine,
                        operation: *support,
                    }
            });
            let fact = accepted.get(obligation);
            let (Some(row), Some(fact)) = (row, fact) else {
                return Err(
                    OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch,
                );
            };
            if row.obligation.proposition != fact.proposition {
                return Err(
                    OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch,
                );
            }
            let proposition =
                psi_terminal_codec::canonical_proposition_order_key(&fact.proposition)
                    .map_err(OptimizationUnitValidationError::ContextIdentity)?;
            projected_facts.push(omega_optimization_unit::AcceptedObligationFact::new(
                seed.psi,
                *proof_fingerprint.as_bytes(),
                function.machine,
                *support,
                *obligation,
                proposition,
            ));
        }
    }
    let projected =
        omega_optimization_unit::attach_accepted_obligation_facts(seed, projected_facts).map_err(
            |_| OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch,
        )?;
    let projected = omega_optimization_unit::attach_proof_questions(projected, proof_questions)
        .map_err(|_| OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch)?;
    let projected =
        omega_optimization_unit::attach_ownership_frontier_facts(projected, ownership_frontiers)
            .map_err(|_| {
                OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch
            })?;
    if (require_initial_revision && projected.identity != unit.identity)
        || projected.accepted_obligation_facts != unit.accepted_obligation_facts
        || projected.proof_questions != unit.proof_questions
    {
        return Err(OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch);
    }

    for function in &unit.functions {
        let Some(frontiers) = context.structural_frontiers().machine(function.machine) else {
            return Err(
                OptimizationUnitValidationError::MissingStructuralFrontierMachine(function.machine),
            );
        };
        for fact in &function.facts {
            let OptimizationFact::OperationObligationReference {
                obligation,
                support,
            } = fact
            else {
                continue;
            };
            let owner_matches = reconstructed.get(obligation).is_some_and(|row| {
                row.owner
                    == psi_terminal_verifier::ReconstructedTerminalObligationOwner::Operation {
                        machine: function.machine,
                        operation: *support,
                    }
            });
            if !owner_matches || !accepted.contains_key(obligation) {
                return Err(
                    OptimizationUnitValidationError::OperationObligationOwnerMismatch {
                        machine: function.machine,
                        operation: *support,
                        obligation: *obligation,
                    },
                );
            }
        }
        for site in function.blocks.iter().flat_map(|block| {
            block
                .nodes
                .iter()
                .flat_map(|node| node.provenance.iter().copied())
        }) {
            match site {
                PsiProvenance::Operation(operation)
                    if frontiers.operation_entry(operation).is_none()
                        || frontiers.operation_exit(operation).is_none() =>
                {
                    return Err(
                        OptimizationUnitValidationError::MissingStructuralOperationFrontier {
                            machine: function.machine,
                            operation,
                        },
                    );
                }
                PsiProvenance::Edge(edge) if frontiers.edge_entry(edge).is_none() => {
                    return Err(
                        OptimizationUnitValidationError::MissingStructuralEdgeFrontier {
                            machine: function.machine,
                            edge,
                        },
                    );
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn attach_verified_structural_context(
    unit: &mut PsiOptimizationUnit,
    module: &psi_terminal::TerminalModule,
) -> Result<(), OptimizationUnitValidationError> {
    unit.structural_domains = module.structural_domains.clone().into();
    unit.services = module.services.clone().into();
    unit.root_service_reach = module.root_service_reach.clone();
    for function in &mut unit.functions {
        let source = module
            .machines
            .iter()
            .find(|machine| machine.id == function.machine)
            .ok_or(OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch)?;
        function.structural_places = source.structural_places.clone();
        function.content_entry_claims = source.content_entry_claims.clone();
        function.verified_contract = Some(source.contract.clone());
        function.evidence_contract_lanes = module
            .evidence_contract_lanes
            .iter()
            .filter(|lane| lane.machine == function.machine)
            .cloned()
            .collect();
    }
    unit.identity = recompute_psi_optimization_unit_identity(unit);
    Ok(())
}

fn independently_project_proof_questions(
    input: &omega_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
) -> Result<Vec<ProofQuestion>, psi_terminal_codec::CodecError> {
    let context = input.context();
    let proof_fingerprint = *context.proof_bundle_fingerprint().as_bytes();
    context
        .reconstructed_obligations()
        .obligations()
        .iter()
        .map(|row| {
            let owner = match row.owner {
                psi_terminal_verifier::ReconstructedTerminalObligationOwner::Operation {
                    machine,
                    operation,
                } => ProofQuestionOwner::Operation { machine, operation },
                psi_terminal_verifier::ReconstructedTerminalObligationOwner::CallRequires {
                    machine,
                    operation,
                    requirement_position,
                } => ProofQuestionOwner::CallRequires {
                    machine,
                    operation,
                    requirement_position,
                },
                psi_terminal_verifier::ReconstructedTerminalObligationOwner::NominalCleanupRequires {
                    machine,
                    edge,
                    cleanup_position,
                    requirement_position,
                } => ProofQuestionOwner::NominalCleanupRequires {
                    machine,
                    edge,
                    cleanup_position,
                    requirement_position,
                },
                psi_terminal_verifier::ReconstructedTerminalObligationOwner::ContractEnsures {
                    machine,
                    contract,
                    clause_position,
                } => ProofQuestionOwner::ContractEnsures {
                    machine,
                    contract,
                    clause_position,
                },
            };
            let class = match row.obligation.class {
                psi_proof_admission::ObligationClass::Derivable => ProofQuestionClass::Derivable,
                psi_proof_admission::ObligationClass::AdmissionAuthorized(admission) => {
                    let kind = match admission.kind {
                        psi_proof_admission::AdmissionKind::ForeignBoundaryGuarantee => {
                            ProofQuestionAdmissionKind::ForeignBoundaryGuarantee
                        }
                        psi_proof_admission::AdmissionKind::ProviderFact => {
                            ProofQuestionAdmissionKind::ProviderFact
                        }
                        psi_proof_admission::AdmissionKind::CheckedAssemblyClaim => {
                            ProofQuestionAdmissionKind::CheckedAssemblyClaim
                        }
                    };
                    ProofQuestionClass::AdmissionAuthorized {
                        site: admission.site,
                        kind,
                        authority_identity: admission.authority_identity,
                    }
                }
            };
            let proposition =
                psi_terminal_codec::canonical_proposition_order_key(&row.obligation.proposition)?;
            let requirements = row
                .requirements
                .iter()
                .map(psi_terminal_codec::canonical_proposition_order_key)
                .collect::<Result<Vec<_>, _>>()?;
            let semantic_axioms = row
                .semantic_axioms
                .iter()
                .map(psi_terminal_codec::canonical_proposition_order_key)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ProofQuestion::new(
                input.plan().psi,
                proof_fingerprint,
                owner,
                row.obligation.id,
                class,
                proposition,
                requirements,
                semantic_axioms,
                row.canonical_certificate,
            ))
        })
        .collect()
}

fn independently_project_ownership_frontiers(
    input: &omega_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
) -> Option<Vec<OwnershipFrontierFact>> {
    let context = input.context();
    let mut facts = Vec::new();
    for machine in &context.module().machines {
        let frontiers = context.structural_frontiers().machine(machine.id)?;
        for block in &machine.blocks {
            push_independent_ownership_frontier(
                &mut facts,
                input.plan().psi,
                machine.id,
                OwnershipFrontierSite::BlockEntry(block.id),
                frontiers.block_entry(block.id)?,
            );
            for operation in &block.operations {
                push_independent_ownership_frontier(
                    &mut facts,
                    input.plan().psi,
                    machine.id,
                    OwnershipFrontierSite::OperationEntry(operation.id),
                    frontiers.operation_entry(operation.id)?,
                );
                push_independent_ownership_frontier(
                    &mut facts,
                    input.plan().psi,
                    machine.id,
                    OwnershipFrontierSite::OperationExit(operation.id),
                    frontiers.operation_exit(operation.id)?,
                );
            }
            for edge in block.terminator.edges() {
                push_independent_ownership_frontier(
                    &mut facts,
                    input.plan().psi,
                    machine.id,
                    OwnershipFrontierSite::EdgeEntry(edge),
                    frontiers.edge_entry(edge)?,
                );
                if let Some(snapshot) = frontiers.edge_exit(edge) {
                    push_independent_ownership_frontier(
                        &mut facts,
                        input.plan().psi,
                        machine.id,
                        OwnershipFrontierSite::EdgeExit(edge),
                        snapshot,
                    );
                }
            }
        }
    }
    facts.sort_by_key(|fact| (fact.machine, fact.site));
    Some(facts)
}

fn push_independent_ownership_frontier(
    facts: &mut Vec<OwnershipFrontierFact>,
    psi: psi_terminal::TerminalPsiIdentity,
    machine: MachineId,
    site: OwnershipFrontierSite,
    snapshot: &psi_terminal_verifier::VerifiedStructuralOwnershipFrontier,
) {
    facts.push(OwnershipFrontierFact::new(
        psi,
        machine,
        site,
        OwnershipFrontierSnapshot {
            claims: snapshot
                .claims()
                .iter()
                .map(|claim| OwnershipFrontierLiveClaim {
                    claim: claim.claim,
                    input: claim.input,
                    path: claim.path.clone(),
                    multiplicity: claim.multiplicity,
                })
                .collect(),
            owned_places: snapshot
                .owned_places()
                .iter()
                .map(|place| OwnershipFrontierOwnedPlace {
                    place: place.place,
                    multiplicity: place.multiplicity,
                })
                .collect(),
            partial_custody: snapshot
                .partial_custody()
                .iter()
                .map(|partial| OwnershipFrontierPartialCustody {
                    place: partial.place,
                    moved_paths: partial.moved_paths.clone(),
                })
                .collect(),
        },
    ));
}

fn same_immutable_signature_custody(
    seed: &PsiOptimizationUnit,
    unit: &PsiOptimizationUnit,
) -> bool {
    seed.psi == unit.psi
        && seed.entry == unit.entry
        && seed.structural_types == unit.structural_types
        && structural_domain_catalog_identity(seed.structural_domains.as_ref())
            == structural_domain_catalog_identity(unit.structural_domains.as_ref())
        && seed.services == unit.services
        && seed.boundary_machines == unit.boundary_machines
        && seed.provider_candidates == unit.provider_candidates
        && source_roster_partition_is_exact(seed, unit)
        && unit.functions.iter().all(|unit| {
            seed.functions
                .iter()
                .find(|seed| seed.machine == unit.machine)
                .is_some_and(|seed| {
                    seed.machine == unit.machine
                        && seed.attachment == unit.attachment
                        && seed.parameters == unit.parameters
                        && seed.structural_parameters == unit.structural_parameters
                        && seed.structural_places == unit.structural_places
                        && seed.result == unit.result
                        && seed.entry_claim_declarations == unit.entry_claim_declarations
                        && seed.content_entry_claims == unit.content_entry_claims
                        && seed.verified_contract == unit.verified_contract
                        && seed.evidence_contract_lanes == unit.evidence_contract_lanes
                        && seed.entry_claims == unit.entry_claims
                        && seed.published_service_ceiling == unit.published_service_ceiling
                })
        })
}

fn source_roster_partition_is_exact(
    seed: &PsiOptimizationUnit,
    unit: &PsiOptimizationUnit,
) -> bool {
    if unit
        .pruned_machines
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return false;
    }
    let active = unit
        .functions
        .iter()
        .map(|function| function.machine)
        .collect::<BTreeSet<_>>();
    let pruned = unit
        .pruned_machines
        .iter()
        .map(|row| (row.source_ordinal, row.machine))
        .collect::<BTreeMap<_, _>>();
    if active.len() != unit.functions.len() || active.len() + pruned.len() != seed.functions.len() {
        return false;
    }
    let mut active_order = unit.functions.iter().map(|function| function.machine);
    for (ordinal, source) in seed.functions.iter().enumerate() {
        let ordinal = u32::try_from(ordinal).ok();
        if active.contains(&source.machine) {
            if active_order.next() != Some(source.machine) {
                return false;
            }
        } else if ordinal.and_then(|ordinal| pruned.get(&ordinal).copied()) != Some(source.machine)
        {
            return false;
        }
    }
    active_order.next().is_none()
}

fn index_structural_catalogs(
    unit: &PsiOptimizationUnit,
) -> Result<
    (
        BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
        BTreeMap<StructuralDomainId, &psi_terminal::StructuralDomainDeclaration>,
    ),
    OptimizationUnitValidationError,
> {
    let mut types = BTreeMap::new();
    let mut type_names = BTreeSet::new();
    for declaration in &unit.structural_types {
        if types.insert(declaration.id, declaration).is_some() {
            return Err(OptimizationUnitValidationError::DuplicateStructuralType(
                declaration.id,
            ));
        }
        if declaration.identity.is_empty() || !type_names.insert(declaration.identity.as_str()) {
            return Err(
                OptimizationUnitValidationError::InvalidStructuralTypeIdentity(declaration.id),
            );
        }
    }
    if unit
        .structural_types
        .windows(2)
        .any(|pair| pair[0].id >= pair[1].id)
    {
        return Err(OptimizationUnitValidationError::NonCanonicalStructuralTypeOrder);
    }
    for declaration in &unit.structural_types {
        match &declaration.shape {
            psi_terminal::StructuralTypeShape::ByteSequence(
                psi_terminal::ByteSequenceCarrier::BorrowedView,
            ) => {}
            psi_terminal::StructuralTypeShape::ByteSequence(
                psi_terminal::ByteSequenceCarrier::BoundedOwned { .. },
            ) => {
                return Err(
                    OptimizationUnitValidationError::InvalidStructuralTypeIdentity(declaration.id),
                );
            }
            psi_terminal::StructuralTypeShape::FixedArray { length: 0, .. } => {
                return Err(
                    OptimizationUnitValidationError::InvalidStructuralArrayLength(declaration.id),
                );
            }
            psi_terminal::StructuralTypeShape::FixedArray { .. } => {}
            psi_terminal::StructuralTypeShape::Record { fields } => {
                validate_structural_fields(unit, declaration.id, None, fields, true)?;
            }
            psi_terminal::StructuralTypeShape::Sum { cases } => {
                validate_structural_cases(unit, declaration.id, cases)?;
            }
            psi_terminal::StructuralTypeShape::Mixed { fields, cases } => {
                validate_structural_fields(unit, declaration.id, None, fields, false)?;
                validate_structural_cases(unit, declaration.id, cases)?;
            }
        }
    }
    for declaration in &unit.structural_types {
        let referenced = match &declaration.shape {
            psi_terminal::StructuralTypeShape::ByteSequence(_) => Vec::new(),
            psi_terminal::StructuralTypeShape::Record { fields } => fields
                .iter()
                .filter_map(|field| match field.field_type {
                    psi_terminal::StructuralFieldType::Structural(target) => Some(target),
                    _ => None,
                })
                .collect(),
            psi_terminal::StructuralTypeShape::FixedArray { element, .. } => vec![*element],
            psi_terminal::StructuralTypeShape::Sum { cases } => cases
                .iter()
                .flat_map(|case| &case.fields)
                .filter_map(|field| match field.field_type {
                    psi_terminal::StructuralFieldType::Structural(target) => Some(target),
                    _ => None,
                })
                .collect(),
            psi_terminal::StructuralTypeShape::Mixed { fields, cases } => fields
                .iter()
                .chain(cases.iter().flat_map(|case| &case.fields))
                .filter_map(|field| match field.field_type {
                    psi_terminal::StructuralFieldType::Structural(target) => Some(target),
                    _ => None,
                })
                .collect(),
        };
        if let Some(target) = referenced.iter().find(|target| !types.contains_key(target)) {
            return Err(OptimizationUnitValidationError::UnknownStructuralType(
                *target,
            ));
        }
    }
    validate_structural_type_graph(&types)?;
    let mut domains = BTreeMap::new();
    let mut names = BTreeSet::new();
    let mut semantic_domains = BTreeSet::new();
    for declaration in unit.structural_domains.iter() {
        if domains.insert(declaration.id, declaration).is_some() {
            return Err(OptimizationUnitValidationError::DuplicateStructuralDomain(
                declaration.id,
            ));
        }
        if declaration.identity.is_empty()
            || !names.insert(declaration.identity.as_str())
            || !semantic_domains.insert(declaration.semantic_domain)
        {
            return Err(
                OptimizationUnitValidationError::InvalidStructuralDomainIdentity(declaration.id),
            );
        }
    }
    if unit
        .structural_domains
        .windows(2)
        .any(|pair| pair[0].id >= pair[1].id)
    {
        return Err(OptimizationUnitValidationError::NonCanonicalStructuralDomainOrder);
    }
    if let Some(carrier) = unit
        .structural_domains
        .iter()
        .map(|declaration| declaration.carrier)
        .find(|carrier| !types.contains_key(carrier))
    {
        return Err(OptimizationUnitValidationError::UnknownStructuralType(
            carrier,
        ));
    }
    for declaration in unit.structural_domains.iter() {
        if declaration
            .content_projection
            .as_ref()
            .is_some_and(|projection| {
                !validate_structural_content_projection(
                    declaration.semantic_domain,
                    declaration.carrier,
                    projection,
                    &types,
                )
            })
        {
            return Err(
                OptimizationUnitValidationError::InvalidStructuralDomainContentProjection(
                    declaration.id,
                ),
            );
        }
    }
    Ok((types, domains))
}

fn validate_content_projection_scalar(
    value: &ContentProjectionScalar,
    carrier: StructuralTypeId,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
    depth: usize,
) -> bool {
    if depth > 256 {
        return false;
    }
    match value {
        ContentProjectionScalar::SubjectField(path)
        | ContentProjectionScalar::RuntimeScalarEmbedding(path) => {
            if path.is_empty() || path.iter().any(String::is_empty) {
                return false;
            }
            let mut current = carrier;
            for (index, segment) in path.iter().enumerate() {
                let Some(declaration) = types.get(&current) else {
                    return false;
                };
                let psi_terminal::StructuralTypeShape::Record { fields } = &declaration.shape
                else {
                    return false;
                };
                let Some(field) = fields.iter().find(|field| field.identity == *segment) else {
                    return false;
                };
                let last = index + 1 == path.len();
                match (&field.field_type, last) {
                    (psi_terminal::StructuralFieldType::Structural(next), false) => {
                        current = *next;
                    }
                    (psi_terminal::StructuralFieldType::Scalar(_), true) => {}
                    _ => return false,
                }
            }
            true
        }
        ContentProjectionScalar::Natural(value) => {
            !value.is_empty()
                && value.bytes().all(|byte| byte.is_ascii_digit())
                && (value == "0" || !value.starts_with('0'))
        }
        ContentProjectionScalar::Successor(inner) => {
            validate_content_projection_scalar(inner, carrier, types, depth + 1)
        }
        ContentProjectionScalar::Add(left, right)
        | ContentProjectionScalar::Subtract(left, right)
        | ContentProjectionScalar::Multiply(left, right) => {
            validate_content_projection_scalar(left, carrier, types, depth + 1)
                && validate_content_projection_scalar(right, carrier, types, depth + 1)
        }
    }
}

fn validate_content_projection_expression(
    expression: &ContentProjectionExpression,
    carrier: StructuralTypeId,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> bool {
    match expression {
        ContentProjectionExpression::IntervalSet(members) => members.iter().all(|(start, end)| {
            validate_content_projection_scalar(start, carrier, types, 0)
                && validate_content_projection_scalar(end, carrier, types, 0)
        }),
        ContentProjectionExpression::CountedQuantity(magnitude) => {
            validate_content_projection_scalar(magnitude, carrier, types, 0)
        }
    }
}

fn validate_structural_content_projection(
    semantic_domain: psi_core::DomainSemanticId,
    carrier: StructuralTypeId,
    projection: &psi_terminal::StructuralContentProjection,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> bool {
    let shape_matches_algebra = matches!(
        (&projection.expression, projection.algebra.kind),
        (
            ContentProjectionExpression::IntervalSet(_),
            psi_core::ContentAlgebraKind::IntervalSet
        ) | (
            ContentProjectionExpression::CountedQuantity(_),
            psi_core::ContentAlgebraKind::CountedQuantity
        )
    );
    projection.identity.domain.get() == semantic_domain.get()
        && projection.identity.projection_fingerprint != 0
        && !projection.algebra.parameter.is_empty()
        && shape_matches_algebra
        && validate_content_projection_expression(&projection.expression, carrier, types)
        && psi_language_semantics::content::terminal_projection_fingerprint(
            &projection.algebra,
            &projection.expression,
        ) == projection.identity.projection_fingerprint
}

fn validate_structural_fields(
    unit: &PsiOptimizationUnit,
    structural_type: StructuralTypeId,
    case: Option<psi_core::StructuralCaseId>,
    fields: &[psi_terminal::StructuralFieldDeclaration],
    permit_provider_attachment: bool,
) -> Result<(), OptimizationUnitValidationError> {
    if fields.windows(2).any(|pair| pair[0].id >= pair[1].id) {
        return Err(
            OptimizationUnitValidationError::NonCanonicalStructuralFieldOrder {
                structural_type,
                case,
            },
        );
    }
    let mut identities = BTreeSet::new();
    for field in fields {
        if field.identity.is_empty() || !identities.insert(field.identity.as_str()) {
            return Err(
                OptimizationUnitValidationError::InvalidStructuralFieldIdentity {
                    structural_type,
                    field: field.id,
                },
            );
        }
        let invalid_erased = || OptimizationUnitValidationError::InvalidErasedStructuralField {
            structural_type,
            field: field.id,
        };
        match (&field.field_type, field.relevance) {
            (psi_terminal::StructuralFieldType::Erased { type_identity }, _)
                if type_identity.is_empty() =>
            {
                return Err(invalid_erased());
            }
            (
                psi_terminal::StructuralFieldType::Erased { .. },
                psi_terminal::BindingRelevance::Erased,
            ) => {}
            (
                psi_terminal::StructuralFieldType::Erased { .. },
                psi_terminal::BindingRelevance::Relevant,
            ) if permit_provider_attachment
                && has_provider_attachment_witness(unit, structural_type, field.id) => {}
            (
                psi_terminal::StructuralFieldType::Erased { .. },
                psi_terminal::BindingRelevance::Relevant,
            ) => return Err(invalid_erased()),
            (
                psi_terminal::StructuralFieldType::Scalar(_)
                | psi_terminal::StructuralFieldType::IeeeFloat(_)
                | psi_terminal::StructuralFieldType::Structural(_),
                psi_terminal::BindingRelevance::Erased,
            ) => return Err(invalid_erased()),
            (
                psi_terminal::StructuralFieldType::Scalar(_)
                | psi_terminal::StructuralFieldType::IeeeFloat(_)
                | psi_terminal::StructuralFieldType::ByteSequence(_)
                | psi_terminal::StructuralFieldType::Structural(_),
                psi_terminal::BindingRelevance::Relevant,
            )
            | (
                psi_terminal::StructuralFieldType::ByteSequence(_),
                psi_terminal::BindingRelevance::Erased,
            ) => {}
        }
    }
    Ok(())
}

fn validate_structural_cases(
    unit: &PsiOptimizationUnit,
    structural_type: StructuralTypeId,
    cases: &[psi_terminal::StructuralCaseDeclaration],
) -> Result<(), OptimizationUnitValidationError> {
    if cases.is_empty() {
        return Err(OptimizationUnitValidationError::EmptyStructuralSum(
            structural_type,
        ));
    }
    if cases.windows(2).any(|pair| pair[0].id >= pair[1].id) {
        return Err(
            OptimizationUnitValidationError::NonCanonicalStructuralCaseOrder(structural_type),
        );
    }
    let mut identities = BTreeSet::new();
    for case in cases {
        if case.identity.is_empty() || !identities.insert(case.identity.as_str()) {
            return Err(
                OptimizationUnitValidationError::InvalidStructuralCaseIdentity {
                    structural_type,
                    case: case.id,
                },
            );
        }
    }
    for case in cases {
        validate_structural_fields(unit, structural_type, Some(case.id), &case.fields, false)?;
    }
    Ok(())
}

fn has_provider_attachment_witness(
    unit: &PsiOptimizationUnit,
    structural_type: StructuralTypeId,
    field: psi_core::StructuralFieldId,
) -> bool {
    unit.functions.iter().any(|function| {
        function.attachment == Some(structural_type)
            && function.structural_places.iter().any(|place| {
                matches!(
                    place.kind,
                    StructuralPlaceKind::ProviderAttachment {
                        attachment,
                        field: provider_field,
                        ..
                    } if attachment == structural_type && provider_field == field
                )
            })
    })
}

fn validate_structural_type_graph(
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    fn visit(
        id: StructuralTypeId,
        types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
        active: &mut BTreeSet<StructuralTypeId>,
        complete: &mut BTreeSet<StructuralTypeId>,
    ) -> Result<(), OptimizationUnitValidationError> {
        if complete.contains(&id) {
            return Ok(());
        }
        if !active.insert(id) {
            return Err(OptimizationUnitValidationError::RecursiveStructuralType(id));
        }
        let declaration = types[&id];
        match &declaration.shape {
            psi_terminal::StructuralTypeShape::ByteSequence(_) => {}
            psi_terminal::StructuralTypeShape::Record { fields } => {
                for field in fields {
                    if let psi_terminal::StructuralFieldType::Structural(target) = field.field_type
                    {
                        visit(target, types, active, complete)?;
                    }
                }
            }
            psi_terminal::StructuralTypeShape::FixedArray { element, .. } => {
                visit(*element, types, active, complete)?;
            }
            psi_terminal::StructuralTypeShape::Sum { cases } => {
                for field in cases.iter().flat_map(|case| &case.fields) {
                    if let psi_terminal::StructuralFieldType::Structural(target) = field.field_type
                    {
                        visit(target, types, active, complete)?;
                    }
                }
            }
            psi_terminal::StructuralTypeShape::Mixed { fields, cases } => {
                for field in fields
                    .iter()
                    .chain(cases.iter().flat_map(|case| &case.fields))
                {
                    if let psi_terminal::StructuralFieldType::Structural(target) = field.field_type
                    {
                        visit(target, types, active, complete)?;
                    }
                }
            }
        }
        active.remove(&id);
        complete.insert(id);
        Ok(())
    }

    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    for id in types.keys().copied() {
        visit(id, types, &mut active, &mut complete)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ValidatorStructuralRootKey {
    Parameter(u32),
    Result,
    OperationResult(OperationId),
    ByteSequenceLiteral(u32),
    ProviderAttachment(
        StructuralTypeId,
        psi_core::StructuralFieldId,
        BoundaryMachineId,
    ),
    TrivialAffineLocal(u32),
}

fn structural_root_key(kind: StructuralPlaceKind) -> ValidatorStructuralRootKey {
    match kind {
        StructuralPlaceKind::Parameter { position, .. } => {
            ValidatorStructuralRootKey::Parameter(position)
        }
        StructuralPlaceKind::Result => ValidatorStructuralRootKey::Result,
        StructuralPlaceKind::OperationResult { producer, .. } => {
            ValidatorStructuralRootKey::OperationResult(producer)
        }
        StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal,
            ..
        } => ValidatorStructuralRootKey::ByteSequenceLiteral(declaration_ordinal),
        StructuralPlaceKind::ProviderAttachment {
            attachment,
            field,
            boundary,
        } => ValidatorStructuralRootKey::ProviderAttachment(attachment, field, boundary),
        StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal,
            ..
        } => ValidatorStructuralRootKey::TrivialAffineLocal(declaration_ordinal),
    }
}

fn validate_function_structural_catalog(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
    domains: &BTreeMap<StructuralDomainId, &psi_terminal::StructuralDomainDeclaration>,
) -> Result<
    (
        Vec<(
            psi_terminal::StructuralPlaceDeclaration,
            psi_terminal::StructuralTypeDeclaration,
        )>,
        Vec<(
            psi_terminal::StructuralPlaceDeclaration,
            psi_terminal::StructuralTypeDeclaration,
        )>,
    ),
    OptimizationUnitValidationError,
> {
    let mismatch = || OptimizationUnitValidationError::StructuralCatalogMismatch {
        machine: Some(function.machine),
    };
    if !structural_signature_matches(
        &function.structural_parameters,
        function.attachment,
        types,
        domains,
    ) {
        return Err(mismatch());
    }
    let mut parameter_places = BTreeSet::new();
    for (position, parameter) in function.structural_parameters.iter().enumerate() {
        if parameter.position != u32::try_from(position).map_err(|_| mismatch())?
            || !parameter_places.insert(parameter.place)
            || !types.contains_key(&parameter.structural_type)
            || !structural_qualifications_match(
                parameter.structural_type,
                &parameter.qualifications,
                domains,
            )
        {
            return Err(mismatch());
        }
    }
    let mut places = BTreeMap::new();
    for place in &function.structural_places {
        if places.insert(place.id, place.kind).is_some() {
            return Err(mismatch());
        }
        let known_type = match place.kind {
            StructuralPlaceKind::Parameter { position, is_self } => function
                .structural_parameters
                .get(position as usize)
                .is_some_and(|parameter| {
                    parameter.place == place.id && parameter.is_self == is_self
                }),
            StructuralPlaceKind::Result => function
                .result
                .structural()
                .is_some_and(|result| result.place == place.id),
            StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            } => {
                types.contains_key(&structural_type)
                    && function
                        .blocks
                        .iter()
                        .flat_map(|block| &block.nodes)
                        .any(|node| {
                            matches!(
                                &node.operation,
                                O::EstablishPayloadlessCase {
                                    psi_operation,
                                    result,
                                    ..
                                }
                                | O::CallStructural { psi_operation, result, .. }
                                    if *psi_operation == producer
                                        && result.place == place.id
                                        && result.structural_type == structural_type
                            )
                        })
            }
            StructuralPlaceKind::ByteSequenceLiteral {
                structural_type: _, ..
            } => true,
            StructuralPlaceKind::TrivialAffineLocal { .. } => true,
            StructuralPlaceKind::ProviderAttachment { attachment, .. } => {
                types.contains_key(&attachment) && function.attachment == Some(attachment)
            }
        };
        if !known_type {
            return Err(mismatch());
        }
    }
    for parameter in &function.structural_parameters {
        if places.get(&parameter.place)
            != Some(&StructuralPlaceKind::Parameter {
                position: parameter.position,
                is_self: parameter.is_self,
            })
        {
            return Err(mismatch());
        }
        if parameter.multiplicity == psi_terminal::StructuralMultiplicity::Linear
            && !function
                .entry_claim_declarations
                .iter()
                .any(|claim| claim.input == parameter.place)
        {
            return Err(mismatch());
        }
    }
    if let Some(result) = function.result.structural() {
        if places.get(&result.place) != Some(&StructuralPlaceKind::Result)
            || !types.contains_key(&result.structural_type)
            || !structural_qualifications_match(
                result.structural_type,
                &result.qualifications,
                domains,
            )
        {
            return Err(mismatch());
        }
    }
    for node in function.blocks.iter().flat_map(|block| &block.nodes) {
        let expected = match &node.operation {
            O::EstablishByteSequenceLiteral { place, .. } => Some((place.id, place.kind)),
            // Trivial affine locals have two faithful representations: an
            // executable establishment in Unit lowering, or an exact typed
            // tuple compressed into ReturnStructural. Their one-to-one
            // recognition is validated together below.
            O::EstablishTrivialAffineLocal { .. } => None,
            O::EstablishPayloadlessCase {
                psi_operation,
                result,
                ..
            }
            | O::CallStructural {
                psi_operation,
                result,
                ..
            } => Some((
                result.place,
                StructuralPlaceKind::OperationResult {
                    producer: *psi_operation,
                    structural_type: result.structural_type,
                },
            )),
            _ => None,
        };
        if expected.is_some_and(|(place, kind)| places.get(&place) != Some(&kind)) {
            return Err(mismatch());
        }
    }
    let mut claim_inputs = Vec::new();
    for (index, claim) in function.entry_claim_declarations.iter().enumerate() {
        let expected = ClaimId::new(
            u64::try_from(index)
                .map_err(|_| mismatch())?
                .checked_add(1)
                .ok_or_else(mismatch)?,
        )
        .ok_or_else(mismatch)?;
        let Some(parameter) = function
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == claim.input)
        else {
            return Err(mismatch());
        };
        if claim.claim != expected
            || parameter.multiplicity == psi_terminal::StructuralMultiplicity::Unrestricted
            || resolve_structural_path(types, parameter.structural_type, &claim.path).is_none()
            || claim_inputs
                .iter()
                .any(|previous: &&psi_terminal::EntryClaim| {
                    previous.input == claim.input
                        && (previous.path.starts_with(&claim.path)
                            || claim.path.starts_with(&previous.path))
                })
        {
            return Err(mismatch());
        }
        claim_inputs.push(claim);
    }
    if function
        .content_entry_claims
        .iter()
        .enumerate()
        .any(|(index, claim)| {
            let expected = u64::try_from(index)
                .ok()
                .and_then(|index| index.checked_add(1))
                .and_then(ClaimId::new);
            let structural_binding_matches = function
                .entry_claim_declarations
                .iter()
                .find(|entry| entry.claim == claim.claim)
                .is_none_or(|entry| {
                    entry.input == claim.input.root
                        && claim.input.segments
                            == entry
                                .path
                                .iter()
                                .map(|segment| match segment {
                                    psi_terminal::StructuralPathSegment::Field(identity) => {
                                        psi_core::ContentPlaceSegment::Field(identity.clone())
                                    }
                                    psi_terminal::StructuralPathSegment::FixedIndex(index) => {
                                        psi_core::ContentPlaceSegment::FixedIndex(*index)
                                    }
                                })
                                .collect::<Vec<_>>()
                });
            expected != Some(claim.claim)
                || claim.input.version != psi_core::ContentPlaceVersion::Entry
                || !parameter_places.contains(&claim.input.root)
                || claim.projections.is_empty()
                || claim.projections.windows(2).any(|pair| pair[0] >= pair[1])
                || !structural_binding_matches
        })
    {
        return Err(mismatch());
    }
    for projection in function
        .content_entry_claims
        .iter()
        .flat_map(|claim| &claim.projections)
    {
        let owner = domains.values().find_map(|domain| {
            domain
                .content_projection
                .as_ref()
                .filter(|owner| owner.identity.domain == projection.projection.domain)
        });
        if !owner.is_some_and(|owner| {
            owner.identity == projection.projection && owner.algebra == projection.algebra
        }) {
            return Err(
                OptimizationUnitValidationError::ContentProjectionOwnerMismatch(
                    projection.projection,
                ),
            );
        }
    }
    let mut byte_sequence_literals = function
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal,
                structural_type,
            } => Some((*place, declaration_ordinal, structural_type)),
            _ => None,
        })
        .collect::<Vec<_>>();
    byte_sequence_literals.sort_by_key(|(_, declaration_ordinal, _)| *declaration_ordinal);
    if byte_sequence_literals
        .iter()
        .enumerate()
        .any(|(expected, (_, declaration_ordinal, _))| {
            u32::try_from(expected).ok() != Some(*declaration_ordinal)
        })
    {
        return Err(
            OptimizationUnitValidationError::NonCanonicalByteSequenceLiterals(function.machine),
        );
    }
    let byte_sequence_literals = byte_sequence_literals
        .into_iter()
        .map(|(place, _, structural_type)| {
            let declaration = types.get(&structural_type).ok_or(
                OptimizationUnitValidationError::UnknownStructuralType(structural_type),
            )?;
            if !matches!(
                declaration.shape,
                psi_terminal::StructuralTypeShape::ByteSequence(
                    psi_terminal::ByteSequenceCarrier::BorrowedView
                )
            ) {
                return Err(
                    OptimizationUnitValidationError::ByteSequenceLiteralDeclarationRequiresBorrowedView {
                        machine: function.machine,
                        place: place.id,
                    },
                );
            }
            Ok((place, (*declaration).clone()))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut trivial_affine_locals = function
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                structural_type,
            } => Some((*place, declaration_ordinal, structural_type)),
            _ => None,
        })
        .collect::<Vec<_>>();
    trivial_affine_locals.sort_by_key(|(_, declaration_ordinal, _)| *declaration_ordinal);
    if trivial_affine_locals
        .iter()
        .enumerate()
        .any(|(expected, (_, declaration_ordinal, _))| {
            u32::try_from(expected).ok() != Some(*declaration_ordinal)
        })
    {
        return Err(
            OptimizationUnitValidationError::NonCanonicalTrivialAffineLocals(function.machine),
        );
    }
    let trivial_affine_locals = trivial_affine_locals
        .into_iter()
        .map(|(place, _, structural_type)| {
            let declaration = types.get(&structural_type).ok_or(
                OptimizationUnitValidationError::UnknownStructuralType(structural_type),
            )?;
            if !matches!(
                declaration.shape,
                psi_terminal::StructuralTypeShape::Record { ref fields } if fields.is_empty()
            ) {
                return Err(
                    OptimizationUnitValidationError::TrivialAffineLocalDeclarationRequiresEmptyRecord {
                        machine: function.machine,
                        place: place.id,
                    },
                );
            }
            Ok((place, (*declaration).clone()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((byte_sequence_literals, trivial_affine_locals))
}

fn validate_byte_sequence_literal_witnesses(
    function: &PsiOptimizationFunction,
    expected_literals: &[(
        psi_terminal::StructuralPlaceDeclaration,
        psi_terminal::StructuralTypeDeclaration,
    )],
) -> Result<(), OptimizationUnitValidationError> {
    let mut expected = expected_literals
        .iter()
        .map(|(place, structural_type)| (place.id, (*place, structural_type)))
        .collect::<BTreeMap<_, _>>();
    let mut actual = 0_usize;
    for node in function.blocks.iter().flat_map(|block| &block.nodes) {
        let O::EstablishByteSequenceLiteral {
            place,
            structural_type,
            ..
        } = &node.operation
        else {
            continue;
        };
        actual += 1;
        if expected
            .remove(&place.id)
            .is_none_or(|(expected_place, expected_type)| {
                *place != expected_place || structural_type != expected_type
            })
        {
            return Err(
                OptimizationUnitValidationError::ByteSequenceLiteralEstablishmentMismatch(
                    function.machine,
                ),
            );
        }
    }
    if actual != expected_literals.len() || !expected.is_empty() {
        return Err(
            OptimizationUnitValidationError::ByteSequenceLiteralEstablishmentMismatch(
                function.machine,
            ),
        );
    }
    Ok(())
}

fn validate_trivial_affine_local_witnesses(
    function: &PsiOptimizationFunction,
    expected_locals: &[(
        psi_terminal::StructuralPlaceDeclaration,
        psi_terminal::StructuralTypeDeclaration,
    )],
) -> Result<(), OptimizationUnitValidationError> {
    let explicit = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter_map(|node| match &node.operation {
            O::EstablishTrivialAffineLocal {
                psi_operation,
                place,
                structural_type,
            } => Some((*psi_operation, *place, structural_type)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let structural_returns = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter_map(|node| match &node.operation {
            O::ReturnStructural {
                trivial_affine_locals,
                trivial_affine_discards,
                ..
            } => Some((trivial_affine_locals, trivial_affine_discards)),
            _ => None,
        })
        .collect::<Vec<_>>();

    if !explicit.is_empty() {
        let exact = structural_returns.is_empty()
            && explicit.len() == expected_locals.len()
            && explicit.iter().zip(expected_locals).all(
                |((_, actual_place, actual_type), (expected_place, expected_type))| {
                    actual_place == expected_place && *actual_type == expected_type
                },
            );
        if !exact {
            return Err(
                OptimizationUnitValidationError::TrivialAffineLocalEstablishmentMismatch(
                    function.machine,
                ),
            );
        }
        return Ok(());
    }

    if !expected_locals.is_empty() && structural_returns.len() != 1 {
        return Err(
            OptimizationUnitValidationError::TrivialAffineLocalEstablishmentMismatch(
                function.machine,
            ),
        );
    }

    let executable_operations = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter(|node| !matches!(node.operation, O::ReturnStructural { .. }))
        .flat_map(|node| expected_provenance(&node.operation))
        .filter_map(|site| match site {
            PsiProvenance::Operation(operation) => Some(operation),
            PsiProvenance::Edge(_) => None,
        })
        .collect::<BTreeSet<_>>();

    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let O::ReturnStructural {
                source,
                trivial_affine_locals,
                trivial_affine_discards,
                ..
            } = &node.operation
            else {
                continue;
            };
            if trivial_affine_locals.is_empty()
                && trivial_affine_discards.is_empty()
                && expected_locals.is_empty()
            {
                continue;
            }
            let node_index = u32::try_from(node_index).expect("unit node index fits u32");
            let mut hidden_operations = BTreeSet::new();
            if trivial_affine_locals.len() != expected_locals.len()
                || trivial_affine_locals.iter().zip(expected_locals).any(
                    |((operation, actual_place, actual_type), (expected_place, expected_type))| {
                        actual_place != expected_place
                            || actual_type != expected_type
                            || !hidden_operations.insert(*operation)
                            || executable_operations.contains(operation)
                    },
                )
            {
                return Err(
                    OptimizationUnitValidationError::StructuralReturnTrivialAffineLocalsMismatch {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    },
                );
            }

            let Some(returned_parameter) = function.structural_parameters.first() else {
                return Err(
                    OptimizationUnitValidationError::StructuralReturnTrivialAffineShapeMismatch {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    },
                );
            };
            let Some(result) = function.result.structural() else {
                return Err(
                    OptimizationUnitValidationError::StructuralReturnTrivialAffineShapeMismatch {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    },
                );
            };
            if !function.parameters.is_empty()
                || returned_parameter.place != *source
                || returned_parameter.is_self
                || returned_parameter.multiplicity != psi_terminal::StructuralMultiplicity::Linear
                || result.multiplicity != psi_terminal::StructuralMultiplicity::Linear
                || returned_parameter.structural_type != result.structural_type
                || returned_parameter.qualifications != result.qualifications
                || returned_parameter.place == result.place
                || function
                    .structural_parameters
                    .iter()
                    .skip(1)
                    .any(|parameter| {
                        parameter.is_self
                            || parameter.multiplicity
                                != psi_terminal::StructuralMultiplicity::Affine
                            || !parameter.qualifications.is_empty()
                    })
            {
                return Err(
                    OptimizationUnitValidationError::StructuralReturnTrivialAffineShapeMismatch {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    },
                );
            }
            let expected_discards = trivial_affine_locals
                .iter()
                .rev()
                .map(|(_, local, _)| local.id)
                .chain(
                    function
                        .structural_parameters
                        .iter()
                        .skip(1)
                        .rev()
                        .map(|parameter| parameter.place),
                )
                .collect::<Vec<_>>();
            if *trivial_affine_discards != expected_discards {
                return Err(
                    OptimizationUnitValidationError::StructuralReturnAffineDiscardsMismatch {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    },
                );
            }
        }
    }
    Ok(())
}

/// Replay the exact specialization which replaces one relevant opaque Record
/// field with a canonical boundary-specific provider-root roster. These roots
/// are retained specialization witnesses, not direct boundary/Unit-call
/// structural arguments.
fn validate_provider_attachment_specialization(
    function: &PsiOptimizationFunction,
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    let provider_roots = function
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::ProviderAttachment {
                attachment,
                field,
                boundary,
            } => Some((place.id, attachment, field, boundary)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let provider_fields = function
        .attachment
        .and_then(|attachment| types.get(&attachment))
        .and_then(|attachment| match &attachment.shape {
            psi_terminal::StructuralTypeShape::Record { fields } => Some(
                fields
                    .iter()
                    .filter(|field| {
                        !field.relevance.is_erased()
                            && matches!(
                                field.field_type,
                                psi_terminal::StructuralFieldType::Erased { .. }
                            )
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .unwrap_or_default();
    if provider_fields.is_empty() && provider_roots.is_empty() {
        return Ok(());
    }

    let invalid = || {
        OptimizationUnitValidationError::InvalidProviderAttachmentSpecialization(function.machine)
    };
    let [provider_field] = provider_fields.as_slice() else {
        return Err(invalid());
    };
    let Some(attachment) = function.attachment else {
        return Err(invalid());
    };
    if provider_roots.is_empty()
        || function
            .structural_parameters
            .iter()
            .any(|parameter| parameter.is_self)
        || provider_roots.windows(2).any(|pair| pair[0].3 >= pair[1].3)
    {
        return Err(invalid());
    }

    let mut specialized_boundaries = BTreeSet::new();
    let provider_places = provider_roots
        .iter()
        .map(|(place, ..)| *place)
        .collect::<BTreeSet<_>>();
    for (_, root_attachment, field, boundary) in &provider_roots {
        let Some(boundary_declaration) = boundary_machines.get(boundary) else {
            return Err(invalid());
        };
        if *root_attachment != attachment
            || *field != provider_field.id
            || boundary_declaration.attachment.is_some()
            || !specialized_boundaries.insert(*boundary)
        {
            return Err(invalid());
        }
    }

    let mut called_boundaries = BTreeSet::new();
    for operation in function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .map(|node| &node.operation)
    {
        match operation {
            O::BoundaryCall {
                boundary,
                structural_arguments,
                ..
            } => {
                called_boundaries.insert(*boundary);
                if structural_arguments
                    .iter()
                    .any(|argument| provider_places.contains(&argument.place))
                {
                    return Err(invalid());
                }
            }
            O::CallUnit {
                structural_arguments,
                ..
            } if structural_arguments
                .iter()
                .any(|argument| provider_places.contains(&argument.place)) =>
            {
                return Err(invalid());
            }
            _ => {}
        }
    }
    if called_boundaries != specialized_boundaries {
        return Err(invalid());
    }
    Ok(())
}

fn structural_qualifications_match(
    carrier: StructuralTypeId,
    qualifications: &[StructuralDomainId],
    domains: &BTreeMap<StructuralDomainId, &psi_terminal::StructuralDomainDeclaration>,
) -> bool {
    !qualifications.windows(2).any(|pair| pair[0] >= pair[1])
        && qualifications.iter().all(|domain| {
            domains
                .get(domain)
                .is_some_and(|domain| domain.carrier == carrier)
        })
}

fn resolve_structural_path(
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
    mut structural_type: StructuralTypeId,
    path: &[psi_terminal::StructuralPathSegment],
) -> Option<StructuralTypeId> {
    types.get(&structural_type)?;
    for segment in path {
        let declaration = types.get(&structural_type)?;
        structural_type = match (segment, &declaration.shape) {
            (
                psi_terminal::StructuralPathSegment::Field(identity),
                psi_terminal::StructuralTypeShape::Record { fields },
            ) => {
                let field = fields
                    .iter()
                    .find(|field| field.identity == *identity && !field.relevance.is_erased())?;
                let psi_terminal::StructuralFieldType::Structural(next) = field.field_type else {
                    return None;
                };
                next
            }
            (
                psi_terminal::StructuralPathSegment::FixedIndex(index),
                psi_terminal::StructuralTypeShape::FixedArray { element, length },
            ) if index < length => *element,
            _ => return None,
        };
    }
    Some(structural_type)
}

fn validate_function(
    function: &PsiOptimizationFunction,
    unit_entry: MachineId,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    services: &BTreeMap<ServiceId, &psi_terminal::ServiceDeclaration>,
    structural_types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
    structural_domains: &BTreeMap<StructuralDomainId, &psi_terminal::StructuralDomainDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    if !valid_service_ceiling(&function.published_service_ceiling, services) {
        return Err(
            OptimizationUnitValidationError::InvalidFunctionServiceCeiling(function.machine),
        );
    }
    let (byte_sequence_literals, trivial_affine_locals) =
        validate_function_structural_catalog(function, structural_types, structural_domains)?;
    validate_provider_attachment_specialization(function, boundary_machines, structural_types)?;
    validate_structural_root_uniqueness(function)?;
    let indexed_entry_claims = function
        .entry_claim_declarations
        .iter()
        .map(|claim| claim.claim)
        .collect::<BTreeSet<_>>();
    if indexed_entry_claims.len() != function.entry_claim_declarations.len()
        || indexed_entry_claims != function.entry_claims
    {
        return Err(OptimizationUnitValidationError::EntryClaimIndexMismatch(
            function.machine,
        ));
    }
    let mut blocks = BTreeMap::new();
    for block in &function.blocks {
        if blocks.insert(block.id, block).is_some() {
            return Err(OptimizationUnitValidationError::DuplicateBlock {
                machine: function.machine,
                block: block.id,
            });
        }
    }
    if !blocks.contains_key(&function.entry) {
        return Err(OptimizationUnitValidationError::MissingEntryBlock {
            machine: function.machine,
            block: function.entry,
        });
    }
    if !blocks[&function.entry].parameters.is_empty() {
        return Err(OptimizationUnitValidationError::EntryBlockHasParameters {
            machine: function.machine,
            block: function.entry,
        });
    }
    validate_parameter_metadata(function)?;

    let mut edge_ids = BTreeSet::new();
    let mut predecessor = function
        .blocks
        .iter()
        .map(|block| (block.id, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut successors = function
        .blocks
        .iter()
        .map(|block| (block.id, Vec::new()))
        .collect::<BTreeMap<_, _>>();
    for block in &function.blocks {
        if block.nodes.is_empty() {
            return Err(OptimizationUnitValidationError::EmptyBlock {
                machine: function.machine,
                block: block.id,
            });
        }
        for (index, node) in block.nodes.iter().enumerate() {
            let node_index = u32::try_from(index).expect("unit node index was built as u32");
            if !provenance_matches_operation(&node.operation, &node.provenance)
                || node.definitions != expected_definitions(&node.operation, block.id, node_index)
                || node.uses != expected_uses(&node.operation, block.id, node_index)
                || !successors_match_operation(&node.operation, &node.successors)
                || node.ownership != expected_ownership(&node.operation)
            {
                return Err(OptimizationUnitValidationError::OperationMetadataMismatch {
                    machine: function.machine,
                    block: block.id,
                    node: node_index,
                });
            }
            let terminal = is_terminator(&node.operation);
            if terminal && index + 1 != block.nodes.len() {
                return Err(OptimizationUnitValidationError::TerminatorNotLast {
                    machine: function.machine,
                    block: block.id,
                });
            }
            for edge in &node.successors {
                if !blocks.contains_key(&edge.target) {
                    return Err(OptimizationUnitValidationError::UnknownSuccessor {
                        machine: function.machine,
                        block: block.id,
                        target: edge.target,
                    });
                }
                if !edge_ids.insert(edge.psi_edge) {
                    return Err(OptimizationUnitValidationError::DuplicateEdge(
                        edge.psi_edge,
                    ));
                }
                predecessor
                    .get_mut(&edge.target)
                    .expect("known target")
                    .insert(block.id);
                successors
                    .get_mut(&block.id)
                    .expect("every block has a successor row")
                    .push(edge.target);
            }
        }
        if !is_terminator(&block.nodes.last().expect("nonempty").operation) {
            return Err(OptimizationUnitValidationError::MissingTerminator {
                machine: function.machine,
                block: block.id,
            });
        }
    }

    validate_total_cfg(function, &blocks, &successors)?;

    for operation in function
        .blocks
        .iter()
        .flat_map(|block| block.nodes.iter().map(|node| &node.operation))
    {
        let matches = match (operation, &function.result) {
            (
                omega_abstract_operations::AbstractOperation::Return {
                    result,
                    scalar_type,
                    ..
                },
                omega_abstract_operations::AbstractFunctionResult::Scalar(signature),
            ) => *result == signature.value && *scalar_type == signature.scalar_type,
            (
                omega_abstract_operations::AbstractOperation::ReturnUnit { .. },
                omega_abstract_operations::AbstractFunctionResult::Unit,
            )
            | (
                omega_abstract_operations::AbstractOperation::ReturnStructural { .. },
                omega_abstract_operations::AbstractFunctionResult::Structural(_),
            ) => true,
            (
                omega_abstract_operations::AbstractOperation::Return { .. }
                | omega_abstract_operations::AbstractOperation::ReturnUnit { .. }
                | omega_abstract_operations::AbstractOperation::ReturnStructural { .. },
                _,
            ) => false,
            _ => continue,
        };
        if !matches {
            return Err(OptimizationUnitValidationError::FunctionResultMismatch(
                function.machine,
            ));
        }
    }

    validate_byte_sequence_literal_witnesses(function, &byte_sequence_literals)?;
    validate_trivial_affine_local_witnesses(function, &trivial_affine_locals)?;
    validate_structural_place_availability(function, &blocks, &predecessor)?;
    validate_structural_root_operations(function, unit_entry, structural_types)?;

    validate_provenance_fuel_effects(function)?;
    validate_fact_index(function)?;
    validate_values_and_bindings(
        function,
        &blocks,
        &predecessor,
        functions,
        boundary_machines,
        services,
        structural_types,
        structural_domains,
    )?;
    validate_places_and_claims(function)?;
    current_ownership::validate_current_ownership_frontier(
        function,
        &blocks,
        &successors,
        functions,
        boundary_machines,
        structural_types,
    )?;
    Ok(())
}

fn validate_structural_root_uniqueness(
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    let mut roots = BTreeSet::new();
    for place in &function.structural_places {
        if !roots.insert(structural_root_key(place.kind)) {
            return Err(
                OptimizationUnitValidationError::DuplicateStructuralPlaceRoot {
                    machine: function.machine,
                    kind: place.kind,
                },
            );
        }
    }
    Ok(())
}

fn validate_parameter_metadata(
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    for (position, parameter) in function.parameters.iter().enumerate() {
        let Ok(position) = u32::try_from(position) else {
            return Err(OptimizationUnitValidationError::ParameterMetadataMismatch {
                machine: function.machine,
                block: None,
            });
        };
        if parameter.site != ValueDefinitionSite::FunctionParameter(position) {
            return Err(OptimizationUnitValidationError::ParameterMetadataMismatch {
                machine: function.machine,
                block: None,
            });
        }
    }
    for block in &function.blocks {
        for (position, parameter) in block.parameters.iter().enumerate() {
            let Ok(position) = u32::try_from(position) else {
                return Err(OptimizationUnitValidationError::ParameterMetadataMismatch {
                    machine: function.machine,
                    block: Some(block.id),
                });
            };
            if parameter.site
                != (ValueDefinitionSite::BlockParameter {
                    block: block.id,
                    position,
                })
            {
                return Err(OptimizationUnitValidationError::ParameterMetadataMismatch {
                    machine: function.machine,
                    block: Some(block.id),
                });
            }
        }
    }
    Ok(())
}

fn validate_total_cfg(
    function: &PsiOptimizationFunction,
    blocks: &BTreeMap<BlockId, &omega_optimization_unit::OptimizationBlock>,
    successors: &BTreeMap<BlockId, Vec<BlockId>>,
) -> Result<(), OptimizationUnitValidationError> {
    let mut reachable = BTreeSet::new();
    let mut pending = vec![function.entry];
    while let Some(block) = pending.pop() {
        if reachable.insert(block) {
            pending.extend(successors[&block].iter().copied());
        }
    }
    if reachable.len() != blocks.len() {
        let block = blocks
            .keys()
            .find(|block| !reachable.contains(block))
            .copied()
            .expect("different block counts have an unreachable block");
        return Err(OptimizationUnitValidationError::UnreachableBlock {
            machine: function.machine,
            block,
        });
    }

    let mut indegree = blocks
        .keys()
        .copied()
        .map(|block| (block, 0usize))
        .collect::<BTreeMap<_, _>>();
    for target in successors.values().flatten() {
        *indegree.get_mut(target).expect("successor was validated") += 1;
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(block, count)| (*count == 0).then_some(*block))
        .collect::<BTreeSet<_>>();
    let mut visited = 0usize;
    while let Some(block) = ready.pop_first() {
        visited += 1;
        for target in &successors[&block] {
            let count = indegree.get_mut(target).expect("successor was validated");
            *count -= 1;
            if *count == 0 {
                ready.insert(*target);
            }
        }
    }
    if visited != blocks.len() {
        let block = indegree
            .iter()
            .find_map(|(block, count)| (*count != 0).then_some(*block))
            .expect("a cyclic graph leaves positive indegree");
        return Err(OptimizationUnitValidationError::ControlCycle {
            machine: function.machine,
            block,
        });
    }
    Ok(())
}

fn validate_fact_index(
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    let expected = reconstruct_fact_index(function);
    if expected != function.facts {
        return Err(OptimizationUnitValidationError::FactIndexMismatch(
            function.machine,
        ));
    }
    Ok(())
}

/// Every executable structural root is available only after its current
/// producer. Immutable source-frontier rows do not authorize a root at a
/// rewritten site. Compressed return-tuple locals are metadata-only and have
/// no executable producer, so they are deliberately absent from this walk.
fn validate_structural_place_availability(
    function: &PsiOptimizationFunction,
    blocks: &BTreeMap<BlockId, &omega_optimization_unit::OptimizationBlock>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> Result<(), OptimizationUnitValidationError> {
    let mut producers = BTreeMap::<PlaceId, (BlockId, u32)>::new();
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let place = match &node.operation {
                O::EstablishPayloadlessCase { result, .. } | O::CallStructural { result, .. } => {
                    Some(result.place)
                }
                O::EstablishByteSequenceLiteral { place, .. }
                | O::EstablishTrivialAffineLocal { place, .. } => Some(place.id),
                _ => None,
            };
            if let Some(place) = place {
                producers.insert(
                    place,
                    (
                        block.id,
                        u32::try_from(node_index).expect("unit node index fits u32"),
                    ),
                );
            }
        }
    }
    let dominators = dominators(function.entry, blocks.keys().copied(), predecessors);
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let node_index = u32::try_from(node_index).expect("unit node index fits u32");
            for place in operation_place_inputs(&node.operation) {
                let Some((producer_block, producer_node)) = producers.get(&place) else {
                    continue;
                };
                let available = (*producer_block == block.id && *producer_node < node_index)
                    || (*producer_block != block.id
                        && dominators
                            .get(&block.id)
                            .is_some_and(|set| set.contains(producer_block)));
                if !available {
                    return Err(
                        OptimizationUnitValidationError::StructuralPlaceNotAvailable {
                            machine: function.machine,
                            block: block.id,
                            node: node_index,
                            place,
                        },
                    );
                }
            }
        }
    }
    Ok(())
}

fn operation_place_inputs(operation: &O) -> Vec<PlaceId> {
    let mut inputs = match operation {
        O::CallUnit {
            structural_arguments,
            ..
        }
        | O::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | O::CallStructural {
            structural_arguments,
            ..
        }
        | O::BoundaryCall {
            structural_arguments,
            ..
        } => structural_arguments
            .iter()
            .map(|argument| argument.place)
            .collect(),
        O::BooleanStructuralField { source, .. } | O::ReturnStructural { source, .. } => {
            vec![*source]
        }
        _ => Vec::new(),
    };
    match operation {
        O::Return {
            cleanup_actions, ..
        }
        | O::ReturnUnit {
            cleanup_actions, ..
        } => inputs.extend(cleanup_actions.iter().map(|cleanup| match cleanup {
            psi_terminal::TerminalAffineCleanupAction::DiscardRoot(place) => *place,
            psi_terminal::TerminalAffineCleanupAction::DiscardResidual(discard) => discard.place,
            psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) => cleanup.place,
        })),
        O::ReturnStructural {
            trivial_affine_discards,
            ..
        } => inputs.extend(trivial_affine_discards.iter().copied()),
        _ => {}
    }
    inputs
}

/// Validate the closed root roles of structural observations and structural
/// returns. This is deliberately independent of the later full ownership walk:
/// it establishes which catalog roots may participate and replays every
/// observation invariant still representable after Terminal-to-Omega lowering.
fn validate_structural_root_operations(
    function: &PsiOptimizationFunction,
    unit_entry: MachineId,
    structural_types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    let place_kinds = function
        .structural_places
        .iter()
        .map(|place| (place.id, place.kind))
        .collect::<BTreeMap<_, _>>();
    let observations = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter_map(|node| match node.operation {
            O::BooleanStructuralField { source, field, .. } => Some((source, field)),
            _ => None,
        })
        .collect::<Vec<_>>();

    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let node_index = u32::try_from(node_index).expect("unit node index fits u32");
            match &node.operation {
                O::BooleanStructuralField { source, field, .. } => {
                    let valid = function.machine == unit_entry
                        && observations
                            .iter()
                            .all(|candidate| candidate == &(*source, *field))
                        && function.content_entry_claims.is_empty()
                        && function
                            .parameters
                            .iter()
                            .any(|parameter| parameter.scalar_type == ScalarType::Boolean)
                        && function
                            .entry_claim_declarations
                            .iter()
                            .all(|claim| claim.input != *source)
                        && matches!(
                            place_kinds.get(source),
                            Some(StructuralPlaceKind::Parameter { .. })
                        )
                        && function
                            .structural_parameters
                            .iter()
                            .find(|parameter| parameter.place == *source)
                            .is_some_and(|parameter| {
                                parameter.multiplicity
                                    == psi_terminal::StructuralMultiplicity::Affine
                                    && parameter.qualifications.is_empty()
                                    && parameter.access
                                        != psi_terminal::StructuralAccess::WriteOnlyBorrow
                                    && function.structural_places.iter().any(|place| {
                                        place.id == parameter.place
                                            && matches!(
                                                place.kind,
                                                StructuralPlaceKind::Parameter {
                                                    position,
                                                    is_self,
                                                } if position == parameter.position
                                                    && is_self == parameter.is_self
                                            )
                                    })
                                    && structural_types
                                        .get(&parameter.structural_type)
                                        .is_some_and(|declaration| {
                                            let psi_terminal::StructuralTypeShape::Record {
                                                fields,
                                            } = &declaration.shape
                                            else {
                                                return false;
                                            };
                                            fields.iter().any(|candidate| {
                                                candidate.id == *field
                                                    && !candidate.relevance.is_erased()
                                                    && candidate.field_type
                                                        == psi_terminal::StructuralFieldType::Scalar(
                                                            ScalarType::Boolean,
                                                        )
                                            })
                                        })
                            })
                        && every_scalar_return_nominally_cleans(function, *source);
                    if !valid {
                        return Err(
                            OptimizationUnitValidationError::InvalidBooleanStructuralField {
                                machine: function.machine,
                                block: block.id,
                                node: node_index,
                            },
                        );
                    }
                }
                O::ReturnStructural { source, .. } => {
                    let Some(signature) = function.result.structural() else {
                        return Err(
                            OptimizationUnitValidationError::StructuralReturnSourceContractMismatch {
                                machine: function.machine,
                                block: block.id,
                                node: node_index,
                            },
                        );
                    };
                    let source_contract = function
                        .structural_parameters
                        .iter()
                        .find(|parameter| {
                            parameter.place == *source
                                && matches!(
                                    place_kinds.get(source),
                                    Some(StructuralPlaceKind::Parameter { position, is_self })
                                        if *position == parameter.position
                                            && *is_self == parameter.is_self
                                )
                        })
                        .map(|parameter| {
                            (
                                parameter.structural_type,
                                parameter.multiplicity,
                                parameter.qualifications.as_slice(),
                            )
                        })
                        .or_else(|| {
                            let Some(StructuralPlaceKind::OperationResult {
                                producer,
                                structural_type,
                            }) = place_kinds.get(source).copied()
                            else {
                                return None;
                            };
                            function
                                .blocks
                                .iter()
                                .flat_map(|block| &block.nodes)
                                .find_map(|node| match &node.operation {
                                    O::EstablishPayloadlessCase {
                                        psi_operation,
                                        result,
                                        ..
                                    }
                                    | O::CallStructural {
                                        psi_operation,
                                        result,
                                        ..
                                    } if *psi_operation == producer
                                        && result.place == *source
                                        && result.structural_type == structural_type =>
                                    {
                                        Some((
                                            result.structural_type,
                                            result.multiplicity,
                                            result.qualifications.as_slice(),
                                        ))
                                    }
                                    _ => None,
                                })
                        });
                    if source_contract.is_none_or(
                        |(structural_type, multiplicity, qualifications)| {
                            structural_type != signature.structural_type
                                || multiplicity != signature.multiplicity
                                || qualifications != signature.qualifications.as_slice()
                        },
                    ) {
                        return Err(
                            OptimizationUnitValidationError::StructuralReturnSourceContractMismatch {
                                machine: function.machine,
                                block: block.id,
                                node: node_index,
                            },
                        );
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn every_scalar_return_nominally_cleans(
    function: &PsiOptimizationFunction,
    source: PlaceId,
) -> bool {
    let mut saw_return = false;
    for operation in function
        .blocks
        .iter()
        .filter_map(|block| block.nodes.last().map(|node| &node.operation))
    {
        match operation {
            O::Return {
                cleanup_actions, ..
            } => {
                saw_return = true;
                if !cleanup_actions.iter().any(|action| {
                    matches!(
                        action,
                        psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup)
                            if cleanup.place == source
                    )
                }) {
                    return false;
                }
            }
            O::ReturnUnit { .. } | O::ReturnStructural { .. } => return false,
            O::Jump { .. } | O::Conditional { .. } | O::Crash { .. } => {}
            _ => return false,
        }
    }
    saw_return
}

fn reconstruct_fact_index(function: &PsiOptimizationFunction) -> Vec<OptimizationFact> {
    use omega_abstract_operations::AbstractOperation as O;

    let mut expected = Vec::new();
    for operation in function
        .blocks
        .iter()
        .flat_map(|block| block.nodes.iter().map(|node| &node.operation))
    {
        match operation {
            O::IntegerExactCast {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerShiftLeft {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerShiftRight {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerAdd {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerSubtract {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerMultiply {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerDivide {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerRemainder {
                psi_operation,
                obligation,
                ..
            }
            | O::WrappingIntegerDivide {
                psi_operation,
                obligation,
                ..
            }
            | O::WrappingIntegerRemainder {
                psi_operation,
                obligation,
                ..
            }
            | O::SaturatingIntegerDivide {
                psi_operation,
                obligation,
                ..
            }
            | O::SaturatingIntegerRemainder {
                psi_operation,
                obligation,
                ..
            } => expected.push(OptimizationFact::OperationObligationReference {
                obligation: *obligation,
                support: *psi_operation,
            }),
            _ => {}
        }
        match operation {
            O::BooleanConstant {
                psi_operation,
                result,
                value,
            } => expected.push(OptimizationFact::BooleanConstant {
                value: *result,
                constant: *value,
                support: *psi_operation,
            }),
            O::IntegerConstant {
                psi_operation,
                result,
                value,
                ..
            } => expected.push(OptimizationFact::IntegerConstant {
                value: *result,
                constant: *value,
                support: *psi_operation,
            }),
            _ => {}
        }
    }
    expected
}

fn validate_provenance_fuel_effects(
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    let mut node_provenance = BTreeMap::<PsiProvenance, Vec<(BlockId, bool)>>::new();
    let mut edge_provenance = BTreeMap::<PsiProvenance, BTreeSet<EdgeId>>::new();
    let mut edge_shapes = BTreeMap::<EdgeId, (BlockId, BlockId)>::new();
    let mut expected_effect = 0u64;
    for block in &function.blocks {
        for (index, node) in block.nodes.iter().enumerate() {
            let index = u32::try_from(index).expect("unit node index was built as u32");
            if node.provenance.is_empty() && node.successors.is_empty() {
                return Err(OptimizationUnitValidationError::IncompleteProvenance {
                    machine: function.machine,
                    block: block.id,
                    node: index,
                });
            }
            let unique_node_sources = node.provenance.iter().copied().collect::<BTreeSet<_>>();
            if unique_node_sources.len() != node.provenance.len() {
                return Err(OptimizationUnitValidationError::DuplicateProvenance(
                    *node
                        .provenance
                        .first()
                        .expect("duplicated provenance is nonempty"),
                ));
            }
            let is_exact_terminal = node.successors.is_empty()
                && matches!(
                    node.operation,
                    O::Return { .. }
                        | O::ReturnUnit { .. }
                        | O::ReturnStructural { .. }
                        | O::Crash { .. }
                );
            for site in &node.provenance {
                if edge_provenance.contains_key(site) {
                    return Err(OptimizationUnitValidationError::DuplicateProvenance(*site));
                }
                node_provenance
                    .entry(*site)
                    .or_default()
                    .push((block.id, is_exact_terminal));
            }
            let source_sites = node.provenance.iter().copied().collect::<BTreeSet<_>>();
            let settled_sites = node
                .fuel
                .iter()
                .map(|settlement| settlement.site)
                .collect::<BTreeSet<_>>();
            if source_sites != settled_sites
                || node.fuel.len() != node.provenance.len()
                || node
                    .fuel
                    .iter()
                    .zip(&node.provenance)
                    .any(|(settlement, source)| settlement.site != *source || settlement.units != 1)
            {
                return Err(
                    OptimizationUnitValidationError::FuelDoesNotMatchProvenance {
                        machine: function.machine,
                        block: block.id,
                        node: index,
                    },
                );
            }
            for settlement in &node.fuel {
                let _ = settlement;
            }
            for edge in &node.successors {
                edge_shapes.insert(edge.psi_edge, (block.id, edge.target));
                if edge.provenance.is_empty()
                    || edge.provenance.first() != Some(&PsiProvenance::Edge(edge.psi_edge))
                    || edge
                        .provenance
                        .iter()
                        .any(|site| !matches!(site, PsiProvenance::Edge(_)))
                {
                    return Err(OptimizationUnitValidationError::IncompleteProvenance {
                        machine: function.machine,
                        block: block.id,
                        node: index,
                    });
                }
                let source_sites = edge.provenance.iter().copied().collect::<BTreeSet<_>>();
                if source_sites.len() != edge.provenance.len()
                    || node_provenance
                        .keys()
                        .any(|site| source_sites.contains(site))
                {
                    return Err(OptimizationUnitValidationError::DuplicateProvenance(
                        *edge
                            .provenance
                            .first()
                            .expect("edge provenance is nonempty"),
                    ));
                }
                if edge.fuel.len() != edge.provenance.len()
                    || edge
                        .fuel
                        .iter()
                        .zip(&edge.provenance)
                        .any(|(settlement, source)| {
                            settlement.site != *source || settlement.units != 1
                        })
                {
                    return Err(
                        OptimizationUnitValidationError::FuelDoesNotMatchProvenance {
                            machine: function.machine,
                            block: block.id,
                            node: index,
                        },
                    );
                }
                for source in &edge.provenance {
                    edge_provenance
                        .entry(*source)
                        .or_default()
                        .insert(edge.psi_edge);
                }
            }
            if node.effect.input != expected_effect || node.effect.output != expected_effect + 1 {
                return Err(OptimizationUnitValidationError::BrokenEffectChain {
                    machine: function.machine,
                    expected: expected_effect,
                    actual: node.effect.input,
                });
            }
            expected_effect += 1;
        }
    }
    let successors = function
        .blocks
        .iter()
        .map(|block| {
            (
                block.id,
                block
                    .nodes
                    .iter()
                    .flat_map(|node| node.successors.iter().map(|edge| edge.target))
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for (source, occurrences) in node_provenance {
        if occurrences.len() < 2 {
            continue;
        }
        if !matches!(source, PsiProvenance::Edge(_))
            || occurrences.iter().any(|(_, terminal)| !terminal)
        {
            return Err(OptimizationUnitValidationError::DuplicateProvenance(source));
        }
        for (index, (left, _)) in occurrences.iter().enumerate() {
            for (right, _) in &occurrences[index + 1..] {
                if left == right
                    || block_reaches(&successors, *left, *right)
                    || block_reaches(&successors, *right, *left)
                {
                    return Err(
                        OptimizationUnitValidationError::CoExecutableProvenanceOccurrences(source),
                    );
                }
            }
        }
    }
    for (source, occurrences) in edge_provenance {
        let occurrences = occurrences.into_iter().collect::<Vec<_>>();
        for (index, left) in occurrences.iter().enumerate() {
            let (_, left_target) = edge_shapes[left];
            for right in &occurrences[index + 1..] {
                let (right_owner, right_target) = edge_shapes[right];
                let (left_owner, _) = edge_shapes[left];
                if block_reaches(&successors, left_target, right_owner)
                    || block_reaches(&successors, right_target, left_owner)
                {
                    return Err(
                        OptimizationUnitValidationError::CoExecutableProvenanceOccurrences(source),
                    );
                }
            }
        }
    }
    Ok(())
}

fn block_reaches(
    successors: &BTreeMap<BlockId, Vec<BlockId>>,
    start: BlockId,
    target: BlockId,
) -> bool {
    let mut visited = BTreeSet::new();
    let mut pending = vec![start];
    while let Some(block) = pending.pop() {
        if block == target {
            return true;
        }
        if visited.insert(block) {
            pending.extend(successors.get(&block).into_iter().flatten().copied());
        }
    }
    false
}

fn validate_values_and_bindings(
    function: &PsiOptimizationFunction,
    blocks: &BTreeMap<BlockId, &omega_optimization_unit::OptimizationBlock>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    services: &BTreeMap<ServiceId, &psi_terminal::ServiceDeclaration>,
    structural_types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
    structural_domains: &BTreeMap<StructuralDomainId, &psi_terminal::StructuralDomainDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    let mut definitions = BTreeMap::new();
    for definition in function
        .parameters
        .iter()
        .chain(function.blocks.iter().flat_map(|block| {
            block
                .parameters
                .iter()
                .chain(block.nodes.iter().flat_map(|node| &node.definitions))
        }))
    {
        if definitions.insert(definition.value, *definition).is_some() {
            return Err(OptimizationUnitValidationError::DuplicateValue(
                definition.value,
            ));
        }
    }

    let dominators = dominators(function.entry, blocks.keys().copied(), predecessors);
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            for use_site in &node.uses {
                let Some(definition) = definitions.get(&use_site.value) else {
                    return Err(OptimizationUnitValidationError::UndefinedValue {
                        machine: function.machine,
                        block: block.id,
                        value: use_site.value,
                    });
                };
                match definition.site {
                    ValueDefinitionSite::FunctionParameter(_) => {}
                    ValueDefinitionSite::BlockParameter {
                        block: defining, ..
                    } => {
                        if !dominators
                            .get(&block.id)
                            .is_some_and(|set| set.contains(&defining))
                        {
                            return Err(OptimizationUnitValidationError::NondominatingValue {
                                machine: function.machine,
                                block: block.id,
                                value: use_site.value,
                            });
                        }
                    }
                    ValueDefinitionSite::Node {
                        block: defining,
                        node,
                    } if defining == block.id => {
                        if usize::try_from(node).expect("u32 fits usize") >= node_index {
                            return Err(OptimizationUnitValidationError::UseBeforeDefinition {
                                machine: function.machine,
                                block: block.id,
                                value: use_site.value,
                            });
                        }
                    }
                    ValueDefinitionSite::Node {
                        block: defining, ..
                    } => {
                        if !dominators
                            .get(&block.id)
                            .is_some_and(|set| set.contains(&defining))
                        {
                            return Err(OptimizationUnitValidationError::NondominatingValue {
                                machine: function.machine,
                                block: block.id,
                                value: use_site.value,
                            });
                        }
                    }
                }
            }
            if !operation_scalar_types_match(
                function,
                &node.operation,
                &definitions,
                functions,
                boundary_machines,
            ) {
                return Err(
                    OptimizationUnitValidationError::ScalarOperationContractMismatch {
                        machine: function.machine,
                        block: block.id,
                        node: u32::try_from(node_index).expect("unit node index fits u32"),
                    },
                );
            }
            if !operation_structural_call_contract_matches(
                function,
                &node.operation,
                functions,
                boundary_machines,
                structural_types,
                structural_domains,
            ) {
                return Err(
                    OptimizationUnitValidationError::StructuralCallContractMismatch {
                        machine: function.machine,
                        block: block.id,
                        node: u32::try_from(node_index).expect("unit node index fits u32"),
                    },
                );
            }
            if !operation_service_contract_matches(
                function,
                &node.operation,
                functions,
                boundary_machines,
                services,
            ) {
                return Err(
                    OptimizationUnitValidationError::OperationServiceContractMismatch {
                        machine: function.machine,
                        block: block.id,
                        node: u32::try_from(node_index).expect("unit node index fits u32"),
                    },
                );
            }
            for edge in &node.successors {
                let target = blocks.get(&edge.target).expect("successor validated");
                if edge.bindings.len() != target.parameters.len() {
                    return Err(OptimizationUnitValidationError::BindingArityMismatch {
                        machine: function.machine,
                        edge: edge.psi_edge,
                    });
                }
                for (binding, parameter) in edge.bindings.iter().zip(&target.parameters) {
                    let source_type = definitions
                        .get(&binding.argument)
                        .map(|row| row.scalar_type);
                    if binding.parameter != parameter.value
                        || binding.scalar_type != parameter.scalar_type
                        || source_type != Some(parameter.scalar_type)
                    {
                        return Err(OptimizationUnitValidationError::BindingTypeMismatch {
                            machine: function.machine,
                            edge: edge.psi_edge,
                            value: binding.argument,
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

fn operation_service_contract_matches(
    caller: &PsiOptimizationFunction,
    operation: &O,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundaries: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    services: &BTreeMap<ServiceId, &psi_terminal::ServiceDeclaration>,
) -> bool {
    let reached_is_published = |reached: &[ServiceId]| {
        reached
            .iter()
            .all(|service| caller.published_service_ceiling.contains(service))
    };
    match operation {
        O::Call { callee, .. }
        | O::CallUnit { callee, .. }
        | O::CallStructuralScalar { callee, .. }
        | O::CallStructural { callee, .. } => functions
            .get(callee)
            .is_some_and(|callee| reached_is_published(&callee.published_service_ceiling)),
        O::BoundaryCall { boundary, .. } => boundaries
            .get(boundary)
            .is_some_and(|boundary| reached_is_published(&boundary.published_service_ceiling)),
        O::PortWrite { service, .. } => {
            services.contains_key(service) && caller.published_service_ceiling.contains(service)
        }
        _ => true,
    }
}

/// Independently reconstruct the structural half of every call contract from
/// verifier-owned module/function catalogs. Call-local source/receipt rows are
/// evidence to compare, never the authority from which the expected contract
/// is inferred.
fn operation_structural_call_contract_matches(
    caller: &PsiOptimizationFunction,
    operation: &O,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
    domains: &BTreeMap<StructuralDomainId, &psi_terminal::StructuralDomainDeclaration>,
) -> bool {
    match operation {
        O::EstablishPayloadlessCase { .. } => {
            payloadless_establishment_matches(caller, operation, types)
        }
        O::CallUnit {
            callee,
            structural_arguments,
            claim_transfers,
            ..
        } => functions.get(callee).is_some_and(|callee| {
            structural_arguments_match(
                caller,
                structural_arguments,
                &callee.structural_parameters,
                types,
                StructuralProjectionPolicy::Unit,
                false,
            ) && validate_internal_claim_transfers(
                caller,
                callee,
                structural_arguments,
                claim_transfers,
            )
        }),
        O::CallStructuralScalar {
            callee,
            structural_arguments,
            claim_transfers,
            ..
        } => functions.get(callee).is_some_and(|callee| {
            structural_arguments_match(
                caller,
                structural_arguments,
                &callee.structural_parameters,
                types,
                StructuralProjectionPolicy::EmptyOnly,
                false,
            ) && validate_internal_claim_transfers(
                caller,
                callee,
                structural_arguments,
                claim_transfers,
            )
        }),
        O::CallStructural {
            result,
            callee,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            ..
        } => functions.get(callee).is_some_and(|callee| {
            structural_arguments_match(
                caller,
                structural_arguments,
                &callee.structural_parameters,
                types,
                StructuralProjectionPolicy::EmptyOnly,
                false,
            ) && validate_internal_claim_transfers(
                caller,
                callee,
                structural_arguments,
                claim_transfers,
            ) && validate_structural_call_result(
                result,
                callee,
                exact_payloadless_structural_call(operation, callee, types),
                claim_transfers,
                returned_claim_transfers,
                types,
            ) && payloadless_selected_evidence_surface_matches(operation, callee, types)
        }),
        O::BoundaryCall {
            boundary,
            structural_arguments,
            completion_claim_sources,
            completion_receipts,
            ..
        } => boundary_machines.get(boundary).is_some_and(|boundary| {
            structural_arguments_match(
                caller,
                structural_arguments,
                &boundary.structural_parameters,
                types,
                StructuralProjectionPolicy::Boundary,
                true,
            ) && boundary_requirements_match(caller, structural_arguments, boundary, domains)
                && boundary_completion_matches(
                    caller,
                    structural_arguments,
                    completion_claim_sources,
                    completion_receipts,
                )
        }),
        _ => true,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StructuralProjectionPolicy {
    Unit,
    EmptyOnly,
    Boundary,
}

#[derive(Debug, Clone, Copy)]
struct StructuralSourceContract<'a> {
    structural_type: StructuralTypeId,
    multiplicity: psi_terminal::StructuralMultiplicity,
    access: psi_terminal::StructuralAccess,
    qualifications: &'a [StructuralDomainId],
}

fn structural_arguments_match(
    caller: &PsiOptimizationFunction,
    arguments: &[psi_terminal::StructuralArgument],
    parameters: &[psi_terminal::StructuralParameterDeclaration],
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
    projection: StructuralProjectionPolicy,
    allow_byte_literal: bool,
) -> bool {
    if arguments.len() != parameters.len() {
        return false;
    }
    for (argument, parameter) in arguments.iter().zip(parameters) {
        let Some(source) = structural_source_contract(caller, argument.place, allow_byte_literal)
        else {
            return false;
        };
        let path_shape_matches = match projection {
            StructuralProjectionPolicy::Unit => {
                argument.path.is_empty()
                    || matches!(
                        argument.path.as_slice(),
                        [psi_terminal::StructuralPathSegment::FixedIndex(_)]
                    )
                    || is_nonempty_field_path(&argument.path)
            }
            StructuralProjectionPolicy::EmptyOnly => argument.path.is_empty(),
            StructuralProjectionPolicy::Boundary => true,
        };
        let Some(actual_type) =
            resolve_structural_path(types, source.structural_type, &argument.path)
        else {
            return false;
        };
        if !path_shape_matches
            || actual_type != parameter.structural_type
            || argument.access != parameter.access
            || !structural_access_can_supply(source.access, argument.access)
        {
            return false;
        }
        let unrestricted_write_only_field = is_nonempty_field_path(&argument.path)
            && argument.access == psi_terminal::StructuralAccess::WriteOnlyBorrow
            && parameter.access == psi_terminal::StructuralAccess::WriteOnlyBorrow
            && source.access == psi_terminal::StructuralAccess::WriteOnlyBorrow
            && parameter.multiplicity == psi_terminal::StructuralMultiplicity::Unrestricted
            && source.multiplicity == psi_terminal::StructuralMultiplicity::Unrestricted;
        let actual_multiplicity = if argument.path.is_empty() {
            source.multiplicity
        } else if unrestricted_write_only_field {
            psi_terminal::StructuralMultiplicity::Unrestricted
        } else if parameter.multiplicity == psi_terminal::StructuralMultiplicity::Affine
            && source.multiplicity == psi_terminal::StructuralMultiplicity::Affine
            && is_bounded_partial_affine_path(types, source.structural_type, &argument.path)
        {
            psi_terminal::StructuralMultiplicity::Affine
        } else {
            psi_terminal::StructuralMultiplicity::Linear
        };
        if actual_multiplicity != parameter.multiplicity
            || parameter.qualifications.iter().any(|qualification| {
                !argument.path.is_empty() || !source.qualifications.contains(qualification)
            })
            || (projection == StructuralProjectionPolicy::Unit
                && !argument.path.is_empty()
                && !source.qualifications.is_empty())
        {
            return false;
        }
    }
    for first in 0..arguments.len() {
        for second in first + 1..arguments.len() {
            let left = &arguments[first];
            let right = &arguments[second];
            if left.place == right.place
                && structural_paths_may_overlap(&left.path, &right.path)
                && (structural_access_is_exclusive(left.access)
                    || structural_access_is_exclusive(right.access))
            {
                return false;
            }
        }
    }
    true
}

fn structural_source_contract(
    caller: &PsiOptimizationFunction,
    place: PlaceId,
    allow_byte_literal: bool,
) -> Option<StructuralSourceContract<'_>> {
    caller
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == place)
        .map(|parameter| StructuralSourceContract {
            structural_type: parameter.structural_type,
            multiplicity: parameter.multiplicity,
            access: parameter.access,
            qualifications: &parameter.qualifications,
        })
        .or_else(|| {
            allow_byte_literal.then_some(())?;
            caller
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .find_map(|node| {
                    let O::EstablishByteSequenceLiteral {
                        place: declaration,
                        structural_type,
                        ..
                    } = &node.operation
                    else {
                        return None;
                    };
                    (declaration.id == place).then_some(StructuralSourceContract {
                        structural_type: structural_type.id,
                        multiplicity: psi_terminal::StructuralMultiplicity::Unrestricted,
                        access: psi_terminal::StructuralAccess::Owned,
                        qualifications: &[],
                    })
                })
        })
}

fn structural_access_can_supply(
    source: psi_terminal::StructuralAccess,
    presented: psi_terminal::StructuralAccess,
) -> bool {
    match source {
        psi_terminal::StructuralAccess::Owned => true,
        psi_terminal::StructuralAccess::SharedBorrow => {
            presented == psi_terminal::StructuralAccess::SharedBorrow
        }
        psi_terminal::StructuralAccess::MutableBorrow => matches!(
            presented,
            psi_terminal::StructuralAccess::SharedBorrow
                | psi_terminal::StructuralAccess::MutableBorrow
                | psi_terminal::StructuralAccess::WriteOnlyBorrow
        ),
        psi_terminal::StructuralAccess::WriteOnlyBorrow => {
            presented == psi_terminal::StructuralAccess::WriteOnlyBorrow
        }
    }
}

fn structural_access_is_exclusive(access: psi_terminal::StructuralAccess) -> bool {
    matches!(
        access,
        psi_terminal::StructuralAccess::MutableBorrow
            | psi_terminal::StructuralAccess::WriteOnlyBorrow
    )
}

fn structural_paths_may_overlap(
    left: &[psi_terminal::StructuralPathSegment],
    right: &[psi_terminal::StructuralPathSegment],
) -> bool {
    left.iter().zip(right).all(|(left, right)| left == right)
}

fn is_nonempty_field_path(path: &[psi_terminal::StructuralPathSegment]) -> bool {
    !path.is_empty()
        && path
            .iter()
            .all(|segment| matches!(segment, psi_terminal::StructuralPathSegment::Field(_)))
}

fn is_bounded_partial_affine_path(
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
    root: StructuralTypeId,
    path: &[psi_terminal::StructuralPathSegment],
) -> bool {
    is_nonempty_field_path(path)
        || (matches!(path, [psi_terminal::StructuralPathSegment::FixedIndex(_)])
            && types.get(&root).is_some_and(|declaration| {
                matches!(
                    (&declaration.shape, path),
                    (
                        psi_terminal::StructuralTypeShape::FixedArray { length: 2, .. },
                        [psi_terminal::StructuralPathSegment::FixedIndex(0 | 1)]
                    ) | (
                        psi_terminal::StructuralTypeShape::FixedArray { length: 3, .. },
                        [psi_terminal::StructuralPathSegment::FixedIndex(0 | 1 | 2)]
                    )
                )
            }))
}

fn validate_internal_claim_transfers(
    caller: &PsiOptimizationFunction,
    callee: &PsiOptimizationFunction,
    arguments: &[psi_terminal::StructuralArgument],
    transfers: &[psi_terminal::ClaimTransfer],
) -> bool {
    for (argument, parameter) in arguments.iter().zip(&callee.structural_parameters) {
        let mut caller_paths = caller
            .entry_claim_declarations
            .iter()
            .filter(|claim| claim.input == argument.place && claim.path.starts_with(&argument.path))
            .map(|claim| &claim.path[argument.path.len()..])
            .collect::<Vec<_>>();
        let mut callee_paths = callee
            .entry_claim_declarations
            .iter()
            .filter(|claim| claim.input == parameter.place)
            .map(|claim| claim.path.as_slice())
            .collect::<Vec<_>>();
        caller_paths.sort();
        callee_paths.sort();
        if caller_paths != callee_paths {
            return false;
        }
        if !argument.path.is_empty()
            && (caller
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == argument.place)
                || callee
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == parameter.place))
        {
            return false;
        }
        let mut caller_content = caller
            .content_entry_claims
            .iter()
            .filter(|claim| claim.input.root == argument.place)
            .map(|claim| (&claim.input.segments, &claim.projections))
            .collect::<Vec<_>>();
        let mut callee_content = callee
            .content_entry_claims
            .iter()
            .filter(|claim| claim.input.root == parameter.place)
            .map(|claim| (&claim.input.segments, &claim.projections))
            .collect::<Vec<_>>();
        caller_content.sort();
        callee_content.sort();
        if caller_content != callee_content {
            return false;
        }
    }
    let callee_claims = callee
        .entry_claim_declarations
        .iter()
        .map(|claim| (claim.claim, claim.input))
        .chain(
            callee
                .content_entry_claims
                .iter()
                .map(|claim| (claim.claim, claim.input.root)),
        )
        .collect::<BTreeMap<_, _>>();
    if transfers.len() != callee_claims.len()
        || transfers.windows(2).any(|pair| pair[0] >= pair[1])
        || transfers
            .iter()
            .map(|transfer| transfer.claim)
            .collect::<BTreeSet<_>>()
            .len()
            != transfers.len()
    {
        return false;
    }
    for transfer in transfers {
        let Some(argument) = arguments.get(transfer.argument_index as usize) else {
            return false;
        };
        let Some((claim_input, claim_path)) = function_claim_input(caller, transfer.claim) else {
            return false;
        };
        let target_place = callee
            .structural_parameters
            .get(transfer.argument_index as usize)
            .map(|parameter| parameter.place);
        let structural_match = claim_path.starts_with(&argument.path)
            && callee.entry_claim_declarations.iter().any(|claim| {
                Some(claim.input) == target_place && claim.path == claim_path[argument.path.len()..]
            });
        let content_match = argument.path.is_empty()
            && caller
                .content_entry_claims
                .iter()
                .any(|claim| claim.claim == transfer.claim && claim.input.root == argument.place)
            && callee
                .content_entry_claims
                .iter()
                .any(|claim| Some(claim.input.root) == target_place);
        if claim_input != argument.place || (!structural_match && !content_match) {
            return false;
        }
    }
    callee_claims.into_values().all(|input| {
        callee
            .structural_parameters
            .iter()
            .position(|parameter| parameter.place == input)
            .is_some_and(|index| {
                transfers
                    .iter()
                    .any(|transfer| transfer.argument_index as usize == index)
            })
    })
}

fn function_claim_input(
    function: &PsiOptimizationFunction,
    claim: ClaimId,
) -> Option<(PlaceId, &[psi_terminal::StructuralPathSegment])> {
    function
        .entry_claim_declarations
        .iter()
        .find_map(|candidate| {
            (candidate.claim == claim).then_some((candidate.input, candidate.path.as_slice()))
        })
        .or_else(|| {
            function.content_entry_claims.iter().find_map(|candidate| {
                (candidate.claim == claim).then_some((
                    candidate.input.root,
                    &[] as &[psi_terminal::StructuralPathSegment],
                ))
            })
        })
}

fn proposition_structural_roots(proposition: &Proposition) -> BTreeSet<PlaceId> {
    fn scalar_term_roots(term: &ScalarTerm, roots: &mut BTreeSet<PlaceId>) {
        match term {
            ScalarTerm::BooleanField { root, .. } | ScalarTerm::IntegerField { root, .. } => {
                roots.insert(*root);
            }
            ScalarTerm::BooleanNot { operand }
            | ScalarTerm::IntegerBitwiseNot { operand, .. }
            | ScalarTerm::IntegerWiden { operand, .. }
            | ScalarTerm::IntegerExactCast { operand, .. } => scalar_term_roots(operand, roots),
            ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. }
            | ScalarTerm::IntegerLessThan { left, right, .. }
            | ScalarTerm::IntegerLessOrEqual { left, right, .. }
            | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
            | ScalarTerm::IntegerBitwiseOr { left, right, .. }
            | ScalarTerm::IntegerBitwiseXor { left, right, .. }
            | ScalarTerm::ExactIntegerAdd { left, right, .. }
            | ScalarTerm::ExactIntegerSubtract { left, right, .. }
            | ScalarTerm::ExactIntegerMultiply { left, right, .. }
            | ScalarTerm::ExactIntegerDivide { left, right, .. }
            | ScalarTerm::ExactIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerDivide { left, right, .. }
            | ScalarTerm::WrappingIntegerRemainder { left, right, .. }
            | ScalarTerm::SaturatingIntegerDivide { left, right, .. }
            | ScalarTerm::SaturatingIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerAdd { left, right, .. }
            | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
            | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
            | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
            | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
            | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
                scalar_term_roots(left, roots);
                scalar_term_roots(right, roots);
            }
            ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
            | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
            | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
            | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
                scalar_term_roots(value, roots);
                scalar_term_roots(count, roots);
            }
            ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => {}
        }
    }

    fn content_term_roots(term: &ContentTerm, roots: &mut BTreeSet<PlaceId>) {
        match term {
            ContentTerm::Projection { subject, .. } => {
                roots.insert(subject.root);
            }
            ContentTerm::Separate(terms) => {
                for term in terms {
                    content_term_roots(term, roots);
                }
            }
        }
    }

    fn collect(proposition: &Proposition, roots: &mut BTreeSet<PlaceId>) {
        match proposition {
            Proposition::Equal(left, right)
            | Proposition::LessThan(left, right)
            | Proposition::LessOrEqual(left, right) => {
                scalar_term_roots(left, roots);
                scalar_term_roots(right, roots);
            }
            Proposition::IeeeFloatComparison { left, right, .. } => {
                roots.insert(left.root());
                roots.insert(right.root());
            }
            Proposition::ByteSequenceEqual { left, right } => {
                roots.insert(left.root());
                roots.insert(right.root());
            }
            Proposition::StructuralCaseMembership { subject, .. } => {
                roots.insert(subject.root());
            }
            Proposition::ContentConservation(conservation) => {
                content_term_roots(conservation.left(), roots);
                content_term_roots(conservation.right(), roots);
            }
            Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
                for proposition in propositions {
                    collect(proposition, roots);
                }
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                collect(premise, roots);
                collect(conclusion, roots);
            }
            Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => {}
        }
    }

    let mut roots = BTreeSet::new();
    collect(proposition, &mut roots);
    roots
}

fn payloadless_establishment_matches(
    function: &PsiOptimizationFunction,
    operation: &O,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> bool {
    let O::EstablishPayloadlessCase {
        psi_operation,
        result,
        result_case,
    } = operation
    else {
        return false;
    };
    function.structural_places.iter().any(|place| {
        place.id == result.place
            && matches!(
                place.kind,
                StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                } if producer == *psi_operation && structural_type == result.structural_type
            )
    }) && result.multiplicity == psi_terminal::StructuralMultiplicity::Unrestricted
        && result.qualifications.is_empty()
        && result.claims.is_empty()
        && types.get(&result.structural_type).is_some_and(|declaration| {
            matches!(
                &declaration.shape,
                psi_terminal::StructuralTypeShape::Sum { cases }
                    if cases.iter().any(|case| case.id == *result_case && case.fields.is_empty())
            )
        })
}

fn exact_payloadless_case_return_exits(
    callee: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> bool {
    let Some(signature) = callee.result.structural() else {
        return false;
    };
    if !signature.qualifications.is_empty()
        || signature.multiplicity != psi_terminal::StructuralMultiplicity::Unrestricted
        || callee
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .any(|node| {
                matches!(
                    node.operation,
                    O::Call { .. }
                        | O::CallUnit { .. }
                        | O::CallStructuralScalar { .. }
                        | O::CallStructural { .. }
                        | O::BoundaryCall { .. }
                )
            })
    {
        return false;
    }
    let mut exits = 0_usize;
    for block in &callee.blocks {
        let Some(node) = block.nodes.last() else {
            return false;
        };
        let O::ReturnStructural {
            source,
            returned_claims,
            ..
        } = &node.operation
        else {
            continue;
        };
        if !returned_claims.is_empty() {
            return false;
        }
        let Some(producer) = callee.structural_places.iter().find_map(|place| {
            (place.id == *source)
                .then_some(place.kind)
                .and_then(|kind| match kind {
                    StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type,
                    } if structural_type == signature.structural_type => Some(producer),
                    _ => None,
                })
        }) else {
            return false;
        };
        let Some(producer) = callee
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .map(|node| &node.operation)
            .find(|operation| {
                matches!(
                    operation,
                    O::EstablishPayloadlessCase { psi_operation, .. }
                        if *psi_operation == producer
                )
            })
        else {
            return false;
        };
        let O::EstablishPayloadlessCase { result, .. } = producer else {
            return false;
        };
        if result.place != *source
            || result.structural_type != signature.structural_type
            || !payloadless_establishment_matches(callee, producer, types)
        {
            return false;
        }
        exits += 1;
    }
    exits != 0
}

fn exact_payloadless_structural_call(
    operation: &O,
    callee: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> bool {
    let O::CallStructural {
        result,
        structural_arguments,
        claim_transfers,
        returned_claim_transfers,
        requirement_obligations,
        crash_continuations,
        selected_evidence: _,
        ..
    } = operation
    else {
        return false;
    };
    let Some(callee_result) = callee.result.structural() else {
        return false;
    };
    let Some(contract) = callee.verified_contract.as_ref() else {
        return false;
    };
    callee.parameters.is_empty()
        && callee.structural_parameters.is_empty()
        && callee.entry_claim_declarations.is_empty()
        && callee.content_entry_claims.is_empty()
        && contract.requires.is_empty()
        && contract.ensures.is_empty()
        && contract.crash_routes.is_empty()
        && callee.evidence_contract_lanes.is_empty()
        && structural_arguments.is_empty()
        && claim_transfers.is_empty()
        && returned_claim_transfers.is_empty()
        && requirement_obligations.is_empty()
        && crash_continuations.is_empty()
        && result.structural_type == callee_result.structural_type
        && result.multiplicity == psi_terminal::StructuralMultiplicity::Unrestricted
        && result.multiplicity == callee_result.multiplicity
        && result.qualifications.is_empty()
        && result.qualifications == callee_result.qualifications
        && result.claims.is_empty()
        && contract.outcome_specific_ensures.iter().all(|row| {
            proposition_structural_roots(&row.proposition)
                .into_iter()
                .all(|root| root == callee_result.place)
        })
        && exact_payloadless_case_return_exits(callee, types)
}

fn payloadless_selected_evidence_surface_matches(
    operation: &O,
    callee: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> bool {
    let O::CallStructural {
        selected_evidence, ..
    } = operation
    else {
        return true;
    };
    selected_evidence.is_none()
        || (exact_payloadless_structural_call(operation, callee, types)
            && callee
                .verified_contract
                .as_ref()
                .is_some_and(|contract| !contract.outcome_specific_ensures.is_empty()))
}

fn validate_structural_call_result(
    result: &psi_terminal::StructuralOperationResult,
    callee: &PsiOptimizationFunction,
    exact_payloadless: bool,
    claim_transfers: &[psi_terminal::ClaimTransfer],
    returned: &[psi_terminal::StructuralResultClaimTransfer],
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> bool {
    let Some(signature) = callee.result.structural() else {
        return false;
    };
    if result.structural_type != signature.structural_type
        || result.multiplicity != signature.multiplicity
        || result.qualifications != signature.qualifications
        || result
            .qualifications
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || result.claims.windows(2).any(|pair| pair[0] >= pair[1])
        || result.claims.iter().any(|claim| {
            resolve_structural_path(types, result.structural_type, &claim.path).is_none()
        })
        || result.claims.iter().enumerate().any(|(index, claim)| {
            result.claims[index + 1..]
                .iter()
                .any(|other| structural_paths_may_overlap(&claim.path, &other.path))
        })
    {
        return false;
    }
    if exact_payloadless {
        return true;
    }
    if callee.entry_claim_declarations.is_empty()
        || result.claims.is_empty()
        || returned.is_empty()
        || returned.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return false;
    }
    let callee_claims = callee
        .entry_claim_declarations
        .iter()
        .map(|claim| (claim.claim, claim.path.as_slice()))
        .collect::<BTreeMap<_, _>>();
    let result_claims = result
        .claims
        .iter()
        .map(|claim| (claim.claim, claim.path.as_slice()))
        .collect::<BTreeMap<_, _>>();
    let transferred = claim_transfers
        .iter()
        .map(|transfer| transfer.claim)
        .collect::<BTreeSet<_>>();
    let returned_callee = returned
        .iter()
        .map(|transfer| transfer.callee_claim)
        .collect::<BTreeSet<_>>();
    let returned_caller = returned
        .iter()
        .map(|transfer| transfer.caller_claim)
        .collect::<BTreeSet<_>>();
    callee_claims.len() == callee.entry_claim_declarations.len()
        && result_claims.len() == result.claims.len()
        && returned_callee.len() == returned.len()
        && returned_caller.len() == returned.len()
        && returned_callee == callee_claims.keys().copied().collect()
        && returned_caller == result_claims.keys().copied().collect()
        && transferred == result_claims.keys().copied().collect()
        && returned.iter().all(|transfer| {
            callee_claims.get(&transfer.callee_claim) == result_claims.get(&transfer.caller_claim)
        })
}

fn boundary_requirements_match(
    caller: &PsiOptimizationFunction,
    arguments: &[psi_terminal::StructuralArgument],
    boundary: &psi_terminal::BoundaryMachineDeclaration,
    domains: &BTreeMap<StructuralDomainId, &psi_terminal::StructuralDomainDeclaration>,
) -> bool {
    boundary.requires.windows(2).all(|pair| pair[0] < pair[1])
        && boundary.requires.iter().all(|requirement| {
            domains.contains_key(&requirement.domain)
                && arguments
                    .get(requirement.argument_index as usize)
                    .and_then(|argument| {
                        caller
                            .structural_parameters
                            .iter()
                            .find(|parameter| parameter.place == argument.place)
                    })
                    .is_some_and(|source| source.qualifications.contains(&requirement.domain))
        })
}

fn boundary_completion_matches(
    caller: &PsiOptimizationFunction,
    arguments: &[psi_terminal::StructuralArgument],
    sources: &[omega_abstract_operations::CompletionClaimSource],
    receipts: &[psi_terminal::CompletionReceipt],
) -> bool {
    let mut expected_sources = caller
        .entry_claim_declarations
        .iter()
        .cloned()
        .map(|entry| omega_abstract_operations::CompletionClaimSource {
            claim: entry.claim,
            entry: Some(entry),
            content: None,
        })
        .collect::<Vec<_>>();
    for content in &caller.content_entry_claims {
        if let Some(source) = expected_sources
            .iter_mut()
            .find(|source| source.claim == content.claim)
        {
            source.content = Some(content.clone());
        } else {
            expected_sources.push(omega_abstract_operations::CompletionClaimSource {
                claim: content.claim,
                entry: None,
                content: Some(content.clone()),
            });
        }
    }
    expected_sources.sort();
    if sources != expected_sources
        || receipts.windows(2).any(|pair| pair[0] >= pair[1])
        || receipts
            .iter()
            .map(|receipt| receipt.claim)
            .collect::<BTreeSet<_>>()
            .len()
            != receipts.len()
    {
        return false;
    }
    let expected = arguments
        .iter()
        .enumerate()
        .flat_map(|(index, argument)| {
            caller
                .entry_claim_declarations
                .iter()
                .filter_map(move |claim| {
                    (claim.input == argument.place
                        && (argument.path.is_empty() || claim.path == argument.path))
                        .then_some((index as u32, claim.claim))
                })
                .chain(caller.content_entry_claims.iter().filter_map(move |claim| {
                    (claim.input.root == argument.place).then_some((index as u32, claim.claim))
                }))
        })
        .collect::<BTreeSet<_>>();
    let actual = receipts
        .iter()
        .map(|receipt| (receipt.argument_index, receipt.claim))
        .collect::<BTreeSet<_>>();
    actual.len() == receipts.len()
        && actual == expected
        && receipts.iter().all(|receipt| {
            arguments
                .get(receipt.argument_index as usize)
                .and_then(|argument| {
                    function_claim_input(caller, receipt.claim).map(|(input, path)| {
                        input == argument.place
                            && (argument.path.is_empty() || path == argument.path.as_slice())
                    })
                })
                == Some(true)
        })
}

fn operation_scalar_types_match(
    function: &PsiOptimizationFunction,
    operation: &O,
    definitions: &BTreeMap<ValueId, ValueDefinition>,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
) -> bool {
    let scalar = |value: ValueId| definitions.get(&value).map(|row| row.scalar_type);
    let integer = |value: ValueId, expected: IntegerType| {
        scalar(value) == Some(ScalarType::Integer(expected))
    };
    let fixed = |integer: IntegerType| integer.carrier() == IntegerCarrier::Fixed;
    let binary = |left: ValueId, right: ValueId, expected: IntegerType| {
        integer(left, expected) && integer(right, expected)
    };
    match operation {
        O::EstablishPayloadlessCase { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::PortWrite { .. }
        | O::BooleanStructuralField { .. }
        | O::ReturnUnit { .. }
        | O::ReturnStructural { .. }
        | O::Crash { .. } => true,
        O::IntegerConstant {
            scalar_type, value, ..
        } => match scalar_type {
            ScalarType::Integer(integer) => integer.admits(*value),
            ScalarType::Boolean => false,
        },
        O::BooleanConstant { .. } => true,
        O::BooleanNot { operand, .. } => scalar(*operand) == Some(ScalarType::Boolean),
        O::BooleanEqual { left, right, .. } => {
            scalar(*left) == Some(ScalarType::Boolean)
                && scalar(*right) == Some(ScalarType::Boolean)
        }
        O::IntegerEqual { left, right, .. }
        | O::IntegerLessThan { left, right, .. }
        | O::IntegerLessOrEqual { left, right, .. } => {
            matches!(scalar(*left), Some(ScalarType::Integer(_))) && scalar(*left) == scalar(*right)
        }
        O::IntegerBitwiseNot {
            scalar_type,
            operand,
            ..
        } => integer(*operand, *scalar_type),
        O::IntegerWiden {
            source_type,
            target_type,
            operand,
            ..
        } => integer(*operand, *source_type) && source_type.can_widen_to(*target_type),
        O::IntegerExactCast {
            source_type,
            target_type,
            operand,
            ..
        } => {
            integer(*operand, *source_type)
                && source_type.can_exact_cast_to(*target_type)
                && !source_type.can_widen_to(*target_type)
                && source_type != target_type
        }
        O::IntegerBitwiseAnd {
            scalar_type,
            left,
            right,
            ..
        }
        | O::IntegerBitwiseOr {
            scalar_type,
            left,
            right,
            ..
        }
        | O::IntegerBitwiseXor {
            scalar_type,
            left,
            right,
            ..
        }
        | O::WrappingIntegerAdd {
            scalar_type,
            left,
            right,
            ..
        }
        | O::SaturatingIntegerAdd {
            scalar_type,
            left,
            right,
            ..
        }
        | O::WrappingIntegerSubtract {
            scalar_type,
            left,
            right,
            ..
        }
        | O::SaturatingIntegerSubtract {
            scalar_type,
            left,
            right,
            ..
        }
        | O::WrappingIntegerMultiply {
            scalar_type,
            left,
            right,
            ..
        }
        | O::SaturatingIntegerMultiply {
            scalar_type,
            left,
            right,
            ..
        } => binary(*left, *right, *scalar_type),
        O::ExactIntegerAdd {
            scalar_type,
            left,
            right,
            ..
        }
        | O::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
            ..
        }
        | O::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
            ..
        }
        | O::ExactIntegerDivide {
            scalar_type,
            left,
            right,
            ..
        }
        | O::ExactIntegerRemainder {
            scalar_type,
            left,
            right,
            ..
        }
        | O::WrappingIntegerDivide {
            scalar_type,
            left,
            right,
            ..
        }
        | O::WrappingIntegerRemainder {
            scalar_type,
            left,
            right,
            ..
        }
        | O::SaturatingIntegerDivide {
            scalar_type,
            left,
            right,
            ..
        }
        | O::SaturatingIntegerRemainder {
            scalar_type,
            left,
            right,
            ..
        } => fixed(*scalar_type) && binary(*left, *right, *scalar_type),
        O::WrappingIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
            ..
        }
        | O::WrappingIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
            ..
        } => integer(*value, *value_type) && integer(*count, *count_type),
        O::ExactIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
            ..
        }
        | O::ExactIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
            ..
        } => {
            fixed(*value_type)
                && fixed(*count_type)
                && integer(*value, *value_type)
                && integer(*count, *count_type)
        }
        O::Jump { .. } => true,
        O::Conditional { condition, .. } => scalar(*condition) == Some(ScalarType::Boolean),
        O::Return {
            result,
            value,
            scalar_type,
            ..
        } => {
            scalar(*value) == Some(*scalar_type)
                && matches!(
                    function.result,
                    omega_abstract_operations::AbstractFunctionResult::Scalar(signature)
                        if signature.value == *result && signature.scalar_type == *scalar_type
                )
        }
        O::Call {
            result: _,
            scalar_type,
            callee,
            arguments,
            ..
        } => functions.get(callee).is_some_and(|callee| {
            callee.structural_parameters.is_empty()
                && callee.declared_places.is_empty()
                && callee.entry_claim_declarations.is_empty()
                && matches!(
                    callee.result,
                    omega_abstract_operations::AbstractFunctionResult::Scalar(signature)
                        if signature.scalar_type == *scalar_type
                )
                && arguments.len() == callee.parameters.len()
                && arguments
                    .iter()
                    .zip(&callee.parameters)
                    .all(|(argument, parameter)| scalar(*argument) == Some(parameter.scalar_type))
        }),
        O::CallUnit { callee, .. } => functions.get(callee).is_some_and(|callee| {
            callee.parameters.is_empty()
                && matches!(
                    callee.result,
                    omega_abstract_operations::AbstractFunctionResult::Unit
                )
        }),
        O::CallStructuralScalar { result, callee, .. } => {
            functions.get(callee).is_some_and(|callee| {
                callee.parameters.is_empty()
                    && matches!(
                        callee.result,
                        omega_abstract_operations::AbstractFunctionResult::Scalar(signature)
                            if signature.scalar_type == result.scalar_type
                    )
            })
        }
        O::CallStructural { callee, .. } => functions.get(callee).is_some_and(|callee| {
            callee.parameters.is_empty()
                && matches!(
                    callee.result,
                    omega_abstract_operations::AbstractFunctionResult::Structural(_)
                )
        }),
        O::BoundaryCall {
            result,
            boundary,
            arguments,
            ..
        } => boundary_machines.get(boundary).is_some_and(|boundary| {
            result.as_ref().map(|result| result.scalar_type) == boundary.result
                && arguments.len() == boundary.scalar_parameters.len()
                && arguments
                    .iter()
                    .zip(&boundary.scalar_parameters)
                    .all(|(argument, parameter)| scalar(*argument) == Some(*parameter))
        }),
    }
}

fn dominators(
    entry: BlockId,
    block_ids: impl Iterator<Item = BlockId>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
    let all = block_ids.collect::<BTreeSet<_>>();
    let mut result = all
        .iter()
        .copied()
        .map(|block| {
            let initial = if block == entry {
                [entry].into_iter().collect()
            } else {
                all.clone()
            };
            (block, initial)
        })
        .collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for block in all.iter().copied().filter(|block| *block != entry) {
            let incoming = predecessors.get(&block).expect("all blocks indexed");
            let mut next = if let Some(first) = incoming.first() {
                result[first].clone()
            } else {
                BTreeSet::new()
            };
            for predecessor in incoming.iter().skip(1) {
                next = next.intersection(&result[predecessor]).copied().collect();
            }
            next.insert(block);
            if result[&block] != next {
                result.insert(block, next);
                changed = true;
            }
        }
        if !changed {
            return result;
        }
    }
}

fn validate_places_and_claims(
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    let known_places = reconstruct_declared_places(function)?;
    for parameter in &function.structural_parameters {
        if !function.declared_places.contains(&parameter.place) {
            return Err(OptimizationUnitValidationError::UnknownPlace {
                machine: function.machine,
                place: parameter.place,
            });
        }
    }
    for block in &function.blocks {
        for node in &block.nodes {
            for event in &node.ownership {
                let claims: &[ClaimId] = match event {
                    omega_optimization_unit::OwnershipEvent::ClaimTransfer(claims)
                    | omega_optimization_unit::OwnershipEvent::ClaimCompletion(claims)
                    | omega_optimization_unit::OwnershipEvent::StructuralReturn(claims)
                    | omega_optimization_unit::OwnershipEvent::CrashFrontier(claims) => claims,
                    omega_optimization_unit::OwnershipEvent::Cleanup(_) => continue,
                };
                for claim in claims {
                    if !function_has_claim(function, *claim) {
                        return Err(OptimizationUnitValidationError::UnknownClaim {
                            machine: function.machine,
                            claim: *claim,
                        });
                    }
                }
            }
        }
    }
    if known_places != function.declared_places {
        let place = known_places
            .symmetric_difference(&function.declared_places)
            .next()
            .copied()
            .expect("different place sets have a difference");
        return Err(OptimizationUnitValidationError::UnknownPlace {
            machine: function.machine,
            place,
        });
    }
    Ok(())
}

/// Terminal ownership treats ordinary and content entry claims as one live
/// claim namespace while retaining their declarations as distinct authority.
/// `entry_claims` remains the independently checked ordinary-claim index;
/// content-only claims are resolved from their complete retained catalog.
fn function_has_claim(function: &PsiOptimizationFunction, claim: ClaimId) -> bool {
    function.entry_claims.contains(&claim)
        || function
            .content_entry_claims
            .iter()
            .any(|candidate| candidate.claim == claim)
}

fn reconstruct_declared_places(
    function: &PsiOptimizationFunction,
) -> Result<BTreeSet<PlaceId>, OptimizationUnitValidationError> {
    let mut known_places = function
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .chain(
            function
                .entry_claim_declarations
                .iter()
                .map(|claim| claim.input),
        )
        .chain(function.result.structural().map(|result| result.place))
        .collect::<BTreeSet<_>>();
    for block in &function.blocks {
        for node in &block.nodes {
            match &node.operation {
                O::EstablishByteSequenceLiteral { place, .. }
                | O::EstablishTrivialAffineLocal { place, .. } => {
                    known_places.insert(place.id);
                }
                O::EstablishPayloadlessCase { result, .. } | O::CallStructural { result, .. } => {
                    known_places.insert(result.place);
                }
                _ => {}
            }
        }
    }
    for block in &function.blocks {
        for node in &block.nodes {
            validate_operation_places(function.machine, &node.operation, &known_places)?;
        }
    }
    Ok(known_places)
}

fn validate_operation_places(
    machine: MachineId,
    operation: &omega_abstract_operations::AbstractOperation,
    known: &BTreeSet<PlaceId>,
) -> Result<(), OptimizationUnitValidationError> {
    use omega_abstract_operations::AbstractOperation as O;
    let require = |place: PlaceId, known: &BTreeSet<PlaceId>| {
        if known.contains(&place) {
            Ok(())
        } else {
            Err(OptimizationUnitValidationError::UnknownPlace { machine, place })
        }
    };
    match operation {
        O::EstablishByteSequenceLiteral { .. } | O::EstablishTrivialAffineLocal { .. } => {}
        O::CallUnit {
            structural_arguments,
            ..
        }
        | O::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | O::CallStructural {
            structural_arguments,
            ..
        }
        | O::BoundaryCall {
            structural_arguments,
            ..
        } => {
            for argument in structural_arguments {
                require(argument.place, known)?;
            }
        }
        O::BooleanStructuralField { source, .. } | O::ReturnStructural { source, .. } => {
            require(*source, known)?;
        }
        O::Return {
            cleanup_actions, ..
        }
        | O::ReturnUnit {
            cleanup_actions, ..
        } => {
            for cleanup in cleanup_actions {
                let place = match cleanup {
                    psi_terminal::TerminalAffineCleanupAction::DiscardRoot(place) => *place,
                    psi_terminal::TerminalAffineCleanupAction::DiscardResidual(discard) => {
                        discard.place
                    }
                    psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                        cleanup.place
                    }
                };
                require(place, known)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn expected_definitions(
    operation: &omega_abstract_operations::AbstractOperation,
    block: BlockId,
    node: u32,
) -> Vec<ValueDefinition> {
    use omega_abstract_operations::AbstractOperation as O;
    let definition = match operation {
        O::Call {
            result,
            scalar_type,
            ..
        }
        | O::IntegerConstant {
            result,
            scalar_type,
            ..
        } => Some((*result, *scalar_type)),
        O::CallStructuralScalar { result, .. } => Some((result.value, result.scalar_type)),
        O::BoundaryCall {
            result: Some(result),
            ..
        } => Some((result.value, result.scalar_type)),
        O::BooleanConstant { result, .. }
        | O::BooleanStructuralField { result, .. }
        | O::BooleanNot { result, .. }
        | O::BooleanEqual { result, .. }
        | O::IntegerEqual { result, .. }
        | O::IntegerLessThan { result, .. }
        | O::IntegerLessOrEqual { result, .. } => Some((*result, ScalarType::Boolean)),
        O::IntegerBitwiseNot {
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseAnd {
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseOr {
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseXor {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerAdd {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerAdd {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerAdd {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerSubtract {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerSubtract {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerSubtract {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerMultiply {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerMultiply {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerDivide {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerRemainder {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerDivide {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerRemainder {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerDivide {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerRemainder {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerMultiply {
            result,
            scalar_type,
            ..
        } => Some((*result, ScalarType::Integer(*scalar_type))),
        O::IntegerWiden {
            result,
            target_type,
            ..
        }
        | O::IntegerExactCast {
            result,
            target_type,
            ..
        } => Some((*result, ScalarType::Integer(*target_type))),
        O::WrappingIntegerShiftLeft {
            result, value_type, ..
        }
        | O::WrappingIntegerShiftRight {
            result, value_type, ..
        }
        | O::ExactIntegerShiftLeft {
            result, value_type, ..
        }
        | O::ExactIntegerShiftRight {
            result, value_type, ..
        } => Some((*result, ScalarType::Integer(*value_type))),
        _ => None,
    };
    definition
        .into_iter()
        .map(|(value, scalar_type)| ValueDefinition {
            value,
            scalar_type,
            site: ValueDefinitionSite::Node { block, node },
        })
        .collect()
}

fn expected_uses(
    operation: &omega_abstract_operations::AbstractOperation,
    block: BlockId,
    node: u32,
) -> Vec<ValueUse> {
    use omega_abstract_operations::AbstractOperation as O;
    let values = match operation {
        O::Call { arguments, .. } | O::BoundaryCall { arguments, .. } => arguments.clone(),
        O::BooleanNot { operand, .. }
        | O::IntegerBitwiseNot { operand, .. }
        | O::IntegerWiden { operand, .. }
        | O::IntegerExactCast { operand, .. } => vec![*operand],
        O::BooleanEqual { left, right, .. }
        | O::IntegerEqual { left, right, .. }
        | O::IntegerLessThan { left, right, .. }
        | O::IntegerLessOrEqual { left, right, .. }
        | O::IntegerBitwiseAnd { left, right, .. }
        | O::IntegerBitwiseOr { left, right, .. }
        | O::IntegerBitwiseXor { left, right, .. }
        | O::WrappingIntegerAdd { left, right, .. }
        | O::ExactIntegerAdd { left, right, .. }
        | O::SaturatingIntegerAdd { left, right, .. }
        | O::WrappingIntegerSubtract { left, right, .. }
        | O::ExactIntegerSubtract { left, right, .. }
        | O::SaturatingIntegerSubtract { left, right, .. }
        | O::WrappingIntegerMultiply { left, right, .. }
        | O::ExactIntegerMultiply { left, right, .. }
        | O::ExactIntegerDivide { left, right, .. }
        | O::ExactIntegerRemainder { left, right, .. }
        | O::WrappingIntegerDivide { left, right, .. }
        | O::WrappingIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerDivide { left, right, .. }
        | O::SaturatingIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerMultiply { left, right, .. } => vec![*left, *right],
        O::WrappingIntegerShiftLeft { value, count, .. }
        | O::WrappingIntegerShiftRight { value, count, .. }
        | O::ExactIntegerShiftLeft { value, count, .. }
        | O::ExactIntegerShiftRight { value, count, .. } => vec![*value, *count],
        O::Jump { bindings, .. } => bindings.iter().map(|binding| binding.argument).collect(),
        O::Conditional {
            condition,
            when_true,
            when_false,
        } => std::iter::once(*condition)
            .chain(when_true.bindings.iter().map(|binding| binding.argument))
            .chain(when_false.bindings.iter().map(|binding| binding.argument))
            .collect(),
        O::Return { value, .. } => vec![*value],
        _ => Vec::new(),
    };
    values
        .into_iter()
        .map(|value| ValueUse { value, block, node })
        .collect()
}

fn expected_provenance(
    operation: &omega_abstract_operations::AbstractOperation,
) -> Vec<PsiProvenance> {
    use omega_abstract_operations::AbstractOperation as O;
    match operation {
        O::Jump { .. } | O::Conditional { .. } => Vec::new(),
        O::Return { psi_edge, .. } | O::ReturnUnit { psi_edge, .. } | O::Crash { psi_edge, .. } => {
            vec![PsiProvenance::Edge(*psi_edge)]
        }
        O::ReturnStructural {
            psi_edge,
            trivial_affine_locals,
            ..
        } => {
            // This is deliberately primary-site-first custody order rather
            // than execution order. The return edge anchors the node; hidden
            // establishments follow in their exact tuple order.
            std::iter::once(PsiProvenance::Edge(*psi_edge))
                .chain(
                    trivial_affine_locals
                        .iter()
                        .map(|(operation, _, _)| PsiProvenance::Operation(*operation)),
                )
                .collect()
        }
        O::EstablishPayloadlessCase { psi_operation, .. }
        | O::EstablishByteSequenceLiteral { psi_operation, .. }
        | O::EstablishTrivialAffineLocal { psi_operation, .. }
        | O::CallUnit { psi_operation, .. }
        | O::CallStructuralScalar { psi_operation, .. }
        | O::CallStructural { psi_operation, .. }
        | O::BoundaryCall { psi_operation, .. }
        | O::PortWrite { psi_operation, .. }
        | O::Call { psi_operation, .. }
        | O::IntegerConstant { psi_operation, .. }
        | O::BooleanConstant { psi_operation, .. }
        | O::BooleanStructuralField { psi_operation, .. }
        | O::BooleanNot { psi_operation, .. }
        | O::BooleanEqual { psi_operation, .. }
        | O::IntegerEqual { psi_operation, .. }
        | O::IntegerLessThan { psi_operation, .. }
        | O::IntegerLessOrEqual { psi_operation, .. }
        | O::IntegerBitwiseNot { psi_operation, .. }
        | O::IntegerWiden { psi_operation, .. }
        | O::IntegerExactCast { psi_operation, .. }
        | O::IntegerBitwiseAnd { psi_operation, .. }
        | O::IntegerBitwiseOr { psi_operation, .. }
        | O::IntegerBitwiseXor { psi_operation, .. }
        | O::WrappingIntegerShiftLeft { psi_operation, .. }
        | O::WrappingIntegerShiftRight { psi_operation, .. }
        | O::ExactIntegerShiftLeft { psi_operation, .. }
        | O::ExactIntegerShiftRight { psi_operation, .. }
        | O::WrappingIntegerAdd { psi_operation, .. }
        | O::ExactIntegerAdd { psi_operation, .. }
        | O::SaturatingIntegerAdd { psi_operation, .. }
        | O::WrappingIntegerSubtract { psi_operation, .. }
        | O::ExactIntegerSubtract { psi_operation, .. }
        | O::SaturatingIntegerSubtract { psi_operation, .. }
        | O::WrappingIntegerMultiply { psi_operation, .. }
        | O::ExactIntegerMultiply { psi_operation, .. }
        | O::ExactIntegerDivide { psi_operation, .. }
        | O::ExactIntegerRemainder { psi_operation, .. }
        | O::WrappingIntegerDivide { psi_operation, .. }
        | O::WrappingIntegerRemainder { psi_operation, .. }
        | O::SaturatingIntegerDivide { psi_operation, .. }
        | O::SaturatingIntegerRemainder { psi_operation, .. }
        | O::SaturatingIntegerMultiply { psi_operation, .. } => {
            vec![PsiProvenance::Operation(*psi_operation)]
        }
    }
}

fn provenance_matches_operation(
    operation: &omega_abstract_operations::AbstractOperation,
    provenance: &[PsiProvenance],
) -> bool {
    let expected = expected_provenance(operation);
    if expected.is_empty() {
        matches!(operation, O::Jump { .. } | O::Conditional { .. }) || provenance.is_empty()
    } else {
        provenance.starts_with(&expected)
    }
}

fn successors_match_operation(
    operation: &omega_abstract_operations::AbstractOperation,
    actual: &[OptimizationEdge],
) -> bool {
    let expected = expected_edges(operation);
    actual.len() == expected.len()
        && actual.iter().zip(expected).all(|(actual, expected)| {
            actual.psi_edge == expected.psi_edge
                && actual.target == expected.target
                && actual.bindings == expected.bindings
                && actual.trivial_affine_discards == expected.trivial_affine_discards
                && actual.provenance.first() == Some(&PsiProvenance::Edge(actual.psi_edge))
                && actual
                    .provenance
                    .iter()
                    .all(|source| matches!(source, PsiProvenance::Edge(_)))
        })
}

fn expected_edges(
    operation: &omega_abstract_operations::AbstractOperation,
) -> Vec<OptimizationEdge> {
    use omega_abstract_operations::AbstractOperation as O;
    match operation {
        O::Jump {
            psi_edge,
            target,
            bindings,
            trivial_affine_discards,
        } => vec![OptimizationEdge {
            psi_edge: *psi_edge,
            target: *target,
            bindings: bindings.clone(),
            trivial_affine_discards: trivial_affine_discards.clone(),
            provenance: vec![PsiProvenance::Edge(*psi_edge)],
            fuel: vec![omega_optimization_unit::FuelSettlement {
                site: PsiProvenance::Edge(*psi_edge),
                units: 1,
            }],
        }],
        O::Conditional {
            when_true,
            when_false,
            ..
        } => [when_true, when_false]
            .into_iter()
            .map(|edge| OptimizationEdge {
                psi_edge: edge.psi_edge,
                target: edge.target,
                bindings: edge.bindings.clone(),
                trivial_affine_discards: edge.trivial_affine_discards.clone(),
                provenance: vec![PsiProvenance::Edge(edge.psi_edge)],
                fuel: vec![omega_optimization_unit::FuelSettlement {
                    site: PsiProvenance::Edge(edge.psi_edge),
                    units: 1,
                }],
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn expected_ownership(
    operation: &omega_abstract_operations::AbstractOperation,
) -> Vec<OwnershipEvent> {
    use omega_abstract_operations::AbstractOperation as O;
    match operation {
        O::CallUnit {
            claim_transfers, ..
        }
        | O::CallStructuralScalar {
            claim_transfers, ..
        }
        | O::CallStructural {
            claim_transfers, ..
        } => vec![OwnershipEvent::ClaimTransfer(
            claim_transfers
                .iter()
                .map(|transfer| transfer.claim)
                .collect(),
        )],
        O::BoundaryCall {
            completion_receipts,
            ..
        } => vec![OwnershipEvent::ClaimCompletion(
            completion_receipts
                .iter()
                .map(|receipt| receipt.claim)
                .collect(),
        )],
        O::Return {
            cleanup_actions, ..
        }
        | O::ReturnUnit {
            cleanup_actions, ..
        } => vec![OwnershipEvent::Cleanup(cleanup_actions.clone())],
        O::ReturnStructural {
            returned_claims, ..
        } => vec![OwnershipEvent::StructuralReturn(returned_claims.clone())],
        O::Crash {
            frontier_lower_bound,
            ..
        } => vec![OwnershipEvent::CrashFrontier(frontier_lower_bound.clone())],
        _ => Vec::new(),
    }
}

fn is_terminator(operation: &omega_abstract_operations::AbstractOperation) -> bool {
    use omega_abstract_operations::AbstractOperation as O;
    matches!(
        operation,
        O::Jump { .. }
            | O::Conditional { .. }
            | O::Return { .. }
            | O::ReturnUnit { .. }
            | O::ReturnStructural { .. }
            | O::Crash { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_abstract_operations::{
        AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
        AbstractOperationPlan, AbstractParameter, AbstractResult,
    };
    use omega_optimization_core::{
        AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
        OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
    };
    use omega_optimization_unit::{
        IntegerConstantRewrite, IntegerEvaluationWitness, NodeLocation, ProvenanceRewrite,
        PsiRewriteCandidate, ValueUse, reconstruct_psi_optimization_unit_seed,
    };
    use psi_core::{
        FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, OperationId, ScalarType,
        ValueId,
    };
    use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    fn refresh_identity(unit: &mut PsiOptimizationUnit) {
        unit.identity = recompute_psi_optimization_unit_identity(unit);
    }

    fn refresh_proof_question_identity(question: &mut ProofQuestion) {
        question.identity = omega_optimization_unit::proof_question_identity(
            question.terminal_psi,
            question.proof_bundle_fingerprint,
            question.owner,
            question.obligation,
            question.class,
            &question.proposition,
            &question.requirements,
            &question.semantic_axioms,
            question.canonical_certificate,
        );
    }

    fn refresh_node_derivatives(
        unit: &mut PsiOptimizationUnit,
        function_index: usize,
        block_index: usize,
        node_index: usize,
    ) {
        let block = unit.functions[function_index].blocks[block_index].id;
        let node_index = u32::try_from(node_index).expect("test node index fits u32");
        let operation = unit.functions[function_index].blocks[block_index].nodes
            [node_index as usize]
            .operation
            .clone();
        let node =
            &mut unit.functions[function_index].blocks[block_index].nodes[node_index as usize];
        node.definitions = expected_definitions(&operation, block, node_index);
        node.uses = expected_uses(&operation, block, node_index);
        node.provenance = expected_provenance(&operation);
        node.successors = expected_edges(&operation);
        node.ownership = expected_ownership(&operation);
        unit.functions[function_index].facts =
            reconstruct_fact_index(&unit.functions[function_index]);
        refresh_identity(unit);
    }

    fn verified_unit() -> omega_psi_to_abstract_operations::VerifiedPsiOptimizationUnit {
        use psi_terminal::{
            Block, ContractClause, MachineContract, TerminalMachine, TerminalMachineResult,
            TerminalModule, Terminator,
        };

        let module = TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: id(101, MachineId::new),
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            proof_output_calls: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            closed_conformance_applications: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines: vec![TerminalMachine {
                id: id(101, MachineId::new),
                attachment: None,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                ranked_scc: None,
                result: TerminalMachineResult::Unit,
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: id(102, BlockId::new),
                blocks: vec![Block {
                    id: id(102, BlockId::new),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: id(103, EdgeId::new),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    id: id(104, psi_core::ContractId::new),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: vec![
                        ContractClause {
                            obligation: id(105, psi_core::ObligationId::new),
                            proposition: Proposition::Truth,
                        },
                        ContractClause {
                            obligation: id(106, psi_core::ObligationId::new),
                            proposition: Proposition::Truth,
                        },
                    ],
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        };
        let proof = psi_terminal_verifier::ProofBundle {
            evidence_producers: Vec::new(),
            evidence: [105, 106]
                .into_iter()
                .map(|obligation| psi_terminal_verifier::ObligationEvidence {
                    obligation: id(obligation, psi_core::ObligationId::new),
                    route: psi_proof_admission::EvidenceRoute::KernelDerived(
                        psi_proof_admission::PrimitiveJudgment::Truth,
                    ),
                })
                .collect(),
        };
        let semantic = psi_terminal_codec::encode_module(&module).expect("encode unit module");
        let proof = psi_terminal_codec::encode_proof_bundle(&proof).expect("encode empty proof");
        let input = omega_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
            &semantic,
            &proof,
            &psi_proof_admission::AdmissionProfile::default(),
        )
        .expect("verified optimizer input");
        omega_psi_to_abstract_operations::build_verified_psi_optimization_unit(
            input,
            TerminalFuelSchedule::CURRENT.identity(),
        )
        .expect("verified optimizer unit")
    }

    fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
        constructor(raw).expect("nonzero test identity")
    }

    fn unit() -> PsiOptimizationUnit {
        let machine = id(1, MachineId::new);
        let block = id(2, BlockId::new);
        let result = id(3, ValueId::new);
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("valid width");
        let plan = AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([11; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry: block,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: result,
                    scalar_type: ScalarType::Integer(integer),
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    AbstractOperation::IntegerConstant {
                        psi_operation: id(4, OperationId::new),
                        result,
                        scalar_type: ScalarType::Integer(integer),
                        value: IntegerValue::Unsigned(7),
                    },
                    AbstractOperation::Return {
                        psi_edge: id(5, EdgeId::new),
                        result,
                        value: result,
                        scalar_type: ScalarType::Integer(integer),
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        };
        reconstruct_psi_optimization_unit_seed(
            &plan,
            FuelScheduleIdentity::new(1).expect("nonzero schedule"),
        )
        .expect("valid unit")
    }

    fn exact_add_unit() -> PsiOptimizationUnit {
        let machine = id(201, MachineId::new);
        let block = id(202, BlockId::new);
        let left = id(203, ValueId::new);
        let right = id(204, ValueId::new);
        let result = id(205, ValueId::new);
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let plan = AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([12; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry: block,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: result,
                    scalar_type: ScalarType::Integer(integer),
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    AbstractOperation::IntegerConstant {
                        psi_operation: id(206, OperationId::new),
                        result: left,
                        scalar_type: ScalarType::Integer(integer),
                        value: IntegerValue::Unsigned(7),
                    },
                    AbstractOperation::IntegerConstant {
                        psi_operation: id(207, OperationId::new),
                        result: right,
                        scalar_type: ScalarType::Integer(integer),
                        value: IntegerValue::Unsigned(8),
                    },
                    AbstractOperation::ExactIntegerAdd {
                        psi_operation: id(208, OperationId::new),
                        obligation: id(209, psi_core::ObligationId::new),
                        result,
                        scalar_type: integer,
                        left,
                        right,
                    },
                    AbstractOperation::Return {
                        psi_edge: id(210, EdgeId::new),
                        result,
                        value: result,
                        scalar_type: ScalarType::Integer(integer),
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        };
        let unit =
            reconstruct_psi_optimization_unit_seed(&plan, FuelScheduleIdentity::new(1).unwrap())
                .unwrap();
        omega_optimization_unit::attach_accepted_obligation_facts(
            unit.clone(),
            vec![omega_optimization_unit::AcceptedObligationFact::new(
                unit.psi,
                [23; 32],
                machine,
                id(208, OperationId::new),
                id(209, psi_core::ObligationId::new),
                b"validation-test-obligation".to_vec(),
            )],
        )
        .unwrap()
    }

    fn scalar_call_unit() -> PsiOptimizationUnit {
        let caller = id(301, MachineId::new);
        let callee = id(302, MachineId::new);
        let caller_block = id(303, BlockId::new);
        let callee_block = id(304, BlockId::new);
        let argument = id(305, ValueId::new);
        let caller_result = id(306, ValueId::new);
        let parameter = id(307, ValueId::new);
        let callee_result = id(308, ValueId::new);
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let scalar_type = ScalarType::Integer(integer);
        let plan = AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([13; 32]),
            },
            entry: caller,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![
                AbstractFunction {
                    machine: caller,
                    attachment: None,
                    entry: caller_block,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    result: AbstractFunctionResult::Scalar(AbstractResult {
                        value: caller_result,
                        scalar_type,
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![AbstractBlockEntry {
                        block: caller_block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations: vec![
                        AbstractOperation::IntegerConstant {
                            psi_operation: id(309, OperationId::new),
                            result: argument,
                            scalar_type,
                            value: IntegerValue::Unsigned(7),
                        },
                        AbstractOperation::Call {
                            psi_operation: id(310, OperationId::new),
                            result: caller_result,
                            scalar_type,
                            callee,
                            arguments: vec![argument],
                        },
                        AbstractOperation::Return {
                            psi_edge: id(311, EdgeId::new),
                            result: caller_result,
                            value: caller_result,
                            scalar_type,
                            cleanup_actions: Vec::new(),
                        },
                    ],
                },
                AbstractFunction {
                    machine: callee,
                    attachment: None,
                    entry: callee_block,
                    parameters: vec![AbstractParameter {
                        value: parameter,
                        scalar_type,
                    }],
                    structural_parameters: Vec::new(),
                    result: AbstractFunctionResult::Scalar(AbstractResult {
                        value: callee_result,
                        scalar_type,
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![AbstractBlockEntry {
                        block: callee_block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations: vec![AbstractOperation::Return {
                        psi_edge: id(312, EdgeId::new),
                        result: callee_result,
                        value: parameter,
                        scalar_type,
                        cleanup_actions: Vec::new(),
                    }],
                },
            ],
        };
        reconstruct_psi_optimization_unit_seed(&plan, FuelScheduleIdentity::new(1).unwrap())
            .unwrap()
    }

    fn scalar_boundary_call_unit() -> PsiOptimizationUnit {
        let machine = id(321, MachineId::new);
        let boundary = id(322, BoundaryMachineId::new);
        let block = id(323, BlockId::new);
        let argument = id(324, ValueId::new);
        let result = id(325, ValueId::new);
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let scalar_type = ScalarType::Integer(integer);
        let plan = AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([14; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            boundary_machines: vec![psi_terminal::BoundaryMachineDeclaration {
                id: boundary,
                identity: "validation::scalar-boundary".into(),
                attachment: None,
                scalar_parameters: vec![scalar_type],
                structural_parameters: Vec::new(),
                result: Some(scalar_type),
                requires: Vec::new(),
                program_local_root_introductions: Vec::new(),
                content_guarantees: Vec::new(),
                published_service_ceiling: Vec::new(),
            }],
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry: block,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: result,
                    scalar_type,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    AbstractOperation::IntegerConstant {
                        psi_operation: id(326, OperationId::new),
                        result: argument,
                        scalar_type,
                        value: IntegerValue::Unsigned(7),
                    },
                    AbstractOperation::BoundaryCall {
                        psi_operation: id(327, OperationId::new),
                        result: Some(AbstractResult {
                            value: result,
                            scalar_type,
                        }),
                        boundary,
                        arguments: vec![argument],
                        structural_arguments: Vec::new(),
                        completion_claim_sources: Vec::new(),
                        completion_receipts: Vec::new(),
                    },
                    AbstractOperation::Return {
                        psi_edge: id(328, EdgeId::new),
                        result,
                        value: result,
                        scalar_type,
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        };
        reconstruct_psi_optimization_unit_seed(&plan, FuelScheduleIdentity::new(1).unwrap())
            .unwrap()
    }

    fn structural_call_unit() -> PsiOptimizationUnit {
        let caller = id(331, MachineId::new);
        let callee = id(332, MachineId::new);
        let caller_block = id(333, BlockId::new);
        let callee_block = id(334, BlockId::new);
        let caller_place = id(335, PlaceId::new);
        let callee_place = id(336, PlaceId::new);
        let structural_type = id(337, psi_core::StructuralTypeId::new);
        let parameter = |place, position| psi_terminal::StructuralParameterDeclaration {
            place,
            position,
            is_self: false,
            structural_type,
            multiplicity: psi_terminal::StructuralMultiplicity::Unrestricted,
            access: psi_terminal::StructuralAccess::Owned,
            qualifications: Vec::new(),
        };
        let plan = AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([15; 32]),
            },
            entry: caller,
            structural_types: vec![psi_terminal::StructuralTypeDeclaration {
                id: structural_type,
                identity: "validation::structural-call-argument".into(),
                shape: psi_terminal::StructuralTypeShape::ByteSequence(
                    psi_terminal::ByteSequenceCarrier::BorrowedView,
                ),
            }],
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![
                AbstractFunction {
                    machine: caller,
                    attachment: None,
                    entry: caller_block,
                    parameters: Vec::new(),
                    structural_parameters: vec![parameter(caller_place, 0)],
                    result: AbstractFunctionResult::Unit,
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![AbstractBlockEntry {
                        block: caller_block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations: vec![
                        AbstractOperation::CallUnit {
                            psi_operation: id(338, OperationId::new),
                            callee,
                            structural_arguments: vec![psi_terminal::StructuralArgument {
                                place: caller_place,
                                path: Vec::new(),
                                access: psi_terminal::StructuralAccess::Owned,
                            }],
                            claim_transfers: Vec::new(),
                        },
                        AbstractOperation::ReturnUnit {
                            psi_edge: id(339, EdgeId::new),
                            cleanup_actions: Vec::new(),
                        },
                    ],
                },
                AbstractFunction {
                    machine: callee,
                    attachment: None,
                    entry: callee_block,
                    parameters: Vec::new(),
                    structural_parameters: vec![parameter(callee_place, 0)],
                    result: AbstractFunctionResult::Unit,
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![AbstractBlockEntry {
                        block: callee_block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations: vec![AbstractOperation::ReturnUnit {
                        psi_edge: id(340, EdgeId::new),
                        cleanup_actions: Vec::new(),
                    }],
                },
            ],
        };
        reconstruct_psi_optimization_unit_seed(&plan, FuelScheduleIdentity::new(1).unwrap())
            .unwrap()
    }

    fn affine_claim_transfer_unit() -> PsiOptimizationUnit {
        let mut unit = structural_call_unit();
        let claim = id(1, ClaimId::new);
        for function in &mut unit.functions {
            function.structural_parameters[0].multiplicity =
                psi_terminal::StructuralMultiplicity::Affine;
            function
                .entry_claim_declarations
                .push(psi_terminal::EntryClaim {
                    claim,
                    input: function.structural_parameters[0].place,
                    path: Vec::new(),
                });
            function.entry_claims.insert(claim);
        }
        let AbstractOperation::CallUnit {
            claim_transfers, ..
        } = &mut unit.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("fixture begins with a structural Unit call")
        };
        claim_transfers.push(psi_terminal::ClaimTransfer {
            claim,
            argument_index: 0,
        });
        refresh_node_derivatives(&mut unit, 0, 0, 0);
        unit
    }

    fn affine_place_transfer_unit() -> PsiOptimizationUnit {
        let mut unit = structural_call_unit();
        for function in &mut unit.functions {
            function.structural_parameters[0].multiplicity =
                psi_terminal::StructuralMultiplicity::Affine;
        }
        let callee_place = unit.functions[1].structural_parameters[0].place;
        let AbstractOperation::ReturnUnit {
            cleanup_actions, ..
        } = &mut unit.functions[1].blocks[0].nodes[0].operation
        else {
            panic!("callee fixture returns Unit")
        };
        cleanup_actions.push(psi_terminal::TerminalAffineCleanupAction::DiscardRoot(
            callee_place,
        ));
        refresh_node_derivatives(&mut unit, 1, 0, 0);
        refresh_identity(&mut unit);
        unit
    }

    fn partial_affine_place_unit() -> PsiOptimizationUnit {
        let caller = id(4_850, MachineId::new);
        let callee = id(4_851, MachineId::new);
        let caller_block = id(4_852, BlockId::new);
        let callee_block = id(4_853, BlockId::new);
        let left = id(4_854, StructuralTypeId::new);
        let right = id(4_855, StructuralTypeId::new);
        let pair = id(4_856, StructuralTypeId::new);
        let caller_place = id(4_857, PlaceId::new);
        let callee_place = id(4_858, PlaceId::new);
        let empty_record =
            |id: StructuralTypeId, identity: &str| psi_terminal::StructuralTypeDeclaration {
                id,
                identity: identity.into(),
                shape: psi_terminal::StructuralTypeShape::Record { fields: Vec::new() },
            };
        let parameter = |place, structural_type| psi_terminal::StructuralParameterDeclaration {
            place,
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: psi_terminal::StructuralMultiplicity::Affine,
            access: psi_terminal::StructuralAccess::Owned,
            qualifications: Vec::new(),
        };
        let plan = AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([49; 32]),
            },
            entry: caller,
            structural_types: vec![
                empty_record(left, "validation::partial-left"),
                empty_record(right, "validation::partial-right"),
                psi_terminal::StructuralTypeDeclaration {
                    id: pair,
                    identity: "validation::partial-pair".into(),
                    shape: psi_terminal::StructuralTypeShape::Record {
                        fields: vec![
                            psi_terminal::StructuralFieldDeclaration {
                                id: id(1, psi_core::StructuralFieldId::new),
                                identity: "left".into(),
                                relevance: psi_terminal::BindingRelevance::Relevant,
                                field_type: psi_terminal::StructuralFieldType::Structural(left),
                            },
                            psi_terminal::StructuralFieldDeclaration {
                                id: id(2, psi_core::StructuralFieldId::new),
                                identity: "right".into(),
                                relevance: psi_terminal::BindingRelevance::Relevant,
                                field_type: psi_terminal::StructuralFieldType::Structural(right),
                            },
                        ],
                    },
                },
            ],
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![
                AbstractFunction {
                    machine: caller,
                    attachment: None,
                    entry: caller_block,
                    parameters: Vec::new(),
                    structural_parameters: vec![parameter(caller_place, pair)],
                    result: AbstractFunctionResult::Unit,
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![AbstractBlockEntry {
                        block: caller_block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations: vec![
                        AbstractOperation::CallUnit {
                            psi_operation: id(4_859, OperationId::new),
                            callee,
                            structural_arguments: vec![psi_terminal::StructuralArgument {
                                place: caller_place,
                                path: vec![psi_terminal::StructuralPathSegment::Field(
                                    "right".into(),
                                )],
                                access: psi_terminal::StructuralAccess::Owned,
                            }],
                            claim_transfers: Vec::new(),
                        },
                        AbstractOperation::ReturnUnit {
                            psi_edge: id(4_860, EdgeId::new),
                            cleanup_actions: vec![
                                psi_terminal::TerminalAffineCleanupAction::DiscardResidual(
                                    psi_terminal::StructuralAffineDiscard {
                                        place: caller_place,
                                        path: vec![psi_terminal::StructuralPathSegment::Field(
                                            "left".into(),
                                        )],
                                        structural_type: left,
                                    },
                                ),
                            ],
                        },
                    ],
                },
                AbstractFunction {
                    machine: callee,
                    attachment: None,
                    entry: callee_block,
                    parameters: Vec::new(),
                    structural_parameters: vec![parameter(callee_place, right)],
                    result: AbstractFunctionResult::Unit,
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![AbstractBlockEntry {
                        block: callee_block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations: vec![AbstractOperation::ReturnUnit {
                        psi_edge: id(4_861, EdgeId::new),
                        cleanup_actions: vec![
                            psi_terminal::TerminalAffineCleanupAction::DiscardRoot(callee_place),
                        ],
                    }],
                },
            ],
        };
        reconstruct_psi_optimization_unit_seed(&plan, FuelScheduleIdentity::new(1).unwrap())
            .unwrap()
    }

    fn affine_place_join_unit(settle_false_arm: bool) -> PsiOptimizationUnit {
        let mut unit = affine_claim_join_unit(settle_false_arm);
        let function = &mut unit.functions[0];
        function.entry_claim_declarations.clear();
        function.entry_claims.clear();
        for node in function
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.nodes)
        {
            if let AbstractOperation::BoundaryCall {
                completion_claim_sources,
                completion_receipts,
                ..
            } = &mut node.operation
            {
                completion_claim_sources.clear();
                completion_receipts.clear();
            }
        }
        refresh_function_derivatives(&mut unit, 0);
        unit
    }

    fn affine_claim_join_unit(settle_false_arm: bool) -> PsiOptimizationUnit {
        let machine = id(4_800, MachineId::new);
        let entry_block = id(4_801, BlockId::new);
        let true_block = id(4_802, BlockId::new);
        let false_block = id(4_803, BlockId::new);
        let join_block = id(4_804, BlockId::new);
        let boundary = id(4_805, BoundaryMachineId::new);
        let structural_type = id(4_806, StructuralTypeId::new);
        let root = id(4_807, PlaceId::new);
        let boundary_root = id(4_808, PlaceId::new);
        let condition = id(4_809, ValueId::new);
        let claim = id(1, ClaimId::new);
        let parameter = |place| psi_terminal::StructuralParameterDeclaration {
            place,
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: psi_terminal::StructuralMultiplicity::Affine,
            access: psi_terminal::StructuralAccess::Owned,
            qualifications: Vec::new(),
        };
        let entry_claim = psi_terminal::EntryClaim {
            claim,
            input: root,
            path: Vec::new(),
        };
        let completion = |psi_operation| AbstractOperation::BoundaryCall {
            psi_operation,
            result: None,
            boundary,
            arguments: Vec::new(),
            structural_arguments: vec![psi_terminal::StructuralArgument {
                place: root,
                path: Vec::new(),
                access: psi_terminal::StructuralAccess::Owned,
            }],
            completion_claim_sources: vec![omega_abstract_operations::CompletionClaimSource {
                claim,
                entry: Some(entry_claim.clone()),
                content: None,
            }],
            completion_receipts: vec![psi_terminal::CompletionReceipt {
                claim,
                argument_index: 0,
            }],
        };
        let mut operations = vec![
            AbstractOperation::BooleanConstant {
                psi_operation: id(4_810, OperationId::new),
                result: condition,
                value: true,
            },
            AbstractOperation::Conditional {
                condition,
                when_true: omega_abstract_operations::AbstractSuccessor {
                    psi_edge: id(4_811, EdgeId::new),
                    target: true_block,
                    bindings: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
                when_false: omega_abstract_operations::AbstractSuccessor {
                    psi_edge: id(4_812, EdgeId::new),
                    target: false_block,
                    bindings: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            },
        ];
        let true_offset = operations.len();
        operations.extend([
            completion(id(4_813, OperationId::new)),
            AbstractOperation::Jump {
                psi_edge: id(4_814, EdgeId::new),
                target: join_block,
                bindings: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
        ]);
        let false_offset = operations.len();
        if settle_false_arm {
            operations.push(completion(id(4_815, OperationId::new)));
        }
        operations.push(AbstractOperation::Jump {
            psi_edge: id(4_816, EdgeId::new),
            target: join_block,
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        });
        let join_offset = operations.len();
        operations.push(AbstractOperation::ReturnUnit {
            psi_edge: id(4_817, EdgeId::new),
            cleanup_actions: Vec::new(),
        });
        let plan = AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([48; 32]),
            },
            entry: machine,
            structural_types: vec![psi_terminal::StructuralTypeDeclaration {
                id: structural_type,
                identity: "validation::affine-claim-join".into(),
                shape: psi_terminal::StructuralTypeShape::Record { fields: Vec::new() },
            }],
            boundary_machines: vec![psi_terminal::BoundaryMachineDeclaration {
                id: boundary,
                identity: "validation::affine-claim-settlement".into(),
                attachment: None,
                scalar_parameters: Vec::new(),
                structural_parameters: vec![parameter(boundary_root)],
                result: None,
                requires: Vec::new(),
                program_local_root_introductions: Vec::new(),
                content_guarantees: Vec::new(),
                published_service_ceiling: Vec::new(),
            }],
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry: entry_block,
                parameters: Vec::new(),
                structural_parameters: vec![parameter(root)],
                result: AbstractFunctionResult::Unit,
                entry_claims: vec![entry_claim],
                published_service_ceiling: Vec::new(),
                block_entries: vec![
                    AbstractBlockEntry {
                        block: entry_block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    },
                    AbstractBlockEntry {
                        block: true_block,
                        parameters: Vec::new(),
                        operation_offset: true_offset,
                    },
                    AbstractBlockEntry {
                        block: false_block,
                        parameters: Vec::new(),
                        operation_offset: false_offset,
                    },
                    AbstractBlockEntry {
                        block: join_block,
                        parameters: Vec::new(),
                        operation_offset: join_offset,
                    },
                ],
                operations,
            }],
        };
        reconstruct_psi_optimization_unit_seed(&plan, FuelScheduleIdentity::new(1).unwrap())
            .unwrap()
    }

    fn boolean_structural_field_unit() -> PsiOptimizationUnit {
        let machine = id(4_700, MachineId::new);
        let block = id(4_701, BlockId::new);
        let place = id(4_702, PlaceId::new);
        let structural_type = id(4_703, StructuralTypeId::new);
        let field = id(4_704, psi_core::StructuralFieldId::new);
        let scalar_parameter = id(4_705, ValueId::new);
        let result = id(4_706, ValueId::new);
        let cleanup_machine = id(4_709, MachineId::new);
        let cleanup_block = id(4_710, BlockId::new);
        reconstruct_psi_optimization_unit_seed(
            &AbstractOperationPlan {
                psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([47; 32]),
                },
                entry: machine,
                structural_types: vec![psi_terminal::StructuralTypeDeclaration {
                    id: structural_type,
                    identity: "validation::observed-affine-record".into(),
                    shape: psi_terminal::StructuralTypeShape::Record {
                        fields: vec![psi_terminal::StructuralFieldDeclaration {
                            id: field,
                            identity: "ready".into(),
                            relevance: psi_terminal::BindingRelevance::Relevant,
                            field_type: psi_terminal::StructuralFieldType::Scalar(
                                ScalarType::Boolean,
                            ),
                        }],
                    },
                }],
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![
                    AbstractFunction {
                        machine,
                        attachment: None,
                        entry: block,
                        parameters: vec![AbstractParameter {
                            value: scalar_parameter,
                            scalar_type: ScalarType::Boolean,
                        }],
                        structural_parameters: vec![psi_terminal::StructuralParameterDeclaration {
                            place,
                            position: 0,
                            is_self: false,
                            structural_type,
                            multiplicity: psi_terminal::StructuralMultiplicity::Affine,
                            access: psi_terminal::StructuralAccess::Owned,
                            qualifications: Vec::new(),
                        }],
                        result: AbstractFunctionResult::Scalar(AbstractResult {
                            value: result,
                            scalar_type: ScalarType::Boolean,
                        }),
                        entry_claims: Vec::new(),
                        published_service_ceiling: Vec::new(),
                        block_entries: vec![AbstractBlockEntry {
                            block,
                            parameters: Vec::new(),
                            operation_offset: 0,
                        }],
                        operations: vec![
                            AbstractOperation::BooleanStructuralField {
                                psi_operation: id(4_707, OperationId::new),
                                result,
                                source: place,
                                field,
                            },
                            AbstractOperation::Return {
                                psi_edge: id(4_708, EdgeId::new),
                                result,
                                value: result,
                                scalar_type: ScalarType::Boolean,
                                cleanup_actions: vec![
                                    psi_terminal::TerminalAffineCleanupAction::InvokeNominal(
                                        psi_terminal::NominalAffineCleanup {
                                            place,
                                            structural_type,
                                            cleanup_machine,
                                            cleanup_receiver: None,
                                            requirement_obligations: Vec::new(),
                                        },
                                    ),
                                ],
                            },
                        ],
                    },
                    AbstractFunction {
                        machine: cleanup_machine,
                        attachment: Some(structural_type),
                        entry: cleanup_block,
                        parameters: Vec::new(),
                        structural_parameters: Vec::new(),
                        result: AbstractFunctionResult::Unit,
                        entry_claims: Vec::new(),
                        published_service_ceiling: Vec::new(),
                        block_entries: vec![AbstractBlockEntry {
                            block: cleanup_block,
                            parameters: Vec::new(),
                            operation_offset: 0,
                        }],
                        operations: vec![AbstractOperation::ReturnUnit {
                            psi_edge: id(4_711, EdgeId::new),
                            cleanup_actions: Vec::new(),
                        }],
                    },
                ],
            },
            FuelScheduleIdentity::new(1).expect("nonzero schedule"),
        )
        .expect("Boolean structural-field fixture")
    }

    fn content_entry_claim(claim: ClaimId, root: PlaceId) -> psi_terminal::ContentEntryClaim {
        let algebra = psi_core::ContentAlgebra {
            kind: psi_core::ContentAlgebraKind::CountedQuantity,
            parameter: "validation::content-only-claim".into(),
        };
        let expression = psi_core::ContentProjectionExpression::CountedQuantity(
            psi_core::ContentProjectionScalar::Natural("1".into()),
        );
        psi_terminal::ContentEntryClaim {
            claim,
            input: psi_core::ContentStructuralPlace {
                version: psi_core::ContentPlaceVersion::Entry,
                root,
                segments: Vec::new(),
            },
            projections: vec![psi_terminal::ClaimContentProjection {
                projection: psi_core::ContentProjectionIdentity {
                    domain: id(1, psi_core::ContentDomainId::new),
                    projection_fingerprint:
                        psi_language_semantics::content::terminal_projection_fingerprint(
                            &algebra,
                            &expression,
                        ),
                },
                algebra,
            }],
        }
    }

    fn install_content_owner(unit: &mut PsiOptimizationUnit) {
        let carrier = unit.structural_types[0].id;
        let semantic_domain = id(1, psi_core::DomainSemanticId::new);
        let algebra = psi_core::ContentAlgebra {
            kind: psi_core::ContentAlgebraKind::CountedQuantity,
            parameter: "validation::content-only-claim".into(),
        };
        let expression = psi_core::ContentProjectionExpression::CountedQuantity(
            psi_core::ContentProjectionScalar::Natural("1".into()),
        );
        unit.structural_domains = vec![psi_terminal::StructuralDomainDeclaration {
            id: id(1, StructuralDomainId::new),
            semantic_domain,
            identity: "validation::content-only-domain".into(),
            carrier,
            content_projection: Some(psi_terminal::StructuralContentProjection {
                identity: psi_core::ContentProjectionIdentity {
                    domain: id(semantic_domain.get(), psi_core::ContentDomainId::new),
                    projection_fingerprint:
                        psi_language_semantics::content::terminal_projection_fingerprint(
                            &algebra,
                            &expression,
                        ),
                },
                algebra,
                expression,
            }),
        }]
        .into();
    }

    fn structural_field(
        raw: u64,
        target: StructuralTypeId,
    ) -> psi_terminal::StructuralFieldDeclaration {
        structural_leaf_field(
            raw,
            psi_terminal::BindingRelevance::Relevant,
            psi_terminal::StructuralFieldType::Structural(target),
        )
    }

    fn structural_leaf_field(
        raw: u64,
        relevance: psi_terminal::BindingRelevance,
        field_type: psi_terminal::StructuralFieldType,
    ) -> psi_terminal::StructuralFieldDeclaration {
        psi_terminal::StructuralFieldDeclaration {
            id: id(raw, psi_core::StructuralFieldId::new),
            identity: format!("validation::field-{raw}"),
            relevance,
            field_type,
        }
    }

    fn structural_case(
        raw: u64,
        fields: Vec<psi_terminal::StructuralFieldDeclaration>,
    ) -> psi_terminal::StructuralCaseDeclaration {
        psi_terminal::StructuralCaseDeclaration {
            id: id(raw, psi_core::StructuralCaseId::new),
            identity: format!("validation::case-{raw}"),
            fields,
        }
    }

    fn structural_type(
        raw: u64,
        shape: psi_terminal::StructuralTypeShape,
    ) -> psi_terminal::StructuralTypeDeclaration {
        psi_terminal::StructuralTypeDeclaration {
            id: id(raw, StructuralTypeId::new),
            identity: format!("validation::type-{raw}"),
            shape,
        }
    }

    fn structural_catalog_unit(
        structural_types: Vec<psi_terminal::StructuralTypeDeclaration>,
    ) -> PsiOptimizationUnit {
        let mut candidate = unit();
        candidate.structural_types = structural_types;
        refresh_identity(&mut candidate);
        candidate
    }

    fn service_declarations() -> Vec<psi_terminal::ServiceDeclaration> {
        let root = id(701, ServiceId::new);
        let middle = id(702, ServiceId::new);
        let leaf = id(703, ServiceId::new);
        vec![
            psi_terminal::ServiceDeclaration {
                id: root,
                identity: "validation::service-root".into(),
                parents: Vec::new(),
            },
            psi_terminal::ServiceDeclaration {
                id: middle,
                identity: "validation::service-middle".into(),
                parents: vec![root],
            },
            psi_terminal::ServiceDeclaration {
                id: leaf,
                identity: "validation::service-leaf".into(),
                parents: vec![root, middle],
            },
        ]
    }

    fn install_service_catalog(unit: &mut PsiOptimizationUnit) {
        let services = service_declarations();
        let ceiling = services
            .iter()
            .map(|service| service.id)
            .collect::<Vec<_>>();
        unit.services = services.into();
        for function in &mut unit.functions {
            function.published_service_ceiling = ceiling.clone();
        }
        for boundary in &mut unit.boundary_machines {
            boundary.published_service_ceiling = ceiling.clone();
        }
        refresh_root_service_reach(unit).expect("service fixture has a closed root reach");
        refresh_identity(unit);
    }

    fn service_effect_unit() -> PsiOptimizationUnit {
        let mut candidate = unit();
        install_service_catalog(&mut candidate);
        let block = candidate.functions[0].blocks[0].id;
        let mut node = candidate.functions[0].blocks[0].nodes[0].clone();
        node.operation = AbstractOperation::PortWrite {
            psi_operation: id(704, OperationId::new),
            service: id(703, ServiceId::new),
            port: 0x3f8,
            value: 0x41,
        };
        node.provenance = expected_provenance(&node.operation);
        node.fuel = node
            .provenance
            .iter()
            .copied()
            .map(|site| omega_optimization_unit::FuelSettlement { site, units: 1 })
            .collect();
        node.definitions = expected_definitions(&node.operation, block, 1);
        node.uses = expected_uses(&node.operation, block, 1);
        node.successors = expected_edges(&node.operation);
        node.ownership = expected_ownership(&node.operation);
        candidate.functions[0].blocks[0].nodes.insert(1, node);
        for index in 0..candidate.functions[0].blocks[0].nodes.len() {
            let operation = candidate.functions[0].blocks[0].nodes[index]
                .operation
                .clone();
            let node = &mut candidate.functions[0].blocks[0].nodes[index];
            node.effect.input = index as u64;
            node.effect.output = index as u64 + 1;
            node.provenance = expected_provenance(&operation);
            node.fuel = node
                .provenance
                .iter()
                .copied()
                .map(|site| omega_optimization_unit::FuelSettlement { site, units: 1 })
                .collect();
            node.definitions = expected_definitions(&operation, block, index as u32);
            node.uses = expected_uses(&operation, block, index as u32);
            node.successors = expected_edges(&operation);
            node.ownership = expected_ownership(&operation);
        }
        candidate.functions[0].facts = reconstruct_fact_index(&candidate.functions[0]);
        refresh_root_service_reach(&mut candidate).expect("PortWrite fixture has exact root reach");
        refresh_identity(&mut candidate);
        candidate
    }

    fn provider_service_unit() -> PsiOptimizationUnit {
        let mut candidate = provider_attachment_specialization_unit();
        install_service_catalog(&mut candidate);
        let boundary = candidate.boundary_machines[0].id;
        let requirement_identity = candidate.boundary_machines[0].identity.clone();
        let callee = candidate.functions[0].machine;
        let ceiling = service_declarations()
            .iter()
            .map(|service| service.id)
            .collect::<Vec<_>>();
        candidate
            .provider_candidates
            .push(psi_terminal::ProviderCandidateConformance {
                boundary,
                requirement_identity,
                provider_identity: "validation::service-provider".into(),
                candidate_identity: "validation::service-provider-candidate".into(),
                candidate: callee,
                signature: psi_terminal::ProviderUnitSignature {
                    parameters: Vec::new(),
                },
                refinement: psi_terminal::ProviderUnitRefinement {
                    positional_parameters: Vec::new(),
                    required_domains: Vec::new(),
                    realized_service_ceiling: ceiling,
                },
            });
        refresh_identity(&mut candidate);
        candidate
    }

    fn installation_root_service_unit() -> PsiOptimizationUnit {
        let mut candidate = scalar_boundary_call_unit();
        install_service_catalog(&mut candidate);
        let boundary = &candidate.boundary_machines[0];
        candidate.root_service_reach.installation_dependencies =
            vec![psi_terminal::InstallationReachDependency {
                requirement_identity: boundary.identity.clone(),
                upper_bound: boundary.published_service_ceiling.clone(),
            }];
        refresh_root_service_reach(&mut candidate)
            .expect("installation-bound fixture has exact root reach");
        refresh_identity(&mut candidate);
        candidate
    }

    fn multiple_installation_root_service_unit() -> PsiOptimizationUnit {
        let mut candidate = provider_attachment_specialization_unit();
        install_service_catalog(&mut candidate);
        candidate.root_service_reach.installation_dependencies = candidate.boundary_machines[..2]
            .iter()
            .map(|boundary| psi_terminal::InstallationReachDependency {
                requirement_identity: boundary.identity.clone(),
                upper_bound: boundary.published_service_ceiling.clone(),
            })
            .collect();
        candidate
            .root_service_reach
            .installation_dependencies
            .sort_by(|left, right| left.requirement_identity.cmp(&right.requirement_identity));
        refresh_root_service_reach(&mut candidate)
            .expect("multi-dependency fixture has exact root reach");
        refresh_identity(&mut candidate);
        candidate
    }

    fn provider_attachment_specialization_unit() -> PsiOptimizationUnit {
        let machine = id(440, MachineId::new);
        let block = id(441, BlockId::new);
        let attachment = id(444, StructuralTypeId::new);
        let provider_field = id(1, psi_core::StructuralFieldId::new);
        let first_boundary = id(446, BoundaryMachineId::new);
        let second_boundary = id(447, BoundaryMachineId::new);
        let unused_boundary = id(448, BoundaryMachineId::new);
        let boundary = |id, identity: &str| psi_terminal::BoundaryMachineDeclaration {
            id,
            identity: identity.into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: None,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        };
        let call = |psi_operation, boundary| AbstractOperation::BoundaryCall {
            psi_operation,
            result: None,
            boundary,
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_claim_sources: Vec::new(),
            completion_receipts: Vec::new(),
        };
        let plan = AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([44; 32]),
            },
            entry: machine,
            structural_types: vec![structural_type(
                444,
                psi_terminal::StructuralTypeShape::Record {
                    fields: vec![structural_leaf_field(
                        1,
                        psi_terminal::BindingRelevance::Relevant,
                        psi_terminal::StructuralFieldType::Erased {
                            type_identity: "validation::provider".into(),
                        },
                    )],
                },
            )],
            boundary_machines: vec![
                boundary(first_boundary, "validation::provider-first"),
                boundary(second_boundary, "validation::provider-second"),
                boundary(unused_boundary, "validation::provider-unused"),
            ],
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: Some(attachment),
                entry: block,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    call(id(449, OperationId::new), first_boundary),
                    call(id(450, OperationId::new), first_boundary),
                    call(id(451, OperationId::new), second_boundary),
                    AbstractOperation::ReturnUnit {
                        psi_edge: id(452, EdgeId::new),
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        };
        let mut unit = reconstruct_psi_optimization_unit_seed(
            &plan,
            FuelScheduleIdentity::new(1).expect("nonzero schedule"),
        )
        .expect("provider specialization fixture");
        unit.functions[0].structural_places.extend([
            psi_terminal::StructuralPlaceDeclaration {
                id: id(445, PlaceId::new),
                kind: StructuralPlaceKind::ProviderAttachment {
                    attachment,
                    field: provider_field,
                    boundary: first_boundary,
                },
            },
            psi_terminal::StructuralPlaceDeclaration {
                id: id(446, PlaceId::new),
                kind: StructuralPlaceKind::ProviderAttachment {
                    attachment,
                    field: provider_field,
                    boundary: second_boundary,
                },
            },
        ]);
        refresh_identity(&mut unit);
        unit
    }

    fn structural_domain(
        raw: u64,
        semantic_raw: u64,
        carrier: StructuralTypeId,
    ) -> psi_terminal::StructuralDomainDeclaration {
        psi_terminal::StructuralDomainDeclaration {
            id: id(raw, StructuralDomainId::new),
            semantic_domain: id(semantic_raw, psi_core::DomainSemanticId::new),
            identity: format!("validation::domain-{raw}"),
            carrier,
            content_projection: None,
        }
    }

    fn structural_result_call_unit() -> PsiOptimizationUnit {
        let caller = id(350, MachineId::new);
        let callee = id(351, MachineId::new);
        let caller_block = id(352, BlockId::new);
        let callee_block = id(353, BlockId::new);
        let structural_type = id(354, psi_core::StructuralTypeId::new);
        let callee_result = id(355, PlaceId::new);
        let call_result = id(356, PlaceId::new);
        let caller_result = id(362, PlaceId::new);
        let caller_input = id(360, PlaceId::new);
        let callee_input = id(361, PlaceId::new);
        let claim = id(1, ClaimId::new);
        let parameter = |place| psi_terminal::StructuralParameterDeclaration {
            place,
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: psi_terminal::StructuralMultiplicity::Linear,
            access: psi_terminal::StructuralAccess::Owned,
            qualifications: Vec::new(),
        };
        let entry_claim = |input| psi_terminal::EntryClaim {
            claim,
            input,
            path: Vec::new(),
        };
        let plan = AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([16; 32]),
            },
            entry: caller,
            structural_types: vec![psi_terminal::StructuralTypeDeclaration {
                id: structural_type,
                identity: "validation::structural-call-result".into(),
                shape: psi_terminal::StructuralTypeShape::ByteSequence(
                    psi_terminal::ByteSequenceCarrier::BorrowedView,
                ),
            }],
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![
                AbstractFunction {
                    machine: caller,
                    attachment: None,
                    entry: caller_block,
                    parameters: Vec::new(),
                    structural_parameters: vec![parameter(caller_input)],
                    result: AbstractFunctionResult::Structural(
                        psi_terminal::StructuralResultDeclaration {
                            place: caller_result,
                            structural_type,
                            multiplicity: psi_terminal::StructuralMultiplicity::Linear,
                            qualifications: Vec::new(),
                        },
                    ),
                    entry_claims: vec![entry_claim(caller_input)],
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![AbstractBlockEntry {
                        block: caller_block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations: vec![
                        AbstractOperation::CallStructural {
                            psi_operation: id(357, OperationId::new),
                            result: psi_terminal::StructuralOperationResult {
                                place: call_result,
                                structural_type,
                                multiplicity: psi_terminal::StructuralMultiplicity::Linear,
                                qualifications: Vec::new(),
                                claims: vec![psi_terminal::StructuralResultClaimBinding {
                                    claim,
                                    path: Vec::new(),
                                }],
                            },
                            callee,
                            structural_arguments: vec![psi_terminal::StructuralArgument {
                                place: caller_input,
                                path: Vec::new(),
                                access: psi_terminal::StructuralAccess::Owned,
                            }],
                            claim_transfers: vec![psi_terminal::ClaimTransfer {
                                claim,
                                argument_index: 0,
                            }],
                            returned_claim_transfers: vec![
                                psi_terminal::StructuralResultClaimTransfer {
                                    callee_claim: claim,
                                    caller_claim: claim,
                                },
                            ],
                            requirement_obligations: Vec::new(),
                            crash_continuations: Vec::new(),
                            selected_evidence: None,
                        },
                        AbstractOperation::ReturnStructural {
                            psi_edge: id(358, EdgeId::new),
                            source: call_result,
                            returned_claims: vec![claim],
                            trivial_affine_locals: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    ],
                },
                AbstractFunction {
                    machine: callee,
                    attachment: None,
                    entry: callee_block,
                    parameters: Vec::new(),
                    structural_parameters: vec![parameter(callee_input)],
                    result: AbstractFunctionResult::Structural(
                        psi_terminal::StructuralResultDeclaration {
                            place: callee_result,
                            structural_type,
                            multiplicity: psi_terminal::StructuralMultiplicity::Linear,
                            qualifications: Vec::new(),
                        },
                    ),
                    entry_claims: vec![entry_claim(callee_input)],
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![AbstractBlockEntry {
                        block: callee_block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations: vec![AbstractOperation::ReturnStructural {
                        psi_edge: id(359, EdgeId::new),
                        source: callee_input,
                        returned_claims: vec![claim],
                        trivial_affine_locals: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    }],
                },
            ],
        };
        reconstruct_psi_optimization_unit_seed(&plan, FuelScheduleIdentity::new(1).unwrap())
            .unwrap()
    }

    fn compressed_trivial_affine_return_unit_with_prefix(
        executable_collision: bool,
        explicit_witnesses: bool,
    ) -> PsiOptimizationUnit {
        let machine = id(360, MachineId::new);
        let block = id(361, BlockId::new);
        let structural_type = id(362, StructuralTypeId::new);
        let source = id(363, PlaceId::new);
        let first_tail = id(364, PlaceId::new);
        let second_tail = id(365, PlaceId::new);
        let result = id(366, PlaceId::new);
        let first_local = id(367, PlaceId::new);
        let second_local = id(368, PlaceId::new);
        let claim = id(1, ClaimId::new);
        let local_type = psi_terminal::StructuralTypeDeclaration {
            id: structural_type,
            identity: "validation::trivial-affine-empty-record".into(),
            shape: psi_terminal::StructuralTypeShape::Record { fields: Vec::new() },
        };
        let parameter =
            |place, position, multiplicity| psi_terminal::StructuralParameterDeclaration {
                place,
                position,
                is_self: false,
                structural_type,
                multiplicity,
                access: psi_terminal::StructuralAccess::Owned,
                qualifications: Vec::new(),
            };
        let local = |place, declaration_ordinal| psi_terminal::StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                structural_type,
            },
        };
        let first_declaration = local(first_local, 0);
        let second_declaration = local(second_local, 1);
        let mut operations = Vec::new();
        if executable_collision {
            operations.push(AbstractOperation::BooleanConstant {
                psi_operation: id(371, OperationId::new),
                result: id(389, ValueId::new),
                value: false,
            });
        }
        if explicit_witnesses {
            operations.extend([
                AbstractOperation::EstablishTrivialAffineLocal {
                    psi_operation: id(373, OperationId::new),
                    place: first_declaration,
                    structural_type: local_type.clone(),
                },
                AbstractOperation::EstablishTrivialAffineLocal {
                    psi_operation: id(374, OperationId::new),
                    place: second_declaration,
                    structural_type: local_type.clone(),
                },
            ]);
        }
        operations.push(AbstractOperation::ReturnStructural {
            psi_edge: id(370, EdgeId::new),
            source,
            returned_claims: vec![claim],
            trivial_affine_locals: vec![
                (
                    id(371, OperationId::new),
                    first_declaration,
                    local_type.clone(),
                ),
                (
                    id(372, OperationId::new),
                    second_declaration,
                    local_type.clone(),
                ),
            ],
            trivial_affine_discards: vec![second_local, first_local, second_tail, first_tail],
        });
        let plan = AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([18; 32]),
            },
            entry: machine,
            structural_types: vec![local_type.clone()],
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry: block,
                parameters: Vec::new(),
                structural_parameters: vec![
                    parameter(source, 0, psi_terminal::StructuralMultiplicity::Linear),
                    parameter(first_tail, 1, psi_terminal::StructuralMultiplicity::Affine),
                    parameter(second_tail, 2, psi_terminal::StructuralMultiplicity::Affine),
                ],
                result: AbstractFunctionResult::Structural(
                    psi_terminal::StructuralResultDeclaration {
                        place: result,
                        structural_type,
                        multiplicity: psi_terminal::StructuralMultiplicity::Linear,
                        qualifications: Vec::new(),
                    },
                ),
                entry_claims: vec![psi_terminal::EntryClaim {
                    claim,
                    input: source,
                    path: Vec::new(),
                }],
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations,
            }],
        };
        let mut unit = reconstruct_psi_optimization_unit_seed(
            &plan,
            FuelScheduleIdentity::new(1).expect("nonzero schedule"),
        )
        .expect("compressed structural return unit");
        for declaration in [first_declaration, second_declaration] {
            if !unit.functions[0]
                .structural_places
                .iter()
                .any(|place| place.id == declaration.id)
            {
                unit.functions[0].structural_places.push(declaration);
            }
        }
        refresh_identity(&mut unit);
        unit
    }

    fn compressed_trivial_affine_return_unit() -> PsiOptimizationUnit {
        compressed_trivial_affine_return_unit_with_prefix(false, false)
    }

    fn explicit_trivial_affine_return_unit() -> PsiOptimizationUnit {
        let machine = id(390, MachineId::new);
        let block = id(391, BlockId::new);
        let structural_type = id(392, StructuralTypeId::new);
        let place = id(393, PlaceId::new);
        let structural_type_declaration = psi_terminal::StructuralTypeDeclaration {
            id: structural_type,
            identity: "validation::explicit-trivial-affine-empty-record".into(),
            shape: psi_terminal::StructuralTypeShape::Record { fields: Vec::new() },
        };
        let place_declaration = psi_terminal::StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal: 0,
                structural_type,
            },
        };
        reconstruct_psi_optimization_unit_seed(
            &AbstractOperationPlan {
                psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([19; 32]),
                },
                entry: machine,
                structural_types: vec![structural_type_declaration.clone()],
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![AbstractFunction {
                    machine,
                    attachment: None,
                    entry: block,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    result: AbstractFunctionResult::Unit,
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![AbstractBlockEntry {
                        block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations: vec![
                        AbstractOperation::EstablishTrivialAffineLocal {
                            psi_operation: id(394, OperationId::new),
                            place: place_declaration,
                            structural_type: structural_type_declaration,
                        },
                        AbstractOperation::ReturnUnit {
                            psi_edge: id(395, EdgeId::new),
                            cleanup_actions: vec![
                                psi_terminal::TerminalAffineCleanupAction::DiscardRoot(place),
                            ],
                        },
                    ],
                }],
            },
            FuelScheduleIdentity::new(1).expect("nonzero schedule"),
        )
        .expect("explicit affine local unit")
    }

    fn byte_literal_boundary_unit() -> PsiOptimizationUnit {
        let machine = id(4_600, MachineId::new);
        let block = id(4_601, BlockId::new);
        let boundary = id(4_602, BoundaryMachineId::new);
        let byte_type = id(4_603, StructuralTypeId::new);
        let literal = id(4_604, PlaceId::new);
        let boundary_place = id(4_605, PlaceId::new);
        let declaration = structural_type(
            4_603,
            psi_terminal::StructuralTypeShape::ByteSequence(
                psi_terminal::ByteSequenceCarrier::BorrowedView,
            ),
        );
        reconstruct_psi_optimization_unit_seed(
            &AbstractOperationPlan {
                psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([20; 32]),
                },
                entry: machine,
                structural_types: vec![declaration.clone()],
                boundary_machines: vec![psi_terminal::BoundaryMachineDeclaration {
                    id: boundary,
                    identity: "validation::byte-literal-boundary".into(),
                    attachment: None,
                    scalar_parameters: Vec::new(),
                    structural_parameters: vec![psi_terminal::StructuralParameterDeclaration {
                        place: boundary_place,
                        position: 0,
                        is_self: false,
                        structural_type: byte_type,
                        multiplicity: psi_terminal::StructuralMultiplicity::Unrestricted,
                        access: psi_terminal::StructuralAccess::SharedBorrow,
                        qualifications: Vec::new(),
                    }],
                    result: None,
                    requires: Vec::new(),
                    program_local_root_introductions: Vec::new(),
                    content_guarantees: Vec::new(),
                    published_service_ceiling: Vec::new(),
                }],
                provider_candidates: Vec::new(),
                functions: vec![AbstractFunction {
                    machine,
                    attachment: None,
                    entry: block,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    result: AbstractFunctionResult::Unit,
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![AbstractBlockEntry {
                        block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations: vec![
                        AbstractOperation::EstablishByteSequenceLiteral {
                            psi_operation: id(4_606, OperationId::new),
                            place: psi_terminal::StructuralPlaceDeclaration {
                                id: literal,
                                kind: StructuralPlaceKind::ByteSequenceLiteral {
                                    declaration_ordinal: 0,
                                    structural_type: byte_type,
                                },
                            },
                            structural_type: declaration,
                            bytes: vec![0, 0x7f, 0x80, 0xff],
                        },
                        AbstractOperation::BoundaryCall {
                            psi_operation: id(4_607, OperationId::new),
                            result: None,
                            boundary,
                            arguments: Vec::new(),
                            structural_arguments: vec![psi_terminal::StructuralArgument {
                                place: literal,
                                access: psi_terminal::StructuralAccess::SharedBorrow,
                                path: Vec::new(),
                            }],
                            completion_claim_sources: Vec::new(),
                            completion_receipts: Vec::new(),
                        },
                        AbstractOperation::ReturnUnit {
                            psi_edge: id(4_608, EdgeId::new),
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }],
            },
            FuelScheduleIdentity::new(1).expect("nonzero schedule"),
        )
        .expect("byte literal boundary unit")
    }

    fn refresh_function_derivatives(unit: &mut PsiOptimizationUnit, function_index: usize) {
        let function = &mut unit.functions[function_index];
        let mut effect = 0_u64;
        for block in &mut function.blocks {
            for (node_index, node) in block.nodes.iter_mut().enumerate() {
                let node_index = u32::try_from(node_index).expect("test node index fits u32");
                node.definitions = expected_definitions(&node.operation, block.id, node_index);
                node.uses = expected_uses(&node.operation, block.id, node_index);
                node.provenance = expected_provenance(&node.operation);
                node.fuel = node
                    .provenance
                    .iter()
                    .copied()
                    .map(|site| omega_optimization_unit::FuelSettlement { site, units: 1 })
                    .collect();
                node.effect = omega_optimization_unit::EffectLink {
                    input: effect,
                    output: effect + 1,
                };
                effect += 1;
                node.successors = expected_edges(&node.operation);
                node.ownership = expected_ownership(&node.operation);
            }
        }
        function.facts = reconstruct_fact_index(function);
        refresh_identity(unit);
    }

    fn byte_literal_dominating_non_topological_unit() -> PsiOptimizationUnit {
        let mut unit = byte_literal_boundary_unit();
        let producer = id(4_601, BlockId::new);
        let use_block = id(4_609, BlockId::new);
        let mut nodes = std::mem::take(&mut unit.functions[0].blocks[0].nodes).into_iter();
        let establish = nodes.next().expect("literal establishment");
        let boundary = nodes.next().expect("literal boundary use");
        let returned = nodes.next().expect("Unit return");
        let mut jump = returned.clone();
        jump.operation = AbstractOperation::Jump {
            psi_edge: id(4_610, EdgeId::new),
            target: use_block,
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        };
        unit.functions[0].entry = producer;
        unit.functions[0].blocks = vec![
            omega_optimization_unit::OptimizationBlock {
                id: use_block,
                parameters: Vec::new(),
                nodes: vec![boundary, returned],
            },
            omega_optimization_unit::OptimizationBlock {
                id: producer,
                parameters: Vec::new(),
                nodes: vec![establish, jump],
            },
        ];
        refresh_function_derivatives(&mut unit, 0);
        unit
    }

    fn byte_literal_sibling_use_unit() -> PsiOptimizationUnit {
        let mut unit = byte_literal_boundary_unit();
        let entry = id(4_611, BlockId::new);
        let producer = id(4_612, BlockId::new);
        let use_block = id(4_613, BlockId::new);
        let condition = id(4_614, ValueId::new);
        let mut nodes = std::mem::take(&mut unit.functions[0].blocks[0].nodes).into_iter();
        let establish = nodes.next().expect("literal establishment");
        let boundary = nodes.next().expect("literal boundary use");
        let returned = nodes.next().expect("Unit return");
        let mut boolean = establish.clone();
        boolean.operation = AbstractOperation::BooleanConstant {
            psi_operation: id(4_615, OperationId::new),
            result: condition,
            value: true,
        };
        let mut conditional = returned.clone();
        conditional.operation = AbstractOperation::Conditional {
            condition,
            when_true: omega_abstract_operations::AbstractSuccessor {
                psi_edge: id(4_616, EdgeId::new),
                target: producer,
                bindings: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
            when_false: omega_abstract_operations::AbstractSuccessor {
                psi_edge: id(4_617, EdgeId::new),
                target: use_block,
                bindings: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
        };
        let mut producer_return = returned.clone();
        producer_return.operation = AbstractOperation::ReturnUnit {
            psi_edge: id(4_618, EdgeId::new),
            cleanup_actions: Vec::new(),
        };
        unit.functions[0].entry = entry;
        unit.functions[0].blocks = vec![
            omega_optimization_unit::OptimizationBlock {
                id: entry,
                parameters: Vec::new(),
                nodes: vec![boolean, conditional],
            },
            omega_optimization_unit::OptimizationBlock {
                id: producer,
                parameters: Vec::new(),
                nodes: vec![establish, producer_return],
            },
            omega_optimization_unit::OptimizationBlock {
                id: use_block,
                parameters: Vec::new(),
                nodes: vec![boundary, returned],
            },
        ];
        refresh_function_derivatives(&mut unit, 0);
        unit
    }

    fn byte_literal_partial_predecessor_unit() -> PsiOptimizationUnit {
        let mut unit = byte_literal_boundary_unit();
        let entry = id(4_630, BlockId::new);
        let producer = id(4_631, BlockId::new);
        let bypass = id(4_632, BlockId::new);
        let join = id(4_633, BlockId::new);
        let condition = id(4_634, ValueId::new);
        let mut nodes = std::mem::take(&mut unit.functions[0].blocks[0].nodes).into_iter();
        let establish = nodes.next().expect("literal establishment");
        let boundary = nodes.next().expect("literal boundary use");
        let returned = nodes.next().expect("Unit return");
        let mut boolean = establish.clone();
        boolean.operation = AbstractOperation::BooleanConstant {
            psi_operation: id(4_635, OperationId::new),
            result: condition,
            value: true,
        };
        let mut conditional = returned.clone();
        conditional.operation = AbstractOperation::Conditional {
            condition,
            when_true: omega_abstract_operations::AbstractSuccessor {
                psi_edge: id(4_636, EdgeId::new),
                target: producer,
                bindings: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
            when_false: omega_abstract_operations::AbstractSuccessor {
                psi_edge: id(4_637, EdgeId::new),
                target: bypass,
                bindings: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
        };
        let jump = |edge| AbstractOperation::Jump {
            psi_edge: id(edge, EdgeId::new),
            target: join,
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        };
        let mut producer_jump = returned.clone();
        producer_jump.operation = jump(4_638);
        let mut bypass_jump = returned.clone();
        bypass_jump.operation = jump(4_639);
        unit.functions[0].entry = entry;
        unit.functions[0].blocks = vec![
            omega_optimization_unit::OptimizationBlock {
                id: entry,
                parameters: Vec::new(),
                nodes: vec![boolean, conditional],
            },
            omega_optimization_unit::OptimizationBlock {
                id: producer,
                parameters: Vec::new(),
                nodes: vec![establish, producer_jump],
            },
            omega_optimization_unit::OptimizationBlock {
                id: bypass,
                parameters: Vec::new(),
                nodes: vec![bypass_jump],
            },
            omega_optimization_unit::OptimizationBlock {
                id: join,
                parameters: Vec::new(),
                nodes: vec![boundary, returned],
            },
        ];
        refresh_function_derivatives(&mut unit, 0);
        unit
    }

    fn explicit_local_dominating_non_topological_unit() -> PsiOptimizationUnit {
        let mut unit = explicit_trivial_affine_return_unit();
        let producer = id(391, BlockId::new);
        let cleanup = id(4_640, BlockId::new);
        let mut nodes = std::mem::take(&mut unit.functions[0].blocks[0].nodes).into_iter();
        let establish = nodes.next().expect("local establishment");
        let returned = nodes.next().expect("local cleanup return");
        let mut jump = returned.clone();
        jump.operation = AbstractOperation::Jump {
            psi_edge: id(4_641, EdgeId::new),
            target: cleanup,
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        };
        unit.functions[0].blocks = vec![
            omega_optimization_unit::OptimizationBlock {
                id: cleanup,
                parameters: Vec::new(),
                nodes: vec![returned],
            },
            omega_optimization_unit::OptimizationBlock {
                id: producer,
                parameters: Vec::new(),
                nodes: vec![establish, jump],
            },
        ];
        refresh_function_derivatives(&mut unit, 0);
        unit
    }

    fn explicit_local_same_block_use_before_definition_unit() -> PsiOptimizationUnit {
        let mut unit = explicit_trivial_affine_return_unit();
        let local = id(393, PlaceId::new);
        let mut observation = unit.functions[0].blocks[0].nodes[0].clone();
        observation.operation = AbstractOperation::BooleanStructuralField {
            psi_operation: id(4_642, OperationId::new),
            result: id(4_643, ValueId::new),
            source: local,
            field: id(4_644, psi_core::StructuralFieldId::new),
        };
        unit.functions[0].blocks[0].nodes.insert(0, observation);
        refresh_function_derivatives(&mut unit, 0);
        unit
    }

    fn explicit_local_sibling_cleanup_unit() -> PsiOptimizationUnit {
        let mut unit = explicit_trivial_affine_return_unit();
        let entry = id(4_620, BlockId::new);
        let producer = id(4_621, BlockId::new);
        let cleanup = id(4_622, BlockId::new);
        let condition = id(4_623, ValueId::new);
        let mut nodes = std::mem::take(&mut unit.functions[0].blocks[0].nodes).into_iter();
        let establish = nodes.next().expect("local establishment");
        let returned = nodes.next().expect("local cleanup return");
        let mut boolean = establish.clone();
        boolean.operation = AbstractOperation::BooleanConstant {
            psi_operation: id(4_624, OperationId::new),
            result: condition,
            value: true,
        };
        let mut conditional = returned.clone();
        conditional.operation = AbstractOperation::Conditional {
            condition,
            when_true: omega_abstract_operations::AbstractSuccessor {
                psi_edge: id(4_625, EdgeId::new),
                target: producer,
                bindings: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
            when_false: omega_abstract_operations::AbstractSuccessor {
                psi_edge: id(4_626, EdgeId::new),
                target: cleanup,
                bindings: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
        };
        let mut producer_return = returned.clone();
        producer_return.operation = AbstractOperation::ReturnUnit {
            psi_edge: id(4_627, EdgeId::new),
            cleanup_actions: match &returned.operation {
                AbstractOperation::ReturnUnit {
                    cleanup_actions, ..
                } => cleanup_actions.clone(),
                _ => unreachable!("fixture return"),
            },
        };
        unit.functions[0].entry = entry;
        unit.functions[0].blocks = vec![
            omega_optimization_unit::OptimizationBlock {
                id: entry,
                parameters: Vec::new(),
                nodes: vec![boolean, conditional],
            },
            omega_optimization_unit::OptimizationBlock {
                id: producer,
                parameters: Vec::new(),
                nodes: vec![establish, producer_return],
            },
            omega_optimization_unit::OptimizationBlock {
                id: cleanup,
                parameters: Vec::new(),
                nodes: vec![returned],
            },
        ];
        refresh_function_derivatives(&mut unit, 0);
        unit
    }

    #[derive(Clone, Copy)]
    enum OperationResultCfgShape {
        DominatingNonTopological,
        SiblingReturn,
        PartialPredecessor,
    }

    fn operation_result_cfg_unit(shape: OperationResultCfgShape) -> PsiOptimizationUnit {
        use omega_abstract_operations::AbstractSuccessor;

        let caller = id(370, MachineId::new);
        let callee = id(371, MachineId::new);
        let entry = id(372, BlockId::new);
        let producer_block = id(373, BlockId::new);
        let bypass_block = id(374, BlockId::new);
        let join = id(375, BlockId::new);
        let callee_block = id(376, BlockId::new);
        let condition = id(377, ValueId::new);
        let structural_type = id(378, StructuralTypeId::new);
        let callee_result = id(379, PlaceId::new);
        let caller_result = id(380, PlaceId::new);
        let call_result = id(381, PlaceId::new);
        let caller_input = id(389, PlaceId::new);
        let callee_input = id(390, PlaceId::new);
        let claim = id(1, ClaimId::new);
        let parameter = |place| psi_terminal::StructuralParameterDeclaration {
            place,
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: psi_terminal::StructuralMultiplicity::Linear,
            access: psi_terminal::StructuralAccess::Owned,
            qualifications: Vec::new(),
        };
        let entry_claim = |input| psi_terminal::EntryClaim {
            claim,
            input,
            path: Vec::new(),
        };
        let call = || AbstractOperation::CallStructural {
            psi_operation: id(382, OperationId::new),
            result: psi_terminal::StructuralOperationResult {
                place: call_result,
                structural_type,
                multiplicity: psi_terminal::StructuralMultiplicity::Linear,
                qualifications: Vec::new(),
                claims: vec![psi_terminal::StructuralResultClaimBinding {
                    claim,
                    path: Vec::new(),
                }],
            },
            callee,
            structural_arguments: vec![psi_terminal::StructuralArgument {
                place: caller_input,
                path: Vec::new(),
                access: psi_terminal::StructuralAccess::Owned,
            }],
            claim_transfers: vec![psi_terminal::ClaimTransfer {
                claim,
                argument_index: 0,
            }],
            returned_claim_transfers: vec![psi_terminal::StructuralResultClaimTransfer {
                callee_claim: claim,
                caller_claim: claim,
            }],
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
            selected_evidence: None,
        };
        let return_result = |edge| AbstractOperation::ReturnStructural {
            psi_edge: edge,
            source: call_result,
            returned_claims: vec![claim],
            trivial_affine_locals: Vec::new(),
            trivial_affine_discards: Vec::new(),
        };
        let jump = |edge| AbstractOperation::Jump {
            psi_edge: edge,
            target: join,
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        };
        let conditional = || AbstractOperation::Conditional {
            condition,
            when_true: AbstractSuccessor {
                psi_edge: id(383, EdgeId::new),
                target: producer_block,
                bindings: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
            when_false: AbstractSuccessor {
                psi_edge: id(384, EdgeId::new),
                target: bypass_block,
                bindings: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
        };
        let (block_entries, operations) = match shape {
            OperationResultCfgShape::DominatingNonTopological => (
                vec![
                    AbstractBlockEntry {
                        block: join,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    },
                    AbstractBlockEntry {
                        block: producer_block,
                        parameters: Vec::new(),
                        operation_offset: 1,
                    },
                    AbstractBlockEntry {
                        block: bypass_block,
                        parameters: Vec::new(),
                        operation_offset: 2,
                    },
                    AbstractBlockEntry {
                        block: entry,
                        parameters: Vec::new(),
                        operation_offset: 3,
                    },
                ],
                vec![
                    return_result(id(385, EdgeId::new)),
                    jump(id(386, EdgeId::new)),
                    jump(id(387, EdgeId::new)),
                    call(),
                    conditional(),
                ],
            ),
            OperationResultCfgShape::SiblingReturn => (
                vec![
                    AbstractBlockEntry {
                        block: entry,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    },
                    AbstractBlockEntry {
                        block: producer_block,
                        parameters: Vec::new(),
                        operation_offset: 1,
                    },
                    AbstractBlockEntry {
                        block: bypass_block,
                        parameters: Vec::new(),
                        operation_offset: 3,
                    },
                ],
                vec![
                    conditional(),
                    call(),
                    return_result(id(385, EdgeId::new)),
                    return_result(id(386, EdgeId::new)),
                ],
            ),
            OperationResultCfgShape::PartialPredecessor => (
                vec![
                    AbstractBlockEntry {
                        block: entry,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    },
                    AbstractBlockEntry {
                        block: producer_block,
                        parameters: Vec::new(),
                        operation_offset: 1,
                    },
                    AbstractBlockEntry {
                        block: bypass_block,
                        parameters: Vec::new(),
                        operation_offset: 3,
                    },
                    AbstractBlockEntry {
                        block: join,
                        parameters: Vec::new(),
                        operation_offset: 4,
                    },
                ],
                vec![
                    conditional(),
                    call(),
                    jump(id(385, EdgeId::new)),
                    jump(id(386, EdgeId::new)),
                    return_result(id(387, EdgeId::new)),
                ],
            ),
        };
        let plan = AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([17; 32]),
            },
            entry: caller,
            structural_types: vec![psi_terminal::StructuralTypeDeclaration {
                id: structural_type,
                identity: "validation::operation-result-availability".into(),
                shape: psi_terminal::StructuralTypeShape::ByteSequence(
                    psi_terminal::ByteSequenceCarrier::BorrowedView,
                ),
            }],
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![
                AbstractFunction {
                    machine: caller,
                    attachment: None,
                    entry,
                    parameters: vec![AbstractParameter {
                        value: condition,
                        scalar_type: ScalarType::Boolean,
                    }],
                    structural_parameters: vec![parameter(caller_input)],
                    result: AbstractFunctionResult::Structural(
                        psi_terminal::StructuralResultDeclaration {
                            place: caller_result,
                            structural_type,
                            multiplicity: psi_terminal::StructuralMultiplicity::Linear,
                            qualifications: Vec::new(),
                        },
                    ),
                    entry_claims: vec![entry_claim(caller_input)],
                    published_service_ceiling: Vec::new(),
                    block_entries,
                    operations,
                },
                AbstractFunction {
                    machine: callee,
                    attachment: None,
                    entry: callee_block,
                    parameters: Vec::new(),
                    structural_parameters: vec![parameter(callee_input)],
                    result: AbstractFunctionResult::Structural(
                        psi_terminal::StructuralResultDeclaration {
                            place: callee_result,
                            structural_type,
                            multiplicity: psi_terminal::StructuralMultiplicity::Linear,
                            qualifications: Vec::new(),
                        },
                    ),
                    entry_claims: vec![entry_claim(callee_input)],
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![AbstractBlockEntry {
                        block: callee_block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations: vec![AbstractOperation::ReturnStructural {
                        psi_edge: id(388, EdgeId::new),
                        source: callee_input,
                        returned_claims: vec![claim],
                        trivial_affine_locals: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    }],
                },
            ],
        };
        reconstruct_psi_optimization_unit_seed(&plan, FuelScheduleIdentity::new(1).unwrap())
            .unwrap()
    }

    fn redundant_parameter_region_fixture() -> (
        PsiOptimizationUnit,
        PsiOptimizationUnit,
        RedundantBlockParameterRewrite,
        Vec<BlockId>,
    ) {
        use omega_abstract_operations::{AbstractSuccessor, ValueBinding};

        let machine = id(701, MachineId::new);
        let entry = id(702, BlockId::new);
        let merge = id(703, BlockId::new);
        let condition = id(704, ValueId::new);
        let shared = id(705, ValueId::new);
        let alternate = id(706, ValueId::new);
        let parameter = id(707, ValueId::new);
        let result = id(708, ValueId::new);
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let scalar_type = ScalarType::Integer(integer);
        let binding = || ValueBinding {
            parameter,
            argument: shared,
            scalar_type,
        };
        let input = reconstruct_psi_optimization_unit_seed(
            &AbstractOperationPlan {
                psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([22; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![AbstractFunction {
                    machine,
                    attachment: None,
                    entry,
                    parameters: vec![
                        AbstractParameter {
                            value: condition,
                            scalar_type: ScalarType::Boolean,
                        },
                        AbstractParameter {
                            value: shared,
                            scalar_type,
                        },
                        AbstractParameter {
                            value: alternate,
                            scalar_type,
                        },
                    ],
                    structural_parameters: Vec::new(),
                    result: AbstractFunctionResult::Scalar(AbstractResult {
                        value: result,
                        scalar_type,
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![
                        AbstractBlockEntry {
                            block: entry,
                            parameters: Vec::new(),
                            operation_offset: 0,
                        },
                        AbstractBlockEntry {
                            block: merge,
                            parameters: vec![AbstractParameter {
                                value: parameter,
                                scalar_type,
                            }],
                            operation_offset: 1,
                        },
                    ],
                    operations: vec![
                        AbstractOperation::Conditional {
                            condition,
                            when_true: AbstractSuccessor {
                                psi_edge: id(709, EdgeId::new),
                                target: merge,
                                bindings: vec![binding()],
                                trivial_affine_discards: Vec::new(),
                            },
                            when_false: AbstractSuccessor {
                                psi_edge: id(710, EdgeId::new),
                                target: merge,
                                bindings: vec![binding()],
                                trivial_affine_discards: Vec::new(),
                            },
                        },
                        AbstractOperation::ExactIntegerAdd {
                            psi_operation: id(711, OperationId::new),
                            obligation: id(713, psi_core::ObligationId::new),
                            result,
                            scalar_type: integer,
                            left: parameter,
                            right: alternate,
                        },
                        AbstractOperation::Return {
                            psi_edge: id(712, EdgeId::new),
                            result,
                            value: result,
                            scalar_type,
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }],
            },
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        let patch = RedundantBlockParameterRewrite {
            machine,
            block: merge,
            position: 0,
            parameter,
            replacement: shared,
            scalar_type,
        };
        let affected = vec![entry, merge];
        let output = normalize_redundant_parameter_observation_input(&input, patch, &affected)
            .expect("exact structural normalization");
        (input, output, patch, affected)
    }

    fn integer_candidate(
        unit: &PsiOptimizationUnit,
        constant: IntegerValue,
    ) -> PsiRewriteCandidate {
        integer_candidate_with_facts(unit, constant, None, None)
    }

    fn integer_candidate_with_facts(
        unit: &PsiOptimizationUnit,
        constant: IntegerValue,
        supplied_left_fact: Option<omega_optimization_core::ScalarConstantFactIdentity>,
        supplied_obligation_fact: Option<omega_optimization_core::AcceptedObligationFactIdentity>,
    ) -> PsiRewriteCandidate {
        integer_candidate_with_facts_and_cost(
            unit,
            constant,
            supplied_left_fact,
            supplied_obligation_fact,
            -1,
        )
    }

    fn integer_candidate_with_facts_and_cost(
        unit: &PsiOptimizationUnit,
        constant: IntegerValue,
        supplied_left_fact: Option<omega_optimization_core::ScalarConstantFactIdentity>,
        supplied_obligation_fact: Option<omega_optimization_core::AcceptedObligationFactIdentity>,
        predicted_cost_delta: i64,
    ) -> PsiRewriteCandidate {
        let function = &unit.functions[0];
        let block = &function.blocks[0];
        let node = &block.nodes[2];
        let AbstractOperation::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } = node.operation
        else {
            panic!("fixture contains exact add")
        };
        let location = NodeLocation {
            machine: function.machine,
            block: block.id,
            node: 2,
        };
        let contract = OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(b"fold-exact-add"),
            OptimizationPassIdentity::from_canonical_bytes(b"constant-evaluation"),
            1,
            AnalysisSet::new([AnalysisKind::ScalarConstants]),
            AnalysisInvalidationSet::new([AnalysisKind::UseDefinition]),
            OptimizationSafetyClass::ProofCertified,
        )
        .unwrap();
        PsiRewriteCandidate::new_integer_evaluation(
            unit.identity,
            contract,
            vec![block.id],
            Vec::new(),
            vec![ProvenanceRewrite {
                input: PsiRealizationSite::Node(location),
                disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(location)),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            }],
            IntegerEvaluationWitness::ProofCertifiedBinary {
                left_fact: supplied_left_fact.unwrap_or_else(|| {
                    literal_scalar_constant_fact_identity(
                        unit.identity,
                        function.machine,
                        scalar_value_definition(function, left).unwrap(),
                        ScalarConstantValue::Integer(IntegerValue::Unsigned(7)),
                        id(206, OperationId::new),
                    )
                    .unwrap()
                }),
                right_fact: literal_scalar_constant_fact_identity(
                    unit.identity,
                    function.machine,
                    scalar_value_definition(function, right).unwrap(),
                    ScalarConstantValue::Integer(IntegerValue::Unsigned(8)),
                    id(207, OperationId::new),
                )
                .unwrap(),
                obligation_fact: supplied_obligation_fact
                    .unwrap_or(unit.accepted_obligation_facts[0].identity),
            },
            predicted_cost_delta,
            IntegerConstantRewrite {
                location,
                source_operation: psi_operation,
                result,
                scalar_type,
                constant,
            },
        )
        .unwrap()
    }

    #[test]
    fn independently_accepts_builder_output() {
        validate_psi_optimization_unit(&unit()).unwrap();
        validate_psi_optimization_unit(&scalar_call_unit()).unwrap();
        validate_psi_optimization_unit(&scalar_boundary_call_unit()).unwrap();
    }

    #[test]
    fn operation_result_return_accepts_cross_block_dominance_independent_of_storage_order() {
        let candidate =
            operation_result_cfg_unit(OperationResultCfgShape::DominatingNonTopological);
        assert_ne!(
            candidate.functions[0].blocks[0].id, candidate.functions[0].entry,
            "fixture stores the return block before its dominating producer block"
        );
        validate_psi_optimization_unit(&candidate)
            .expect("CallStructural result dominates the structural return through the CFG");
    }

    #[test]
    fn byte_literal_catalog_and_exact_establishment_correspondence_validate() {
        let baseline = byte_literal_boundary_unit();
        validate_psi_optimization_unit(&baseline)
            .expect("one exact borrowed-view literal establishment validates");

        let mut ordinal_gap = baseline.clone();
        let StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal,
            ..
        } = &mut ordinal_gap.functions[0].structural_places[0].kind
        else {
            panic!("fixture retains its byte-literal place")
        };
        *declaration_ordinal = 1;
        let AbstractOperation::EstablishByteSequenceLiteral { place, .. } =
            &mut ordinal_gap.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("fixture begins with its literal establishment")
        };
        let StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal,
            ..
        } = &mut place.kind
        else {
            panic!("establishment retains its literal declaration")
        };
        *declaration_ordinal = 1;
        refresh_function_derivatives(&mut ordinal_gap, 0);
        assert_eq!(
            validate_psi_optimization_unit(&ordinal_gap),
            Err(
                OptimizationUnitValidationError::NonCanonicalByteSequenceLiterals(id(
                    4_600,
                    MachineId::new
                ))
            )
        );

        let mut wrong_carrier = baseline.clone();
        wrong_carrier.structural_types[0].shape =
            psi_terminal::StructuralTypeShape::Record { fields: Vec::new() };
        refresh_identity(&mut wrong_carrier);
        assert_eq!(
            validate_psi_optimization_unit(&wrong_carrier),
            Err(
                OptimizationUnitValidationError::ByteSequenceLiteralDeclarationRequiresBorrowedView {
                    machine: id(4_600, MachineId::new),
                    place: id(4_604, PlaceId::new),
                }
            )
        );

        let expected = OptimizationUnitValidationError::ByteSequenceLiteralEstablishmentMismatch(
            id(4_600, MachineId::new),
        );
        let mut missing = baseline.clone();
        missing.functions[0].blocks[0].nodes.remove(0);
        refresh_function_derivatives(&mut missing, 0);
        assert_eq!(
            validate_psi_optimization_unit(&missing),
            Err(expected.clone())
        );

        let mut forged_type = baseline.clone();
        let AbstractOperation::EstablishByteSequenceLiteral {
            structural_type, ..
        } = &mut forged_type.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("fixture begins with its literal establishment")
        };
        structural_type.identity.push_str("::forged");
        refresh_function_derivatives(&mut forged_type, 0);
        assert_eq!(
            validate_psi_optimization_unit(&forged_type),
            Err(expected.clone())
        );

        let mut duplicate = baseline;
        let mut second = duplicate.functions[0].blocks[0].nodes[0].clone();
        let AbstractOperation::EstablishByteSequenceLiteral { psi_operation, .. } =
            &mut second.operation
        else {
            panic!("fixture begins with its literal establishment")
        };
        *psi_operation = id(4_619, OperationId::new);
        duplicate.functions[0].blocks[0].nodes.insert(1, second);
        refresh_function_derivatives(&mut duplicate, 0);
        assert_eq!(
            validate_psi_optimization_unit(&duplicate),
            Err(
                OptimizationUnitValidationError::ByteSequenceLiteralEstablishmentMismatch(id(
                    4_600,
                    MachineId::new
                ))
            )
        );

        let mut two_literals = byte_literal_boundary_unit();
        let second_place = id(4_645, PlaceId::new);
        let second_declaration = psi_terminal::StructuralPlaceDeclaration {
            id: second_place,
            kind: StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal: 1,
                structural_type: id(4_603, StructuralTypeId::new),
            },
        };
        let mut second = two_literals.functions[0].blocks[0].nodes[0].clone();
        let AbstractOperation::EstablishByteSequenceLiteral {
            psi_operation,
            place,
            bytes,
            ..
        } = &mut second.operation
        else {
            panic!("fixture begins with its literal establishment")
        };
        *psi_operation = id(4_646, OperationId::new);
        *place = second_declaration;
        *bytes = vec![1, 2, 3];
        two_literals.functions[0]
            .structural_places
            .push(second_declaration);
        two_literals.functions[0]
            .declared_places
            .insert(second_place);
        two_literals.functions[0].blocks[0].nodes.insert(1, second);
        refresh_function_derivatives(&mut two_literals, 0);
        validate_psi_optimization_unit(&two_literals)
            .expect("two dense exact literal witnesses validate independent of use count");
    }

    #[test]
    fn explicit_structural_roots_require_current_cfg_availability() {
        let dominating = byte_literal_dominating_non_topological_unit();
        assert_ne!(
            dominating.functions[0].blocks[0].id, dominating.functions[0].entry,
            "fixture stores the literal use block before its producer block"
        );
        validate_psi_optimization_unit(&dominating)
            .expect("a dominating byte-literal producer is available independent of storage order");

        let local_dominating = explicit_local_dominating_non_topological_unit();
        assert_ne!(
            local_dominating.functions[0].blocks[0].id, local_dominating.functions[0].entry,
            "fixture stores the local cleanup block before its producer block"
        );
        validate_psi_optimization_unit(&local_dominating)
            .expect("a dominating explicit local establishment reaches cleanup");

        let mut same_block = byte_literal_boundary_unit();
        same_block.functions[0].blocks[0].nodes.swap(0, 1);
        refresh_function_derivatives(&mut same_block, 0);
        assert_eq!(
            validate_psi_optimization_unit(&same_block),
            Err(
                OptimizationUnitValidationError::StructuralPlaceNotAvailable {
                    machine: id(4_600, MachineId::new),
                    block: id(4_601, BlockId::new),
                    node: 0,
                    place: id(4_604, PlaceId::new),
                }
            )
        );

        let sibling = byte_literal_sibling_use_unit();
        assert_eq!(
            validate_psi_optimization_unit(&sibling),
            Err(
                OptimizationUnitValidationError::StructuralPlaceNotAvailable {
                    machine: id(4_600, MachineId::new),
                    block: id(4_613, BlockId::new),
                    node: 0,
                    place: id(4_604, PlaceId::new),
                }
            )
        );

        let partial = byte_literal_partial_predecessor_unit();
        assert_eq!(
            validate_psi_optimization_unit(&partial),
            Err(
                OptimizationUnitValidationError::StructuralPlaceNotAvailable {
                    machine: id(4_600, MachineId::new),
                    block: id(4_633, BlockId::new),
                    node: 0,
                    place: id(4_604, PlaceId::new),
                }
            )
        );

        let local_same_block = explicit_local_same_block_use_before_definition_unit();
        assert_eq!(
            validate_psi_optimization_unit(&local_same_block),
            Err(
                OptimizationUnitValidationError::StructuralPlaceNotAvailable {
                    machine: id(390, MachineId::new),
                    block: id(391, BlockId::new),
                    node: 0,
                    place: id(393, PlaceId::new),
                }
            )
        );

        let local_cleanup = explicit_local_sibling_cleanup_unit();
        assert_eq!(
            validate_psi_optimization_unit(&local_cleanup),
            Err(
                OptimizationUnitValidationError::StructuralPlaceNotAvailable {
                    machine: id(390, MachineId::new),
                    block: id(4_622, BlockId::new),
                    node: 0,
                    place: id(393, PlaceId::new),
                }
            )
        );
    }

    #[test]
    fn trivial_affine_locals_accept_explicit_and_exact_compressed_witnesses() {
        let compressed = compressed_trivial_affine_return_unit();
        let local_places = compressed.functions[0]
            .structural_places
            .iter()
            .filter_map(|place| {
                matches!(place.kind, StructuralPlaceKind::TrivialAffineLocal { .. })
                    .then_some(place.id)
            })
            .collect::<Vec<_>>();
        assert_eq!(local_places.len(), 2);
        assert!(
            local_places
                .iter()
                .all(|place| !compressed.functions[0].declared_places.contains(place)),
            "compressed no-ABI locals are not executable place roots"
        );
        let node = &compressed.functions[0].blocks[0].nodes[0];
        let O::ReturnStructural {
            psi_edge,
            trivial_affine_locals,
            ..
        } = &node.operation
        else {
            panic!("compressed fixture returns structurally")
        };
        let expected_custody = std::iter::once(PsiProvenance::Edge(*psi_edge))
            .chain(
                trivial_affine_locals
                    .iter()
                    .map(|(operation, _, _)| PsiProvenance::Operation(*operation)),
            )
            .collect::<Vec<_>>();
        assert_eq!(node.provenance, expected_custody);
        assert_eq!(
            node.fuel
                .iter()
                .map(|settlement| (settlement.site, settlement.units))
                .collect::<Vec<_>>(),
            expected_custody
                .iter()
                .copied()
                .map(|site| (site, 1))
                .collect::<Vec<_>>()
        );
        validate_psi_optimization_unit(&compressed)
            .expect("exact compressed local declarations and reverse cleanup validate");
        validate_psi_optimization_unit(&explicit_trivial_affine_return_unit())
            .expect("an exact executable establishment remains a valid local witness");
    }

    #[test]
    fn trivial_affine_local_catalog_requires_dense_empty_record_declarations() {
        let machine = id(360, MachineId::new);
        let second_local = id(368, PlaceId::new);

        let mut ordinal_gap = compressed_trivial_affine_return_unit();
        let second = ordinal_gap.functions[0]
            .structural_places
            .iter_mut()
            .find(|place| place.id == second_local)
            .expect("second local catalog row");
        let StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal,
            ..
        } = &mut second.kind
        else {
            panic!("fixture local has a local kind")
        };
        *declaration_ordinal = 2;
        refresh_identity(&mut ordinal_gap);
        assert_eq!(
            validate_psi_optimization_unit(&ordinal_gap),
            Err(OptimizationUnitValidationError::NonCanonicalTrivialAffineLocals(machine))
        );

        let mut nonempty_carrier = compressed_trivial_affine_return_unit();
        nonempty_carrier.structural_types[0].shape =
            psi_terminal::StructuralTypeShape::ByteSequence(
                psi_terminal::ByteSequenceCarrier::BorrowedView,
            );
        refresh_identity(&mut nonempty_carrier);
        assert_eq!(
            validate_psi_optimization_unit(&nonempty_carrier),
            Err(
                OptimizationUnitValidationError::TrivialAffineLocalDeclarationRequiresEmptyRecord {
                    machine,
                    place: id(367, PlaceId::new),
                }
            )
        );

        let mut forged_explicit = explicit_trivial_affine_return_unit();
        let AbstractOperation::EstablishTrivialAffineLocal {
            structural_type, ..
        } = &mut forged_explicit.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("fixture begins with an explicit local establishment")
        };
        structural_type.identity.push_str("::forged");
        refresh_node_derivatives(&mut forged_explicit, 0, 0, 0);
        assert_eq!(
            validate_psi_optimization_unit(&forged_explicit),
            Err(
                OptimizationUnitValidationError::TrivialAffineLocalEstablishmentMismatch(id(
                    390,
                    MachineId::new
                ))
            )
        );
    }

    #[test]
    fn compressed_trivial_affine_tuple_is_exact_and_hidden_operations_are_unique() {
        let expected =
            OptimizationUnitValidationError::StructuralReturnTrivialAffineLocalsMismatch {
                machine: id(360, MachineId::new),
                block: id(361, BlockId::new),
                node: 0,
            };

        let mut missing = compressed_trivial_affine_return_unit();
        let AbstractOperation::ReturnStructural {
            trivial_affine_locals,
            ..
        } = &mut missing.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("fixture is a structural return")
        };
        trivial_affine_locals.pop();
        refresh_node_derivatives(&mut missing, 0, 0, 0);
        assert_eq!(
            validate_psi_optimization_unit(&missing),
            Err(expected.clone())
        );

        let mut extra = compressed_trivial_affine_return_unit();
        let AbstractOperation::ReturnStructural {
            trivial_affine_locals,
            ..
        } = &mut extra.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("fixture is a structural return")
        };
        trivial_affine_locals.push(trivial_affine_locals[0].clone());
        refresh_node_derivatives(&mut extra, 0, 0, 0);
        assert_eq!(
            validate_psi_optimization_unit(&extra),
            Err(expected.clone())
        );

        let mut reordered = compressed_trivial_affine_return_unit();
        let AbstractOperation::ReturnStructural {
            trivial_affine_locals,
            ..
        } = &mut reordered.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("fixture is a structural return")
        };
        trivial_affine_locals.swap(0, 1);
        refresh_node_derivatives(&mut reordered, 0, 0, 0);
        assert_eq!(
            validate_psi_optimization_unit(&reordered),
            Err(expected.clone())
        );

        let mut forged_place = compressed_trivial_affine_return_unit();
        let AbstractOperation::ReturnStructural {
            trivial_affine_locals,
            ..
        } = &mut forged_place.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("fixture is a structural return")
        };
        trivial_affine_locals[0].1.id = id(389, PlaceId::new);
        refresh_node_derivatives(&mut forged_place, 0, 0, 0);
        assert_eq!(
            validate_psi_optimization_unit(&forged_place),
            Err(expected.clone())
        );

        let mut forged_type = compressed_trivial_affine_return_unit();
        let AbstractOperation::ReturnStructural {
            trivial_affine_locals,
            ..
        } = &mut forged_type.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("fixture is a structural return")
        };
        trivial_affine_locals[0].2.identity.push_str("::forged");
        refresh_node_derivatives(&mut forged_type, 0, 0, 0);
        assert_eq!(
            validate_psi_optimization_unit(&forged_type),
            Err(expected.clone())
        );

        let mut duplicate_operation = compressed_trivial_affine_return_unit();
        let AbstractOperation::ReturnStructural {
            trivial_affine_locals,
            ..
        } = &mut duplicate_operation.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("fixture is a structural return")
        };
        trivial_affine_locals[1].0 = trivial_affine_locals[0].0;
        refresh_node_derivatives(&mut duplicate_operation, 0, 0, 0);
        assert_eq!(
            validate_psi_optimization_unit(&duplicate_operation),
            Err(expected.clone())
        );

        let executable_collision = compressed_trivial_affine_return_unit_with_prefix(true, false);
        assert_eq!(
            validate_psi_optimization_unit(&executable_collision),
            Err(
                OptimizationUnitValidationError::StructuralReturnTrivialAffineLocalsMismatch {
                    machine: id(360, MachineId::new),
                    block: id(361, BlockId::new),
                    node: 1,
                }
            )
        );

        let mixed_witnesses = compressed_trivial_affine_return_unit_with_prefix(false, true);
        assert_eq!(
            validate_psi_optimization_unit(&mixed_witnesses),
            Err(
                OptimizationUnitValidationError::TrivialAffineLocalEstablishmentMismatch(id(
                    360,
                    MachineId::new
                ))
            )
        );
    }

    #[test]
    fn retained_affine_authority_rejects_order_and_frontier_corruption() {
        let unit = compressed_trivial_affine_return_unit();
        let function = &unit.functions[0];
        let owned = |place, multiplicity| OwnershipFrontierOwnedPlace {
            place: id(place, PlaceId::new),
            multiplicity,
        };
        let entry = OwnershipFrontierSnapshot {
            claims: Vec::new(),
            owned_places: vec![
                owned(363, psi_terminal::StructuralMultiplicity::Linear),
                owned(364, psi_terminal::StructuralMultiplicity::Affine),
                owned(365, psi_terminal::StructuralMultiplicity::Affine),
                owned(367, psi_terminal::StructuralMultiplicity::Affine),
                owned(368, psi_terminal::StructuralMultiplicity::Affine),
            ],
            partial_custody: Vec::new(),
        };
        let exit = OwnershipFrontierSnapshot {
            claims: Vec::new(),
            owned_places: vec![owned(363, psi_terminal::StructuralMultiplicity::Linear)],
            partial_custody: Vec::new(),
        };
        let exact_discards = [
            id(368, PlaceId::new),
            id(367, PlaceId::new),
            id(365, PlaceId::new),
            id(364, PlaceId::new),
        ];
        assert!(valid_edge_affine_transition(
            function,
            &entry,
            &exit,
            &exact_discards,
        ));

        let mut reordered = exact_discards;
        reordered.swap(0, 1);
        assert!(!valid_edge_affine_transition(
            function, &entry, &exit, &reordered,
        ));
        assert!(!valid_edge_affine_transition(
            function,
            &entry,
            &entry,
            &exact_discards,
        ));

        let hidden_entry = OwnershipFrontierSnapshot {
            claims: Vec::new(),
            owned_places: vec![owned(363, psi_terminal::StructuralMultiplicity::Linear)],
            partial_custody: Vec::new(),
        };
        let hidden_exit = OwnershipFrontierSnapshot {
            claims: Vec::new(),
            owned_places: vec![
                owned(363, psi_terminal::StructuralMultiplicity::Linear),
                owned(367, psi_terminal::StructuralMultiplicity::Affine),
            ],
            partial_custody: Vec::new(),
        };
        assert!(valid_hidden_affine_establishment(
            &hidden_entry,
            &hidden_exit,
            id(367, PlaceId::new),
        ));
        let mut wrong_hidden_exit = hidden_exit;
        wrong_hidden_exit.owned_places[1].multiplicity =
            psi_terminal::StructuralMultiplicity::Unrestricted;
        assert!(!valid_hidden_affine_establishment(
            &hidden_entry,
            &wrong_hidden_exit,
            id(367, PlaceId::new),
        ));
    }

    #[test]
    fn compressed_trivial_affine_return_requires_exact_shape_and_reverse_discards() {
        let mut wrong_shape = compressed_trivial_affine_return_unit();
        wrong_shape.functions[0].structural_parameters[1].multiplicity =
            psi_terminal::StructuralMultiplicity::Unrestricted;
        refresh_identity(&mut wrong_shape);
        assert_eq!(
            validate_psi_optimization_unit(&wrong_shape),
            Err(
                OptimizationUnitValidationError::StructuralReturnTrivialAffineShapeMismatch {
                    machine: id(360, MachineId::new),
                    block: id(361, BlockId::new),
                    node: 0,
                }
            )
        );

        let mut wrong_discards = compressed_trivial_affine_return_unit();
        let AbstractOperation::ReturnStructural {
            trivial_affine_discards,
            ..
        } = &mut wrong_discards.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("fixture is a structural return")
        };
        trivial_affine_discards.swap(0, 1);
        refresh_node_derivatives(&mut wrong_discards, 0, 0, 0);
        assert_eq!(
            validate_psi_optimization_unit(&wrong_discards),
            Err(
                OptimizationUnitValidationError::StructuralReturnAffineDiscardsMismatch {
                    machine: id(360, MachineId::new),
                    block: id(361, BlockId::new),
                    node: 0,
                }
            )
        );

        let mut missing_discard = compressed_trivial_affine_return_unit();
        let AbstractOperation::ReturnStructural {
            trivial_affine_discards,
            ..
        } = &mut missing_discard.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("fixture is a structural return")
        };
        trivial_affine_discards.pop();
        refresh_node_derivatives(&mut missing_discard, 0, 0, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&missing_discard),
            Err(OptimizationUnitValidationError::StructuralReturnAffineDiscardsMismatch { .. })
        ));

        let mut extra_discard = compressed_trivial_affine_return_unit();
        let AbstractOperation::ReturnStructural {
            trivial_affine_discards,
            ..
        } = &mut extra_discard.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("fixture is a structural return")
        };
        trivial_affine_discards.push(id(388, PlaceId::new));
        refresh_node_derivatives(&mut extra_discard, 0, 0, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&extra_discard),
            Err(OptimizationUnitValidationError::StructuralReturnAffineDiscardsMismatch { .. })
        ));
    }

    #[test]
    fn operation_result_return_rejects_sibling_and_partial_predecessor_producers() {
        let call_result = id(381, PlaceId::new);

        let mut sibling = operation_result_cfg_unit(OperationResultCfgShape::SiblingReturn);
        refresh_node_derivatives(&mut sibling, 0, 2, 0);
        assert_eq!(
            validate_psi_optimization_unit(&sibling),
            Err(
                OptimizationUnitValidationError::StructuralPlaceNotAvailable {
                    machine: id(370, MachineId::new),
                    block: id(374, BlockId::new),
                    node: 0,
                    place: call_result,
                }
            )
        );

        let mut partial = operation_result_cfg_unit(OperationResultCfgShape::PartialPredecessor);
        refresh_node_derivatives(&mut partial, 0, 3, 0);
        assert_eq!(
            validate_psi_optimization_unit(&partial),
            Err(
                OptimizationUnitValidationError::StructuralPlaceNotAvailable {
                    machine: id(370, MachineId::new),
                    block: id(375, BlockId::new),
                    node: 0,
                    place: call_result,
                }
            )
        );
    }

    #[test]
    fn structural_type_graph_accepts_dag_shared_descendants_and_disconnected_components() {
        let root = id(401, StructuralTypeId::new);
        let left = id(402, StructuralTypeId::new);
        let right = id(403, StructuralTypeId::new);
        let leaf = id(404, StructuralTypeId::new);
        let disconnected_sum = id(405, StructuralTypeId::new);
        let disconnected_leaf = id(406, StructuralTypeId::new);
        let mut candidate = unit();
        candidate.structural_types = vec![
            structural_type(
                401,
                psi_terminal::StructuralTypeShape::Mixed {
                    fields: vec![structural_field(2, left)],
                    cases: vec![structural_case(1, vec![structural_field(3, right)])],
                },
            ),
            structural_type(
                402,
                psi_terminal::StructuralTypeShape::Record {
                    fields: vec![
                        structural_field(1, leaf),
                        structural_leaf_field(
                            5,
                            psi_terminal::BindingRelevance::Relevant,
                            psi_terminal::StructuralFieldType::Scalar(ScalarType::Boolean),
                        ),
                        structural_leaf_field(
                            6,
                            psi_terminal::BindingRelevance::Relevant,
                            psi_terminal::StructuralFieldType::IeeeFloat(
                                psi_core::IeeeFloatFormat::Binary32,
                            ),
                        ),
                        structural_leaf_field(
                            7,
                            psi_terminal::BindingRelevance::Relevant,
                            psi_terminal::StructuralFieldType::ByteSequence(
                                psi_terminal::ByteSequenceCarrier::BorrowedView,
                            ),
                        ),
                        structural_leaf_field(
                            8,
                            psi_terminal::BindingRelevance::Erased,
                            psi_terminal::StructuralFieldType::Erased {
                                type_identity: "validation::proof-only-leaf".into(),
                            },
                        ),
                    ],
                },
            ),
            structural_type(
                403,
                psi_terminal::StructuralTypeShape::FixedArray {
                    element: leaf,
                    length: 2,
                },
            ),
            structural_type(
                404,
                psi_terminal::StructuralTypeShape::ByteSequence(
                    psi_terminal::ByteSequenceCarrier::BorrowedView,
                ),
            ),
            structural_type(
                405,
                psi_terminal::StructuralTypeShape::Sum {
                    cases: vec![structural_case(
                        2,
                        vec![structural_field(4, disconnected_leaf)],
                    )],
                },
            ),
            structural_type(
                406,
                psi_terminal::StructuralTypeShape::ByteSequence(
                    psi_terminal::ByteSequenceCarrier::BorrowedView,
                ),
            ),
        ];
        refresh_identity(&mut candidate);

        validate_psi_optimization_unit(&candidate).expect(
            "an acyclic catalog may share descendants and contain disconnected declarations",
        );
        assert_eq!(
            candidate.structural_types[0].id, root,
            "the mixed root precedes every structural target it references"
        );
        assert_eq!(
            candidate.structural_types[4].id, disconnected_sum,
            "the disconnected sum precedes its structural target"
        );
    }

    #[test]
    fn structural_type_graph_rejects_cycles_through_every_structural_edge_shape() {
        let recursive = id(410, StructuralTypeId::new);
        let shapes = vec![
            psi_terminal::StructuralTypeShape::Record {
                fields: vec![structural_field(10, recursive)],
            },
            psi_terminal::StructuralTypeShape::FixedArray {
                element: recursive,
                length: 1,
            },
            psi_terminal::StructuralTypeShape::Sum {
                cases: vec![structural_case(10, vec![structural_field(11, recursive)])],
            },
            psi_terminal::StructuralTypeShape::Mixed {
                fields: vec![structural_field(12, recursive)],
                cases: vec![structural_case(11, Vec::new())],
            },
            psi_terminal::StructuralTypeShape::Mixed {
                fields: Vec::new(),
                cases: vec![structural_case(12, vec![structural_field(13, recursive)])],
            },
        ];

        for shape in shapes {
            let mut candidate = unit();
            candidate.structural_types = vec![structural_type(410, shape)];
            refresh_identity(&mut candidate);
            assert_eq!(
                validate_psi_optimization_unit(&candidate),
                Err(OptimizationUnitValidationError::RecursiveStructuralType(
                    recursive
                ))
            );
        }
    }

    #[test]
    fn structural_type_graph_rejects_an_unused_disconnected_cycle() {
        let first = id(420, StructuralTypeId::new);
        let second = id(421, StructuralTypeId::new);
        let mut candidate = unit();
        candidate.structural_types = vec![
            structural_type(
                419,
                psi_terminal::StructuralTypeShape::ByteSequence(
                    psi_terminal::ByteSequenceCarrier::BorrowedView,
                ),
            ),
            structural_type(
                420,
                psi_terminal::StructuralTypeShape::Record {
                    fields: vec![structural_field(20, second)],
                },
            ),
            structural_type(
                421,
                psi_terminal::StructuralTypeShape::FixedArray {
                    element: first,
                    length: 1,
                },
            ),
        ];
        refresh_identity(&mut candidate);

        assert_eq!(
            validate_psi_optimization_unit(&candidate),
            Err(OptimizationUnitValidationError::RecursiveStructuralType(
                first
            ))
        );
    }

    #[test]
    fn structural_type_graph_reports_unknown_targets_before_recursion() {
        let recursive = id(430, StructuralTypeId::new);
        let unknown = id(431, StructuralTypeId::new);
        let mut candidate = unit();
        candidate.structural_types = vec![structural_type(
            430,
            psi_terminal::StructuralTypeShape::Record {
                fields: vec![
                    structural_field(30, recursive),
                    structural_field(31, unknown),
                ],
            },
        )];
        refresh_identity(&mut candidate);

        assert_eq!(
            validate_psi_optimization_unit(&candidate),
            Err(OptimizationUnitValidationError::UnknownStructuralType(
                unknown
            ))
        );
    }

    #[test]
    fn top_level_structural_type_roster_is_canonical_and_identity_unique() {
        let first = structural_type(
            450,
            psi_terminal::StructuralTypeShape::ByteSequence(
                psi_terminal::ByteSequenceCarrier::BorrowedView,
            ),
        );
        let second = structural_type(
            451,
            psi_terminal::StructuralTypeShape::ByteSequence(
                psi_terminal::ByteSequenceCarrier::BorrowedView,
            ),
        );

        let candidate = structural_catalog_unit(vec![second.clone(), first.clone()]);
        assert_eq!(
            validate_psi_optimization_unit(&candidate),
            Err(OptimizationUnitValidationError::NonCanonicalStructuralTypeOrder)
        );

        let candidate = structural_catalog_unit(vec![first.clone(), first.clone()]);
        assert_eq!(
            validate_psi_optimization_unit(&candidate),
            Err(OptimizationUnitValidationError::DuplicateStructuralType(
                first.id
            ))
        );

        let mut empty_identity = first.clone();
        empty_identity.identity.clear();
        let candidate = structural_catalog_unit(vec![empty_identity]);
        assert_eq!(
            validate_psi_optimization_unit(&candidate),
            Err(OptimizationUnitValidationError::InvalidStructuralTypeIdentity(first.id))
        );

        let mut duplicate_identity = second;
        duplicate_identity.identity = first.identity.clone();
        let candidate = structural_catalog_unit(vec![first, duplicate_identity.clone()]);
        assert_eq!(
            validate_psi_optimization_unit(&candidate),
            Err(
                OptimizationUnitValidationError::InvalidStructuralTypeIdentity(
                    duplicate_identity.id
                )
            )
        );
    }

    #[test]
    fn top_level_structural_carriers_are_exact_without_narrowing_field_carriers() {
        let borrowed = id(460, StructuralTypeId::new);
        let array = id(461, StructuralTypeId::new);
        let record = id(462, StructuralTypeId::new);
        let candidate = structural_catalog_unit(vec![
            structural_type(
                460,
                psi_terminal::StructuralTypeShape::ByteSequence(
                    psi_terminal::ByteSequenceCarrier::BorrowedView,
                ),
            ),
            structural_type(
                461,
                psi_terminal::StructuralTypeShape::FixedArray {
                    element: borrowed,
                    length: 1,
                },
            ),
            structural_type(
                462,
                psi_terminal::StructuralTypeShape::Record {
                    fields: vec![structural_leaf_field(
                        1,
                        psi_terminal::BindingRelevance::Relevant,
                        psi_terminal::StructuralFieldType::ByteSequence(
                            psi_terminal::ByteSequenceCarrier::BoundedOwned { capacity: 8 },
                        ),
                    )],
                },
            ),
        ]);
        validate_psi_optimization_unit(&candidate).expect(
            "BorrowedView and positive arrays are valid while field-level owned bytes stay legal",
        );
        assert_eq!(candidate.structural_types[2].id, record);

        for capacity in [0, 8] {
            let candidate = structural_catalog_unit(vec![structural_type(
                460,
                psi_terminal::StructuralTypeShape::ByteSequence(
                    psi_terminal::ByteSequenceCarrier::BoundedOwned { capacity },
                ),
            )]);
            assert_eq!(
                validate_psi_optimization_unit(&candidate),
                Err(OptimizationUnitValidationError::InvalidStructuralTypeIdentity(borrowed))
            );
        }

        let candidate = structural_catalog_unit(vec![
            structural_type(
                460,
                psi_terminal::StructuralTypeShape::ByteSequence(
                    psi_terminal::ByteSequenceCarrier::BorrowedView,
                ),
            ),
            structural_type(
                461,
                psi_terminal::StructuralTypeShape::FixedArray {
                    element: borrowed,
                    length: 0,
                },
            ),
        ]);
        assert_eq!(
            validate_psi_optimization_unit(&candidate),
            Err(OptimizationUnitValidationError::InvalidStructuralArrayLength(array))
        );
    }

    #[test]
    fn structural_domain_roster_is_canonical_unique_and_carrier_closed() {
        let carrier = id(470, StructuralTypeId::new);
        let types = vec![structural_type(
            470,
            psi_terminal::StructuralTypeShape::ByteSequence(
                psi_terminal::ByteSequenceCarrier::BorrowedView,
            ),
        )];
        let first = structural_domain(1, 11, carrier);
        let second = structural_domain(2, 12, carrier);

        let mut candidate = structural_catalog_unit(types.clone());
        candidate.structural_domains = vec![first.clone(), second.clone()].into();
        refresh_identity(&mut candidate);
        validate_psi_optimization_unit(&candidate)
            .expect("distinct canonical domains may share one exact carrier");

        let mut candidate = structural_catalog_unit(types.clone());
        candidate.structural_domains = vec![second.clone(), first.clone()].into();
        refresh_identity(&mut candidate);
        assert_eq!(
            validate_psi_optimization_unit(&candidate),
            Err(OptimizationUnitValidationError::NonCanonicalStructuralDomainOrder)
        );

        let mut candidate = structural_catalog_unit(types.clone());
        candidate.structural_domains = vec![first.clone(), first.clone()].into();
        refresh_identity(&mut candidate);
        assert_eq!(
            validate_psi_optimization_unit(&candidate),
            Err(OptimizationUnitValidationError::DuplicateStructuralDomain(
                first.id
            ))
        );

        let mut invalid_identities = Vec::new();
        let mut empty_identity = first.clone();
        empty_identity.identity.clear();
        invalid_identities.push((vec![empty_identity], first.id));
        let mut duplicate_name = second.clone();
        duplicate_name.identity = first.identity.clone();
        invalid_identities.push((vec![first.clone(), duplicate_name], second.id));
        let mut duplicate_semantic = second.clone();
        duplicate_semantic.semantic_domain = first.semantic_domain;
        invalid_identities.push((vec![first.clone(), duplicate_semantic], second.id));
        for (domains, expected) in invalid_identities {
            let mut candidate = structural_catalog_unit(types.clone());
            candidate.structural_domains = domains.into();
            refresh_identity(&mut candidate);
            assert_eq!(
                validate_psi_optimization_unit(&candidate),
                Err(OptimizationUnitValidationError::InvalidStructuralDomainIdentity(expected))
            );
        }

        let unknown = id(471, StructuralTypeId::new);
        let mut candidate = structural_catalog_unit(types);
        candidate.structural_domains = vec![structural_domain(1, 11, unknown)].into();
        refresh_identity(&mut candidate);
        assert_eq!(
            validate_psi_optimization_unit(&candidate),
            Err(OptimizationUnitValidationError::UnknownStructuralType(
                unknown
            ))
        );
    }

    #[test]
    fn structural_domain_content_projection_replays_terminal_contract() {
        let carrier = id(480, StructuralTypeId::new);
        let nested = id(481, StructuralTypeId::new);
        let non_record = id(482, StructuralTypeId::new);
        let types = vec![
            structural_type(
                480,
                psi_terminal::StructuralTypeShape::Record {
                    fields: vec![
                        structural_leaf_field(
                            1,
                            psi_terminal::BindingRelevance::Relevant,
                            psi_terminal::StructuralFieldType::Structural(nested),
                        ),
                        structural_leaf_field(
                            2,
                            psi_terminal::BindingRelevance::Relevant,
                            psi_terminal::StructuralFieldType::Scalar(ScalarType::Boolean),
                        ),
                        structural_leaf_field(
                            3,
                            psi_terminal::BindingRelevance::Relevant,
                            psi_terminal::StructuralFieldType::Structural(non_record),
                        ),
                        structural_leaf_field(
                            4,
                            psi_terminal::BindingRelevance::Relevant,
                            psi_terminal::StructuralFieldType::IeeeFloat(
                                psi_core::IeeeFloatFormat::Binary32,
                            ),
                        ),
                        structural_leaf_field(
                            5,
                            psi_terminal::BindingRelevance::Relevant,
                            psi_terminal::StructuralFieldType::ByteSequence(
                                psi_terminal::ByteSequenceCarrier::BorrowedView,
                            ),
                        ),
                        structural_leaf_field(
                            6,
                            psi_terminal::BindingRelevance::Erased,
                            psi_terminal::StructuralFieldType::Erased {
                                type_identity: "validation::erased".into(),
                            },
                        ),
                    ],
                },
            ),
            structural_type(
                481,
                psi_terminal::StructuralTypeShape::Record {
                    fields: vec![structural_leaf_field(
                        1,
                        psi_terminal::BindingRelevance::Relevant,
                        psi_terminal::StructuralFieldType::Scalar(ScalarType::Boolean),
                    )],
                },
            ),
            structural_type(
                482,
                psi_terminal::StructuralTypeShape::ByteSequence(
                    psi_terminal::ByteSequenceCarrier::BorrowedView,
                ),
            ),
        ];
        let domain_id = id(1, StructuralDomainId::new);
        let semantic_domain = id(31, psi_core::DomainSemanticId::new);
        let projection = |kind, parameter: &str, expression| {
            let algebra = psi_core::ContentAlgebra {
                kind,
                parameter: parameter.into(),
            };
            psi_terminal::StructuralContentProjection {
                identity: psi_core::ContentProjectionIdentity {
                    domain: id(semantic_domain.get(), psi_core::ContentDomainId::new),
                    projection_fingerprint:
                        psi_language_semantics::content::terminal_projection_fingerprint(
                            &algebra,
                            &expression,
                        ),
                },
                algebra,
                expression,
            }
        };
        let candidate_with = |projection| {
            let mut candidate = structural_catalog_unit(types.clone());
            let mut domain = structural_domain(1, semantic_domain.get(), carrier);
            domain.content_projection = Some(projection);
            candidate.structural_domains = vec![domain].into();
            refresh_identity(&mut candidate);
            candidate
        };
        let rejects = |projection| {
            assert_eq!(
                validate_psi_optimization_unit(&candidate_with(projection)),
                Err(
                    OptimizationUnitValidationError::InvalidStructuralDomainContentProjection(
                        domain_id,
                    ),
                )
            );
        };

        let nested_path = vec!["validation::field-1".into(), "validation::field-1".into()];
        let expression =
            ContentProjectionExpression::CountedQuantity(ContentProjectionScalar::Add(
                Box::new(ContentProjectionScalar::SubjectField(nested_path.clone())),
                Box::new(ContentProjectionScalar::Multiply(
                    Box::new(ContentProjectionScalar::RuntimeScalarEmbedding(vec![
                        "validation::field-2".into(),
                    ])),
                    Box::new(ContentProjectionScalar::Subtract(
                        Box::new(ContentProjectionScalar::Successor(Box::new(
                            ContentProjectionScalar::Natural("0".into()),
                        ))),
                        Box::new(ContentProjectionScalar::Natural("1".into())),
                    )),
                )),
            ));
        validate_psi_optimization_unit(&candidate_with(projection(
            psi_core::ContentAlgebraKind::CountedQuantity,
            "validation::unit",
            expression.clone(),
        )))
        .expect("the complete closed scalar grammar and nested Record paths are valid");

        for members in [
            Vec::new(),
            vec![(
                ContentProjectionScalar::SubjectField(nested_path),
                ContentProjectionScalar::Natural("9".into()),
            )],
        ] {
            validate_psi_optimization_unit(&candidate_with(projection(
                psi_core::ContentAlgebraKind::IntervalSet,
                "validation::coordinate-space",
                ContentProjectionExpression::IntervalSet(members),
            )))
            .expect("Terminal permits empty and symbolic interval sets");
        }

        let valid = projection(
            psi_core::ContentAlgebraKind::CountedQuantity,
            "validation::unit",
            expression,
        );
        let mut invalid = valid.clone();
        invalid.identity.domain = id(32, psi_core::ContentDomainId::new);
        rejects(invalid);
        let mut invalid = valid.clone();
        invalid.identity.projection_fingerprint = 0;
        rejects(invalid);
        let mut invalid = valid.clone();
        invalid.algebra.parameter.clear();
        rejects(invalid);
        let mut invalid = valid.clone();
        invalid.algebra.kind = psi_core::ContentAlgebraKind::IntervalSet;
        rejects(invalid);
        let invalid = projection(
            psi_core::ContentAlgebraKind::CountedQuantity,
            "validation::unit",
            ContentProjectionExpression::IntervalSet(Vec::new()),
        );
        rejects(invalid);
        let mut invalid = valid.clone();
        invalid.expression = ContentProjectionExpression::CountedQuantity(
            ContentProjectionScalar::Natural("2".into()),
        );
        rejects(invalid);

        for value in ["", "00", "01", "1x", "١"] {
            rejects(projection(
                psi_core::ContentAlgebraKind::CountedQuantity,
                "validation::unit",
                ContentProjectionExpression::CountedQuantity(ContentProjectionScalar::Natural(
                    value.into(),
                )),
            ));
        }
        for path in [
            Vec::new(),
            vec![String::new()],
            vec!["validation::missing".into()],
            vec!["validation::field-2".into(), "validation::field-1".into()],
            vec!["validation::field-1".into()],
            vec!["validation::field-3".into(), "validation::field-1".into()],
            vec!["validation::field-4".into()],
            vec!["validation::field-5".into()],
            vec!["validation::field-6".into()],
        ] {
            rejects(projection(
                psi_core::ContentAlgebraKind::CountedQuantity,
                "validation::unit",
                ContentProjectionExpression::CountedQuantity(
                    ContentProjectionScalar::SubjectField(path),
                ),
            ));
        }

        let nested_successors = |depth| {
            let mut scalar = ContentProjectionScalar::Natural("0".into());
            for _ in 0..depth {
                scalar = ContentProjectionScalar::Successor(Box::new(scalar));
            }
            scalar
        };
        validate_psi_optimization_unit(&candidate_with(projection(
            psi_core::ContentAlgebraKind::CountedQuantity,
            "validation::unit",
            ContentProjectionExpression::CountedQuantity(nested_successors(256)),
        )))
        .expect("Terminal's inclusive depth-256 boundary remains valid");
        rejects(projection(
            psi_core::ContentAlgebraKind::CountedQuantity,
            "validation::unit",
            ContentProjectionExpression::CountedQuantity(nested_successors(257)),
        ));
    }

    #[test]
    fn structural_field_namespaces_require_canonical_ids_and_unique_nonempty_identities() {
        let owner = id(440, StructuralTypeId::new);
        let scalar_field = |raw| {
            structural_leaf_field(
                raw,
                psi_terminal::BindingRelevance::Relevant,
                psi_terminal::StructuralFieldType::Scalar(ScalarType::Boolean),
            )
        };

        let mut descending = vec![scalar_field(2), scalar_field(1)];
        let candidate = structural_catalog_unit(vec![structural_type(
            440,
            psi_terminal::StructuralTypeShape::Record {
                fields: descending.clone(),
            },
        )]);
        assert_eq!(
            validate_psi_optimization_unit(&candidate),
            Err(
                OptimizationUnitValidationError::NonCanonicalStructuralFieldOrder {
                    structural_type: owner,
                    case: None,
                }
            )
        );

        descending.reverse();
        descending[1].identity = descending[0].identity.clone();
        let duplicate_name = descending[1].id;
        let candidate = structural_catalog_unit(vec![structural_type(
            440,
            psi_terminal::StructuralTypeShape::Mixed {
                fields: descending,
                cases: vec![structural_case(1, Vec::new())],
            },
        )]);
        assert_eq!(
            validate_psi_optimization_unit(&candidate),
            Err(
                OptimizationUnitValidationError::InvalidStructuralFieldIdentity {
                    structural_type: owner,
                    field: duplicate_name,
                }
            )
        );

        let mut empty_name = scalar_field(1);
        empty_name.identity.clear();
        let empty_name_id = empty_name.id;
        let case = structural_case(1, vec![empty_name]);
        let case_id = case.id;
        let candidate = structural_catalog_unit(vec![structural_type(
            440,
            psi_terminal::StructuralTypeShape::Sum { cases: vec![case] },
        )]);
        assert_eq!(
            validate_psi_optimization_unit(&candidate),
            Err(
                OptimizationUnitValidationError::InvalidStructuralFieldIdentity {
                    structural_type: owner,
                    field: empty_name_id,
                }
            )
        );

        let duplicate_id = vec![scalar_field(1), scalar_field(1)];
        let candidate = structural_catalog_unit(vec![structural_type(
            440,
            psi_terminal::StructuralTypeShape::Sum {
                cases: vec![structural_case(1, duplicate_id)],
            },
        )]);
        assert_eq!(
            validate_psi_optimization_unit(&candidate),
            Err(
                OptimizationUnitValidationError::NonCanonicalStructuralFieldOrder {
                    structural_type: owner,
                    case: Some(case_id),
                }
            )
        );
    }

    #[test]
    fn structural_cases_require_canonical_unique_nonempty_declarations() {
        let owner = id(441, StructuralTypeId::new);
        for shape in [
            psi_terminal::StructuralTypeShape::Sum { cases: Vec::new() },
            psi_terminal::StructuralTypeShape::Mixed {
                fields: Vec::new(),
                cases: Vec::new(),
            },
        ] {
            let candidate = structural_catalog_unit(vec![structural_type(441, shape)]);
            assert_eq!(
                validate_psi_optimization_unit(&candidate),
                Err(OptimizationUnitValidationError::EmptyStructuralSum(owner))
            );
        }

        let candidate = structural_catalog_unit(vec![structural_type(
            441,
            psi_terminal::StructuralTypeShape::Sum {
                cases: vec![
                    structural_case(2, Vec::new()),
                    structural_case(1, Vec::new()),
                ],
            },
        )]);
        assert_eq!(
            validate_psi_optimization_unit(&candidate),
            Err(OptimizationUnitValidationError::NonCanonicalStructuralCaseOrder(owner))
        );

        let first = structural_case(1, Vec::new());
        let mut duplicate = structural_case(2, Vec::new());
        duplicate.identity = first.identity.clone();
        let duplicate_id = duplicate.id;
        let candidate = structural_catalog_unit(vec![structural_type(
            441,
            psi_terminal::StructuralTypeShape::Mixed {
                fields: Vec::new(),
                cases: vec![first, duplicate],
            },
        )]);
        assert_eq!(
            validate_psi_optimization_unit(&candidate),
            Err(
                OptimizationUnitValidationError::InvalidStructuralCaseIdentity {
                    structural_type: owner,
                    case: duplicate_id,
                }
            )
        );

        let mut empty = structural_case(1, Vec::new());
        empty.identity.clear();
        let empty_id = empty.id;
        let candidate = structural_catalog_unit(vec![structural_type(
            441,
            psi_terminal::StructuralTypeShape::Sum { cases: vec![empty] },
        )]);
        assert_eq!(
            validate_psi_optimization_unit(&candidate),
            Err(
                OptimizationUnitValidationError::InvalidStructuralCaseIdentity {
                    structural_type: owner,
                    case: empty_id,
                }
            )
        );
    }

    #[test]
    fn structural_field_namespaces_are_independent_and_payloadless_cases_are_valid() {
        let shared_field = || {
            structural_leaf_field(
                1,
                psi_terminal::BindingRelevance::Relevant,
                psi_terminal::StructuralFieldType::Scalar(ScalarType::Boolean),
            )
        };
        let candidate = structural_catalog_unit(vec![
            structural_type(
                442,
                psi_terminal::StructuralTypeShape::Mixed {
                    fields: vec![shared_field()],
                    cases: vec![
                        structural_case(1, vec![shared_field()]),
                        structural_case(2, vec![shared_field()]),
                        structural_case(3, Vec::new()),
                    ],
                },
            ),
            structural_type(
                448,
                psi_terminal::StructuralTypeShape::Sum {
                    cases: vec![structural_case(1, Vec::new())],
                },
            ),
        ]);
        validate_psi_optimization_unit(&candidate)
            .expect("field namespaces are independent and Sum/Mixed cases may be payloadless");
    }

    #[test]
    fn structural_field_erasure_matrix_matches_canonical_terminal_admission() {
        let owner = id(443, StructuralTypeId::new);
        let invalid = vec![
            psi_terminal::StructuralFieldType::Scalar(ScalarType::Boolean),
            psi_terminal::StructuralFieldType::IeeeFloat(psi_core::IeeeFloatFormat::Binary32),
            psi_terminal::StructuralFieldType::Structural(owner),
        ];
        for field_type in invalid {
            let field =
                structural_leaf_field(1, psi_terminal::BindingRelevance::Erased, field_type);
            let field_id = field.id;
            let candidate = structural_catalog_unit(vec![structural_type(
                443,
                psi_terminal::StructuralTypeShape::Record {
                    fields: vec![field],
                },
            )]);
            assert_eq!(
                validate_psi_optimization_unit(&candidate),
                Err(
                    OptimizationUnitValidationError::InvalidErasedStructuralField {
                        structural_type: owner,
                        field: field_id,
                    }
                )
            );
        }

        for (raw, field_type) in [
            (
                1,
                psi_terminal::StructuralFieldType::ByteSequence(
                    psi_terminal::ByteSequenceCarrier::BorrowedView,
                ),
            ),
            (
                2,
                psi_terminal::StructuralFieldType::Erased {
                    type_identity: "validation::proof-only".into(),
                },
            ),
        ] {
            let candidate = structural_catalog_unit(vec![structural_type(
                443,
                psi_terminal::StructuralTypeShape::Record {
                    fields: vec![structural_leaf_field(
                        raw,
                        psi_terminal::BindingRelevance::Erased,
                        field_type,
                    )],
                },
            )]);
            validate_psi_optimization_unit(&candidate)
                .expect("Terminal admits the exact proof-side leaf carrier");
        }

        let empty_erased = structural_leaf_field(
            1,
            psi_terminal::BindingRelevance::Erased,
            psi_terminal::StructuralFieldType::Erased {
                type_identity: String::new(),
            },
        );
        let empty_erased_id = empty_erased.id;
        let candidate = structural_catalog_unit(vec![structural_type(
            443,
            psi_terminal::StructuralTypeShape::Sum {
                cases: vec![structural_case(1, vec![empty_erased])],
            },
        )]);
        assert_eq!(
            validate_psi_optimization_unit(&candidate),
            Err(
                OptimizationUnitValidationError::InvalidErasedStructuralField {
                    structural_type: owner,
                    field: empty_erased_id,
                }
            )
        );
    }

    #[test]
    fn relevant_erased_field_requires_an_exact_record_provider_attachment_witness() {
        let owner = id(444, StructuralTypeId::new);
        let field = id(1, psi_core::StructuralFieldId::new);
        let provider_field = || {
            structural_leaf_field(
                1,
                psi_terminal::BindingRelevance::Relevant,
                psi_terminal::StructuralFieldType::Erased {
                    type_identity: "validation::provider".into(),
                },
            )
        };
        let provider_place =
            |attachment, provider_field| psi_terminal::StructuralPlaceDeclaration {
                id: id(445, PlaceId::new),
                kind: StructuralPlaceKind::ProviderAttachment {
                    attachment,
                    field: provider_field,
                    boundary: id(446, BoundaryMachineId::new),
                },
            };

        let valid = provider_attachment_specialization_unit();
        validate_psi_optimization_unit(&valid)
            .expect("a complete provider specialization witnesses its relevant erased field");

        for (attachment, provider_field_id) in [
            (None, None),
            (Some(owner), Some(id(2, psi_core::StructuralFieldId::new))),
            (Some(id(447, StructuralTypeId::new)), Some(field)),
        ] {
            let mut invalid = structural_catalog_unit(vec![structural_type(
                444,
                psi_terminal::StructuralTypeShape::Record {
                    fields: vec![provider_field()],
                },
            )]);
            if let (Some(attachment), Some(provider_field_id)) = (attachment, provider_field_id) {
                invalid.functions[0].attachment = Some(attachment);
                invalid.functions[0]
                    .structural_places
                    .push(provider_place(attachment, provider_field_id));
            }
            refresh_identity(&mut invalid);
            assert_eq!(
                validate_psi_optimization_unit(&invalid),
                Err(
                    OptimizationUnitValidationError::InvalidErasedStructuralField {
                        structural_type: owner,
                        field,
                    }
                )
            );
        }

        for shape in [
            psi_terminal::StructuralTypeShape::Sum {
                cases: vec![structural_case(1, vec![provider_field()])],
            },
            psi_terminal::StructuralTypeShape::Mixed {
                fields: vec![provider_field()],
                cases: vec![structural_case(1, Vec::new())],
            },
            psi_terminal::StructuralTypeShape::Mixed {
                fields: Vec::new(),
                cases: vec![structural_case(1, vec![provider_field()])],
            },
        ] {
            let mut invalid = structural_catalog_unit(vec![structural_type(444, shape)]);
            invalid.functions[0].attachment = Some(owner);
            invalid.functions[0]
                .structural_places
                .push(provider_place(owner, field));
            refresh_identity(&mut invalid);
            assert_eq!(
                validate_psi_optimization_unit(&invalid),
                Err(
                    OptimizationUnitValidationError::InvalidErasedStructuralField {
                        structural_type: owner,
                        field,
                    }
                )
            );
        }
    }

    #[test]
    fn structural_signatures_replay_attachment_and_unique_self_legality() {
        let mut attached = structural_call_unit();
        let structural_type = attached.structural_types[0].id;
        attached.functions[0].attachment = Some(structural_type);
        attached.functions[0].structural_parameters[0].is_self = true;
        let StructuralPlaceKind::Parameter { is_self, .. } =
            &mut attached.functions[0].structural_places[0].kind
        else {
            panic!("fixture retains its parameter root")
        };
        *is_self = true;
        refresh_identity(&mut attached);
        validate_psi_optimization_unit(&attached)
            .expect("one attachment-typed self parameter is canonical");

        let mut self_without_attachment = attached.clone();
        self_without_attachment.functions[0].attachment = None;
        refresh_identity(&mut self_without_attachment);
        assert!(matches!(
            validate_psi_optimization_unit(&self_without_attachment),
            Err(OptimizationUnitValidationError::StructuralCatalogMismatch { machine: Some(_) })
        ));

        let mut mismatched_self = attached.clone();
        let alternate = id(4_710, StructuralTypeId::new);
        mismatched_self
            .structural_types
            .push(psi_terminal::StructuralTypeDeclaration {
                id: alternate,
                identity: "validation::alternate-attachment".into(),
                shape: psi_terminal::StructuralTypeShape::Record { fields: Vec::new() },
            });
        mismatched_self.functions[0].attachment = Some(alternate);
        refresh_identity(&mut mismatched_self);
        assert!(matches!(
            validate_psi_optimization_unit(&mismatched_self),
            Err(OptimizationUnitValidationError::StructuralCatalogMismatch { machine: Some(_) })
        ));

        let mut duplicate_self = attached.clone();
        let mut second = duplicate_self.functions[0].structural_parameters[0].clone();
        second.place = id(4_711, PlaceId::new);
        second.position = 1;
        duplicate_self.functions[0]
            .structural_parameters
            .push(second.clone());
        duplicate_self.functions[0].structural_places.push(
            psi_terminal::StructuralPlaceDeclaration {
                id: second.place,
                kind: StructuralPlaceKind::Parameter {
                    position: 1,
                    is_self: true,
                },
            },
        );
        refresh_identity(&mut duplicate_self);
        assert!(matches!(
            validate_psi_optimization_unit(&duplicate_self),
            Err(OptimizationUnitValidationError::StructuralCatalogMismatch { machine: Some(_) })
        ));

        let mut unknown_function_attachment = structural_call_unit();
        unknown_function_attachment.functions[0].attachment =
            Some(id(4_799, StructuralTypeId::new));
        refresh_identity(&mut unknown_function_attachment);
        assert!(matches!(
            validate_psi_optimization_unit(&unknown_function_attachment),
            Err(OptimizationUnitValidationError::StructuralCatalogMismatch { machine: Some(_) })
        ));

        let mut boundary_self = byte_literal_boundary_unit();
        let boundary_type =
            boundary_self.boundary_machines[0].structural_parameters[0].structural_type;
        boundary_self.boundary_machines[0].attachment = Some(boundary_type);
        boundary_self.boundary_machines[0].structural_parameters[0].is_self = true;
        refresh_identity(&mut boundary_self);
        validate_psi_optimization_unit(&boundary_self)
            .expect("boundary self uses the exact known attachment type");

        boundary_self.boundary_machines[0].attachment = Some(id(4_798, StructuralTypeId::new));
        refresh_identity(&mut boundary_self);
        assert_eq!(
            validate_psi_optimization_unit(&boundary_self),
            Err(OptimizationUnitValidationError::StructuralCatalogMismatch { machine: None })
        );
    }

    #[test]
    fn logical_structural_roots_are_unique_beyond_place_identity() {
        let mut duplicate = structural_result_call_unit();
        let first_call = duplicate.functions[0].blocks[0].nodes[0].clone();
        let (psi_operation, result_type) = match &first_call.operation {
            O::CallStructural {
                psi_operation,
                result,
                ..
            } => (*psi_operation, result.structural_type),
            _ => panic!("fixture begins with one structural call"),
        };
        let duplicate_place = id(4_712, PlaceId::new);
        let mut duplicate_call = first_call;
        let O::CallStructural {
            result: duplicate_result,
            ..
        } = &mut duplicate_call.operation
        else {
            unreachable!()
        };
        duplicate_result.place = duplicate_place;
        duplicate.functions[0].blocks[0]
            .nodes
            .insert(1, duplicate_call);
        duplicate.functions[0]
            .structural_places
            .push(psi_terminal::StructuralPlaceDeclaration {
                id: duplicate_place,
                kind: StructuralPlaceKind::OperationResult {
                    producer: psi_operation,
                    structural_type: result_type,
                },
            });
        refresh_function_derivatives(&mut duplicate, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&duplicate),
            Err(
                OptimizationUnitValidationError::DuplicateStructuralPlaceRoot {
                    machine: _,
                    kind: StructuralPlaceKind::OperationResult { .. },
                }
            )
        ));
    }

    #[test]
    fn boolean_structural_field_replays_terminal_root_and_cleanup_contract() {
        let baseline = boolean_structural_field_unit();
        validate_psi_optimization_unit(&baseline)
            .expect("exact affine readable Boolean observation validates");
        let invalid = |mut candidate: PsiOptimizationUnit| {
            refresh_identity(&mut candidate);
            assert!(matches!(
                validate_psi_optimization_unit(&candidate),
                Err(OptimizationUnitValidationError::InvalidBooleanStructuralField { .. })
            ));
        };

        let mut non_entry = baseline.clone();
        non_entry.entry = id(4_799, MachineId::new);
        invalid(non_entry);

        let mut unrestricted = baseline.clone();
        unrestricted.functions[0].structural_parameters[0].multiplicity =
            psi_terminal::StructuralMultiplicity::Unrestricted;
        invalid(unrestricted);

        let mut write_only = baseline.clone();
        write_only.functions[0].structural_parameters[0].access =
            psi_terminal::StructuralAccess::WriteOnlyBorrow;
        invalid(write_only);

        let mut qualified = baseline.clone();
        let domain = id(1, StructuralDomainId::new);
        qualified.structural_domains =
            vec![structural_domain(1, 1, qualified.structural_types[0].id)].into();
        qualified.functions[0].structural_parameters[0]
            .qualifications
            .push(domain);
        invalid(qualified);

        let mut claimed = baseline.clone();
        let claim = id(1, ClaimId::new);
        let source = claimed.functions[0].structural_parameters[0].place;
        claimed.functions[0]
            .entry_claim_declarations
            .push(psi_terminal::EntryClaim {
                claim,
                input: source,
                path: Vec::new(),
            });
        claimed.functions[0].entry_claims.insert(claim);
        invalid(claimed);

        let mut content_claimed = baseline.clone();
        install_content_owner(&mut content_claimed);
        content_claimed.functions[0]
            .content_entry_claims
            .push(content_entry_claim(claim, source));
        invalid(content_claimed);

        let mut no_boolean_parameter = baseline.clone();
        no_boolean_parameter.functions[0].parameters.clear();
        invalid(no_boolean_parameter);

        let mut missing_cleanup = baseline.clone();
        let O::Return {
            cleanup_actions, ..
        } = &mut missing_cleanup.functions[0].blocks[0].nodes[1].operation
        else {
            panic!("fixture ends in a scalar return")
        };
        cleanup_actions.clear();
        refresh_node_derivatives(&mut missing_cleanup, 0, 0, 1);
        invalid(missing_cleanup);

        let mut wrong_field = baseline.clone();
        let O::BooleanStructuralField { field, .. } =
            &mut wrong_field.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("fixture begins with its observation")
        };
        *field = id(4_799, psi_core::StructuralFieldId::new);
        refresh_node_derivatives(&mut wrong_field, 0, 0, 0);
        invalid(wrong_field);

        let mut non_boolean_field = baseline.clone();
        let psi_terminal::StructuralTypeShape::Record { fields } =
            &mut non_boolean_field.structural_types[0].shape
        else {
            unreachable!()
        };
        fields[0].field_type = psi_terminal::StructuralFieldType::Scalar(ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
        ));
        invalid(non_boolean_field);

        let mut differing_observation = baseline;
        let mut second = differing_observation.functions[0].blocks[0].nodes[0].clone();
        let second_field = id(4_713, psi_core::StructuralFieldId::new);
        let O::BooleanStructuralField {
            psi_operation,
            result,
            field,
            ..
        } = &mut second.operation
        else {
            unreachable!()
        };
        *psi_operation = id(4_714, OperationId::new);
        *result = id(4_715, ValueId::new);
        *field = second_field;
        let psi_terminal::StructuralTypeShape::Record { fields } =
            &mut differing_observation.structural_types[0].shape
        else {
            unreachable!()
        };
        fields.push(psi_terminal::StructuralFieldDeclaration {
            id: second_field,
            identity: "validation::other-ready".into(),
            relevance: psi_terminal::BindingRelevance::Relevant,
            field_type: psi_terminal::StructuralFieldType::Scalar(ScalarType::Boolean),
        });
        differing_observation.functions[0].blocks[0]
            .nodes
            .insert(1, second);
        refresh_function_derivatives(&mut differing_observation, 0);
        invalid(differing_observation);
    }

    #[test]
    fn structural_returns_reject_non_source_roots_and_signature_drift() {
        let mut result_root = structural_result_call_unit();
        let result_place = result_root.functions[1]
            .result
            .structural()
            .expect("structural result")
            .place;
        let return_node = result_root.functions[1].blocks[0].nodes.len() - 1;
        let O::ReturnStructural { source, .. } =
            &mut result_root.functions[1].blocks[0].nodes[return_node].operation
        else {
            panic!("fixture returns structurally")
        };
        *source = result_place;
        refresh_node_derivatives(&mut result_root, 1, 0, return_node);
        assert!(matches!(
            validate_psi_optimization_unit(&result_root),
            Err(OptimizationUnitValidationError::StructuralReturnSourceContractMismatch { .. })
        ));

        let mut literal_root = structural_result_call_unit();
        let literal_type = psi_terminal::StructuralTypeDeclaration {
            id: id(4_716, StructuralTypeId::new),
            identity: "validation::return-source-literal".into(),
            shape: psi_terminal::StructuralTypeShape::ByteSequence(
                psi_terminal::ByteSequenceCarrier::BorrowedView,
            ),
        };
        let literal = psi_terminal::StructuralPlaceDeclaration {
            id: id(4_717, PlaceId::new),
            kind: StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal: 0,
                structural_type: literal_type.id,
            },
        };
        literal_root.structural_types.push(literal_type.clone());
        literal_root.functions[1].structural_places.push(literal);
        let establishment_node = literal_root.functions[1].blocks[0].nodes[0].clone();
        literal_root.functions[1].blocks[0]
            .nodes
            .insert(0, establishment_node);
        literal_root.functions[1].blocks[0].nodes[0].operation = O::EstablishByteSequenceLiteral {
            psi_operation: id(4_718, OperationId::new),
            place: literal,
            structural_type: literal_type,
            bytes: b"return-source".to_vec(),
        };
        let O::ReturnStructural { source, .. } =
            &mut literal_root.functions[1].blocks[0].nodes[1].operation
        else {
            unreachable!()
        };
        *source = literal.id;
        refresh_function_derivatives(&mut literal_root, 1);
        assert!(matches!(
            validate_psi_optimization_unit(&literal_root),
            Err(OptimizationUnitValidationError::StructuralReturnSourceContractMismatch { .. })
        ));

        let mut wrong_signature =
            operation_result_cfg_unit(OperationResultCfgShape::DominatingNonTopological);
        let O::CallStructural { result, .. } =
            &mut wrong_signature.functions[0].blocks[3].nodes[0].operation
        else {
            panic!("non-topological fixture stores its call in the entry block")
        };
        result.multiplicity = psi_terminal::StructuralMultiplicity::Affine;
        refresh_node_derivatives(&mut wrong_signature, 0, 3, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&wrong_signature),
            Err(OptimizationUnitValidationError::StructuralReturnSourceContractMismatch { .. })
        ));
    }

    #[test]
    fn provider_attachment_specialization_replays_exact_roots_calls_and_nonuse() {
        let baseline = provider_attachment_specialization_unit();
        validate_psi_optimization_unit(&baseline)
            .expect("repeated calls share one canonical provider requirement root");
        let machine = baseline.functions[0].machine;
        let invalid =
            OptimizationUnitValidationError::InvalidProviderAttachmentSpecialization(machine);
        let attachment = baseline.functions[0]
            .attachment
            .expect("provider fixture attachment");
        let first_boundary = baseline.boundary_machines[0].id;
        let second_boundary = baseline.boundary_machines[1].id;
        let unused_boundary = baseline.boundary_machines[2].id;
        let first_provider_place = baseline.functions[0].structural_places[0].id;

        let assert_invalid = |mut unit: PsiOptimizationUnit| {
            refresh_identity(&mut unit);
            assert_eq!(validate_psi_optimization_unit(&unit), Err(invalid.clone()));
        };

        let mut missing_root = baseline.clone();
        missing_root.functions[0].structural_places.pop();
        assert_invalid(missing_root);

        let mut extra_root = baseline.clone();
        extra_root.functions[0]
            .structural_places
            .push(psi_terminal::StructuralPlaceDeclaration {
                id: id(453, PlaceId::new),
                kind: StructuralPlaceKind::ProviderAttachment {
                    attachment,
                    field: id(1, psi_core::StructuralFieldId::new),
                    boundary: unused_boundary,
                },
            });
        assert_invalid(extra_root);

        let mut reordered_roots = baseline.clone();
        reordered_roots.functions[0].structural_places.swap(0, 1);
        assert_invalid(reordered_roots);

        let mut duplicate_root = baseline.clone();
        let duplicate_kind = duplicate_root.functions[0].structural_places[1].kind;
        duplicate_root.functions[0].structural_places.push(
            psi_terminal::StructuralPlaceDeclaration {
                id: id(453, PlaceId::new),
                kind: duplicate_kind,
            },
        );
        assert_invalid(duplicate_root);

        let mut wrong_field = baseline.clone();
        let StructuralPlaceKind::ProviderAttachment { field, .. } =
            &mut wrong_field.functions[0].structural_places[1].kind
        else {
            panic!("provider fixture root")
        };
        *field = id(2, psi_core::StructuralFieldId::new);
        assert_invalid(wrong_field);

        let mut unknown_boundary = baseline.clone();
        let StructuralPlaceKind::ProviderAttachment { boundary, .. } =
            &mut unknown_boundary.functions[0].structural_places[1].kind
        else {
            panic!("provider fixture root")
        };
        *boundary = id(999, BoundaryMachineId::new);
        assert_invalid(unknown_boundary);

        let mut attached_boundary = baseline.clone();
        attached_boundary.boundary_machines[1].attachment = Some(attachment);
        assert_invalid(attached_boundary);

        let mut self_parameter = baseline.clone();
        let parameter_place = id(454, PlaceId::new);
        self_parameter.functions[0].structural_parameters.push(
            psi_terminal::StructuralParameterDeclaration {
                place: parameter_place,
                position: 0,
                is_self: true,
                structural_type: attachment,
                multiplicity: psi_terminal::StructuralMultiplicity::Unrestricted,
                access: psi_terminal::StructuralAccess::Owned,
                qualifications: Vec::new(),
            },
        );
        self_parameter.functions[0].structural_places.push(
            psi_terminal::StructuralPlaceDeclaration {
                id: parameter_place,
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: true,
                },
            },
        );
        assert_invalid(self_parameter);

        let mut missing_call = baseline.clone();
        let AbstractOperation::BoundaryCall { boundary, .. } =
            &mut missing_call.functions[0].blocks[0].nodes[2].operation
        else {
            panic!("provider fixture call")
        };
        *boundary = first_boundary;
        assert_invalid(missing_call);

        let mut extra_call = baseline.clone();
        let AbstractOperation::BoundaryCall { boundary, .. } =
            &mut extra_call.functions[0].blocks[0].nodes[1].operation
        else {
            panic!("provider fixture call")
        };
        *boundary = unused_boundary;
        assert_invalid(extra_call);

        let provider_argument = psi_terminal::StructuralArgument {
            place: first_provider_place,
            path: Vec::new(),
            access: psi_terminal::StructuralAccess::Owned,
        };
        let mut boundary_use = baseline.clone();
        let AbstractOperation::BoundaryCall {
            structural_arguments,
            ..
        } = &mut boundary_use.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("provider fixture call")
        };
        structural_arguments.push(provider_argument.clone());
        assert_invalid(boundary_use);

        let mut unit_use = baseline.clone();
        let psi_operation = match unit_use.functions[0].blocks[0].nodes[0].operation {
            AbstractOperation::BoundaryCall { psi_operation, .. } => psi_operation,
            _ => panic!("provider fixture call"),
        };
        unit_use.functions[0].blocks[0].nodes[0].operation = AbstractOperation::CallUnit {
            psi_operation,
            callee: machine,
            structural_arguments: vec![provider_argument],
            claim_transfers: Vec::new(),
        };
        refresh_node_derivatives(&mut unit_use, 0, 0, 0);
        assert_invalid(unit_use);

        let mut multiple_fields = baseline;
        let psi_terminal::StructuralTypeShape::Record { fields } =
            &mut multiple_fields.structural_types[0].shape
        else {
            panic!("provider fixture attachment record")
        };
        fields.push(structural_leaf_field(
            2,
            psi_terminal::BindingRelevance::Relevant,
            psi_terminal::StructuralFieldType::Erased {
                type_identity: "validation::second-provider".into(),
            },
        ));
        multiple_fields.functions[0].structural_places.push(
            psi_terminal::StructuralPlaceDeclaration {
                id: id(453, PlaceId::new),
                kind: StructuralPlaceKind::ProviderAttachment {
                    attachment,
                    field: id(2, psi_core::StructuralFieldId::new),
                    boundary: unused_boundary,
                },
            },
        );
        assert_invalid(multiple_fields);

        assert!(first_boundary < second_boundary);
    }

    #[test]
    fn rejects_self_consistent_scalar_operation_contract_corruption() {
        let mut arithmetic = exact_add_unit();
        let (psi_operation, result) = match &arithmetic.functions[0].blocks[0].nodes[1].operation {
            AbstractOperation::IntegerConstant {
                psi_operation,
                result,
                ..
            } => (*psi_operation, *result),
            _ => panic!("fixture right operand is an integer constant"),
        };
        arithmetic.functions[0].blocks[0].nodes[1].operation = AbstractOperation::BooleanConstant {
            psi_operation,
            result,
            value: true,
        };
        refresh_node_derivatives(&mut arithmetic, 0, 0, 1);
        assert_eq!(
            validate_psi_optimization_unit(&arithmetic),
            Err(
                OptimizationUnitValidationError::ScalarOperationContractMismatch {
                    machine: id(201, MachineId::new),
                    block: id(202, BlockId::new),
                    node: 2,
                }
            )
        );

        let mut out_of_range = unit();
        let AbstractOperation::IntegerConstant { value, .. } =
            &mut out_of_range.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("fixture begins with an integer constant")
        };
        *value = IntegerValue::Unsigned(256);
        refresh_node_derivatives(&mut out_of_range, 0, 0, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&out_of_range),
            Err(OptimizationUnitValidationError::ScalarOperationContractMismatch { node: 0, .. })
        ));
    }

    #[test]
    fn rejects_self_consistent_control_and_return_type_corruption() {
        let mut conditional = redundant_parameter_region_fixture().0;
        conditional.functions[0].parameters[0].scalar_type =
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("valid integer"));
        refresh_identity(&mut conditional);
        assert!(matches!(
            validate_psi_optimization_unit(&conditional),
            Err(OptimizationUnitValidationError::ScalarOperationContractMismatch { node: 0, .. })
        ));

        let mut scalar_return = unit();
        let (psi_operation, result) = match &scalar_return.functions[0].blocks[0].nodes[0].operation
        {
            AbstractOperation::IntegerConstant {
                psi_operation,
                result,
                ..
            } => (*psi_operation, *result),
            _ => panic!("fixture begins with an integer constant"),
        };
        scalar_return.functions[0].blocks[0].nodes[0].operation =
            AbstractOperation::BooleanConstant {
                psi_operation,
                result,
                value: true,
            };
        refresh_node_derivatives(&mut scalar_return, 0, 0, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&scalar_return),
            Err(OptimizationUnitValidationError::ScalarOperationContractMismatch { node: 1, .. })
        ));
    }

    #[test]
    fn rejects_self_consistent_call_signature_corruption() {
        let mut call = scalar_call_unit();
        let (psi_operation, result) = match &call.functions[0].blocks[0].nodes[0].operation {
            AbstractOperation::IntegerConstant {
                psi_operation,
                result,
                ..
            } => (*psi_operation, *result),
            _ => panic!("caller begins with an integer constant"),
        };
        call.functions[0].blocks[0].nodes[0].operation = AbstractOperation::BooleanConstant {
            psi_operation,
            result,
            value: true,
        };
        refresh_node_derivatives(&mut call, 0, 0, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&call),
            Err(OptimizationUnitValidationError::ScalarOperationContractMismatch { node: 1, .. })
        ));

        let mut boundary = scalar_boundary_call_unit();
        let (psi_operation, result) = match &boundary.functions[0].blocks[0].nodes[0].operation {
            AbstractOperation::IntegerConstant {
                psi_operation,
                result,
                ..
            } => (*psi_operation, *result),
            _ => panic!("boundary caller begins with an integer constant"),
        };
        boundary.functions[0].blocks[0].nodes[0].operation = AbstractOperation::BooleanConstant {
            psi_operation,
            result,
            value: true,
        };
        refresh_node_derivatives(&mut boundary, 0, 0, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&boundary),
            Err(OptimizationUnitValidationError::ScalarOperationContractMismatch { node: 1, .. })
        ));

        let mut duplicate_boundary = scalar_boundary_call_unit();
        duplicate_boundary
            .boundary_machines
            .push(duplicate_boundary.boundary_machines[0].clone());
        refresh_identity(&mut duplicate_boundary);
        assert!(matches!(
            validate_psi_optimization_unit(&duplicate_boundary),
            Err(OptimizationUnitValidationError::DuplicateBoundaryMachine(_))
        ));
    }

    #[test]
    fn rejects_structural_call_argument_arity_and_access_corruption() {
        let baseline = structural_call_unit();
        validate_psi_optimization_unit(&baseline)
            .expect("matching structural argument access should validate");

        let mut access = baseline.clone();
        let AbstractOperation::CallUnit {
            structural_arguments,
            ..
        } = &mut access.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("fixture begins with a structural Unit call")
        };
        structural_arguments[0].access = psi_terminal::StructuralAccess::SharedBorrow;
        refresh_node_derivatives(&mut access, 0, 0, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&access),
            Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
        ));

        let mut arity = baseline;
        let AbstractOperation::CallUnit {
            structural_arguments,
            ..
        } = &mut arity.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("fixture begins with a structural Unit call")
        };
        structural_arguments.clear();
        refresh_node_derivatives(&mut arity, 0, 0, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&arity),
            Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
        ));

        let mut boundary = structural_call_unit();
        let boundary_id = id(341, BoundaryMachineId::new);
        boundary
            .boundary_machines
            .push(psi_terminal::BoundaryMachineDeclaration {
                id: boundary_id,
                identity: "validation::structural-boundary".into(),
                attachment: None,
                scalar_parameters: Vec::new(),
                structural_parameters: vec![boundary.functions[1].structural_parameters[0].clone()],
                result: None,
                requires: Vec::new(),
                program_local_root_introductions: Vec::new(),
                content_guarantees: Vec::new(),
                published_service_ceiling: Vec::new(),
            });
        let (psi_operation, structural_arguments) =
            match &boundary.functions[0].blocks[0].nodes[0].operation {
                AbstractOperation::CallUnit {
                    psi_operation,
                    structural_arguments,
                    ..
                } => (*psi_operation, structural_arguments.clone()),
                _ => panic!("fixture begins with a structural Unit call"),
            };
        boundary.functions[0].blocks[0].nodes[0].operation = AbstractOperation::BoundaryCall {
            psi_operation,
            result: None,
            boundary: boundary_id,
            arguments: Vec::new(),
            structural_arguments,
            completion_claim_sources: Vec::new(),
            completion_receipts: Vec::new(),
        };
        refresh_node_derivatives(&mut boundary, 0, 0, 0);
        validate_psi_optimization_unit(&boundary)
            .expect("matching boundary structural access should validate");

        boundary.boundary_machines[0].structural_parameters[0].access =
            psi_terminal::StructuralAccess::SharedBorrow;
        refresh_identity(&mut boundary);
        assert!(matches!(
            validate_psi_optimization_unit(&boundary),
            Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
        ));
    }

    #[test]
    fn rejects_structural_call_path_type_multiplicity_and_qualification_corruption() {
        let baseline = structural_call_unit();

        let mut path = baseline.clone();
        let AbstractOperation::CallUnit {
            structural_arguments,
            ..
        } = &mut path.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("fixture begins with a structural Unit call")
        };
        structural_arguments[0].path = vec![psi_terminal::StructuralPathSegment::FixedIndex(0)];
        refresh_node_derivatives(&mut path, 0, 0, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&path),
            Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
        ));

        let mut wrong_type = baseline.clone();
        let alternate = id(342, psi_core::StructuralTypeId::new);
        wrong_type
            .structural_types
            .push(psi_terminal::StructuralTypeDeclaration {
                id: alternate,
                identity: "validation::alternate-structural-call-argument".into(),
                shape: psi_terminal::StructuralTypeShape::ByteSequence(
                    psi_terminal::ByteSequenceCarrier::BorrowedView,
                ),
            });
        wrong_type.functions[1].structural_parameters[0].structural_type = alternate;
        refresh_identity(&mut wrong_type);
        assert!(matches!(
            validate_psi_optimization_unit(&wrong_type),
            Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
        ));

        let mut multiplicity = baseline.clone();
        multiplicity.functions[1].structural_parameters[0].multiplicity =
            psi_terminal::StructuralMultiplicity::Affine;
        refresh_identity(&mut multiplicity);
        assert!(matches!(
            validate_psi_optimization_unit(&multiplicity),
            Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
        ));

        let mut source_access = baseline.clone();
        source_access.functions[0].structural_parameters[0].access =
            psi_terminal::StructuralAccess::SharedBorrow;
        refresh_identity(&mut source_access);
        assert!(matches!(
            validate_psi_optimization_unit(&source_access),
            Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
        ));

        let mut qualified = baseline;
        let domain = id(343, psi_core::StructuralDomainId::new);
        qualified.structural_domains = vec![psi_terminal::StructuralDomainDeclaration {
            id: domain,
            semantic_domain: id(344, psi_core::DomainSemanticId::new),
            identity: "validation::structural-call-domain".into(),
            carrier: qualified.structural_types[0].id,
            content_projection: None,
        }]
        .into();
        qualified.functions[0].structural_parameters[0].qualifications = vec![domain];
        qualified.functions[1].structural_parameters[0].qualifications = vec![domain];
        refresh_identity(&mut qualified);
        validate_psi_optimization_unit(&qualified)
            .expect("an exact retained argument qualification should validate");

        qualified.functions[0].structural_parameters[0]
            .qualifications
            .clear();
        refresh_identity(&mut qualified);
        assert!(matches!(
            validate_psi_optimization_unit(&qualified),
            Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
        ));
    }

    #[test]
    fn rejects_self_consistent_internal_claim_transfer_and_boundary_completion_corruption() {
        let internal = affine_claim_transfer_unit();
        let claim = id(1, ClaimId::new);
        validate_psi_optimization_unit(&internal)
            .expect("exact ordinary claim correspondence should validate");

        let mut missing_transfer = internal.clone();
        let AbstractOperation::CallUnit {
            claim_transfers, ..
        } = &mut missing_transfer.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("fixture begins with a structural Unit call")
        };
        claim_transfers.clear();
        refresh_node_derivatives(&mut missing_transfer, 0, 0, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&missing_transfer),
            Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
        ));

        let mut boundary = structural_call_unit();
        boundary.functions[0].structural_parameters[0].multiplicity =
            psi_terminal::StructuralMultiplicity::Affine;
        let entry = psi_terminal::EntryClaim {
            claim,
            input: boundary.functions[0].structural_parameters[0].place,
            path: Vec::new(),
        };
        boundary.functions[0]
            .entry_claim_declarations
            .push(entry.clone());
        boundary.functions[0].entry_claims.insert(claim);
        let boundary_id = id(345, BoundaryMachineId::new);
        let mut parameter = boundary.functions[1].structural_parameters[0].clone();
        parameter.multiplicity = psi_terminal::StructuralMultiplicity::Affine;
        boundary
            .boundary_machines
            .push(psi_terminal::BoundaryMachineDeclaration {
                id: boundary_id,
                identity: "validation::claim-completing-boundary".into(),
                attachment: None,
                scalar_parameters: Vec::new(),
                structural_parameters: vec![parameter],
                result: None,
                requires: Vec::new(),
                program_local_root_introductions: Vec::new(),
                content_guarantees: Vec::new(),
                published_service_ceiling: Vec::new(),
            });
        let (psi_operation, structural_arguments) =
            match &boundary.functions[0].blocks[0].nodes[0].operation {
                AbstractOperation::CallUnit {
                    psi_operation,
                    structural_arguments,
                    ..
                } => (*psi_operation, structural_arguments.clone()),
                _ => panic!("fixture begins with a structural Unit call"),
            };
        boundary.functions[0].blocks[0].nodes[0].operation = AbstractOperation::BoundaryCall {
            psi_operation,
            result: None,
            boundary: boundary_id,
            arguments: Vec::new(),
            structural_arguments,
            completion_claim_sources: vec![omega_abstract_operations::CompletionClaimSource {
                claim,
                entry: Some(entry),
                content: None,
            }],
            completion_receipts: vec![psi_terminal::CompletionReceipt {
                claim,
                argument_index: 0,
            }],
        };
        refresh_node_derivatives(&mut boundary, 0, 0, 0);
        validate_psi_optimization_unit(&boundary)
            .expect("exact boundary completion evidence should validate");

        let AbstractOperation::BoundaryCall {
            completion_claim_sources,
            completion_receipts,
            ..
        } = &mut boundary.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("fixture now contains a boundary call")
        };
        completion_claim_sources.clear();
        completion_receipts.clear();
        refresh_node_derivatives(&mut boundary, 0, 0, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&boundary),
            Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
        ));
    }

    #[test]
    fn current_claim_replay_rejects_double_transfer_stale_crash_and_invalid_returns() {
        let claim = id(1, ClaimId::new);
        validate_psi_optimization_unit(&affine_claim_join_unit(true))
            .expect("equal current claim settlement on both arms joins exactly");
        assert!(matches!(
            validate_psi_optimization_unit(&affine_claim_join_unit(false)),
            Err(OptimizationUnitValidationError::CurrentClaimJoinMismatch { .. })
        ));

        let baseline = affine_claim_transfer_unit();
        validate_psi_optimization_unit(&baseline).expect("one affine claim transfer is live");

        let mut double_transfer = baseline.clone();
        let mut repeated = double_transfer.functions[0].blocks[0].nodes[0].clone();
        let AbstractOperation::CallUnit { psi_operation, .. } = &mut repeated.operation else {
            unreachable!("fixture starts with a Unit call")
        };
        *psi_operation = id(341, OperationId::new);
        double_transfer.functions[0].blocks[0]
            .nodes
            .insert(1, repeated);
        refresh_function_derivatives(&mut double_transfer, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&double_transfer),
            Err(OptimizationUnitValidationError::CurrentClaimNotLive {
                node: 1,
                claim: actual,
                ..
            }) if actual == claim
        ));

        let mut stale_crash = baseline;
        let return_node = stale_crash.functions[0].blocks[0].nodes.len() - 1;
        let psi_edge = match stale_crash.functions[0].blocks[0].nodes[return_node].operation {
            AbstractOperation::ReturnUnit { psi_edge, .. } => psi_edge,
            _ => unreachable!("fixture returns Unit"),
        };
        stale_crash.functions[0].blocks[0].nodes[return_node].operation =
            AbstractOperation::Crash {
                psi_edge,
                cause: psi_terminal::CrashCause::Trap,
                site_guard: Vec::new(),
                frontier_lower_bound: vec![claim],
            };
        refresh_node_derivatives(&mut stale_crash, 0, 0, return_node);
        assert!(matches!(
            validate_psi_optimization_unit(&stale_crash),
            Err(OptimizationUnitValidationError::CurrentCrashClaimFrontierMismatch { .. })
        ));

        let baseline = structural_result_call_unit();
        let mut missing_return = baseline.clone();
        let return_node = missing_return.functions[0].blocks[0].nodes.len() - 1;
        let AbstractOperation::ReturnStructural {
            returned_claims, ..
        } = &mut missing_return.functions[0].blocks[0].nodes[return_node].operation
        else {
            unreachable!("fixture returns the structural call result")
        };
        returned_claims.clear();
        refresh_node_derivatives(&mut missing_return, 0, 0, return_node);
        assert!(matches!(
            validate_psi_optimization_unit(&missing_return),
            Err(OptimizationUnitValidationError::CurrentStructuralReturnClaimSetMismatch { .. })
        ));

        let mut linear_unit_return = baseline;
        let result_place = linear_unit_return.functions[0]
            .result
            .structural()
            .expect("fixture has a structural result")
            .place;
        linear_unit_return.functions[0].result = AbstractFunctionResult::Unit;
        linear_unit_return.functions[0]
            .structural_places
            .retain(|place| place.id != result_place);
        linear_unit_return.functions[0]
            .declared_places
            .remove(&result_place);
        let return_node = linear_unit_return.functions[0].blocks[0].nodes.len() - 1;
        let psi_edge = match linear_unit_return.functions[0].blocks[0].nodes[return_node].operation
        {
            AbstractOperation::ReturnStructural { psi_edge, .. } => psi_edge,
            _ => unreachable!("fixture returns structurally"),
        };
        linear_unit_return.functions[0].blocks[0].nodes[return_node].operation =
            AbstractOperation::ReturnUnit {
                psi_edge,
                cleanup_actions: Vec::new(),
            };
        refresh_node_derivatives(&mut linear_unit_return, 0, 0, return_node);
        assert!(matches!(
            validate_psi_optimization_unit(&linear_unit_return),
            Err(OptimizationUnitValidationError::CurrentLinearClaimAtReturn {
                claim: actual,
                ..
            }) if actual == claim
        ));
    }

    #[test]
    fn current_owned_place_replay_rejects_double_moves_unequal_joins_and_bad_residuals() {
        let baseline = affine_place_transfer_unit();
        validate_psi_optimization_unit(&baseline)
            .expect("one claim-free affine whole-root transfer is exact");

        let mut double_move = baseline;
        let mut repeated = double_move.functions[0].blocks[0].nodes[0].clone();
        let AbstractOperation::CallUnit { psi_operation, .. } = &mut repeated.operation else {
            unreachable!("fixture begins with a Unit call")
        };
        *psi_operation = id(4_862, OperationId::new);
        double_move.functions[0].blocks[0].nodes.insert(1, repeated);
        refresh_function_derivatives(&mut double_move, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&double_move),
            Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive { node: 1, .. })
        ));

        validate_psi_optimization_unit(&affine_place_join_unit(true))
            .expect("equal whole-root settlement on both arms joins exactly");
        assert!(matches!(
            validate_psi_optimization_unit(&affine_place_join_unit(false)),
            Err(OptimizationUnitValidationError::CurrentOwnedPlaceJoinMismatch { .. })
        ));

        let baseline = partial_affine_place_unit();
        validate_psi_optimization_unit(&baseline)
            .expect("one projected move and its exact residual cleanup validate");

        let mut overlap = baseline.clone();
        let mut repeated = overlap.functions[0].blocks[0].nodes[0].clone();
        let AbstractOperation::CallUnit { psi_operation, .. } = &mut repeated.operation else {
            unreachable!("fixture begins with a projected Unit call")
        };
        *psi_operation = id(4_863, OperationId::new);
        overlap.functions[0].blocks[0].nodes.insert(1, repeated);
        refresh_function_derivatives(&mut overlap, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&overlap),
            Err(OptimizationUnitValidationError::CurrentProjectedMoveOverlap { node: 1, .. })
        ));

        let mutate_residual =
            |unit: &mut PsiOptimizationUnit,
             mutate: &dyn Fn(&mut psi_terminal::StructuralAffineDiscard)| {
                let return_node = unit.functions[0].blocks[0].nodes.len() - 1;
                let AbstractOperation::ReturnUnit {
                    cleanup_actions, ..
                } = &mut unit.functions[0].blocks[0].nodes[return_node].operation
                else {
                    unreachable!("fixture returns Unit")
                };
                let [psi_terminal::TerminalAffineCleanupAction::DiscardResidual(residual)] =
                    cleanup_actions.as_mut_slice()
                else {
                    unreachable!("fixture has one residual cleanup")
                };
                mutate(residual);
                refresh_node_derivatives(unit, 0, 0, return_node);
            };

        let mut wrong_path = baseline.clone();
        mutate_residual(&mut wrong_path, &|residual| {
            residual.path = vec![psi_terminal::StructuralPathSegment::Field("right".into())];
        });
        assert!(matches!(
            validate_psi_optimization_unit(&wrong_path),
            Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
        ));

        let mut wrong_type = baseline.clone();
        let pair_type = wrong_type.functions[0].structural_parameters[0].structural_type;
        mutate_residual(&mut wrong_type, &|residual| {
            residual.structural_type = pair_type;
        });
        assert!(matches!(
            validate_psi_optimization_unit(&wrong_type),
            Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
        ));

        let mut missing = baseline;
        let return_node = missing.functions[0].blocks[0].nodes.len() - 1;
        let AbstractOperation::ReturnUnit {
            cleanup_actions, ..
        } = &mut missing.functions[0].blocks[0].nodes[return_node].operation
        else {
            unreachable!("fixture returns Unit")
        };
        cleanup_actions.clear();
        refresh_node_derivatives(&mut missing, 0, 0, return_node);
        assert!(matches!(
            validate_psi_optimization_unit(&missing),
            Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
        ));

        let nominal = boolean_structural_field_unit();
        let mut missing_target = nominal.clone();
        missing_target.functions.pop();
        refresh_identity(&mut missing_target);
        assert!(matches!(
            validate_psi_optimization_unit(&missing_target),
            Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
        ));

        let mut wrong_attachment = nominal.clone();
        wrong_attachment.functions[1].attachment = None;
        refresh_identity(&mut wrong_attachment);
        assert!(matches!(
            validate_psi_optimization_unit(&wrong_attachment),
            Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
        ));

        let mut unnormalized = nominal;
        let AbstractOperation::Return {
            cleanup_actions, ..
        } = &mut unnormalized.functions[0].blocks[0].nodes[1].operation
        else {
            unreachable!("nominal fixture returns a scalar")
        };
        let [psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup)] =
            cleanup_actions.as_mut_slice()
        else {
            unreachable!("nominal fixture has one cleanup")
        };
        cleanup.cleanup_receiver = Some(id(4_864, PlaceId::new));
        refresh_node_derivatives(&mut unnormalized, 0, 0, 1);
        assert!(matches!(
            validate_psi_optimization_unit(&unnormalized),
            Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
        ));
    }

    #[test]
    fn accepts_content_only_internal_claim_transfer_and_rejects_interface_corruption() {
        let mut baseline = structural_call_unit();
        install_content_owner(&mut baseline);
        let claim = id(1, ClaimId::new);
        for function in &mut baseline.functions {
            let root = function.structural_parameters[0].place;
            function
                .content_entry_claims
                .push(content_entry_claim(claim, root));
        }
        let AbstractOperation::CallUnit {
            claim_transfers, ..
        } = &mut baseline.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("fixture begins with a structural Unit call")
        };
        claim_transfers.push(psi_terminal::ClaimTransfer {
            claim,
            argument_index: 0,
        });
        refresh_node_derivatives(&mut baseline, 0, 0, 0);
        validate_psi_optimization_unit(&baseline)
            .expect("content-only claims participate in the live transfer namespace");

        let mut missing_transfer = baseline.clone();
        let AbstractOperation::CallUnit {
            claim_transfers, ..
        } = &mut missing_transfer.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("fixture begins with a structural Unit call")
        };
        claim_transfers.clear();
        refresh_node_derivatives(&mut missing_transfer, 0, 0, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&missing_transfer),
            Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
        ));

        let mut substituted_projection = baseline.clone();
        substituted_projection.functions[0].content_entry_claims[0].projections[0]
            .algebra
            .parameter = "validation::substituted-content".into();
        refresh_identity(&mut substituted_projection);
        assert!(matches!(
            validate_psi_optimization_unit(&substituted_projection),
            Err(OptimizationUnitValidationError::ContentProjectionOwnerMismatch(_))
        ));

        let mutate_projection = [
            |projection: &mut psi_terminal::ClaimContentProjection| {
                projection.projection.domain = id(99, psi_core::ContentDomainId::new);
            },
            |projection: &mut psi_terminal::ClaimContentProjection| {
                projection.projection.projection_fingerprint ^= 1;
            },
            |projection: &mut psi_terminal::ClaimContentProjection| {
                projection.algebra.kind = psi_core::ContentAlgebraKind::IntervalSet;
            },
        ];
        for mutate in mutate_projection {
            let mut candidate = baseline.clone();
            mutate(&mut candidate.functions[0].content_entry_claims[0].projections[0]);
            refresh_identity(&mut candidate);
            assert!(matches!(
                validate_psi_optimization_unit(&candidate),
                Err(OptimizationUnitValidationError::ContentProjectionOwnerMismatch(_))
            ));
        }

        let mut mismatched_interface = baseline.clone();
        let semantic_domain = id(2, psi_core::DomainSemanticId::new);
        let algebra = psi_core::ContentAlgebra {
            kind: psi_core::ContentAlgebraKind::CountedQuantity,
            parameter: "validation::alternate-content".into(),
        };
        let expression = psi_core::ContentProjectionExpression::CountedQuantity(
            psi_core::ContentProjectionScalar::Natural("2".into()),
        );
        let identity = psi_core::ContentProjectionIdentity {
            domain: id(semantic_domain.get(), psi_core::ContentDomainId::new),
            projection_fingerprint:
                psi_language_semantics::content::terminal_projection_fingerprint(
                    &algebra,
                    &expression,
                ),
        };
        let mut domains = mismatched_interface.structural_domains.to_vec();
        domains.push(psi_terminal::StructuralDomainDeclaration {
            id: id(2, StructuralDomainId::new),
            semantic_domain,
            identity: "validation::alternate-content-domain".into(),
            carrier: mismatched_interface.structural_types[0].id,
            content_projection: Some(psi_terminal::StructuralContentProjection {
                identity,
                algebra: algebra.clone(),
                expression,
            }),
        });
        mismatched_interface.structural_domains = domains.into();
        let callee_projection =
            &mut mismatched_interface.functions[1].content_entry_claims[0].projections[0];
        callee_projection.projection = identity;
        callee_projection.algebra = algebra;
        refresh_identity(&mut mismatched_interface);
        assert!(matches!(
            validate_psi_optimization_unit(&mismatched_interface),
            Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
        ));
    }

    #[test]
    fn accepts_content_only_boundary_completion_and_rejects_correspondence_corruption() {
        let mut baseline = structural_call_unit();
        install_content_owner(&mut baseline);
        let claim = id(1, ClaimId::new);
        let caller_root = baseline.functions[0].structural_parameters[0].place;
        let content = content_entry_claim(claim, caller_root);
        baseline.functions[0]
            .content_entry_claims
            .push(content.clone());
        let boundary_id = id(346, BoundaryMachineId::new);
        baseline
            .boundary_machines
            .push(psi_terminal::BoundaryMachineDeclaration {
                id: boundary_id,
                identity: "validation::content-only-boundary".into(),
                attachment: None,
                scalar_parameters: Vec::new(),
                structural_parameters: vec![baseline.functions[1].structural_parameters[0].clone()],
                result: None,
                requires: Vec::new(),
                program_local_root_introductions: Vec::new(),
                content_guarantees: Vec::new(),
                published_service_ceiling: Vec::new(),
            });
        let (psi_operation, structural_arguments) =
            match &baseline.functions[0].blocks[0].nodes[0].operation {
                AbstractOperation::CallUnit {
                    psi_operation,
                    structural_arguments,
                    ..
                } => (*psi_operation, structural_arguments.clone()),
                _ => panic!("fixture begins with a structural Unit call"),
            };
        baseline.functions[0].blocks[0].nodes[0].operation = AbstractOperation::BoundaryCall {
            psi_operation,
            result: None,
            boundary: boundary_id,
            arguments: Vec::new(),
            structural_arguments,
            completion_claim_sources: vec![omega_abstract_operations::CompletionClaimSource {
                claim,
                entry: None,
                content: Some(content),
            }],
            completion_receipts: vec![psi_terminal::CompletionReceipt {
                claim,
                argument_index: 0,
            }],
        };
        refresh_node_derivatives(&mut baseline, 0, 0, 0);
        validate_psi_optimization_unit(&baseline)
            .expect("content-only claims participate in the live completion namespace");

        let mut narrowed = baseline.clone();
        let AbstractOperation::BoundaryCall {
            completion_claim_sources,
            completion_receipts,
            ..
        } = &mut narrowed.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("fixture contains a boundary call")
        };
        completion_claim_sources.clear();
        completion_receipts.clear();
        refresh_node_derivatives(&mut narrowed, 0, 0, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&narrowed),
            Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
        ));

        let mut wrong_claim = baseline;
        let AbstractOperation::BoundaryCall {
            completion_receipts,
            ..
        } = &mut wrong_claim.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("fixture contains a boundary call")
        };
        completion_receipts[0].claim = id(2, ClaimId::new);
        refresh_node_derivatives(&mut wrong_claim, 0, 0, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&wrong_claim),
            Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
        ));
    }

    #[test]
    fn rejects_structural_call_result_signature_and_claim_interface_corruption() {
        let baseline = structural_result_call_unit();
        validate_psi_optimization_unit(&baseline)
            .expect("exact linear structural result should validate");

        let mut wrong_type = baseline.clone();
        let alternate = id(360, psi_core::StructuralTypeId::new);
        wrong_type
            .structural_types
            .push(psi_terminal::StructuralTypeDeclaration {
                id: alternate,
                identity: "validation::alternate-call-result".into(),
                shape: psi_terminal::StructuralTypeShape::ByteSequence(
                    psi_terminal::ByteSequenceCarrier::BorrowedView,
                ),
            });
        let AbstractOperation::CallStructural { result, .. } =
            &mut wrong_type.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("fixture begins with a structural-result call")
        };
        let result_place = result.place;
        result.structural_type = alternate;
        let StructuralPlaceKind::OperationResult {
            structural_type, ..
        } = &mut wrong_type.functions[0]
            .structural_places
            .iter_mut()
            .find(|place| place.id == result_place)
            .expect("caller retains its operation-result place")
            .kind
        else {
            unreachable!("call result has its operation-result root kind")
        };
        *structural_type = alternate;
        let AbstractFunctionResult::Structural(result) = &mut wrong_type.functions[0].result else {
            unreachable!("fixture has a structural result")
        };
        result.structural_type = alternate;
        refresh_node_derivatives(&mut wrong_type, 0, 0, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&wrong_type),
            Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
        ));

        let mut wrong_multiplicity = baseline.clone();
        let AbstractOperation::CallStructural { result, .. } =
            &mut wrong_multiplicity.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("fixture begins with a structural-result call")
        };
        result.multiplicity = psi_terminal::StructuralMultiplicity::Affine;
        let AbstractFunctionResult::Structural(result) =
            &mut wrong_multiplicity.functions[0].result
        else {
            unreachable!("fixture has a structural result")
        };
        result.multiplicity = psi_terminal::StructuralMultiplicity::Affine;
        refresh_node_derivatives(&mut wrong_multiplicity, 0, 0, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&wrong_multiplicity),
            Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
        ));

        let mut invented_claim = baseline;
        let AbstractOperation::CallStructural { result, .. } =
            &mut invented_claim.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("fixture begins with a structural-result call")
        };
        result
            .claims
            .push(psi_terminal::StructuralResultClaimBinding {
                claim: id(1, ClaimId::new),
                path: Vec::new(),
            });
        refresh_node_derivatives(&mut invented_claim, 0, 0, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&invented_claim),
            Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
        ));
    }

    #[test]
    fn replays_service_catalog_hierarchy_ceilings_and_concrete_effects() {
        let baseline = service_effect_unit();
        validate_psi_optimization_unit(&baseline)
            .expect("complete service closure and published PortWrite should validate");
        let root = id(701, ServiceId::new);
        let middle = id(702, ServiceId::new);
        let leaf = id(703, ServiceId::new);
        let unknown = id(799, ServiceId::new);

        let mut duplicate_id = baseline.clone();
        duplicate_id.services = duplicate_id
            .services
            .iter()
            .cloned()
            .chain(std::iter::once(duplicate_id.services[0].clone()))
            .collect::<Vec<_>>()
            .into();
        refresh_identity(&mut duplicate_id);
        assert_eq!(
            validate_psi_optimization_unit(&duplicate_id),
            Err(OptimizationUnitValidationError::DuplicateService(root))
        );

        let mut empty_identity = baseline.clone();
        let mut services = empty_identity.services.to_vec();
        services[0].identity.clear();
        empty_identity.services = services.into();
        refresh_identity(&mut empty_identity);
        assert_eq!(
            validate_psi_optimization_unit(&empty_identity),
            Err(OptimizationUnitValidationError::InvalidServiceIdentity(
                root
            ))
        );

        let mut duplicate_identity = baseline.clone();
        let mut services = duplicate_identity.services.to_vec();
        services[1].identity = services[0].identity.clone();
        duplicate_identity.services = services.into();
        refresh_identity(&mut duplicate_identity);
        assert_eq!(
            validate_psi_optimization_unit(&duplicate_identity),
            Err(OptimizationUnitValidationError::InvalidServiceIdentity(
                middle
            ))
        );

        for (parents, expected) in [
            (
                vec![leaf],
                OptimizationUnitValidationError::InvalidServiceParent {
                    service: leaf,
                    parent: leaf,
                },
            ),
            (
                vec![unknown],
                OptimizationUnitValidationError::InvalidServiceParent {
                    service: leaf,
                    parent: unknown,
                },
            ),
            (
                vec![root, root],
                OptimizationUnitValidationError::InvalidServiceParent {
                    service: leaf,
                    parent: root,
                },
            ),
            (
                vec![middle, root],
                OptimizationUnitValidationError::NonCanonicalServiceParents(leaf),
            ),
        ] {
            let mut candidate = baseline.clone();
            let mut services = candidate.services.to_vec();
            services[2].parents = parents;
            candidate.services = services.into();
            refresh_identity(&mut candidate);
            assert_eq!(validate_psi_optimization_unit(&candidate), Err(expected));
        }

        let mut cycle = baseline.clone();
        let mut services = cycle.services.to_vec();
        services[0].parents = vec![leaf];
        cycle.services = services.into();
        refresh_identity(&mut cycle);
        assert_eq!(
            validate_psi_optimization_unit(&cycle),
            Err(OptimizationUnitValidationError::RecursiveServiceHierarchy(
                root
            ))
        );

        let mut incomplete = baseline.clone();
        let mut services = incomplete.services.to_vec();
        services[2].parents = vec![middle];
        incomplete.services = services.into();
        refresh_identity(&mut incomplete);
        assert_eq!(
            validate_psi_optimization_unit(&incomplete),
            Err(
                OptimizationUnitValidationError::IncompleteServiceParentClosure {
                    service: leaf,
                    ancestor: root,
                }
            )
        );

        for ceiling in [
            vec![unknown],
            vec![root, root],
            vec![leaf, middle, root],
            vec![leaf],
        ] {
            let mut candidate = baseline.clone();
            candidate.functions[0].published_service_ceiling = ceiling;
            refresh_identity(&mut candidate);
            assert_eq!(
                validate_psi_optimization_unit(&candidate),
                Err(
                    OptimizationUnitValidationError::InvalidFunctionServiceCeiling(
                        candidate.functions[0].machine
                    )
                )
            );
        }

        let mut unknown_effect = baseline.clone();
        let AbstractOperation::PortWrite { service, .. } =
            &mut unknown_effect.functions[0].blocks[0].nodes[1].operation
        else {
            panic!("service fixture contains PortWrite")
        };
        *service = unknown;
        refresh_node_derivatives(&mut unknown_effect, 0, 0, 1);
        assert!(matches!(
            validate_psi_optimization_unit(&unknown_effect),
            Err(OptimizationUnitValidationError::OperationServiceContractMismatch { node: 1, .. })
        ));

        let mut outside_ceiling = baseline;
        outside_ceiling.functions[0].published_service_ceiling = vec![root, middle];
        refresh_identity(&mut outside_ceiling);
        assert!(matches!(
            validate_psi_optimization_unit(&outside_ceiling),
            Err(OptimizationUnitValidationError::OperationServiceContractMismatch { node: 1, .. })
        ));

        let mut invalid_boundary = scalar_boundary_call_unit();
        install_service_catalog(&mut invalid_boundary);
        invalid_boundary.boundary_machines[0].published_service_ceiling = vec![leaf];
        refresh_identity(&mut invalid_boundary);
        assert_eq!(
            validate_psi_optimization_unit(&invalid_boundary),
            Err(
                OptimizationUnitValidationError::InvalidBoundaryServiceCeiling(
                    invalid_boundary.boundary_machines[0].id
                )
            )
        );
    }

    #[test]
    fn replays_every_call_reach_lane_and_provider_service_refinement() {
        let root = id(701, ServiceId::new);
        let middle = id(702, ServiceId::new);

        let mut scalar = scalar_call_unit();
        install_service_catalog(&mut scalar);
        scalar.functions[0].published_service_ceiling = vec![root, middle];
        refresh_identity(&mut scalar);
        assert!(matches!(
            validate_psi_optimization_unit(&scalar),
            Err(OptimizationUnitValidationError::OperationServiceContractMismatch { .. })
        ));

        let mut structural_unit = structural_call_unit();
        install_service_catalog(&mut structural_unit);
        structural_unit.functions[0].published_service_ceiling = vec![root, middle];
        refresh_identity(&mut structural_unit);
        assert!(matches!(
            validate_psi_optimization_unit(&structural_unit),
            Err(OptimizationUnitValidationError::OperationServiceContractMismatch { .. })
        ));

        let mut structural_result = structural_result_call_unit();
        install_service_catalog(&mut structural_result);
        structural_result.functions[0].published_service_ceiling = vec![root, middle];
        refresh_identity(&mut structural_result);
        assert!(matches!(
            validate_psi_optimization_unit(&structural_result),
            Err(OptimizationUnitValidationError::OperationServiceContractMismatch { .. })
        ));

        let functions = scalar
            .functions
            .iter()
            .map(|function| (function.machine, function))
            .collect::<BTreeMap<_, _>>();
        let services = scalar
            .services
            .iter()
            .map(|service| (service.id, service))
            .collect::<BTreeMap<_, _>>();
        let caller = &scalar.functions[0];
        let callee = scalar.functions[1].machine;
        let dummy_result = AbstractResult {
            value: id(706, ValueId::new),
            scalar_type: ScalarType::Boolean,
        };
        let dummy_structural_result = psi_terminal::StructuralOperationResult {
            place: id(707, PlaceId::new),
            structural_type: id(708, StructuralTypeId::new),
            multiplicity: psi_terminal::StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            claims: Vec::new(),
        };
        let calls = [
            AbstractOperation::Call {
                psi_operation: id(709, OperationId::new),
                result: dummy_result.value,
                scalar_type: dummy_result.scalar_type,
                callee,
                arguments: Vec::new(),
            },
            AbstractOperation::CallUnit {
                psi_operation: id(710, OperationId::new),
                callee,
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
            },
            AbstractOperation::CallStructuralScalar {
                psi_operation: id(711, OperationId::new),
                result: dummy_result,
                callee,
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
            },
            AbstractOperation::CallStructural {
                psi_operation: id(712, OperationId::new),
                result: dummy_structural_result,
                callee,
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                returned_claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
                selected_evidence: None,
            },
        ];
        for call in &calls {
            assert!(!operation_service_contract_matches(
                caller,
                call,
                &functions,
                &BTreeMap::new(),
                &services,
            ));
        }

        let mut boundary = scalar_boundary_call_unit();
        install_service_catalog(&mut boundary);
        boundary.functions[0].published_service_ceiling = vec![root, middle];
        refresh_identity(&mut boundary);
        assert!(matches!(
            validate_psi_optimization_unit(&boundary),
            Err(OptimizationUnitValidationError::OperationServiceContractMismatch { .. })
        ));

        let provider = provider_service_unit();
        validate_psi_optimization_unit(&provider)
            .expect("provider realized reach exactly refines its boundary");
        let mut mismatched = provider.clone();
        mismatched.provider_candidates[0]
            .refinement
            .realized_service_ceiling
            .pop();
        refresh_identity(&mut mismatched);
        assert!(matches!(
            validate_psi_optimization_unit(&mismatched),
            Err(OptimizationUnitValidationError::InvalidProviderServiceRefinement { .. })
        ));

        let mut outside = provider;
        outside.boundary_machines[0].published_service_ceiling = vec![root, middle];
        refresh_identity(&mut outside);
        assert!(matches!(
            validate_psi_optimization_unit(&outside),
            Err(OptimizationUnitValidationError::InvalidProviderServiceRefinement { .. })
        ));
    }

    #[test]
    fn replays_exact_root_service_reach_shape_and_installation_dependencies() {
        let baseline = installation_root_service_unit();
        validate_psi_optimization_unit(&baseline)
            .expect("one exact installation-bound root dependency validates");
        assert!(baseline.root_service_reach.concrete.is_empty());
        assert_eq!(
            baseline.root_service_reach.installation_dependencies.len(),
            1
        );
        let root = id(701, ServiceId::new);
        let middle = id(702, ServiceId::new);
        let leaf = id(703, ServiceId::new);
        let unknown = id(799, ServiceId::new);

        for concrete in [
            vec![unknown],
            vec![root, root],
            vec![leaf, middle, root],
            vec![leaf],
        ] {
            let mut invalid_concrete = baseline.clone();
            invalid_concrete.root_service_reach.concrete = concrete;
            refresh_identity(&mut invalid_concrete);
            assert_eq!(
                validate_psi_optimization_unit(&invalid_concrete),
                Err(OptimizationUnitValidationError::InvalidRootConcreteServiceReach)
            );
        }

        let mut mismatched_concrete = baseline.clone();
        mismatched_concrete.root_service_reach.concrete = vec![root, middle, leaf];
        refresh_identity(&mut mismatched_concrete);
        assert!(matches!(
            validate_psi_optimization_unit(&mismatched_concrete),
            Err(OptimizationUnitValidationError::RootConcreteServiceReachMismatch { .. })
        ));

        for upper_bound in [
            vec![unknown],
            vec![root, root],
            vec![leaf, middle, root],
            vec![leaf],
        ] {
            let mut invalid = baseline.clone();
            invalid.root_service_reach.installation_dependencies[0].upper_bound = upper_bound;
            refresh_identity(&mut invalid);
            assert_eq!(
                validate_psi_optimization_unit(&invalid),
                Err(OptimizationUnitValidationError::InvalidRootInstallationReachDependency(0))
            );
        }

        let mut empty_identity = baseline.clone();
        empty_identity.root_service_reach.installation_dependencies[0]
            .requirement_identity
            .clear();
        refresh_identity(&mut empty_identity);
        assert_eq!(
            validate_psi_optimization_unit(&empty_identity),
            Err(OptimizationUnitValidationError::InvalidRootInstallationReachDependency(0))
        );

        let mut duplicate = baseline.clone();
        duplicate
            .root_service_reach
            .installation_dependencies
            .push(duplicate.root_service_reach.installation_dependencies[0].clone());
        refresh_identity(&mut duplicate);
        assert_eq!(
            validate_psi_optimization_unit(&duplicate),
            Err(OptimizationUnitValidationError::InvalidRootInstallationReachDependency(1))
        );

        let mut boundary_mismatch = baseline.clone();
        boundary_mismatch
            .root_service_reach
            .installation_dependencies[0]
            .upper_bound = vec![root, middle];
        refresh_identity(&mut boundary_mismatch);
        assert_eq!(
            validate_psi_optimization_unit(&boundary_mismatch),
            Err(
                OptimizationUnitValidationError::RootInstallationReachBoundaryMismatch(
                    boundary_mismatch.boundary_machines[0].id
                )
            )
        );

        let mut missing = baseline.clone();
        missing.root_service_reach.installation_dependencies.clear();
        refresh_identity(&mut missing);
        assert!(matches!(
            validate_psi_optimization_unit(&missing),
            Err(OptimizationUnitValidationError::RootConcreteServiceReachMismatch { .. })
        ));

        let mut unused = baseline.clone();
        unused.root_service_reach.installation_dependencies.push(
            psi_terminal::InstallationReachDependency {
                requirement_identity: "zz-validation::unused-boundary".into(),
                upper_bound: vec![root, middle, leaf],
            },
        );
        refresh_identity(&mut unused);
        assert_eq!(
            validate_psi_optimization_unit(&unused),
            Err(OptimizationUnitValidationError::RootInstallationReachDependenciesMismatch)
        );

        let mut noncanonical = multiple_installation_root_service_unit();
        noncanonical
            .root_service_reach
            .installation_dependencies
            .reverse();
        refresh_identity(&mut noncanonical);
        assert_eq!(
            validate_psi_optimization_unit(&noncanonical),
            Err(OptimizationUnitValidationError::NonCanonicalRootInstallationReachDependencies)
        );

        let repeated = multiple_installation_root_service_unit();
        let boundary_call_count = repeated.functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .filter(|node| matches!(node.operation, AbstractOperation::BoundaryCall { .. }))
            .count();
        assert!(boundary_call_count > repeated.root_service_reach.installation_dependencies.len());
        validate_psi_optimization_unit(&repeated)
            .expect("repeated calls consume one canonical dependency row");

        let mut overlap = baseline;
        let block = overlap.functions[0].blocks[0].id;
        let insertion = overlap.functions[0].blocks[0].nodes.len() - 1;
        let mut write = overlap.functions[0].blocks[0].nodes[0].clone();
        write.operation = AbstractOperation::PortWrite {
            psi_operation: id(729, OperationId::new),
            service: leaf,
            port: 0x3f8,
            value: 0x41,
        };
        overlap.functions[0].blocks[0]
            .nodes
            .insert(insertion, write);
        for (index, node) in overlap.functions[0].blocks[0].nodes.iter_mut().enumerate() {
            node.effect.input = index as u64;
            node.effect.output = index as u64 + 1;
            node.provenance = expected_provenance(&node.operation);
            node.fuel = node
                .provenance
                .iter()
                .copied()
                .map(|site| omega_optimization_unit::FuelSettlement { site, units: 1 })
                .collect();
            node.definitions = expected_definitions(&node.operation, block, index as u32);
            node.uses = expected_uses(&node.operation, block, index as u32);
            node.successors = expected_edges(&node.operation);
            node.ownership = expected_ownership(&node.operation);
        }
        overlap.functions[0].facts = reconstruct_fact_index(&overlap.functions[0]);
        refresh_root_service_reach(&mut overlap)
            .expect("concrete reach remains distinct from installation bounds");
        refresh_identity(&mut overlap);
        validate_psi_optimization_unit(&overlap)
            .expect("concrete and installation-bound reach may overlap");
        assert_eq!(overlap.root_service_reach.concrete, [root, middle, leaf]);
        assert_eq!(
            overlap.root_service_reach.installation_dependencies.len(),
            1
        );
    }

    #[test]
    fn root_service_reach_traverses_every_internal_call_lane_and_ignores_detached_effects() {
        let service = id(703, ServiceId::new);
        let mut baseline = scalar_call_unit();
        install_service_catalog(&mut baseline);
        let callee = baseline.functions[1].machine;
        let mut write = baseline.functions[1].blocks[0].nodes[0].clone();
        write.operation = AbstractOperation::PortWrite {
            psi_operation: id(720, OperationId::new),
            service,
            port: 0x3f8,
            value: 0x41,
        };
        baseline.functions[1].blocks[0].nodes.insert(0, write);
        let calls = [
            AbstractOperation::Call {
                psi_operation: id(721, OperationId::new),
                result: id(722, ValueId::new),
                scalar_type: ScalarType::Boolean,
                callee,
                arguments: Vec::new(),
            },
            AbstractOperation::CallUnit {
                psi_operation: id(723, OperationId::new),
                callee,
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
            },
            AbstractOperation::CallStructuralScalar {
                psi_operation: id(724, OperationId::new),
                result: AbstractResult {
                    value: id(725, ValueId::new),
                    scalar_type: ScalarType::Boolean,
                },
                callee,
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
            },
            AbstractOperation::CallStructural {
                psi_operation: id(726, OperationId::new),
                result: psi_terminal::StructuralOperationResult {
                    place: id(727, PlaceId::new),
                    structural_type: id(728, StructuralTypeId::new),
                    multiplicity: psi_terminal::StructuralMultiplicity::Unrestricted,
                    qualifications: Vec::new(),
                    claims: Vec::new(),
                },
                callee,
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                returned_claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
                selected_evidence: None,
            },
        ];
        for call in calls {
            let mut candidate = baseline.clone();
            let call_node = candidate.functions[0].blocks[0]
                .nodes
                .iter_mut()
                .find(|node| matches!(node.operation, AbstractOperation::Call { .. }))
                .expect("scalar fixture contains one internal call");
            call_node.operation = call;
            refresh_root_service_reach(&mut candidate)
                .expect("every internal call lane reaches the concrete effect");
            assert_eq!(
                candidate.root_service_reach.concrete,
                vec![id(701, ServiceId::new), id(702, ServiceId::new), service]
            );
        }

        let mut detached = baseline;
        detached.functions[0].blocks[0].nodes.clear();
        refresh_root_service_reach(&mut detached)
            .expect("detached function effects do not belong to root reach");
        assert!(detached.root_service_reach.concrete.is_empty());
    }

    #[test]
    fn stale_stored_content_identity_is_rejected_before_structural_validation() {
        let mut stale = unit();
        stale.functions[0].blocks[0].nodes[0].effect.output += 1;
        let recomputed = recompute_psi_optimization_unit_identity(&stale);
        assert!(matches!(
            validate_psi_optimization_unit(&stale),
            Err(OptimizationUnitValidationError::ContentIdentityMismatch {
                stored,
                recomputed: actual,
            }) if stored == stale.identity && actual == recomputed
        ));
    }

    #[test]
    fn recomputed_immutable_signature_forgery_is_rejected_by_verified_context() {
        let verified = verified_unit();
        let structural_type = id(120, psi_core::StructuralTypeId::new);
        let boundary = id(121, psi_core::BoundaryMachineId::new);
        let service = id(122, psi_core::ServiceId::new);
        let mut forged = Vec::new();

        let mut unit = verified.unit().clone();
        unit.structural_types
            .push(psi_terminal::StructuralTypeDeclaration {
                id: structural_type,
                identity: "forged-structural-type".into(),
                shape: psi_terminal::StructuralTypeShape::ByteSequence(
                    psi_terminal::ByteSequenceCarrier::BorrowedView,
                ),
            });
        forged.push(unit);

        let mut unit = verified.unit().clone();
        unit.boundary_machines
            .push(psi_terminal::BoundaryMachineDeclaration {
                id: boundary,
                identity: "forged-boundary".into(),
                attachment: None,
                scalar_parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: None,
                requires: Vec::new(),
                program_local_root_introductions: Vec::new(),
                content_guarantees: Vec::new(),
                published_service_ceiling: Vec::new(),
            });
        forged.push(unit);

        let mut unit = verified.unit().clone();
        unit.provider_candidates
            .push(psi_terminal::ProviderCandidateConformance {
                boundary,
                requirement_identity: "forged-requirement".into(),
                provider_identity: "forged-provider".into(),
                candidate_identity: "forged-candidate".into(),
                candidate: unit.functions[0].machine,
                signature: psi_terminal::ProviderUnitSignature {
                    parameters: Vec::new(),
                },
                refinement: psi_terminal::ProviderUnitRefinement {
                    positional_parameters: Vec::new(),
                    required_domains: Vec::new(),
                    realized_service_ceiling: Vec::new(),
                },
            });
        forged.push(unit);

        let mut unit = verified.unit().clone();
        unit.functions[0].attachment = Some(structural_type);
        forged.push(unit);

        let mut unit = verified.unit().clone();
        let result_value = id(126, ValueId::new);
        unit.functions[0].result = AbstractFunctionResult::Scalar(AbstractResult {
            value: result_value,
            scalar_type: ScalarType::Boolean,
        });
        unit.functions[0].parameters.push(ValueDefinition {
            value: result_value,
            scalar_type: ScalarType::Boolean,
            site: ValueDefinitionSite::FunctionParameter(0),
        });
        let block = unit.functions[0].blocks[0].id;
        let node = &mut unit.functions[0].blocks[0].nodes[0];
        let psi_edge = match &node.operation {
            AbstractOperation::ReturnUnit { psi_edge, .. } => *psi_edge,
            _ => panic!("verified fixture must return Unit"),
        };
        node.operation = AbstractOperation::Return {
            psi_edge,
            result: result_value,
            value: result_value,
            scalar_type: ScalarType::Boolean,
            cleanup_actions: Vec::new(),
        };
        node.uses = vec![ValueUse {
            value: result_value,
            block,
            node: 0,
        }];
        forged.push(unit);

        let mut unit = verified.unit().clone();
        unit.services = vec![psi_terminal::ServiceDeclaration {
            id: service,
            identity: "forged-service".into(),
            parents: Vec::new(),
        }]
        .into();
        forged.push(unit);

        let mut unit = verified.unit().clone();
        unit.services = vec![psi_terminal::ServiceDeclaration {
            id: service,
            identity: "forged-service".into(),
            parents: Vec::new(),
        }]
        .into();
        unit.functions[0].published_service_ceiling.push(service);
        forged.push(unit);

        let mut unit = verified.unit().clone();
        let claim = id(123, ClaimId::new);
        let place = id(124, PlaceId::new);
        unit.functions[0]
            .entry_claim_declarations
            .push(psi_terminal::EntryClaim {
                claim,
                input: place,
                path: Vec::new(),
            });
        unit.functions[0].entry_claims.insert(claim);
        unit.functions[0].declared_places.insert(place);
        forged.push(unit);

        for (index, mut unit) in forged.into_iter().enumerate() {
            refresh_identity(&mut unit);
            let result = validate_transformed_psi_optimization_unit(verified.input(), &unit);
            assert!(
                matches!(
                    result,
                    Err(
                        OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch
                    ) | Err(OptimizationUnitValidationError::StructuralCatalogMismatch { .. })
                        | Err(
                            OptimizationUnitValidationError::InvalidProviderServiceRefinement { .. }
                        )
                ),
                "forgery class {index} returned {result:?}"
            );
        }
    }

    #[test]
    fn ownership_frontier_catalog_rejects_reordering_duplication_and_context_forgery() {
        let verified = verified_unit();
        let original = verified.unit();
        assert!(original.ownership_frontier_facts.len() >= 2);

        let mut reordered = original.clone();
        reordered.ownership_frontier_facts.swap(0, 1);
        refresh_identity(&mut reordered);
        assert_eq!(
            validate_psi_optimization_unit(&reordered),
            Err(OptimizationUnitValidationError::OwnershipFrontierFactIndexMismatch)
        );

        let mut duplicated = original.clone();
        duplicated
            .ownership_frontier_facts
            .insert(1, duplicated.ownership_frontier_facts[0].clone());
        refresh_identity(&mut duplicated);
        assert_eq!(
            validate_psi_optimization_unit(&duplicated),
            Err(OptimizationUnitValidationError::OwnershipFrontierFactIndexMismatch)
        );

        let mut missing = original.clone();
        missing.ownership_frontier_facts.pop();
        refresh_identity(&mut missing);
        assert_eq!(
            validate_transformed_psi_optimization_unit(verified.input(), &missing),
            Err(OptimizationUnitValidationError::OwnershipFrontierFactIndexMismatch)
        );

        let mut forged = original.clone();
        let prior = forged.ownership_frontier_facts[0].clone();
        let mut snapshot = prior.snapshot;
        snapshot.owned_places.push(OwnershipFrontierOwnedPlace {
            place: id(130, PlaceId::new),
            multiplicity: psi_terminal::StructuralMultiplicity::Affine,
        });
        snapshot.owned_places.sort_by_key(|place| place.place);
        forged.ownership_frontier_facts[0] =
            OwnershipFrontierFact::new(prior.psi, prior.machine, prior.site, snapshot);
        refresh_identity(&mut forged);
        assert_eq!(
            validate_transformed_psi_optimization_unit(verified.input(), &forged),
            Err(OptimizationUnitValidationError::OwnershipFrontierFactIndexMismatch)
        );
    }

    #[test]
    fn proof_question_catalog_rejects_missing_reordered_duplicate_and_forged_rows() {
        let verified = verified_unit();
        let original = verified.unit();
        assert_eq!(original.proof_questions.len(), 2);
        assert!(
            original.proof_questions.iter().all(|question| matches!(
                question.owner,
                ProofQuestionOwner::ContractEnsures { .. }
            ))
        );

        let mut reordered = original.clone();
        reordered.proof_questions.swap(0, 1);
        refresh_identity(&mut reordered);
        assert_eq!(
            validate_transformed_psi_optimization_unit(verified.input(), &reordered),
            Err(OptimizationUnitValidationError::ProofQuestionIndexMismatch)
        );

        let mut duplicated = original.clone();
        duplicated
            .proof_questions
            .insert(1, duplicated.proof_questions[0].clone());
        refresh_identity(&mut duplicated);
        assert_eq!(
            validate_psi_optimization_unit(&duplicated),
            Err(OptimizationUnitValidationError::ProofQuestionIndexMismatch)
        );

        let mut missing = original.clone();
        missing.proof_questions.pop();
        refresh_identity(&mut missing);
        assert_eq!(
            validate_transformed_psi_optimization_unit(verified.input(), &missing),
            Err(OptimizationUnitValidationError::ProofQuestionIndexMismatch)
        );

        let mut corruptions = Vec::new();
        let mut owner = original.clone();
        owner.proof_questions[0].owner = ProofQuestionOwner::ContractEnsures {
            machine: owner.functions[0].machine,
            contract: id(104, psi_core::ContractId::new),
            clause_position: 7,
        };
        corruptions.push(owner);
        let mut obligation = original.clone();
        obligation.proof_questions[0].obligation = id(107, psi_core::ObligationId::new);
        corruptions.push(obligation);
        let mut class = original.clone();
        class.proof_questions[0].class = ProofQuestionClass::AdmissionAuthorized {
            site: id(108, psi_core::AdmissionSiteId::new),
            kind: ProofQuestionAdmissionKind::CheckedAssemblyClaim,
            authority_identity: id(109, psi_core::EvidenceIdentity::new),
        };
        corruptions.push(class);
        let mut proposition = original.clone();
        proposition.proof_questions[0].proposition.push(1);
        corruptions.push(proposition);
        let mut requirements = original.clone();
        requirements.proof_questions[0].requirements.push(vec![2]);
        corruptions.push(requirements);
        let mut axioms = original.clone();
        axioms.proof_questions[0].semantic_axioms.push(vec![3]);
        corruptions.push(axioms);
        let mut certificate = original.clone();
        certificate.proof_questions[0].canonical_certificate = true;
        corruptions.push(certificate);
        let mut fingerprint = original.clone();
        fingerprint.proof_questions[0].proof_bundle_fingerprint[0] ^= 1;
        corruptions.push(fingerprint);

        for (index, mut corruption) in corruptions.into_iter().enumerate() {
            refresh_proof_question_identity(&mut corruption.proof_questions[0]);
            refresh_identity(&mut corruption);
            assert_eq!(
                validate_transformed_psi_optimization_unit(verified.input(), &corruption),
                Err(OptimizationUnitValidationError::ProofQuestionIndexMismatch),
                "self-consistent proof-question forgery {index}"
            );
        }
    }

    #[test]
    fn bare_unit_result_signature_must_match_normal_exits() {
        let mut forged = verified_unit().unit().clone();
        forged.functions[0].result = AbstractFunctionResult::Scalar(AbstractResult {
            value: id(125, ValueId::new),
            scalar_type: ScalarType::Boolean,
        });
        refresh_identity(&mut forged);
        assert!(matches!(
            validate_psi_optimization_unit(&forged),
            Err(OptimizationUnitValidationError::FunctionResultMismatch(_))
        ));
    }

    #[test]
    fn independently_accepts_verified_context_and_frontier_coverage() {
        validate_verified_psi_optimization_unit(&verified_unit()).unwrap();
    }

    #[test]
    fn redundant_parameter_region_observation_is_canonical_and_axis_complete() {
        let (input, output, patch, affected) = redundant_parameter_region_fixture();
        let normalized = normalize_redundant_parameter_observation_input(&input, patch, &affected)
            .expect("independent input normalization");
        let expected = reconstruct_psi_closed_region_observation(
            &normalized,
            patch.machine,
            &[affected[1], affected[0], affected[1]],
        )
        .expect("canonical normalized region");
        let baseline = reconstruct_psi_closed_region_observation(&output, patch.machine, &affected)
            .expect("canonical output region");
        assert_eq!(expected.semantics, baseline.semantics);
        assert_ne!(input.identity, output.identity);
        assert_eq!(baseline.semantics.blocks.len(), 2);
        assert!(baseline.semantics.incoming_edges.is_empty());
        assert!(baseline.semantics.outgoing_edges.is_empty());
        assert_eq!(baseline.semantics.scalar_live_ins.len(), 3);
        assert!(baseline.semantics.scalar_live_outs.is_empty());
        let merge_only =
            reconstruct_psi_closed_region_observation(&output, patch.machine, &[patch.block])
                .expect("single-block graph cut");
        assert_eq!(merge_only.semantics.incoming_edges.len(), 2);
        assert!(merge_only.semantics.outgoing_edges.is_empty());
        assert_eq!(merge_only.semantics.scalar_live_ins.len(), 2);
        assert!(unchanged_outside_redundant_parameter_region(
            &input,
            &output,
            patch.machine,
            &affected,
        ));
        let mut outside_region = output.clone();
        outside_region.fuel_schedule = FuelScheduleIdentity::new(2).unwrap();
        assert!(!unchanged_outside_redundant_parameter_region(
            &input,
            &outside_region,
            patch.machine,
            &affected,
        ));

        let mut corruptions = Vec::new();

        let mut arithmetic_policy = output.clone();
        let node = &mut arithmetic_policy.functions[0].blocks[1].nodes[0];
        let AbstractOperation::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } = node.operation.clone()
        else {
            unreachable!()
        };
        node.operation = AbstractOperation::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        };
        corruptions.push(("arithmetic policy", arithmetic_policy));

        let mut edge = output.clone();
        let AbstractOperation::Conditional { when_true, .. } =
            &mut edge.functions[0].blocks[0].nodes[0].operation
        else {
            unreachable!()
        };
        when_true.psi_edge = id(799, EdgeId::new);
        corruptions.push(("control edge", edge));

        let mut successor = output.clone();
        successor.functions[0].blocks[0].nodes[0].successors[0].psi_edge = id(796, EdgeId::new);
        corruptions.push(("successor row", successor));

        let mut normal_exit = output.clone();
        let AbstractOperation::Return { psi_edge, .. } =
            &mut normal_exit.functions[0].blocks[1].nodes[1].operation
        else {
            unreachable!()
        };
        *psi_edge = id(798, EdgeId::new);
        corruptions.push(("normal exit", normal_exit));

        let mut effect = output.clone();
        effect.functions[0].blocks[1].nodes[0].effect.output += 1;
        corruptions.push(("effect", effect));

        let mut ownership = output.clone();
        ownership.functions[0].blocks[1].nodes[0]
            .ownership
            .push(OwnershipEvent::ClaimCompletion(Vec::new()));
        corruptions.push(("ownership/cleanup", ownership));

        let mut provenance = output.clone();
        provenance.functions[0].blocks[1].nodes[0]
            .provenance
            .push(PsiProvenance::Edge(id(797, EdgeId::new)));
        corruptions.push(("provenance", provenance));

        let mut fuel = output.clone();
        fuel.functions[0].blocks[1].nodes[0].fuel[0].units += 1;
        corruptions.push(("fuel", fuel));

        let mut call_and_suspension = output.clone();
        call_and_suspension.functions[0].blocks[1].nodes[0].operation = AbstractOperation::Call {
            psi_operation: id(711, OperationId::new),
            result: id(708, ValueId::new),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
            callee: patch.machine,
            arguments: vec![patch.replacement],
        };
        corruptions.push(("call/crash/suspension", call_and_suspension));

        let mut live_boundary = output.clone();
        live_boundary.functions[0].blocks[1].nodes[0].uses[0].value = id(704, ValueId::new);
        corruptions.push(("typed scalar boundary", live_boundary));

        let mut frontier = output.clone();
        frontier
            .ownership_frontier_facts
            .push(OwnershipFrontierFact::new(
                frontier.psi,
                patch.machine,
                OwnershipFrontierSite::BlockEntry(affected[0]),
                OwnershipFrontierSnapshot {
                    claims: Vec::new(),
                    owned_places: Vec::new(),
                    partial_custody: Vec::new(),
                },
            ));
        corruptions.push(("verifier frontier", frontier));

        for (axis, corrupted) in corruptions {
            let observed =
                reconstruct_psi_closed_region_observation(&corrupted, patch.machine, &affected)
                    .expect("corrupted region remains observable");
            assert_ne!(baseline.semantics, observed.semantics, "{axis}");
        }
    }

    #[test]
    fn independent_integer_rewrite_constructor_accepts_only_declared_evaluation() {
        let input = exact_add_unit();
        let candidate = integer_candidate(&input, IntegerValue::Unsigned(15));
        let replay = integer_candidate(&input, IntegerValue::Unsigned(15));
        assert_eq!(candidate.identity(), replay.identity());
        let input_boundary = reconstruct_closed_scalar_node_boundary(
            &input,
            NodeLocation {
                machine: id(201, MachineId::new),
                block: id(202, BlockId::new),
                node: 2,
            },
        )
        .unwrap();
        let accepted = validate_integer_evaluation_candidate(&input, &candidate).unwrap();
        let output_boundary =
            reconstruct_closed_scalar_node_boundary(accepted.unit(), input_boundary.location)
                .unwrap();
        assert_eq!(input_boundary.live_in.len(), 2);
        assert!(output_boundary.live_in.is_empty());
        assert_eq!(input_boundary.live_out, output_boundary.live_out);
        assert_eq!(accepted.candidate(), candidate.identity());
        assert_ne!(accepted.unit().identity, input.identity);
        assert_eq!(
            accepted.unit().identity,
            recompute_psi_optimization_unit_identity(accepted.unit())
        );
        assert_eq!(
            accepted.unit().functions[0].blocks[0].nodes[2].provenance,
            input.functions[0].blocks[0].nodes[2].provenance
        );
        assert_eq!(
            accepted.unit().functions[0].blocks[0].nodes[2].fuel,
            input.functions[0].blocks[0].nodes[2].fuel
        );
        assert!(matches!(
            accepted.unit().functions[0].blocks[0].nodes[2].operation,
            AbstractOperation::IntegerConstant {
                value: IntegerValue::Unsigned(15),
                ..
            }
        ));
        assert!(matches!(
            accepted.unit().functions[0].facts[2],
            OptimizationFact::IntegerConstant {
                constant: IntegerValue::Unsigned(15),
                ..
            }
        ));
        assert!(matches!(
            input.functions[0].blocks[0].nodes[2].operation,
            AbstractOperation::ExactIntegerAdd { .. }
        ));

        let wrong = integer_candidate(&input, IntegerValue::Unsigned(14));
        assert!(matches!(
            validate_integer_evaluation_candidate(&input, &wrong),
            Err(OptimizationUnitValidationError::CandidateEvaluationMismatch)
        ));

        let foreign_fact = integer_candidate_with_facts(
            &input,
            IntegerValue::Unsigned(15),
            Some(
                omega_optimization_core::ScalarConstantFactIdentity::from_canonical_bytes(
                    b"fact from another revision",
                ),
            ),
            None,
        );
        assert!(matches!(
            validate_integer_evaluation_candidate(&input, &foreign_fact),
            Err(OptimizationUnitValidationError::CandidateOperandFactMismatch)
        ));

        let foreign_obligation = integer_candidate_with_facts(
            &input,
            IntegerValue::Unsigned(15),
            None,
            Some(
                omega_optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
                    b"fact admitted for another operation",
                ),
            ),
        );
        assert!(matches!(
            validate_integer_evaluation_candidate(&input, &foreign_obligation),
            Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
        ));
    }

    #[test]
    fn candidate_history_does_not_declare_the_accepted_content_identity() {
        let input = exact_add_unit();
        let first = integer_candidate_with_facts_and_cost(
            &input,
            IntegerValue::Unsigned(15),
            None,
            None,
            -1,
        );
        let second = integer_candidate_with_facts_and_cost(
            &input,
            IntegerValue::Unsigned(15),
            None,
            None,
            -2,
        );
        assert_ne!(first.identity(), second.identity());

        let first_output = validate_integer_evaluation_candidate(&input, &first).unwrap();
        let second_output = validate_integer_evaluation_candidate(&input, &second).unwrap();
        assert_eq!(first_output.unit(), second_output.unit());
        assert_eq!(
            first_output.unit().identity,
            recompute_psi_optimization_unit_identity(first_output.unit())
        );
    }

    #[test]
    fn corruption_classes_fail_independently() {
        let mut accepted_fact = exact_add_unit();
        accepted_fact.accepted_obligation_facts[0].proof_bundle_fingerprint[0] ^= 1;
        refresh_identity(&mut accepted_fact);
        assert!(matches!(
            validate_psi_optimization_unit(&accepted_fact),
            Err(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch)
        ));

        let mut provenance = unit();
        provenance.functions[0].blocks[0].nodes[0]
            .provenance
            .clear();
        refresh_identity(&mut provenance);
        assert!(matches!(
            validate_psi_optimization_unit(&provenance),
            Err(OptimizationUnitValidationError::OperationMetadataMismatch { .. })
        ));

        let mut fuel = unit();
        fuel.functions[0].blocks[0].nodes[0].fuel.clear();
        refresh_identity(&mut fuel);
        assert!(matches!(
            validate_psi_optimization_unit(&fuel),
            Err(OptimizationUnitValidationError::FuelDoesNotMatchProvenance { .. })
        ));

        let mut effects = unit();
        effects.functions[0].blocks[0].nodes[1].effect.input = 99;
        refresh_identity(&mut effects);
        assert!(matches!(
            validate_psi_optimization_unit(&effects),
            Err(OptimizationUnitValidationError::BrokenEffectChain { .. })
        ));

        let mut facts = unit();
        facts.functions[0].facts.clear();
        refresh_identity(&mut facts);
        assert!(matches!(
            validate_psi_optimization_unit(&facts),
            Err(OptimizationUnitValidationError::FactIndexMismatch(_))
        ));

        let mut forged_uses = unit();
        let block = forged_uses.functions[0].blocks[0].id;
        forged_uses.functions[0].blocks[0].nodes[1]
            .uses
            .push(ValueUse {
                value: id(99, ValueId::new),
                block,
                node: 1,
            });
        refresh_identity(&mut forged_uses);
        assert!(matches!(
            validate_psi_optimization_unit(&forged_uses),
            Err(OptimizationUnitValidationError::OperationMetadataMismatch { .. })
        ));

        let mut forged_definitions = unit();
        forged_definitions.functions[0].blocks[0].nodes[0]
            .definitions
            .clear();
        refresh_identity(&mut forged_definitions);
        assert!(matches!(
            validate_psi_optimization_unit(&forged_definitions),
            Err(OptimizationUnitValidationError::OperationMetadataMismatch { .. })
        ));

        let mut undefined = unit();
        let unknown = id(99, ValueId::new);
        let AbstractOperation::Return { value, .. } =
            &mut undefined.functions[0].blocks[0].nodes[1].operation
        else {
            panic!("unit ends in return")
        };
        *value = unknown;
        undefined.functions[0].blocks[0].nodes[1].uses = vec![ValueUse {
            value: unknown,
            block,
            node: 1,
        }];
        refresh_identity(&mut undefined);
        assert!(matches!(
            validate_psi_optimization_unit(&undefined),
            Err(OptimizationUnitValidationError::UndefinedValue { .. })
        ));

        let mut place = unit();
        place.functions[0]
            .declared_places
            .insert(id(88, PlaceId::new));
        refresh_identity(&mut place);
        assert!(matches!(
            validate_psi_optimization_unit(&place),
            Err(OptimizationUnitValidationError::UnknownPlace { .. })
        ));

        let mut cleanup = unit();
        cleanup.functions[0].blocks[0].nodes[1].ownership.clear();
        refresh_identity(&mut cleanup);
        assert!(matches!(
            validate_psi_optimization_unit(&cleanup),
            Err(OptimizationUnitValidationError::OperationMetadataMismatch { .. })
        ));

        let mut cfg = unit();
        cfg.functions[0].blocks[0].nodes[1].operation = AbstractOperation::Jump {
            psi_edge: id(5, EdgeId::new),
            target: id(77, BlockId::new),
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        };
        cfg.functions[0].blocks[0].nodes[1].successors =
            expected_edges(&cfg.functions[0].blocks[0].nodes[1].operation);
        cfg.functions[0].blocks[0].nodes[1].uses.clear();
        cfg.functions[0].blocks[0].nodes[1].ownership.clear();
        cfg.functions[0].blocks[0].nodes[1].provenance.clear();
        cfg.functions[0].blocks[0].nodes[1].fuel.clear();
        refresh_identity(&mut cfg);
        assert!(matches!(
            validate_psi_optimization_unit(&cfg),
            Err(OptimizationUnitValidationError::UnknownSuccessor { .. })
        ));

        let mut entry_parameters = unit();
        let block = entry_parameters.functions[0].entry;
        entry_parameters.functions[0].blocks[0]
            .parameters
            .push(ValueDefinition {
                value: id(76, ValueId::new),
                scalar_type: ScalarType::Boolean,
                site: ValueDefinitionSite::BlockParameter { block, position: 0 },
            });
        refresh_identity(&mut entry_parameters);
        assert!(matches!(
            validate_psi_optimization_unit(&entry_parameters),
            Err(OptimizationUnitValidationError::EntryBlockHasParameters { .. })
        ));

        let mut unreachable = unit();
        let block = id(75, BlockId::new);
        let mut detached = unreachable.functions[0].blocks[0].clone();
        detached.id = block;
        for (node_index, node) in detached.nodes.iter_mut().enumerate() {
            let node_index = u32::try_from(node_index).unwrap();
            node.definitions = expected_definitions(&node.operation, block, node_index);
            node.uses = expected_uses(&node.operation, block, node_index);
        }
        unreachable.functions[0].blocks.push(detached);
        refresh_identity(&mut unreachable);
        assert!(matches!(
            validate_psi_optimization_unit(&unreachable),
            Err(OptimizationUnitValidationError::UnreachableBlock { .. })
        ));

        let mut cycle = unit();
        let block = cycle.functions[0].entry;
        let operation = AbstractOperation::Jump {
            psi_edge: id(5, EdgeId::new),
            target: block,
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        };
        let node = &mut cycle.functions[0].blocks[0].nodes[1];
        node.operation = operation;
        node.provenance = expected_provenance(&node.operation);
        node.uses = expected_uses(&node.operation, block, 1);
        node.successors = expected_edges(&node.operation);
        node.ownership = expected_ownership(&node.operation);
        refresh_identity(&mut cycle);
        assert!(matches!(
            validate_psi_optimization_unit(&cycle),
            Err(OptimizationUnitValidationError::ControlCycle { .. })
        ));
    }

    #[test]
    fn unknown_claim_frontier_is_rejected() {
        let mut unit = unit();
        let claim = id(71, ClaimId::new);
        let edge = id(5, EdgeId::new);
        let operation = AbstractOperation::Crash {
            psi_edge: edge,
            cause: psi_terminal::CrashCause::Trap,
            site_guard: Vec::new(),
            frontier_lower_bound: vec![claim],
        };
        let node = &mut unit.functions[0].blocks[0].nodes[1];
        node.operation = operation;
        node.provenance = expected_provenance(&node.operation);
        node.fuel[0].site = PsiProvenance::Edge(edge);
        node.uses.clear();
        node.successors.clear();
        node.ownership = expected_ownership(&node.operation);
        refresh_identity(&mut unit);
        assert!(matches!(
            validate_psi_optimization_unit(&unit),
            Err(OptimizationUnitValidationError::UnknownClaim { .. })
        ));
    }
}
