#![forbid(unsafe_code)]

//! Psi checked-semantics to terminal-Psi lowering.
//!
//! This producer proves a real source program can cross the terminal boundary
//! without retaining source trees. Its executable surface is deliberately tiny
//! and exact; unsupported source constructs fail closed instead of being
//! dropped. Checked content conservation, identity reshuffles, and direct
//! partition composition lower into the corresponding terminal-Psi evidence.

use std::collections::{BTreeMap, BTreeSet};

use psi_checked_trees::{
    CheckedBooleanExpression, CheckedBoundaryMachinePlan, CheckedBoundaryScalarReturnMachinePlan,
    CheckedIntegerBinaryKind, CheckedIntegerComparisonKind,
    CheckedNominalAffineUnitCleanupMachinePlan, CheckedPartialAffineUnitCleanupMachinePlan,
    CheckedPropositionBinderArgumentKind, CheckedPropositionBinderKind, CheckedPropositionEvidence,
    CheckedScalarBindingValue, CheckedScalarExpression, CheckedScalarExpressionRole,
    CheckedScalarMachineGraph, CheckedScalarStateTerminator, CheckedScalarSuccessor,
    CheckedStructuralReturnMachinePlan, CheckedStructuralScalarIntegerBoundKind,
    CheckedStructuralScalarIntegerBoundPlan, CheckedStructuralScalarReturnCleanupAction,
    CheckedStructuralScalarReturnMachinePlan, CheckedStructuralUnitControlMachinePlan,
    CheckedStructuralUnitControlTerminatorPlan, CheckedTerminalMachineDebugPlan,
    CheckedTerminalMachineSelection, CheckedTerminalSignatureEligibility, CheckedTrees,
    CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
    CheckedUnitPartialAffineDiscardPlan, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralTypePlan,
    CheckedUnitStructuralTypeShape, ClosedScalarContractValue, ClosedScalarValueContractPlan,
    ContentIdentityReshuffleFact, ContentPartitionCompositionFact, types::PrimitiveType,
};
use psi_core::{
    BlockId, BoundaryMachineId, CanonicalStructuralPathSegment, ClaimId, ContentAlgebra,
    ContentAlgebraKind, ContentConservation, ContentDomainId, ContentPlaceSegment,
    ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace, ContentTerm,
    ContractId, EdgeId, EvidenceIdentity, EvidenceTermId, IntegerSign, IntegerType, IntegerValue,
    MachineId, ObligationId, OperationId, PlaceId, Proposition, PropositionContext,
    PropositionError, PropositionId, ScalarTerm, ScalarType, ServiceId, StructuralDomainId,
    StructuralFieldId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use psi_language_semantics::content::{
    ContentAlgebraIdentity as CheckedContentAlgebraIdentity, ContentConservationEquation,
    ContentConservationOwnerKind, ContentConservationPlan,
    ContentConservationTerm as CheckedContentConservationTerm,
    ContentPlaceRoot as CheckedContentPlaceRoot, ContentPlaceSegment as CheckedContentPlaceSegment,
    ContentPlaceVersion as CheckedContentPlaceVersion,
    ContentStructuralPlace as CheckedContentStructuralPlace, conservation_fingerprint,
};
use psi_language_semantics::{
    CarryPolicy, Multiplicity, PermissionClaimIdentity, SemanticDomainId, ServiceReachId,
    ServiceReachInterface, ServiceReachPlan, ServiceReachRowId, ServiceReachSummary,
};
use psi_proof_kernel::{
    CertificateEnvelope, EvidenceRoute, PrimitiveJudgment, ProofNode, ProofRule, ProofSystemMarker,
};
use psi_terminal::{
    Block, BoundaryMachineDeclaration, ClaimContentProjection, ClaimTransfer, CompletionReceipt,
    ContentEntryClaim, ContentIdentityReshuffle, ContentPartitionComposition,
    ContentPlaceSubstitution, ContractClause, CrashCause as TerminalCrashCause, EntryClaim,
    EvidenceContractLane, EvidenceContractLaneKind, EvidenceInterfaceIdentity,
    EvidencePackageInvocation, EvidencePackageOutputBinding, EvidencePackageRuntimeCall,
    EvidenceProjectionIdentity, EvidenceRequirementIdentity, EvidenceTermDeclaration,
    MachineContract, NominalAffineCleanup, Operation, OperationKind,
    PropositionApplicationIdentity, PropositionBinderArgumentIdentity,
    PropositionBinderArgumentKind, PropositionBinderDeclaration, PropositionBinderKind,
    PropositionDeclaration, PropositionEvidence, ProviderCandidateConformance,
    ProviderParameterRefinement, ProviderSignatureParameter, ProviderUnitRefinement,
    ProviderUnitSignature, ServiceDeclaration, StructuralAffineDiscard, StructuralArgument,
    StructuralDomainDeclaration, StructuralDomainRequirement, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralResultDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge, TerminalAffineCleanupAction,
    TerminalMachine, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
    VocabularyMarker,
};
use psi_terminal_codec::{
    DebugFileId, DebugSite, DebugSourceFile, DebugSourceOrigin, DebugSourceSpan, DebugSubject,
    TerminalDebugMap, source_digest, terminal_psi_identity, validate_debug_map,
};
use psi_terminal_verifier::{
    EvidenceProducerProvenance, EvidenceProducerRealization, EvidenceProducerRowSource,
    ObligationEvidence, ProofBundle, reconstruct_operation_obligations,
};

mod attached_unit;
mod shared_runtime_parameters;
mod structural_scalar_return;
mod structural_unit_control;
mod unit_cleanup;
use attached_unit::{
    checked_unit_boundary_identity, checked_unit_call_closure_including,
    checked_unit_target_reach_matches, collect_contract_services, collect_service_summary,
    lower_attached_unit_closure, lower_attached_unit_closure_including,
    lower_published_service_ceiling, lower_structural_arguments, lower_structural_path,
    lower_unit_parameters, unique_unit_machine, validate_transfer_shape,
};
use shared_runtime_parameters::{
    normalize_shared_boolean_comparison_leaves, resolve_shared_boolean_member_fields,
    shared_boolean_runtime_parameters, valid_shared_boolean_runtime_inputs,
};
use structural_scalar_return::lower_structural_scalar_return_machine;
use structural_unit_control::lower_structural_unit_control_machine;
use unit_cleanup::{
    lower_nominal_affine_unit_cleanup_machine, lower_partial_affine_unit_cleanup_machine,
};

/// Semantic module and separate replaceable proof artifact produced by the
/// Psi frontend producer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredTerminalPsi {
    pub semantic_module: TerminalModule,
    pub proof_bundle: ProofBundle,
    /// Replaceable presentation metadata. The public producer always fills
    /// this after semantic identities are final; private builders leave it
    /// empty until that finalization step.
    pub debug_map: Option<TerminalDebugMap>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LoweredDirectExpression {
    Parameter {
        position: usize,
        scalar_type: ScalarType,
    },
    Local {
        position: usize,
        scalar_type: ScalarType,
    },
    IntegerLiteral {
        value: IntegerValue,
        scalar_type: ScalarType,
    },
    IntegerBinary {
        kind: LoweredIntegerBinaryKind,
        scalar_type: ScalarType,
        left: Box<LoweredDirectExpression>,
        right: Box<LoweredDirectExpression>,
    },
    IntegerBitwiseNot {
        scalar_type: ScalarType,
        operand: Box<LoweredDirectExpression>,
    },
    IntegerWiden {
        scalar_type: ScalarType,
        operand: Box<LoweredDirectExpression>,
    },
    IntegerExactCast {
        scalar_type: ScalarType,
        operand: Box<LoweredDirectExpression>,
    },
    Boolean {
        expression: Box<LoweredBooleanReturnExpression>,
    },
}

impl LoweredDirectExpression {
    const fn scalar_type(&self) -> ScalarType {
        match self {
            Self::Parameter { scalar_type, .. }
            | Self::Local { scalar_type, .. }
            | Self::IntegerLiteral { scalar_type, .. }
            | Self::IntegerBinary { scalar_type, .. }
            | Self::IntegerBitwiseNot { scalar_type, .. }
            | Self::IntegerWiden { scalar_type, .. }
            | Self::IntegerExactCast { scalar_type, .. } => *scalar_type,
            Self::Boolean { .. } => ScalarType::Boolean,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LoweredBooleanReturnExpression {
    Constant {
        value: bool,
    },
    Parameter {
        position: usize,
    },
    Local {
        position: usize,
    },
    UnresolvedStructuralParameterField {
        parameter_position: u32,
        path: Vec<String>,
    },
    StructuralField {
        source: PlaceId,
        field: StructuralFieldId,
    },
    Not {
        operand: Box<LoweredBooleanReturnExpression>,
    },
    Equal {
        left: Box<LoweredBooleanReturnExpression>,
        right: Box<LoweredBooleanReturnExpression>,
    },
    IntegerComparison {
        kind: LoweredIntegerComparisonKind,
        left: Box<LoweredDirectExpression>,
        right: Box<LoweredDirectExpression>,
    },
    And {
        left: Box<LoweredBooleanReturnExpression>,
        right: Box<LoweredBooleanReturnExpression>,
    },
    Or {
        left: Box<LoweredBooleanReturnExpression>,
        right: Box<LoweredBooleanReturnExpression>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoweredIntegerComparisonKind {
    Equal,
    LessThan,
    LessOrEqual,
}

impl LoweredIntegerComparisonKind {
    const fn operation(self, left: ValueId, right: ValueId) -> OperationKind {
        match self {
            Self::Equal => OperationKind::IntegerEqual { left, right },
            Self::LessThan => OperationKind::IntegerLessThan { left, right },
            Self::LessOrEqual => OperationKind::IntegerLessOrEqual { left, right },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LoweredBooleanDecision {
    Value(LoweredBooleanReturnExpression),
    Test {
        condition: LoweredBooleanReturnExpression,
        when_true: Box<LoweredBooleanDecision>,
        when_false: Box<LoweredBooleanDecision>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoweredBooleanDecisionExit {
    Return,
    Jump { target: BlockId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoweredIntegerBinaryKind {
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    WrappingShiftLeft,
    WrappingShiftRight,
    ExactShiftLeft,
    ExactShiftRight,
    ExactAdd,
    ExactSubtract,
    ExactMultiply,
    ExactDivide,
    ExactRemainder,
    WrappingDivide,
    WrappingRemainder,
    SaturatingDivide,
    SaturatingRemainder,
    WrappingAdd,
    SaturatingAdd,
    WrappingSubtract,
    SaturatingSubtract,
    WrappingMultiply,
    SaturatingMultiply,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LoweredScalarBranchTerminator {
    Jump {
        target: usize,
        arguments: Vec<LoweredDirectExpression>,
    },
    Conditional {
        condition: LoweredBooleanReturnExpression,
        when_true_target: usize,
        when_true_arguments: Vec<LoweredDirectExpression>,
        when_false_target: usize,
        when_false_arguments: Vec<LoweredDirectExpression>,
    },
    Return {
        expression: LoweredDirectExpression,
    },
    Crash(LoweredCrashExit),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoweredCrashExit {
    cause: TerminalCrashCause,
    site_guard: Vec<CheckedBooleanExpression>,
    frontier_lower_bound: Vec<ClaimId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoweredScalarBranchState {
    parameter_types: Vec<ScalarType>,
    bindings: Vec<LoweredScalarBinding>,
    terminator: LoweredScalarBranchTerminator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LoweredScalarBinding {
    Expression(LoweredDirectExpression),
    DirectCall(LoweredDirectCallBinding),
}

impl LoweredScalarBinding {
    const fn scalar_type(&self) -> ScalarType {
        match self {
            Self::Expression(expression) => expression.scalar_type(),
            Self::DirectCall(call) => call.result_type,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoweredDirectCallBinding {
    target_machine: psi_symbols::SymbolHandle,
    result_type: ScalarType,
    arguments: Vec<LoweredDirectExpression>,
    crash_continuations: Vec<psi_checked_trees::CrashRouteBucket>,
    parameter_relative_crash_routes: Vec<psi_checked_trees::CrashRouteBucket>,
}

struct PreparedScalarMachine {
    source_machine: psi_symbols::SymbolHandle,
    states: Vec<LoweredScalarBranchState>,
    result_type: ScalarType,
    contract_value: Option<KnownDirectScalar>,
    crash_routes: Vec<psi_checked_trees::CrashRouteBucket>,
    identity_reshuffles: LoweredContentIdentityReshuffles,
    partition_compositions: LoweredContentPartitionCompositions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingConditionalBindingBlock {
    id: BlockId,
    parameters: Vec<ValueDeclaration>,
    target: BlockId,
    arguments: Vec<LoweredDirectExpression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingMixedTupleBindingBlocks {
    first_id: BlockId,
    original_parameter_count: usize,
    arguments: Vec<LoweredDirectExpression>,
    stage_parameters: Vec<Vec<ValueDeclaration>>,
    target: BlockId,
}

const TERMINAL_MACHINE_IDENTITY_STRIDE: u64 = 1_u64 << 32;
// Structural Unit call requirements occupy the upper half of the module-wide
// obligation namespace. Existing contract and cleanup producers allocate from
// the lower half, so composing their proof bundles cannot alias a call site.
const TERMINAL_UNIT_CALL_OBLIGATION_BASE: u64 = 1_u64 << 63;

/// Module-wide operation identities for one machine namespace. Machine zero
/// uses the historical one-based range; additional machines receive disjoint
/// ranges when source call-closure production composes them.
struct OperationBuffer {
    next_identity: u64,
    operations: Vec<Operation>,
}

impl OperationBuffer {
    fn new(identity_base: u64) -> Self {
        Self {
            next_identity: identity_base
                .checked_add(1)
                .expect("operation identity base admits one-based identities"),
            operations: Vec::new(),
        }
    }

    fn allocate(&mut self) -> OperationId {
        let id = operation_id(self.next_identity);
        self.next_identity = self
            .next_identity
            .checked_add(1)
            .expect("terminal operation identities advance");
        id
    }
}

impl std::ops::Deref for OperationBuffer {
    type Target = Vec<Operation>;

    fn deref(&self) -> &Self::Target {
        &self.operations
    }
}

impl std::ops::DerefMut for OperationBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.operations
    }
}

struct CallEmissionContext<'a> {
    machine_ids: &'a [(psi_symbols::SymbolHandle, MachineId)],
    requirement_counts: &'a [(psi_symbols::SymbolHandle, usize)],
    next_obligation_identity: u64,
    obligation_limit: u64,
}

impl CallEmissionContext<'_> {
    fn allocate_requirement(&mut self) -> Result<ObligationId, LoweringError> {
        if self.next_obligation_identity >= self.obligation_limit {
            return unsupported("terminal call obligations exceed their machine identity range");
        }
        let obligation = obligation_id(self.next_obligation_identity);
        self.next_obligation_identity = self
            .next_obligation_identity
            .checked_add(1)
            .expect("terminal call obligation identities advance");
        Ok(obligation)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PendingNestedBlockGroup {
    ConditionalBinding(PendingConditionalBindingBlock),
    TupleBinding(PendingMixedTupleBindingBlocks),
}

impl PendingNestedBlockGroup {
    fn first_id(&self) -> BlockId {
        match self {
            Self::ConditionalBinding(block) => block.id,
            Self::TupleBinding(blocks) => blocks.first_id,
        }
    }
}

impl LoweredIntegerBinaryKind {
    fn operation(self, operation: OperationId, left: ValueId, right: ValueId) -> OperationKind {
        match self {
            Self::BitwiseAnd => OperationKind::IntegerBitwiseAnd { left, right },
            Self::BitwiseOr => OperationKind::IntegerBitwiseOr { left, right },
            Self::BitwiseXor => OperationKind::IntegerBitwiseXor { left, right },
            Self::WrappingShiftLeft => OperationKind::WrappingIntegerShiftLeft {
                value: left,
                count: right,
            },
            Self::WrappingShiftRight => OperationKind::WrappingIntegerShiftRight {
                value: left,
                count: right,
            },
            Self::ExactShiftLeft => OperationKind::ExactIntegerShiftLeft {
                value: left,
                count: right,
                obligation: obligation_id(
                    operation
                        .get()
                        .checked_add(1)
                        .expect("exact-shift obligation follows its operation identity"),
                ),
            },
            Self::ExactShiftRight => OperationKind::ExactIntegerShiftRight {
                value: left,
                count: right,
                obligation: obligation_id(
                    operation
                        .get()
                        .checked_add(1)
                        .expect("exact-shift obligation follows its operation identity"),
                ),
            },
            Self::ExactAdd => OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation: obligation_id(
                    operation
                        .get()
                        .checked_add(1)
                        .expect("exact-add obligation follows its operation identity"),
                ),
            },
            Self::ExactSubtract => OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation: obligation_id(
                    operation
                        .get()
                        .checked_add(1)
                        .expect("exact-subtract obligation follows its operation identity"),
                ),
            },
            Self::ExactMultiply => OperationKind::ExactIntegerMultiply {
                left,
                right,
                obligation: obligation_id(
                    operation
                        .get()
                        .checked_add(1)
                        .expect("exact-multiply obligation follows its operation identity"),
                ),
            },
            Self::ExactDivide => OperationKind::ExactIntegerDivide {
                left,
                right,
                obligation: obligation_id(
                    operation
                        .get()
                        .checked_add(1)
                        .expect("exact-divide obligation follows its operation identity"),
                ),
            },
            Self::ExactRemainder => OperationKind::ExactIntegerRemainder {
                left,
                right,
                obligation: obligation_id(
                    operation
                        .get()
                        .checked_add(1)
                        .expect("exact-remainder obligation follows its operation identity"),
                ),
            },
            Self::WrappingDivide => OperationKind::WrappingIntegerDivide {
                left,
                right,
                obligation: obligation_id(
                    operation
                        .get()
                        .checked_add(1)
                        .expect("wrapping-divide obligation follows its operation identity"),
                ),
            },
            Self::WrappingRemainder => OperationKind::WrappingIntegerRemainder {
                left,
                right,
                obligation: obligation_id(
                    operation
                        .get()
                        .checked_add(1)
                        .expect("wrapping-remainder obligation follows its operation identity"),
                ),
            },
            Self::SaturatingDivide => OperationKind::SaturatingIntegerDivide {
                left,
                right,
                obligation: obligation_id(
                    operation
                        .get()
                        .checked_add(1)
                        .expect("saturating-divide obligation follows its operation identity"),
                ),
            },
            Self::SaturatingRemainder => OperationKind::SaturatingIntegerRemainder {
                left,
                right,
                obligation: obligation_id(
                    operation
                        .get()
                        .checked_add(1)
                        .expect("saturating-remainder obligation follows its operation identity"),
                ),
            },
            Self::WrappingAdd => OperationKind::WrappingIntegerAdd { left, right },
            Self::SaturatingAdd => OperationKind::SaturatingIntegerAdd { left, right },
            Self::WrappingSubtract => OperationKind::WrappingIntegerSubtract { left, right },
            Self::SaturatingSubtract => OperationKind::SaturatingIntegerSubtract { left, right },
            Self::WrappingMultiply => OperationKind::WrappingIntegerMultiply { left, right },
            Self::SaturatingMultiply => OperationKind::SaturatingIntegerMultiply { left, right },
        }
    }
}

/// One checked content equation translated into terminal-Psi identities.
/// Arena-local domain, projection-machine, and field symbols are deliberately
/// absent; only normalized semantic identities and stable spellings survive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredContentConservation {
    pub source_fingerprint: u64,
    pub structural_places: Vec<StructuralPlaceDeclaration>,
    pub proposition: Proposition,
}

/// Canonical terminal-Psi carrier for checker-derived one-to-one claim
/// reshuffles. Source claim identities are used only to group exact projection
/// facts; the emitted IDs are dense and determined by the semantic rows, so no
/// arena-local symbol identity crosses the terminal boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredContentIdentityReshuffles {
    pub structural_places: Vec<StructuralPlaceDeclaration>,
    pub entry_claims: Vec<ContentEntryClaim>,
    pub reshuffles: Vec<ContentIdentityReshuffle>,
    /// Source checked identities paired with their dense terminal IDs.
    /// This map never enters terminal Psi; later derived rows consume it while
    /// the producer still owns both representations.
    pub source_claims: Vec<(PermissionClaimIdentity, ClaimId)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredContentPartitionCompositions {
    pub structural_places: Vec<StructuralPlaceDeclaration>,
    pub compositions: Vec<ContentPartitionComposition>,
}

/// Lower a validated checked-tree content equation into the current terminal-Psi
/// proposition vocabulary. This translation is independent of the narrow
/// executable source slice so broader terminal lowering can reuse it directly.
pub fn lower_content_conservation_plan(
    plan: &ContentConservationPlan,
) -> Result<LoweredContentConservation, LoweringError> {
    let expected_fingerprint = conservation_fingerprint(&plan.algebra, &plan.equation);
    if plan.fingerprint != expected_fingerprint {
        return Err(LoweringError::ContentConservationFingerprintMismatch {
            expected: expected_fingerprint,
            actual: plan.fingerprint,
        });
    }

    let algebra = match &plan.algebra {
        CheckedContentAlgebraIdentity::IntervalSet { coordinate_space } => ContentAlgebra {
            kind: ContentAlgebraKind::IntervalSet,
            parameter: coordinate_space.clone(),
        },
        CheckedContentAlgebraIdentity::CountedQuantity { unit } => ContentAlgebra {
            kind: ContentAlgebraKind::CountedQuantity,
            parameter: unit.clone(),
        },
    };
    let mut structural_places = BTreeMap::new();
    let left = lower_content_term(plan.equation.left(), &mut structural_places, 0)?;
    let right = lower_content_term(plan.equation.right(), &mut structural_places, 0)?;
    let proposition =
        Proposition::ContentConservation(ContentConservation::new(algebra, left, right));
    let context = PropositionContext::from_value_types_and_places(
        [],
        structural_places.iter().map(|(id, kind)| (*id, *kind)),
    )
    .map_err(LoweringError::InvalidContentProposition)?;
    context
        .validate(&proposition)
        .map_err(LoweringError::InvalidContentProposition)?;

    Ok(LoweredContentConservation {
        source_fingerprint: plan.fingerprint,
        structural_places: structural_places
            .into_iter()
            .map(|(id, kind)| StructuralPlaceDeclaration { id, kind })
            .collect(),
        proposition,
    })
}

/// Revalidate and lower all identity facts for one checked callable.
///
/// Multiple exact projections of the same checked claim are grouped into one
/// terminal row. The checked plan remains authoritative for the stable paths;
/// diagnostic arena spans on the fact are intentionally not serialized.
pub fn lower_content_identity_reshuffles(
    facts: &[ContentIdentityReshuffleFact],
) -> Result<LoweredContentIdentityReshuffles, LoweringError> {
    #[derive(Debug)]
    struct Group {
        source_claim: PermissionClaimIdentity,
        input: ContentStructuralPlace,
        output: ContentStructuralPlace,
        projections: Vec<ClaimContentProjection>,
    }

    let Some(first) = facts.first() else {
        return Ok(LoweredContentIdentityReshuffles {
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            reshuffles: Vec::new(),
            source_claims: Vec::new(),
        });
    };
    let callable = (first.machine_symbol, first.state_symbol);
    let mut structural_places = BTreeMap::new();
    let mut projection_algebras = BTreeMap::<ContentProjectionIdentity, ContentAlgebra>::new();
    let mut groups = Vec::<Group>::new();

    for fact in facts {
        if (fact.machine_symbol, fact.state_symbol) != callable
            || fact.plan.owner_kind != ContentConservationOwnerKind::Machine
            || fact.plan.owner != fact.machine_symbol
            || fact.plan.callable != fact.state_symbol
        {
            return Err(LoweringError::ContentIdentityFactOwnerMismatch);
        }
        if fact.claim_identity == PermissionClaimIdentity::Unknown {
            return Err(LoweringError::UnknownContentClaimIdentity);
        }
        validate_identity_input_symbol(fact)?;

        let lowered = lower_content_conservation_plan(&fact.plan)?;
        for declaration in lowered.structural_places {
            if let Some(previous) = structural_places.insert(declaration.id, declaration.kind)
                && previous != declaration.kind
            {
                return Err(LoweringError::ConflictingContentPlaceRoot {
                    id: declaration.id,
                    first: previous,
                    second: declaration.kind,
                });
            }
        }
        let Proposition::ContentConservation(conservation) = lowered.proposition else {
            unreachable!("content plan lowering always yields content conservation")
        };
        let (input, output, projection) = direct_identity_projection(&conservation)?;
        let content = ClaimContentProjection {
            projection,
            algebra: conservation.algebra().clone(),
        };
        if let Some(previous) =
            projection_algebras.insert(content.projection, content.algebra.clone())
            && previous != content.algebra
        {
            return Err(LoweringError::ContentProjectionAlgebraMismatch(
                content.projection,
            ));
        }

        if let Some(group) = groups
            .iter_mut()
            .find(|group| group.source_claim == fact.claim_identity)
        {
            if group.input != input || group.output != output {
                return Err(LoweringError::ContentIdentityClaimMapsMultiplePlaces);
            }
            group.projections.push(content);
        } else {
            groups.push(Group {
                source_claim: fact.claim_identity,
                input,
                output,
                projections: vec![content],
            });
        }
    }

    for group in &mut groups {
        group.projections.sort();
        if group.projections.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(LoweringError::DuplicateContentIdentityProjection);
        }
    }
    groups.sort_by(|left, right| {
        (&left.input, &left.output, &left.projections).cmp(&(
            &right.input,
            &right.output,
            &right.projections,
        ))
    });
    let mut inputs = BTreeSet::<&ContentStructuralPlace>::new();
    for group in &groups {
        if !inputs.insert(&group.input) {
            return Err(LoweringError::DuplicateContentIdentityInput);
        }
        if inputs.iter().any(|previous| {
            **previous != group.input && content_places_overlap(previous, &group.input)
        }) {
            return Err(LoweringError::OverlappingContentIdentityInput);
        }
    }
    let mut outputs = BTreeSet::<&ContentStructuralPlace>::new();
    for group in &groups {
        if !outputs.insert(&group.output) {
            return Err(LoweringError::DuplicateContentIdentityOutput);
        }
        if outputs.iter().any(|previous| {
            **previous != group.output && content_places_overlap(previous, &group.output)
        }) {
            return Err(LoweringError::OverlappingContentIdentityOutput);
        }
    }

    let mut source_claims = Vec::new();
    let reshuffles = groups
        .into_iter()
        .enumerate()
        .map(|(index, group)| {
            let claim = ClaimId::new(
                u64::try_from(index)
                    .expect("an in-memory fact count fits u64")
                    .checked_add(1)
                    .expect("an in-memory fact count cannot exhaust u64"),
            )
            .expect("dense claim identities begin at one");
            source_claims.push((group.source_claim, claim));
            ContentIdentityReshuffle {
                claim,
                input: group.input,
                output: group.output,
                projections: group.projections,
            }
        })
        .collect::<Vec<_>>();
    let entry_claims = reshuffles
        .iter()
        .map(|reshuffle| ContentEntryClaim {
            claim: reshuffle.claim,
            input: reshuffle.input.clone(),
            projections: reshuffle.projections.clone(),
        })
        .collect();
    Ok(LoweredContentIdentityReshuffles {
        structural_places: structural_places
            .into_iter()
            .map(|(id, kind)| StructuralPlaceDeclaration { id, kind })
            .collect(),
        entry_claims,
        reshuffles,
        source_claims,
    })
}

/// Lower checker-proved direct partition composition into terminal Psi.
/// The terminal row retains both equations and the exact place substitution so
/// the verifier can replay it and reject any manufactured `separate(...)` node.
pub fn lower_content_partition_compositions(
    facts: &[ContentPartitionCompositionFact],
    identity_reshuffles: &mut LoweredContentIdentityReshuffles,
) -> Result<LoweredContentPartitionCompositions, LoweringError> {
    let mut rebuilt_identity_reshuffles = identity_reshuffles.clone();
    let Some(first) = facts.first() else {
        rebuild_content_entry_claims(&mut rebuilt_identity_reshuffles, facts)?;
        *identity_reshuffles = rebuilt_identity_reshuffles;
        return Ok(LoweredContentPartitionCompositions {
            structural_places: Vec::new(),
            compositions: Vec::new(),
        });
    };
    let callable = (first.machine_symbol, first.state_symbol);
    for fact in facts {
        if fact.source_derivation_depth != 0 {
            return Err(LoweringError::ContentPartitionDerivedSourceUnsupported);
        }
        if !fact.result_rewrites.is_empty() {
            return Err(LoweringError::ContentPartitionResultRewriteUnsupported);
        }
        if (fact.machine_symbol, fact.state_symbol) != callable
            || fact.plan.owner_kind != ContentConservationOwnerKind::Machine
            || fact.plan.owner != fact.machine_symbol
            || fact.plan.callable != fact.state_symbol
            || fact.source_plan.callable != fact.source_callable
            || fact.source_plan.fingerprint != fact.source_fingerprint
        {
            return Err(LoweringError::ContentPartitionFactOwnerMismatch);
        }
        revalidate_content_partition_fact(fact)?;
    }
    rebuild_content_entry_claims(&mut rebuilt_identity_reshuffles, facts)?;
    let mut target_places = BTreeMap::new();
    let mut compositions = Vec::new();

    for fact in facts {
        let source = lower_content_conservation_plan(&fact.source_plan)?;
        let derived = lower_content_conservation_plan(&fact.plan)?;
        let source_conservation = lowered_conservation(source.proposition)?;
        let derived_conservation = lowered_conservation(derived.proposition)?;
        for declaration in derived.structural_places {
            merge_content_place_declaration(&mut target_places, declaration)?;
        }

        let mut source_places = source
            .structural_places
            .iter()
            .map(|place| (place.id, place.kind))
            .collect::<BTreeMap<_, _>>();
        let mut substitution_target_places = target_places.clone();
        let mut substitutions = fact
            .substitutions
            .iter()
            .map(|substitution| {
                Ok(ContentPlaceSubstitution {
                    source: lower_content_place(&substitution.source, &mut source_places)?,
                    target: lower_content_place(
                        &substitution.target,
                        &mut substitution_target_places,
                    )?,
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        substitutions.sort();
        if substitutions.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(LoweringError::DuplicateContentPartitionSubstitution);
        }
        target_places = substitution_target_places;

        let mut input_claims = fact
            .input_claim_identities
            .iter()
            .map(|identity| {
                rebuilt_identity_reshuffles
                    .source_claims
                    .iter()
                    .find_map(|(source, claim)| (source == identity).then_some(*claim))
                    .ok_or(LoweringError::ContentPartitionInputClaimNotLowered)
            })
            .collect::<Result<Vec<_>, _>>()?;
        input_claims.sort();
        input_claims.dedup();
        if input_claims.is_empty() {
            return Err(LoweringError::ContentPartitionInputClaimNotLowered);
        }
        let mut source_structural_places = source.structural_places.into_iter().collect::<Vec<_>>();
        source_structural_places.sort();
        compositions.push(ContentPartitionComposition {
            source_fingerprint: fact.source_fingerprint,
            source_structural_places,
            source: source_conservation,
            input_claims,
            substitutions,
            derived: derived_conservation,
        });
    }

    compositions.sort();
    if compositions.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(LoweringError::DuplicateContentPartitionComposition);
    }
    *identity_reshuffles = rebuilt_identity_reshuffles;
    Ok(LoweredContentPartitionCompositions {
        structural_places: target_places
            .into_iter()
            .map(|(id, kind)| StructuralPlaceDeclaration { id, kind })
            .collect(),
        compositions,
    })
}

fn rebuild_content_entry_claims(
    lowered: &mut LoweredContentIdentityReshuffles,
    partition_facts: &[ContentPartitionCompositionFact],
) -> Result<(), LoweringError> {
    #[derive(Debug)]
    struct Group {
        source_claim: PermissionClaimIdentity,
        input: ContentStructuralPlace,
        projections: Vec<ClaimContentProjection>,
    }

    let mut groups = lowered
        .source_claims
        .iter()
        .map(|(source_claim, claim)| {
            let reshuffle = lowered
                .reshuffles
                .iter()
                .find(|reshuffle| reshuffle.claim == *claim)
                .expect("lowered source claim names its reshuffle");
            Group {
                source_claim: *source_claim,
                input: reshuffle.input.clone(),
                projections: reshuffle.projections.clone(),
            }
        })
        .collect::<Vec<_>>();

    for fact in partition_facts {
        let mut listed = Vec::new();
        for identity in &fact.input_claim_identities {
            if !listed.contains(identity) {
                listed.push(*identity);
            }
        }
        let mut bound = Vec::new();
        for binding in &fact.input_claim_bindings {
            if !bound.contains(&binding.claim_identity) {
                bound.push(binding.claim_identity);
            }
        }
        if bound.is_empty()
            || listed.len() != bound.len()
            || listed.iter().any(|identity| !bound.contains(identity))
        {
            return Err(LoweringError::ContentPartitionInputClaimBindingMismatch);
        }

        let lowered_plan = lower_content_conservation_plan(&fact.plan)?;
        let Proposition::ContentConservation(conservation) = lowered_plan.proposition else {
            unreachable!("content plan lowering always yields content conservation")
        };
        for binding in &fact.input_claim_bindings {
            if binding.claim_identity == PermissionClaimIdentity::Unknown {
                return Err(LoweringError::UnknownContentClaimIdentity);
            }
            let mut places = BTreeMap::new();
            let input = lower_content_place(&binding.entry_place, &mut places)?;
            if input.version != ContentPlaceVersion::Entry {
                return Err(LoweringError::ContentEntryClaimRequiresEntryPlace);
            }
            let mut projections = Vec::new();
            collect_terminal_content_projections(
                conservation.left(),
                conservation.algebra(),
                &input,
                &mut projections,
            );
            collect_terminal_content_projections(
                conservation.right(),
                conservation.algebra(),
                &input,
                &mut projections,
            );
            projections.sort();
            projections.dedup();
            if projections.is_empty() {
                return Err(LoweringError::ContentEntryClaimHasNoProjection);
            }
            if let Some(group) = groups
                .iter_mut()
                .find(|group| group.source_claim == binding.claim_identity)
            {
                if group.input != input {
                    return Err(LoweringError::ContentEntryClaimMapsMultiplePlaces);
                }
                group.projections.extend(projections);
            } else {
                groups.push(Group {
                    source_claim: binding.claim_identity,
                    input,
                    projections,
                });
            }
        }
    }

    for group in &mut groups {
        group.projections.sort();
        group.projections.dedup();
    }
    groups.sort_by(|left, right| {
        (&left.input, &left.projections).cmp(&(&right.input, &right.projections))
    });
    for (index, group) in groups.iter().enumerate() {
        if groups[..index]
            .iter()
            .any(|previous| previous.input == group.input)
        {
            return Err(LoweringError::DuplicateContentEntryClaimInput);
        }
        if groups[..index]
            .iter()
            .any(|previous| content_places_overlap(&previous.input, &group.input))
        {
            return Err(LoweringError::OverlappingContentEntryClaimInput);
        }
    }

    let mut source_claims = Vec::with_capacity(groups.len());
    let mut entry_claims = Vec::with_capacity(groups.len());
    for (index, group) in groups.into_iter().enumerate() {
        let claim = ClaimId::new(
            u64::try_from(index)
                .expect("an in-memory fact count fits u64")
                .checked_add(1)
                .expect("an in-memory fact count cannot exhaust u64"),
        )
        .expect("dense claim identities begin at one");
        source_claims.push((group.source_claim, claim));
        entry_claims.push(ContentEntryClaim {
            claim,
            input: group.input,
            projections: group.projections,
        });
    }
    for reshuffle in &mut lowered.reshuffles {
        let source = lowered
            .source_claims
            .iter()
            .find_map(|(source, old)| (*old == reshuffle.claim).then_some(*source))
            .expect("every reshuffle has a checked source claim");
        reshuffle.claim = source_claims
            .iter()
            .find_map(|(candidate, claim)| (*candidate == source).then_some(*claim))
            .expect("every reshuffle source survives entry-claim rebuilding");
    }
    lowered.reshuffles.sort_by_key(|reshuffle| reshuffle.claim);
    lowered.source_claims = source_claims;
    lowered.entry_claims = entry_claims;
    Ok(())
}

fn collect_terminal_content_projections(
    term: &ContentTerm,
    algebra: &ContentAlgebra,
    subject: &ContentStructuralPlace,
    output: &mut Vec<ClaimContentProjection>,
) {
    match term {
        ContentTerm::Projection {
            projection,
            subject: candidate,
        } if candidate == subject => output.push(ClaimContentProjection {
            projection: *projection,
            algebra: algebra.clone(),
        }),
        ContentTerm::Projection { .. } => {}
        ContentTerm::Separate(terms) => {
            for term in terms {
                collect_terminal_content_projections(term, algebra, subject, output);
            }
        }
    }
}

fn revalidate_content_partition_fact(
    fact: &ContentPartitionCompositionFact,
) -> Result<(), LoweringError> {
    if fact.substitutions.is_empty()
        || !matches!(
            fact.source_plan.equation.left(),
            CheckedContentConservationTerm::Separate(_)
        ) && !matches!(
            fact.source_plan.equation.right(),
            CheckedContentConservationTerm::Separate(_)
        )
    {
        return Err(LoweringError::ContentPartitionSubstitutionCoverageMismatch);
    }
    for (index, substitution) in fact.substitutions.iter().enumerate() {
        if fact.substitutions[..index]
            .iter()
            .any(|previous| previous.source == substitution.source)
            || fact.substitutions[..index]
                .iter()
                .any(|previous| previous.target == substitution.target)
        {
            return Err(LoweringError::DuplicateContentPartitionSubstitution);
        }
        if !checked_partition_term_contains_subject(
            fact.source_plan.equation.left(),
            &substitution.source,
        ) && !checked_partition_term_contains_subject(
            fact.source_plan.equation.right(),
            &substitution.source,
        ) {
            return Err(LoweringError::ContentPartitionSubstitutionCoverageMismatch);
        }
    }
    let replay = |term| replay_checked_partition_term(term, &fact.substitutions);
    let equation = ContentConservationEquation::new(
        replay(fact.source_plan.equation.left())?,
        replay(fact.source_plan.equation.right())?,
    );
    if fact.source_plan.algebra != fact.plan.algebra || equation != fact.plan.equation {
        return Err(LoweringError::ContentPartitionReplayMismatch);
    }
    Ok(())
}

fn checked_partition_term_contains_subject(
    term: &CheckedContentConservationTerm,
    expected: &CheckedContentStructuralPlace,
) -> bool {
    match term {
        CheckedContentConservationTerm::Projection { subject, .. } => subject == expected,
        CheckedContentConservationTerm::Separate(terms) => terms
            .iter()
            .any(|term| checked_partition_term_contains_subject(term, expected)),
    }
}

fn replay_checked_partition_term(
    term: &CheckedContentConservationTerm,
    substitutions: &[psi_checked_trees::ContentPartitionPlaceSubstitution],
) -> Result<CheckedContentConservationTerm, LoweringError> {
    match term {
        CheckedContentConservationTerm::Projection {
            domain,
            semantic_domain,
            projection_machine,
            projection_fingerprint,
            subject,
        } => {
            let target = substitutions
                .iter()
                .find_map(|substitution| {
                    (substitution.source == *subject).then_some(substitution.target.clone())
                })
                .ok_or(LoweringError::ContentPartitionSubstitutionCoverageMismatch)?;
            Ok(CheckedContentConservationTerm::Projection {
                domain: *domain,
                semantic_domain: *semantic_domain,
                projection_machine: *projection_machine,
                projection_fingerprint: *projection_fingerprint,
                subject: target,
            })
        }
        CheckedContentConservationTerm::Separate(terms) => {
            Ok(CheckedContentConservationTerm::separate(
                terms
                    .iter()
                    .map(|term| replay_checked_partition_term(term, substitutions))
                    .collect::<Result<Vec<_>, _>>()?,
            ))
        }
    }
}

fn lowered_conservation(proposition: Proposition) -> Result<ContentConservation, LoweringError> {
    match proposition {
        Proposition::ContentConservation(conservation) => Ok(conservation),
        _ => Err(LoweringError::ContentPartitionNotConservation),
    }
}

fn merge_content_place_declaration(
    places: &mut BTreeMap<PlaceId, StructuralPlaceKind>,
    declaration: StructuralPlaceDeclaration,
) -> Result<(), LoweringError> {
    if let Some(previous) = places.insert(declaration.id, declaration.kind)
        && previous != declaration.kind
    {
        return Err(LoweringError::ConflictingContentPlaceRoot {
            id: declaration.id,
            first: previous,
            second: declaration.kind,
        });
    }
    Ok(())
}

fn validate_identity_input_symbol(
    fact: &ContentIdentityReshuffleFact,
) -> Result<(), LoweringError> {
    let roots = [fact.plan.equation.left(), fact.plan.equation.right()];
    let has_input = roots.iter().any(|term| {
        matches!(
            term,
            CheckedContentConservationTerm::Projection {
                subject: CheckedContentStructuralPlace {
                    version: CheckedContentPlaceVersion::Entry,
                    root: CheckedContentPlaceRoot::Parameter { symbol, .. },
                    ..
                },
                ..
            } if *symbol == fact.input_parameter_symbol
        )
    });
    if has_input {
        Ok(())
    } else {
        Err(LoweringError::ContentIdentityInputParameterMismatch)
    }
}

fn direct_identity_projection(
    conservation: &ContentConservation,
) -> Result<
    (
        ContentStructuralPlace,
        ContentStructuralPlace,
        ContentProjectionIdentity,
    ),
    LoweringError,
> {
    let projection = |term: &ContentTerm| match term {
        ContentTerm::Projection {
            projection,
            subject,
        } => Some((*projection, subject.clone())),
        ContentTerm::Separate(_) => None,
    };
    let (left_projection, left) =
        projection(conservation.left()).ok_or(LoweringError::ContentIdentityNotDirectEquality)?;
    let (right_projection, right) =
        projection(conservation.right()).ok_or(LoweringError::ContentIdentityNotDirectEquality)?;
    if left_projection != right_projection {
        return Err(LoweringError::ContentIdentityProjectionMismatch);
    }
    let (input, output) = match (left.version, right.version) {
        (ContentPlaceVersion::Entry, ContentPlaceVersion::Current) => (left, right),
        (ContentPlaceVersion::Current, ContentPlaceVersion::Entry) => (right, left),
        _ => return Err(LoweringError::ContentIdentityDirectionMismatch),
    };
    if input.root.get() >= RESULT_STRUCTURAL_PLACE_ID
        || output.root.get() != RESULT_STRUCTURAL_PLACE_ID
    {
        return Err(LoweringError::ContentIdentityRootMismatch);
    }
    Ok((input, output, left_projection))
}

fn content_places_overlap(left: &ContentStructuralPlace, right: &ContentStructuralPlace) -> bool {
    if left.version != right.version || left.root != right.root {
        return false;
    }
    let shared = left.segments.len().min(right.segments.len());
    left.segments[..shared] == right.segments[..shared]
}

const MAX_CONTENT_TERM_DEPTH: usize = 256;
/// First identity after the complete `parameter position + 1` range.
const RESULT_STRUCTURAL_PLACE_ID: u64 = 4_294_967_297;

fn lower_content_term(
    term: &CheckedContentConservationTerm,
    structural_places: &mut BTreeMap<PlaceId, StructuralPlaceKind>,
    depth: usize,
) -> Result<ContentTerm, LoweringError> {
    if depth > MAX_CONTENT_TERM_DEPTH {
        return Err(LoweringError::ContentTermNestingTooDeep);
    }
    match term {
        CheckedContentConservationTerm::Projection {
            semantic_domain,
            projection_fingerprint,
            subject,
            ..
        } => {
            let domain = ContentDomainId::new(u64::from(semantic_domain.0))
                .ok_or(LoweringError::InvalidContentDomainIdentity)?;
            if *projection_fingerprint == 0 {
                return Err(LoweringError::ZeroContentProjectionFingerprint);
            }
            Ok(ContentTerm::Projection {
                projection: ContentProjectionIdentity {
                    domain,
                    projection_fingerprint: *projection_fingerprint,
                },
                subject: lower_content_place(subject, structural_places)?,
            })
        }
        CheckedContentConservationTerm::Separate(terms) => ContentTerm::separate(
            terms
                .iter()
                .map(|term| lower_content_term(term, structural_places, depth + 1))
                .collect::<Result<Vec<_>, _>>()?,
        )
        .map_err(LoweringError::InvalidContentProposition),
    }
}

fn lower_content_place(
    place: &CheckedContentStructuralPlace,
    structural_places: &mut BTreeMap<PlaceId, StructuralPlaceKind>,
) -> Result<ContentStructuralPlace, LoweringError> {
    let version = match place.version {
        CheckedContentPlaceVersion::Entry => ContentPlaceVersion::Entry,
        CheckedContentPlaceVersion::Current => ContentPlaceVersion::Current,
    };
    let (root, kind) = match &place.root {
        CheckedContentPlaceRoot::Parameter {
            position, is_self, ..
        } => (
            PlaceId::new(u64::from(*position) + 1)
                .expect("a parameter position plus one is nonzero"),
            StructuralPlaceKind::Parameter {
                position: *position,
                is_self: *is_self,
            },
        ),
        CheckedContentPlaceRoot::Result => (
            PlaceId::new(RESULT_STRUCTURAL_PLACE_ID).expect("the reserved result place is nonzero"),
            StructuralPlaceKind::Result,
        ),
    };
    if let Some(previous) = structural_places.insert(root, kind)
        && previous != kind
    {
        return Err(LoweringError::ConflictingContentPlaceRoot {
            id: root,
            first: previous,
            second: kind,
        });
    }
    let segments = place
        .segments
        .iter()
        .map(|segment| match segment {
            CheckedContentPlaceSegment::Case(case) => ContentPlaceSegment::Case(case.name.clone()),
            CheckedContentPlaceSegment::Field(field) => {
                ContentPlaceSegment::Field(field.name.clone())
            }
            CheckedContentPlaceSegment::FixedIndex(index) => {
                ContentPlaceSegment::FixedIndex(*index)
            }
        })
        .collect();
    Ok(ContentStructuralPlace {
        version,
        root,
        segments,
    })
}

/// Lower one named checked free machine and its reachable checked scalar callees
/// through the current terminal-Psi source slice.
///
/// Accepted shape:
///
/// ```text
/// machine name(p0: bool, ...) -> bool
/// requires B == B
/// ensures B == B
/// {
///     B | pN
/// }
///
/// machine name(p0: bool, ...) -> bool
/// requires B == B
/// ensures B == B
/// {
///     transition { _ -> next(B | pN) }
///     state next(value: bool) -> bool { B | value }
/// }
///
/// machine name(p0: integer, ...) -> integer
/// requires C == C
/// ensures C == C
/// {
///     E
/// }
/// E := pN | L | E (+|-|*) E
///
/// machine name(p0: integer, ...) -> integer
/// requires L == L
/// ensures L == L
/// {
///     transition { _ -> next(E0, E1, ...) }
///     state next(p0: integer, p1: integer, ...) -> integer {
///         transition { _ -> done(E0, E1, ...) }
///     }
///     state done(p0: integer, p1: integer, ...) -> integer { E }
/// }
/// ```
///
/// The first explicit-crash slice also accepts a one-state scalar machine whose
/// sole statement is `crash Cause;` and whose checked site cites a prechecked
/// same-cause route bucket. It emits a distinct terminal-
/// Psi crash terminator; it never reuses ordinary return lowering.
pub fn lower_machine(
    checked: &CheckedTrees,
    machine_name: &str,
) -> Result<LoweredTerminalPsi, LoweringError> {
    let mut matches = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .filter(|machine| machine.name == machine_name);
    let selection = matches
        .next()
        .ok_or_else(|| LoweringError::MachineNotFound(machine_name.to_owned()))?;
    if matches.next().is_some() {
        return Err(LoweringError::AmbiguousMachineName(machine_name.to_owned()));
    }
    let mut lowered = lower_selected_machine(checked, selection)?;
    let evidence_term_ids = lower_evidence_term_ids(checked, selection.machine)?;
    let (declarations, applications, declaration_ids) =
        lower_proposition_vocabulary(checked, &evidence_term_ids.term_ids)?;
    let evidence_terms = lower_evidence_terms(
        checked,
        selection.machine,
        &declaration_ids,
        &applications,
        evidence_term_ids.term_ids,
    )?;
    let evidence_contract_lanes = lower_evidence_contract_lanes(
        checked,
        selection.machine,
        lowered.semantic_module.entry,
        &evidence_terms.term_ids,
    )?;
    let evidence_package_invocations = lower_evidence_package_invocations(
        checked,
        selection.machine,
        lowered.semantic_module.entry,
        &lowered.semantic_module,
        &evidence_terms.term_ids,
    )?;
    lowered.proof_bundle.evidence_producers =
        lower_evidence_producer_provenance(checked, selection.machine, &evidence_terms.term_ids)?;
    lowered.semantic_module.proposition_declarations = declarations;
    lowered.semantic_module.proposition_applications = applications;
    lowered.semantic_module.evidence_terms = evidence_terms.declarations;
    lowered.semantic_module.evidence_contract_lanes = evidence_contract_lanes;
    lowered.semantic_module.evidence_package_invocations = evidence_package_invocations;
    psi_terminal_verifier::validate_module(&lowered.semantic_module)
        .map_err(LoweringError::InvalidTerminalModule)?;
    lowered.debug_map = if selection.signature == CheckedTerminalSignatureEligibility::Eligible {
        checked
            .facts
            .flow
            .terminal_debug
            .for_machine(selection.machine)
            .map(|plan| build_debug_map(plan, &lowered.semantic_module))
            .transpose()?
    } else {
        None
    };
    Ok(lowered)
}

fn lower_proposition_vocabulary(
    checked: &CheckedTrees,
    term_ids: &[Option<EvidenceTermId>],
) -> Result<
    (
        Vec<PropositionDeclaration>,
        Vec<PropositionApplicationIdentity>,
        Vec<(psi_symbols::SymbolHandle, PropositionId)>,
    ),
    LoweringError,
> {
    let placeholder = proposition_id(1);
    let mut declarations = checked
        .facts
        .proof
        .proposition_vocabulary
        .declarations
        .iter()
        .map(|declaration| {
            let evidence = match &declaration.evidence {
                CheckedPropositionEvidence::FactOnly => PropositionEvidence::FactOnly,
                CheckedPropositionEvidence::Witness { evidence_type } => {
                    PropositionEvidence::Witness {
                        evidence_type: evidence_type.clone(),
                    }
                }
            };
            let binders = declaration
                .binders
                .iter()
                .map(|binder| PropositionBinderDeclaration {
                    name: binder.name.clone(),
                    kind: match &binder.kind {
                        CheckedPropositionBinderKind::Type => PropositionBinderKind::Type,
                        CheckedPropositionBinderKind::Const { type_identity } => {
                            PropositionBinderKind::Const {
                                type_identity: type_identity.clone(),
                            }
                        }
                        CheckedPropositionBinderKind::Machine => PropositionBinderKind::Machine,
                    },
                })
                .collect();
            (
                declaration.symbol,
                PropositionDeclaration {
                    id: placeholder,
                    name: declaration.name.clone(),
                    binders,
                    parameter_types: declaration.parameter_types.clone(),
                    evidence,
                },
            )
        })
        .collect::<Vec<_>>();
    declarations.sort_by(|left, right| left.1.cmp(&right.1));
    for (index, (_, declaration)) in declarations.iter_mut().enumerate() {
        declaration.id = proposition_id(
            u64::try_from(index)
                .expect("proposition declaration count fits u64")
                .checked_add(1)
                .expect("one-based proposition identity fits u64"),
        );
    }
    let declaration_ids = declarations
        .iter()
        .map(|(symbol, declaration)| (*symbol, declaration.id))
        .collect::<Vec<_>>();

    let mut applications = Vec::new();
    for application in &checked.facts.proof.proposition_vocabulary.applications {
        let Some(declaration) = declaration_ids
            .iter()
            .find_map(|(symbol, id)| (*symbol == application.declaration).then_some(*id))
        else {
            continue;
        };
        let mut binder_arguments = Vec::new();
        let mut belongs_to_selected_machine = true;
        for argument in &application.binder_arguments {
            let evidence_projection = if let Some(projection) = &argument.evidence_projection {
                let index = usize::try_from(projection.term.arena_index() - 1)
                    .expect("arena indices fit the host address space");
                let Some(term) = term_ids.get(index).copied().flatten() else {
                    belongs_to_selected_machine = false;
                    break;
                };
                Some(EvidenceProjectionIdentity {
                    term,
                    declaring_trait_identity: checked
                        .symbols
                        .display_path(projection.declaring_trait, "::"),
                    declaring_trait_arguments: projection.declaring_trait_arguments.clone(),
                    requirement_identity: checked_evidence_requirement_identity(
                        checked,
                        projection.declaring_trait,
                        projection.requirement,
                    )?,
                })
            } else {
                None
            };
            binder_arguments.push(PropositionBinderArgumentIdentity {
                kind: match argument.kind {
                    CheckedPropositionBinderArgumentKind::Type => {
                        PropositionBinderArgumentKind::Type
                    }
                    CheckedPropositionBinderArgumentKind::Const => {
                        PropositionBinderArgumentKind::Const
                    }
                    CheckedPropositionBinderArgumentKind::Machine => {
                        PropositionBinderArgumentKind::Machine
                    }
                },
                identity: argument.identity.clone(),
                evidence_projection,
            });
        }
        if !belongs_to_selected_machine {
            continue;
        }
        applications.push(PropositionApplicationIdentity {
            id: placeholder,
            declaration,
            binder_arguments,
            arguments: application.arguments.clone(),
            evidence_interface: application
                .evidence_interface
                .as_ref()
                .map(|interface| lower_evidence_interface(checked, interface))
                .transpose()?,
        });
    }
    applications.sort();
    applications.dedup();
    for (index, application) in applications.iter_mut().enumerate() {
        application.id = proposition_id(
            u64::try_from(index)
                .expect("proposition application count fits u64")
                .checked_add(1)
                .expect("one-based proposition identity fits u64"),
        );
    }
    Ok((
        declarations
            .into_iter()
            .map(|(_, declaration)| declaration)
            .collect(),
        applications,
        declaration_ids,
    ))
}

/// Retain one terminal identity per distinct checked evidence term. Direct
/// forwarding aliases its output to the exact source term and therefore does
/// not mint a second identity. A selected producer keeps its output identity
/// distinct; its conformance provenance is lowered into the proof bundle.
struct LoweredEvidenceTerms {
    declarations: Vec<EvidenceTermDeclaration>,
    term_ids: Vec<Option<EvidenceTermId>>,
}

struct LoweredEvidenceTermIds {
    term_ids: Vec<Option<EvidenceTermId>>,
}

fn lower_evidence_term_ids(
    checked: &CheckedTrees,
    selected_machine: psi_symbols::SymbolHandle,
) -> Result<LoweredEvidenceTermIds, LoweringError> {
    let mut parents = (0..checked.facts.proof.evidence_terms.len()).collect::<Vec<_>>();
    for (_, forwarding) in checked.facts.proof.evidence_forwardings.iter() {
        if forwarding.machine_symbol != selected_machine {
            continue;
        }
        if let psi_checked_trees::EvidenceAssignmentSource::Forwarded { term: source } =
            &forwarding.source
        {
            let output = usize::try_from(forwarding.output.arena_index() - 1)
                .expect("arena indices fit the host address space");
            let source = usize::try_from(source.arena_index() - 1)
                .expect("arena indices fit the host address space");
            let output_root = evidence_term_root(&mut parents, output);
            let source_root = evidence_term_root(&mut parents, source);
            parents[output_root] = source_root;
        }
    }

    let mut roots = BTreeMap::<usize, (u8, usize)>::new();
    for (handle, term) in checked.facts.proof.evidence_terms.iter() {
        if term.owner
            != (psi_checked_trees::ContractProofFactOwner::Machine {
                machine_symbol: selected_machine,
            })
        {
            continue;
        }
        let index = usize::try_from(handle.arena_index() - 1)
            .expect("arena indices fit the host address space");
        let root = evidence_term_root(&mut parents, index);
        let lane_key = match term.kind {
            psi_checked_trees::ContractProofFactKind::Requires => (0_u8, term.lane_position),
            psi_checked_trees::ContractProofFactKind::Ensures => (1_u8, term.lane_position),
            _ => {
                return Err(LoweringError::Unsupported(
                    "terminal evidence term is not a named requires/ensures lane",
                ));
            }
        };
        roots
            .entry(root)
            .and_modify(|previous| *previous = (*previous).min(lane_key))
            .or_insert(lane_key);
    }
    let mut package_identity_position = 0_usize;
    for (_, invocation) in checked
        .facts
        .proof
        .evidence_package_invocations
        .iter()
        .filter(|(_, invocation)| invocation.caller_machine_symbol == selected_machine)
    {
        for output in &invocation.outputs {
            for handle in [output.callee_output, output.output] {
                let index = usize::try_from(handle.arena_index() - 1)
                    .expect("arena indices fit the host address space");
                let root = evidence_term_root(&mut parents, index);
                roots
                    .entry(root)
                    .or_insert((2_u8, package_identity_position));
                package_identity_position = package_identity_position
                    .checked_add(1)
                    .expect("evidence package identity order fits usize");
            }
        }
    }
    let mut roots = roots
        .into_iter()
        .map(|(root, lane_key)| (lane_key, root))
        .collect::<Vec<_>>();
    roots.sort_unstable();
    let root_ids = roots
        .into_iter()
        .enumerate()
        .map(|(index, (_, root))| {
            let id = EvidenceTermId::new(
                u64::try_from(index)
                    .expect("evidence term count fits u64")
                    .checked_add(1)
                    .expect("one-based evidence term identity fits u64"),
            )
            .expect("one-based evidence term identity is nonzero");
            (root, id)
        })
        .collect::<BTreeMap<_, _>>();
    let mut term_ids = vec![None; parents.len()];
    for (handle, _) in checked.facts.proof.evidence_terms.iter() {
        let index = usize::try_from(handle.arena_index() - 1)
            .expect("arena indices fit the host address space");
        let root = evidence_term_root(&mut parents, index);
        term_ids[index] = root_ids.get(&root).copied();
    }
    Ok(LoweredEvidenceTermIds { term_ids })
}

fn lower_evidence_terms(
    checked: &CheckedTrees,
    _selected_machine: psi_symbols::SymbolHandle,
    declaration_ids: &[(psi_symbols::SymbolHandle, PropositionId)],
    applications: &[PropositionApplicationIdentity],
    term_ids: Vec<Option<EvidenceTermId>>,
) -> Result<LoweredEvidenceTerms, LoweringError> {
    let mut identities_by_id =
        BTreeMap::<EvidenceTermId, (PropositionId, EvidenceInterfaceIdentity)>::new();
    for (handle, term) in checked.facts.proof.evidence_terms.iter() {
        let index = usize::try_from(handle.arena_index() - 1)
            .expect("arena indices fit the host address space");
        if term_ids.get(index).copied().flatten().is_none() {
            continue;
        }
        let id = term_ids[index].ok_or(LoweringError::Unsupported(
            "selected terminal evidence term has no canonical identity",
        ))?;
        let declaration = declaration_ids
            .iter()
            .find_map(|(symbol, id)| (*symbol == term.proposition.declaration).then_some(*id))
            .ok_or(LoweringError::Unsupported(
                "checked evidence term has no terminal proposition declaration",
            ))?;
        let binder_arguments = term
            .proposition
            .binder_arguments
            .iter()
            .map(|argument| {
                let evidence_projection = argument
                    .evidence_projection
                    .as_ref()
                    .map(|projection| {
                        let projection_index = usize::try_from(projection.term.arena_index() - 1)
                            .expect("arena indices fit the host address space");
                        Ok(EvidenceProjectionIdentity {
                            term: term_ids.get(projection_index).copied().flatten().ok_or(
                                LoweringError::Unsupported(
                                    "evidence-term proposition projects an unrelated term",
                                ),
                            )?,
                            declaring_trait_identity: checked
                                .symbols
                                .display_path(projection.declaring_trait, "::"),
                            declaring_trait_arguments: projection.declaring_trait_arguments.clone(),
                            requirement_identity: checked_evidence_requirement_identity(
                                checked,
                                projection.declaring_trait,
                                projection.requirement,
                            )?,
                        })
                    })
                    .transpose()?;
                Ok(PropositionBinderArgumentIdentity {
                    kind: match argument.kind {
                        CheckedPropositionBinderArgumentKind::Type => {
                            PropositionBinderArgumentKind::Type
                        }
                        CheckedPropositionBinderArgumentKind::Const => {
                            PropositionBinderArgumentKind::Const
                        }
                        CheckedPropositionBinderArgumentKind::Machine => {
                            PropositionBinderArgumentKind::Machine
                        }
                    },
                    identity: argument.identity.clone(),
                    evidence_projection,
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let proposition = applications
            .iter()
            .find(|application| {
                application.declaration == declaration
                    && application.binder_arguments == binder_arguments
                    && application.arguments == term.proposition.arguments
            })
            .map(|application| application.id)
            .ok_or(LoweringError::Unsupported(
                "checked evidence term has no terminal proposition application",
            ))?;
        let checked_interface =
            term.evidence_interface
                .as_ref()
                .ok_or(LoweringError::Unsupported(
                    "terminal evidence term has an unresolved carrierless interface",
                ))?;
        let interface = lower_evidence_interface(checked, checked_interface)?;
        if let Some((previous_proposition, previous_interface)) = identities_by_id.get(&id) {
            if *previous_proposition != proposition || *previous_interface != interface {
                return Err(LoweringError::Unsupported(
                    "forwarded evidence terms disagree on exact terminal identity",
                ));
            }
        } else {
            identities_by_id.insert(id, (proposition, interface));
        }
    }
    let declarations = identities_by_id
        .into_iter()
        .map(|(id, (proposition, interface))| EvidenceTermDeclaration {
            id,
            proposition,
            interface,
        })
        .collect();
    Ok(LoweredEvidenceTerms {
        declarations,
        term_ids,
    })
}

fn lower_evidence_interface(
    checked: &CheckedTrees,
    interface: &psi_checked_trees::CheckedEvidenceInterfaceIdentity,
) -> Result<EvidenceInterfaceIdentity, LoweringError> {
    let mut requirements = interface
        .requirements
        .iter()
        .map(|requirement| {
            Ok(EvidenceRequirementIdentity {
                declaring_trait_identity: checked
                    .symbols
                    .display_path(requirement.declaring_trait, "::"),
                declaring_trait_arguments: requirement.declaring_trait_arguments.clone(),
                requirement_identity: checked_evidence_requirement_identity(
                    checked,
                    requirement.declaring_trait,
                    requirement.requirement,
                )?,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    requirements.sort();
    requirements.dedup();
    Ok(EvidenceInterfaceIdentity {
        trait_identity: checked.symbols.display_path(interface.trait_symbol, "::"),
        arguments: interface.arguments.iter().cloned().collect(),
        requirements,
    })
}

fn lower_evidence_contract_lanes(
    checked: &CheckedTrees,
    selected_machine: psi_symbols::SymbolHandle,
    terminal_machine: MachineId,
    term_ids: &[Option<EvidenceTermId>],
) -> Result<Vec<EvidenceContractLane>, LoweringError> {
    let mut lanes = checked
        .facts
        .proof
        .evidence_terms
        .iter()
        .filter_map(|(handle, term)| {
            (term.owner
                == psi_checked_trees::ContractProofFactOwner::Machine {
                    machine_symbol: selected_machine,
                })
            .then_some((handle, term))
        })
        .map(|(handle, term)| {
            let index = usize::try_from(handle.arena_index() - 1)
                .expect("arena indices fit the host address space");
            let term_id =
                term_ids
                    .get(index)
                    .copied()
                    .flatten()
                    .ok_or(LoweringError::Unsupported(
                        "selected terminal contract lane has no evidence-term identity",
                    ))?;
            let kind = match term.kind {
                psi_checked_trees::ContractProofFactKind::Requires => {
                    EvidenceContractLaneKind::Requires
                }
                psi_checked_trees::ContractProofFactKind::Ensures => {
                    EvidenceContractLaneKind::Ensures
                }
                _ => {
                    return Err(LoweringError::Unsupported(
                        "terminal evidence term is not a named requires/ensures lane",
                    ));
                }
            };
            Ok(EvidenceContractLane {
                machine: terminal_machine,
                kind,
                position: u32::try_from(term.lane_position).map_err(|_| {
                    LoweringError::Unsupported(
                        "terminal evidence contract lane position exceeds u32",
                    )
                })?,
                term: term_id,
                output_field: (kind == EvidenceContractLaneKind::Ensures)
                    .then(|| term.name.clone()),
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    lanes.sort_unstable();
    Ok(lanes)
}

fn lower_evidence_package_invocations(
    checked: &CheckedTrees,
    selected_machine: psi_symbols::SymbolHandle,
    terminal_machine: MachineId,
    semantic_module: &TerminalModule,
    term_ids: &[Option<EvidenceTermId>],
) -> Result<Vec<EvidencePackageInvocation>, LoweringError> {
    let mut invocations = checked
        .facts
        .proof
        .evidence_package_invocations
        .iter()
        .filter_map(|(_, invocation)| {
            (invocation.caller_machine_symbol == selected_machine).then_some(invocation)
        })
        .enumerate()
        .map(|(ordinal, invocation)| {
            let (runtime_value, runtime_call) = lower_evidence_package_runtime_call(
                checked,
                selected_machine,
                terminal_machine,
                semantic_module,
                invocation,
            )?;
            let outputs = invocation
                .outputs
                .iter()
                .map(|output| {
                    let callee_output =
                        checked.facts.proof.evidence_terms.get(output.callee_output);
                    Ok(EvidencePackageOutputBinding {
                        output_position: u32::try_from(output.output_position).map_err(|_| {
                            LoweringError::Unsupported(
                                "terminal evidence package output position exceeds u32",
                            )
                        })?,
                        output_field: callee_output.name.clone(),
                        callee_output: term_ids
                            .get(
                                usize::try_from(output.callee_output.arena_index() - 1)
                                    .expect("arena indices fit the host address space"),
                            )
                            .copied()
                            .flatten()
                            .ok_or(LoweringError::Unsupported(
                                "terminal evidence package callee term has no canonical identity",
                            ))?,
                        output: term_ids
                            .get(
                                usize::try_from(output.output.arena_index() - 1)
                                    .expect("arena indices fit the host address space"),
                            )
                            .copied()
                            .flatten()
                            .ok_or(LoweringError::Unsupported(
                                "terminal evidence package output term has no canonical identity",
                            ))?,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            Ok(EvidencePackageInvocation {
                caller: terminal_machine,
                ordinal: u32::try_from(ordinal).map_err(|_| {
                    LoweringError::Unsupported(
                        "terminal evidence package invocation count exceeds u32",
                    )
                })?,
                target_machine_identity: checked_evidence_machine_identity(
                    checked,
                    invocation.target_machine_symbol,
                )?,
                runtime_value,
                runtime_call,
                outputs,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    invocations.sort_unstable();
    Ok(invocations)
}

fn lower_evidence_package_runtime_call(
    checked: &CheckedTrees,
    selected_machine: psi_symbols::SymbolHandle,
    terminal_machine: MachineId,
    semantic_module: &TerminalModule,
    invocation: &psi_checked_trees::EvidencePackageInvocationFact,
) -> Result<(Option<ScalarType>, Option<EvidencePackageRuntimeCall>), LoweringError> {
    let Some(runtime_call) = invocation.runtime_call else {
        return Ok((None, None));
    };
    let target_state = checked
        .typed
        .machines()
        .iter()
        .flat_map(|machine| checked.typed.machine_states(machine))
        .find(|state| state.symbol == invocation.target_state_symbol)
        .ok_or(LoweringError::Unsupported(
            "runtime evidence package target state is absent",
        ))?;
    let runtime_value = checked
        .typed
        .primitive_type_reference(target_state.return_type)
        .map(terminal_scalar_type)
        .transpose()?
        .ok_or(LoweringError::Unsupported(
            "runtime evidence package target is not scalar-result",
        ))?;
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(selected_machine)
        .ok_or(LoweringError::Unsupported(
            "runtime evidence package caller has no checked scalar graph",
        ))?;
    let mut direct_call_position = None;
    let mut next_position = 0usize;
    for state in &graph.states {
        for binding in &state.bindings {
            let psi_checked_trees::CheckedScalarBindingValue::DirectCall {
                target_machine,
                target_state,
                call_ordinal,
                ..
            } = &binding.value
            else {
                continue;
            };
            if state.state == invocation.caller_state_symbol
                && usize::try_from(binding.statement_ordinal).ok()
                    == Some(runtime_call.statement_index)
                && usize::try_from(*call_ordinal).ok() == Some(runtime_call.call_ordinal)
            {
                if *target_machine != invocation.target_machine_symbol
                    || *target_state != invocation.target_state_symbol
                {
                    return unsupported(
                        "runtime evidence package target disagrees with its scalar call plan",
                    );
                }
                if direct_call_position.replace(next_position).is_some() {
                    return unsupported(
                        "runtime evidence package scalar call coordinate is not unique",
                    );
                }
            }
            next_position = next_position
                .checked_add(1)
                .expect("checked scalar direct-call count advances");
        }
    }
    let direct_call_position = direct_call_position.ok_or(LoweringError::Unsupported(
        "runtime evidence package scalar call coordinate is absent",
    ))?;
    let caller = semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == terminal_machine)
        .ok_or(LoweringError::Unsupported(
            "runtime evidence package caller is absent from terminal Psi",
        ))?;
    let mut calls = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind, OperationKind::Call { .. }))
        .collect::<Vec<_>>();
    calls.sort_unstable_by_key(|operation| operation.id);
    let operation = calls
        .get(direct_call_position)
        .copied()
        .ok_or(LoweringError::Unsupported(
            "runtime evidence package call has no emitted terminal operation",
        ))?;
    let (Some(result), OperationKind::Call { callee, .. }) =
        (operation.result.scalar(), &operation.kind)
    else {
        return unsupported("runtime evidence package operation is not an ordinary scalar call");
    };
    if result.scalar_type != runtime_value {
        return unsupported("runtime evidence package operation result type disagrees");
    }
    Ok((
        Some(runtime_value),
        Some(EvidencePackageRuntimeCall {
            operation: operation.id,
            callee: *callee,
        }),
    ))
}

fn lower_evidence_producer_provenance(
    checked: &CheckedTrees,
    selected_machine: psi_symbols::SymbolHandle,
    term_ids: &[Option<EvidenceTermId>],
) -> Result<Vec<EvidenceProducerProvenance>, LoweringError> {
    let package_callees = checked
        .facts
        .proof
        .evidence_package_invocations
        .iter()
        .filter_map(|(_, invocation)| {
            (invocation.caller_machine_symbol == selected_machine)
                .then_some(invocation.target_machine_symbol)
        })
        .collect::<Vec<_>>();
    let mut producers =
        checked
            .facts
            .proof
            .evidence_forwardings
            .iter()
            .filter_map(|(_, forwarding)| {
                if forwarding.machine_symbol != selected_machine
                    && !package_callees.contains(&forwarding.machine_symbol)
                {
                    return None;
                }
                let psi_checked_trees::EvidenceAssignmentSource::ProducerConformance {
                    conformance,
                    evidence_trait,
                    rows,
                } = &forwarding.source
                else {
                    return None;
                };
                let output_index = usize::try_from(forwarding.output.arena_index() - 1)
                    .expect("arena indices fit the host address space");
                Some((
                    term_ids.get(output_index).copied().flatten().ok_or(
                        LoweringError::Unsupported(
                            "selected evidence producer has no terminal term identity",
                        ),
                    ),
                    forwarding.output,
                    *conformance,
                    *evidence_trait,
                    rows,
                ))
            })
            .map(|(term, output, conformance, evidence_trait, rows)| {
                let interface = checked
                    .facts
                    .proof
                    .evidence_terms
                    .get(output)
                    .evidence_interface
                    .as_ref()
                    .ok_or(LoweringError::Unsupported(
                        "selected evidence producer has an unresolved interface",
                    ))?;
                let mut lowered_rows = rows
                    .iter()
                    .map(|row| {
                        let mut requirement_rows = interface.requirements.iter().filter(|entry| {
                            entry.declaring_trait == row.declaring_trait
                                && entry.requirement == row.requirement
                        });
                        let requirement_row = requirement_rows.next().ok_or(
                            LoweringError::Unsupported(
                                "selected evidence producer row is absent from its interface",
                            ),
                        )?;
                        if requirement_rows.next().is_some() {
                            return unsupported(
                                "selected evidence producer row has ambiguous instantiated interface arguments",
                            );
                        }
                        Ok(EvidenceProducerRealization {
                            declaring_trait_identity: checked
                                .symbols
                                .display_path(row.declaring_trait, "::"),
                            declaring_trait_arguments: requirement_row
                                .declaring_trait_arguments
                                .clone(),
                            requirement_identity: checked_evidence_requirement_identity(
                                checked,
                                row.declaring_trait,
                                row.requirement,
                            )?,
                            realization_machine_identity: checked_evidence_machine_identity(
                                checked,
                                row.realization_machine,
                            )?,
                            realization_state_identity: checked
                                .symbols
                                .display_path(row.realization_state, "::"),
                            source: match row.source {
                                psi_checked_trees::DynamicConformanceRowSource::Inline => {
                                    EvidenceProducerRowSource::Inline
                                }
                                psi_checked_trees::DynamicConformanceRowSource::Reference => {
                                    EvidenceProducerRowSource::Reference
                                }
                                psi_checked_trees::DynamicConformanceRowSource::TraitDefault => {
                                    EvidenceProducerRowSource::TraitDefault
                                }
                            },
                        })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                lowered_rows.sort();
                Ok(EvidenceProducerProvenance {
                    id: EvidenceIdentity::new(1).expect("placeholder identity is nonzero"),
                    term: term?,
                    conformance_identity: checked.symbols.display_path(conformance, "::"),
                    evidence_trait_identity: checked.symbols.display_path(evidence_trait, "::"),
                    rows: lowered_rows,
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
    producers.sort_by_key(|producer| producer.term);
    for (index, producer) in producers.iter_mut().enumerate() {
        producer.id = EvidenceIdentity::new(
            u64::try_from(index)
                .expect("evidence producer count fits u64")
                .checked_add(1)
                .expect("one-based evidence producer identity fits u64"),
        )
        .expect("one-based evidence producer identity is nonzero");
    }
    Ok(producers)
}

fn checked_evidence_requirement_identity(
    checked: &CheckedTrees,
    declaring_trait: psi_symbols::SymbolHandle,
    requirement: psi_symbols::SymbolHandle,
) -> Result<String, LoweringError> {
    let mut matches = checked
        .typed
        .traits()
        .iter()
        .filter(|definition| definition.symbol == declaring_trait)
        .flat_map(|definition| {
            checked
                .typed
                .trait_machine_signatures(definition)
                .iter()
                .filter(move |signature| signature.symbol == requirement)
                .map(move |signature| (definition, signature))
        });
    let (definition, signature) = matches.next().ok_or(LoweringError::Unsupported(
        "evidence producer row has no exact trait requirement",
    ))?;
    if matches.next().is_some() {
        return unsupported("evidence producer row has an ambiguous trait requirement");
    }
    let identity = checked
        .typed
        .normalized_trait_requirement_overload_identity(definition, signature)
        .identity();
    if identity.is_empty() {
        return unsupported("evidence producer row has an empty requirement identity");
    }
    Ok(identity)
}

fn checked_evidence_machine_identity(
    checked: &CheckedTrees,
    machine: psi_symbols::SymbolHandle,
) -> Result<String, LoweringError> {
    let mut matches = checked
        .typed
        .machines()
        .iter()
        .filter(|candidate| candidate.symbol == machine);
    let machine = matches.next().ok_or(LoweringError::Unsupported(
        "evidence producer row has no exact realization machine",
    ))?;
    if matches.next().is_some() {
        return unsupported("evidence producer row has an ambiguous realization machine");
    }
    let identity = checked
        .typed
        .normalized_machine_overload_identity(machine)
        .ok_or(LoweringError::Unsupported(
            "evidence producer realization has no callable identity",
        ))?
        .identity();
    if identity.is_empty() {
        return unsupported("evidence producer realization has an empty machine identity");
    }
    Ok(identity)
}

fn evidence_term_root(parents: &mut [usize], mut index: usize) -> usize {
    while parents[index] != index {
        let parent = parents[index];
        parents[index] = parents[parent];
        index = parents[index];
    }
    index
}

fn lower_selected_machine(
    checked: &CheckedTrees,
    selection: &CheckedTerminalMachineSelection,
) -> Result<LoweredTerminalPsi, LoweringError> {
    // A result-bearing structural plan owns both the scalar result and its
    // post-result cleanup.  It must win over the overlapping Unit-only
    // nominal-cleanup eligibility for the same attached machine.
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(selection.machine)
    {
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("structural scalar return plan requires an attached signature");
        }
        return lower_structural_scalar_return_machine(checked, plan);
    }
    let mut nominal_matches = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .machines
        .iter()
        .filter(|plan| plan.machine.machine == selection.machine);
    if let Some(plan) = nominal_matches.next() {
        if nominal_matches.next().is_some() {
            return unsupported("nominal affine Unit cleanup plan is duplicated");
        }
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("nominal affine Unit cleanup requires an attached signature");
        }
        return lower_nominal_affine_unit_cleanup_machine(checked, plan);
    }
    let mut partial_matches = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .machines
        .iter()
        .filter(|plan| plan.machine.machine == selection.machine);
    if let Some(plan) = partial_matches.next() {
        if partial_matches.next().is_some() {
            return unsupported("partial affine Unit cleanup plan is duplicated");
        }
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("partial affine Unit cleanup requires an attached signature");
        }
        return lower_partial_affine_unit_cleanup_machine(checked, plan);
    }
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_boundary_scalar_returns
        .for_machine(selection.machine)
    {
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("result-bearing boundary custody requires an attached signature");
        }
        return lower_boundary_scalar_return_machine(checked, plan);
    }
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_structural_returns
        .for_machine(selection.machine)
    {
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("structural result transfer requires an attached signature");
        }
        return lower_structural_return_machine(checked, plan);
    }
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_structural_unit_controls
        .for_machine(selection.machine)
    {
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("structural Unit control plan requires an attached signature");
        }
        return lower_structural_unit_control_machine(checked, plan);
    }
    match selection.signature {
        CheckedTerminalSignatureEligibility::Eligible => {}
        CheckedTerminalSignatureEligibility::Attached => {
            return lower_attached_unit_closure(checked, selection.machine);
        }
        CheckedTerminalSignatureEligibility::Unsupported => {
            return unsupported(
                "machine signature is outside the current terminal-Psi source slice",
            );
        }
    }

    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(selection.machine)
        .ok_or(LoweringError::Unsupported(
            "machine has no source-independent checked scalar control plan",
        ))?;
    let closure = checked_scalar_call_closure(checked, selection.machine)?;
    if closure.len() == 1 {
        lower_scalar_graph_machine(checked, selection.machine, graph)
    } else {
        lower_scalar_call_closure(checked, &closure)
    }
}

fn lower_boundary_scalar_return_machine(
    checked: &CheckedTrees,
    plan: &CheckedBoundaryScalarReturnMachinePlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    let plans = &checked.facts.flow.terminal_boundary_scalar_returns;
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        coordinate,
        target_machine,
        target_state,
        target_contract_fingerprint,
        service_reach,
        structural_arguments,
        completion_receipts,
    } = &plan.boundary_call
    else {
        return unsupported("result-bearing boundary plan does not contain a boundary call");
    };
    if coordinate.statement_index != 0
        || coordinate.call_ordinal != 0
        || plan.return_statement_ordinal != 1
    {
        return unsupported("result-bearing boundary call coordinates are not canonical");
    }
    let mut matches = plans
        .boundary_machines
        .iter()
        .filter(|boundary| boundary.machine == *target_machine);
    let boundary = matches.next().ok_or(LoweringError::Unsupported(
        "result-bearing boundary target is absent from its checked plan",
    ))?;
    if matches.next().is_some()
        || boundary.state != *target_state
        || boundary.contract_fingerprint != *target_contract_fingerprint
        || boundary.result_type != Some(plan.result_type)
        || !checked_unit_target_reach_matches(*service_reach, boundary.contract_service_reach)
    {
        return unsupported("result-bearing boundary call disagrees with its exact checked target");
    }

    let (structural_types, type_ids) = lower_structural_type_plans(&plans.structural_types)?;
    let (structural_domains, domain_ids) =
        lower_boundary_scalar_domains(plans, plan, boundary, &type_ids)?;
    let (services, service_ids) =
        lower_boundary_scalar_services(checked, plan, boundary, *service_reach)?;
    let mut next_place = 1_u64;
    let parameters = lower_unit_parameters(
        &plan.structural_parameters,
        &type_ids,
        &domain_ids,
        &mut next_place,
    )?;
    let boundary_parameters = lower_unit_parameters(
        &boundary.structural_parameters,
        &type_ids,
        &domain_ids,
        &mut next_place,
    )?;
    let mut requires = boundary
        .domain_requirements
        .iter()
        .map(|requirement| {
            Ok(StructuralDomainRequirement {
                argument_index: requirement.argument_index,
                domain: lookup_domain_id(&domain_ids, requirement.domain)?,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    requires.sort();
    requires.dedup();
    let boundary_id = boundary_machine_id(1);
    let boundary_declaration = BoundaryMachineDeclaration {
        id: boundary_id,
        identity: checked_unit_boundary_identity(checked, boundary.machine)?,
        attachment: boundary
            .attachment_type_identity
            .as_ref()
            .map(|identity| lookup_type_id(&type_ids, identity))
            .transpose()?,
        structural_parameters: boundary_parameters,
        result: Some(terminal_scalar_type(plan.result_type)?),
        requires,
        published_service_ceiling: lower_published_service_ceiling(
            &checked.facts.service_reaches.rows,
            boundary.contract_service_reach,
            boundary.service_reach,
            &service_ids,
        )?,
    };

    let mut next_claim = 1_u64;
    let mut entry_claims = Vec::with_capacity(plan.entry_claims.len());
    let mut claim_bindings = Vec::with_capacity(plan.entry_claims.len());
    for claim in &plan.entry_claims {
        if claim.carry != CarryPolicy::STRICT {
            return unsupported("result-bearing boundary entry claim has non-default carry");
        }
        let parameter = parameters
            .get(usize::try_from(claim.parameter_index).map_err(|_| {
                LoweringError::Unsupported("boundary entry claim parameter exceeds usize")
            })?)
            .ok_or(LoweringError::Unsupported(
                "result-bearing boundary entry claim has an invalid parameter",
            ))?;
        let PermissionClaimIdentity::Established {
            machine_symbol,
            state_symbol,
            source: psi_language_semantics::PermissionEventSource::StateEntry,
            ..
        } = claim.claim_identity
        else {
            return unsupported("result-bearing boundary entry claim is not exact");
        };
        if machine_symbol != plan.machine || state_symbol != plan.state {
            return unsupported("result-bearing boundary entry claim belongs to another state");
        }
        let id = claim_id(allocate_dense(&mut next_claim)?);
        entry_claims.push(EntryClaim {
            claim: id,
            input: parameter.place,
            path: lower_structural_path(&claim.path),
        });
        claim_bindings.push((claim.claim_identity, id));
    }
    let expected_claim_arguments = structural_arguments
        .iter()
        .enumerate()
        .flat_map(|(argument_index, argument)| {
            plan.entry_claims
                .iter()
                .filter(move |claim| {
                    claim.parameter_index == argument.source_parameter_index
                        && (argument.path.is_empty() || claim.path == argument.path)
                })
                .map(move |_| {
                    u32::try_from(argument_index).map_err(|_| {
                        LoweringError::Unsupported("boundary argument index exceeds u32")
                    })
                })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    validate_transfer_shape(
        structural_arguments,
        completion_receipts,
        &parameters,
        &boundary.structural_parameters,
        &type_ids,
        &expected_claim_arguments,
    )?;
    let scalar_type = terminal_scalar_type(plan.result_type)?;
    let call_result = ValueDeclaration {
        id: value_id(1),
        scalar_type,
    };
    let operation = Operation {
        id: operation_id(1),
        result: psi_terminal::OperationResult::Scalar(call_result),
        kind: OperationKind::BoundaryCall {
            boundary: boundary_id,
            structural_arguments: lower_structural_arguments(structural_arguments, &parameters)?,
            completion_receipts: completion_receipts
                .iter()
                .map(|receipt| {
                    Ok(CompletionReceipt {
                        claim: lookup_claim_id(&claim_bindings, receipt.claim_identity)?,
                        argument_index: receipt.argument_index,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?,
            requirement_obligations: Vec::new(),
        },
    };
    let machine_result = ValueDeclaration {
        id: value_id(2),
        scalar_type,
    };
    let machine = TerminalMachine {
        id: machine_id(1),
        attachment: Some(lookup_type_id(&type_ids, &plan.attachment_type_identity)?),
        parameters: Vec::new(),
        structural_parameters: parameters.clone(),
        result: TerminalMachineResult::Scalar(machine_result),
        structural_places: parameters
            .iter()
            .map(|parameter| StructuralPlaceDeclaration {
                id: parameter.place,
                kind: StructuralPlaceKind::Parameter {
                    position: parameter.position,
                    is_self: parameter.is_self,
                },
            })
            .collect(),
        entry_claims,
        published_service_ceiling: lower_published_service_ceiling(
            &checked.facts.service_reaches.rows,
            plan.contract_service_reach,
            plan.service_reach,
            &service_ids,
        )?,
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(1),
        blocks: vec![Block {
            id: block_id(1),
            parameters: Vec::new(),
            operations: vec![operation],
            terminator: Terminator::Return {
                edge: edge_id(1),
                value: call_result.id,
                cleanup_actions: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: contract_id(1),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
        },
    };
    let mut lowered = LoweredTerminalPsi {
        semantic_module: TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine.id,
            structural_types,
            structural_domains,
            services,
            boundary_machines: vec![boundary_declaration],
            provider_candidates: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            evidence_package_invocations: Vec::new(),
            machines: vec![machine],
        },
        proof_bundle: ProofBundle::default(),
        debug_map: None,
    };
    finalize_operation_proofs(&mut lowered)?;
    Ok(lowered)
}

fn lower_boundary_scalar_domains(
    plans: &psi_checked_trees::CheckedBoundaryScalarReturnPlans,
    machine: &CheckedBoundaryScalarReturnMachinePlan,
    boundary: &CheckedBoundaryMachinePlan,
    type_ids: &[(String, StructuralTypeId)],
) -> Result<
    (
        Vec<StructuralDomainDeclaration>,
        Vec<(SemanticDomainId, StructuralDomainId)>,
    ),
    LoweringError,
> {
    let mut selected = machine
        .structural_parameters
        .iter()
        .flat_map(|parameter| parameter.qualifications.iter().copied())
        .chain(
            boundary
                .structural_parameters
                .iter()
                .flat_map(|parameter| parameter.qualifications.iter().copied()),
        )
        .chain(
            boundary
                .domain_requirements
                .iter()
                .map(|requirement| requirement.domain),
        )
        .collect::<Vec<_>>();
    selected.sort_by_key(|domain| domain.0);
    selected.dedup();
    let mut selected_plans = selected
        .iter()
        .map(|domain| {
            plans
                .structural_domains
                .iter()
                .find(|plan| plan.domain == *domain)
                .ok_or(LoweringError::Unsupported(
                    "result-bearing boundary references a missing structural domain",
                ))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    selected_plans.sort_by(|left, right| left.identity.cmp(&right.identity));
    if selected_plans
        .windows(2)
        .any(|pair| pair[0].identity == pair[1].identity)
    {
        return unsupported("result-bearing boundary has duplicate structural domains");
    }
    let domain_ids = selected_plans
        .iter()
        .enumerate()
        .map(|(index, plan)| Ok((plan.domain, structural_domain_id(dense_identity(index)?))))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let declarations = selected_plans
        .into_iter()
        .map(|plan| {
            Ok(StructuralDomainDeclaration {
                id: lookup_domain_id(&domain_ids, plan.domain)?,
                identity: plan.identity.clone(),
                carrier: lookup_type_id(type_ids, &plan.carrier_type_identity)?,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    Ok((declarations, domain_ids))
}

fn lower_boundary_scalar_services(
    checked: &CheckedTrees,
    machine: &CheckedBoundaryScalarReturnMachinePlan,
    boundary: &CheckedBoundaryMachinePlan,
    call_reach: ServiceReachSummary,
) -> Result<(Vec<ServiceDeclaration>, Vec<(ServiceReachId, ServiceId)>), LoweringError> {
    let facts = &checked.facts.service_reaches;
    let mut selected = Vec::new();
    collect_contract_services(
        &facts.rows,
        machine.contract_service_reach,
        machine.service_reach,
        &mut selected,
    )?;
    collect_contract_services(
        &facts.rows,
        boundary.contract_service_reach,
        boundary.service_reach,
        &mut selected,
    )?;
    collect_service_summary(&facts.rows, call_reach, &mut selected)?;
    let mut next = 0;
    while let Some(service) = selected.get(next).copied() {
        next += 1;
        let definition = facts
            .services
            .definition(service)
            .ok_or(LoweringError::Unsupported(
                "result-bearing boundary references an unknown service",
            ))?;
        for parent in &definition.parents {
            if !selected.contains(parent) {
                selected.push(*parent);
            }
        }
    }
    let mut definitions = selected
        .into_iter()
        .map(|service| {
            facts
                .services
                .definition(service)
                .map(|definition| (service, definition))
                .ok_or(LoweringError::Unsupported(
                    "result-bearing boundary references an unknown service",
                ))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    definitions.sort_by(|left, right| left.1.name.cmp(&right.1.name));
    if definitions
        .windows(2)
        .any(|pair| pair[0].1.name == pair[1].1.name)
    {
        return unsupported("result-bearing boundary has duplicate service identities");
    }
    let service_ids = definitions
        .iter()
        .enumerate()
        .map(|(index, (source, _))| Ok((*source, service_id(dense_identity(index)?))))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let declarations = definitions
        .into_iter()
        .map(|(source, definition)| {
            let mut parents = definition
                .parents
                .iter()
                .map(|parent| lookup_service_id(&service_ids, *parent))
                .collect::<Result<Vec<_>, LoweringError>>()?;
            parents.sort();
            parents.dedup();
            Ok(ServiceDeclaration {
                id: lookup_service_id(&service_ids, source)?,
                identity: definition.name.clone(),
                parents,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    Ok((declarations, service_ids))
}

fn lower_structural_return_machine(
    checked: &CheckedTrees,
    plan: &CheckedStructuralReturnMachinePlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    let plans = &checked.facts.flow.terminal_structural_returns;
    let Some(returned_plan) = plan.structural_parameters.first() else {
        return unsupported(
            "structural result plan is not one exact whole-root linear transfer with affine cleanup",
        );
    };
    let discarded_plans = &plan.structural_parameters[1..];
    let expected_discards = (1..plan.structural_parameters.len())
        .rev()
        .map(|position| u32::try_from(position).ok())
        .collect::<Option<Vec<_>>>()
        .ok_or(LoweringError::Unsupported(
            "structural result cleanup position is not representable",
        ))?;
    let expected_local_discards = plan
        .trivial_affine_locals
        .iter()
        .rev()
        .map(|local| local.declaration_ordinal)
        .collect::<Vec<_>>();
    if plan.returned_parameter_index != 0
        || plan.trivial_affine_discards != expected_discards
        || plan.trivial_affine_local_discard_ordinals != expected_local_discards
        || plan
            .trivial_affine_locals
            .iter()
            .enumerate()
            .any(|(index, local)| {
                u32::try_from(index).ok() != Some(local.declaration_ordinal)
                    || local.type_identity.is_empty()
            })
        || returned_plan.multiplicity != Multiplicity::Linear
        || returned_plan.is_self
        || discarded_plans
            .iter()
            .any(|discarded| discarded.multiplicity != Multiplicity::Affine || discarded.is_self)
        || plan.result.multiplicity != Multiplicity::Linear
        || plan.entry_claim.parameter_index != 0
        || !plan.entry_claim.path.is_empty()
        || plan.entry_claim.carry != CarryPolicy::STRICT
        || plan.entry_claim.claim_identity != plan.transferred_claim
        || returned_plan.type_identity != plan.result.type_identity
        || returned_plan.qualifications != plan.result.qualifications
    {
        return unsupported(
            "structural result plan is not one exact whole-root linear transfer with affine cleanup",
        );
    }
    let PermissionClaimIdentity::Established {
        machine_symbol,
        state_symbol,
        source: psi_language_semantics::PermissionEventSource::StateEntry,
        ..
    } = plan.transferred_claim
    else {
        return unsupported("structural result claim is not an exact checked state-entry claim");
    };
    if machine_symbol != plan.machine || state_symbol != plan.state {
        return unsupported("structural result claim belongs to another checked state");
    }

    let (structural_types, type_ids) = lower_structural_type_plans(&plans.structural_types)?;
    let (structural_domains, domain_ids) =
        lower_structural_domain_plans(&plans.structural_domains, &type_ids)?;
    let mut next_place = 1_u64;
    let parameters = lower_unit_parameters(
        &plan.structural_parameters,
        &type_ids,
        &domain_ids,
        &mut next_place,
    )?;
    let input = parameters.first().ok_or(LoweringError::Unsupported(
        "structural result plan has no input",
    ))?;
    let discarded = &parameters[1..];
    let result_place = place_id(RESULT_STRUCTURAL_PLACE_ID);
    if input.place == result_place {
        return unsupported("structural result place collides with its input namespace");
    }
    let mut result_qualifications = plan
        .result
        .qualifications
        .iter()
        .map(|domain| lookup_domain_id(&domain_ids, *domain))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    result_qualifications.sort();
    result_qualifications.dedup();
    if result_qualifications.len() != plan.result.qualifications.len() {
        return unsupported("structural result repeats a qualification");
    }
    let local_places = plan
        .trivial_affine_locals
        .iter()
        .map(|local| {
            let place = place_id(allocate_dense(&mut next_place)?);
            let structural_type = lookup_type_id(&type_ids, &local.type_identity)?;
            let Some(declaration) = structural_types
                .iter()
                .find(|declaration| declaration.id == structural_type)
            else {
                return unsupported("trivial affine local has no structural type declaration");
            };
            let StructuralTypeShape::Record { fields } = &declaration.shape else {
                return unsupported("trivial affine local is not a record");
            };
            if !fields.is_empty() {
                return unsupported("trivial affine local is not an empty record");
            }
            Ok((local.declaration_ordinal, structural_type, place))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;

    let identity_facts = checked
        .facts
        .qualifications
        .content
        .identity_reshuffles
        .iter()
        .filter(|fact| fact.machine_symbol == plan.machine && fact.state_symbol == plan.state)
        .cloned()
        .collect::<Vec<_>>();
    let identity = lower_content_identity_reshuffles(&identity_facts)?;
    let [(source_claim, content_claim)] = identity.source_claims.as_slice() else {
        return unsupported("structural result requires one exact identity reshuffle claim");
    };
    let [content_entry] = identity.entry_claims.as_slice() else {
        return unsupported("structural result requires one content entry binding");
    };
    let [reshuffle] = identity.reshuffles.as_slice() else {
        return unsupported("structural result requires one content identity reshuffle");
    };
    let claim = claim_id(1);
    if *source_claim != plan.transferred_claim
        || *content_claim != claim
        || content_entry.claim != claim
        || reshuffle.claim != claim
        || content_entry.input.root != input.place
        || reshuffle.input.root != input.place
        || reshuffle.output.root != result_place
        || !content_entry.input.segments.is_empty()
        || !reshuffle.input.segments.is_empty()
        || !reshuffle.output.segments.is_empty()
    {
        return unsupported("structural result claim/content identities do not unify exactly");
    }
    let content_places = BTreeMap::from([
        (
            input.place,
            StructuralPlaceKind::Parameter {
                position: 0,
                is_self: input.is_self,
            },
        ),
        (result_place, StructuralPlaceKind::Result),
    ]);
    let actual_places = identity
        .structural_places
        .iter()
        .map(|place| (place.id, place.kind))
        .collect::<BTreeMap<_, _>>();
    if actual_places != content_places {
        return unsupported("structural result content roots do not match the checked signature");
    }
    let mut expected_places = content_places;
    for discarded in discarded {
        expected_places.insert(
            discarded.place,
            StructuralPlaceKind::Parameter {
                position: discarded.position,
                is_self: discarded.is_self,
            },
        );
    }
    for (declaration_ordinal, structural_type, place) in &local_places {
        expected_places.insert(
            *place,
            StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal: *declaration_ordinal,
                structural_type: *structural_type,
            },
        );
    }
    let input_place = input.place;
    let terminal_discards = local_places
        .iter()
        .rev()
        .map(|(_, _, place)| *place)
        .chain(discarded.iter().rev().map(|value| value.place))
        .collect();

    let terminal_machine = machine_id(1);
    let machine = TerminalMachine {
        id: terminal_machine,
        attachment: Some(lookup_type_id(&type_ids, &plan.attachment_type_identity)?),
        parameters: Vec::new(),
        structural_parameters: parameters,
        result: TerminalMachineResult::Structural(StructuralResultDeclaration {
            place: result_place,
            structural_type: lookup_type_id(&type_ids, &plan.result.type_identity)?,
            multiplicity: StructuralMultiplicity::Linear,
            qualifications: result_qualifications,
        }),
        structural_places: expected_places
            .into_iter()
            .map(|(id, kind)| StructuralPlaceDeclaration { id, kind })
            .collect(),
        entry_claims: vec![EntryClaim {
            claim,
            input: input_place,
            path: Vec::new(),
        }],
        published_service_ceiling: Vec::new(),
        content_entry_claims: identity.entry_claims,
        content_identity_reshuffles: identity.reshuffles,
        content_partition_compositions: Vec::new(),
        entry: block_id(1),
        blocks: vec![Block {
            id: block_id(1),
            parameters: Vec::new(),
            operations: local_places
                .iter()
                .enumerate()
                .map(|(index, (_, _, destination))| {
                    Ok(Operation {
                        id: operation_id(dense_identity(index)?),
                        result: psi_terminal::OperationResult::Unit,
                        kind: OperationKind::EstablishTrivialAffineLocal {
                            destination: *destination,
                        },
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?,
            terminator: Terminator::ReturnStructural {
                edge: edge_id(1),
                source: input_place,
                returned_claims: vec![claim],
                trivial_affine_discards: terminal_discards,
            },
        }],
        contract: MachineContract {
            id: contract_id(1),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
        },
    };
    Ok(LoweredTerminalPsi {
        semantic_module: TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: terminal_machine,
            structural_types,
            structural_domains,
            services: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            evidence_package_invocations: Vec::new(),
            machines: vec![machine],
        },
        proof_bundle: ProofBundle::default(),
        debug_map: None,
    })
}

fn lower_structural_domain_plans(
    plans: &[psi_checked_trees::CheckedUnitStructuralDomainPlan],
    type_ids: &[(String, StructuralTypeId)],
) -> Result<
    (
        Vec<StructuralDomainDeclaration>,
        Vec<(SemanticDomainId, StructuralDomainId)>,
    ),
    LoweringError,
> {
    let mut ordered = plans.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        (&left.identity, left.domain.0).cmp(&(&right.identity, right.domain.0))
    });
    if ordered.iter().any(|plan| {
        !plan.domain.is_valid() || plan.identity.is_empty() || plan.carrier_type_identity.is_empty()
    }) || ordered
        .windows(2)
        .any(|pair| pair[0].domain == pair[1].domain || pair[0].identity == pair[1].identity)
    {
        return unsupported("structural result domains are invalid or noncanonical");
    }
    let domain_ids = ordered
        .iter()
        .enumerate()
        .map(|(index, plan)| Ok((plan.domain, structural_domain_id(dense_identity(index)?))))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let declarations = ordered
        .into_iter()
        .map(|plan| {
            Ok(StructuralDomainDeclaration {
                id: lookup_domain_id(&domain_ids, plan.domain)?,
                identity: plan.identity.clone(),
                carrier: lookup_type_id(type_ids, &plan.carrier_type_identity)?,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    Ok((declarations, domain_ids))
}

fn retain_additional_structural_types(
    module: &mut TerminalModule,
    plans: &[CheckedUnitStructuralTypePlan],
    root_identities: impl IntoIterator<Item = String>,
) -> Result<(), LoweringError> {
    fn collect(
        plans: &[CheckedUnitStructuralTypePlan],
        identity: &str,
        active: &mut Vec<String>,
        selected: &mut Vec<String>,
    ) -> Result<(), LoweringError> {
        if active.iter().any(|candidate| candidate == identity) {
            return unsupported("recursive structural type is outside scalar cleanup lowering");
        }
        if selected.iter().any(|candidate| candidate == identity) {
            return Ok(());
        }
        let mut matches = plans.iter().filter(|plan| plan.identity == identity);
        let plan = matches.next().ok_or(LoweringError::Unsupported(
            "scalar cleanup references a missing structural type",
        ))?;
        if matches.next().is_some() || identity.is_empty() {
            return unsupported("scalar cleanup structural type identity is invalid");
        }
        active.push(identity.to_owned());
        match &plan.shape {
            CheckedUnitStructuralTypeShape::Record { fields } => {
                for field in fields {
                    if let CheckedUnitStructuralFieldType::Structural { type_identity } =
                        &field.field_type
                    {
                        collect(plans, type_identity, active, selected)?;
                    }
                }
            }
            CheckedUnitStructuralTypeShape::FixedArray {
                element_type_identity,
                ..
            } => collect(plans, element_type_identity, active, selected)?,
        }
        active.pop();
        selected.push(identity.to_owned());
        Ok(())
    }

    let mut selected = Vec::new();
    let mut active = Vec::new();
    for identity in root_identities {
        collect(plans, &identity, &mut active, &mut selected)?;
    }
    selected.retain(|identity| {
        !module
            .structural_types
            .iter()
            .any(|declaration| declaration.identity == *identity)
    });
    selected.sort();
    selected.dedup();
    if selected.is_empty() {
        return Ok(());
    }

    let mut next_type = module
        .structural_types
        .iter()
        .map(|declaration| declaration.id.get())
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "scalar cleanup structural type identity space is exhausted",
        ))?;
    let mut type_ids = module
        .structural_types
        .iter()
        .map(|declaration| (declaration.identity.clone(), declaration.id))
        .collect::<Vec<_>>();
    for identity in &selected {
        type_ids.push((
            identity.clone(),
            structural_type_id(allocate_dense(&mut next_type)?),
        ));
    }
    let mut next_field = module
        .structural_types
        .iter()
        .flat_map(|declaration| match &declaration.shape {
            StructuralTypeShape::Record { fields } => fields.as_slice(),
            StructuralTypeShape::FixedArray { .. } => &[],
        })
        .map(|field| field.id.get())
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "scalar cleanup structural field identity space is exhausted",
        ))?;
    for identity in selected {
        let plan = plans
            .iter()
            .find(|plan| plan.identity == identity)
            .expect("selected scalar structural type was validated");
        let shape = match &plan.shape {
            CheckedUnitStructuralTypeShape::Record { fields } => {
                let mut identities = BTreeSet::new();
                let fields = fields
                    .iter()
                    .map(|field| {
                        if field.identity.is_empty() || !identities.insert(&field.identity) {
                            return Err(LoweringError::Unsupported(
                                "scalar cleanup structural fields are invalid",
                            ));
                        }
                        let field_type = match &field.field_type {
                            CheckedUnitStructuralFieldType::Scalar(primitive) => {
                                StructuralFieldType::Scalar(terminal_scalar_type(*primitive)?)
                            }
                            CheckedUnitStructuralFieldType::Structural { type_identity } => {
                                StructuralFieldType::Structural(lookup_type_id(
                                    &type_ids,
                                    type_identity,
                                )?)
                            }
                            CheckedUnitStructuralFieldType::Erased { type_identity } => {
                                StructuralFieldType::Erased {
                                    type_identity: type_identity.clone(),
                                }
                            }
                        };
                        Ok(StructuralFieldDeclaration {
                            id: structural_field_id(allocate_dense(&mut next_field)?),
                            identity: field.identity.clone(),
                            relevance: field.relevance,
                            field_type,
                        })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                StructuralTypeShape::Record { fields }
            }
            CheckedUnitStructuralTypeShape::FixedArray {
                element_type_identity,
                length,
            } => StructuralTypeShape::FixedArray {
                element: lookup_type_id(&type_ids, element_type_identity)?,
                length: *length,
            },
        };
        module.structural_types.push(StructuralTypeDeclaration {
            id: lookup_type_id(&type_ids, &identity)?,
            identity,
            shape,
        });
    }
    Ok(())
}

fn lower_structural_type_plans(
    plans: &[psi_checked_trees::CheckedUnitStructuralTypePlan],
) -> Result<
    (
        Vec<StructuralTypeDeclaration>,
        Vec<(String, StructuralTypeId)>,
    ),
    LoweringError,
> {
    if plans.iter().any(|plan| plan.identity.is_empty()) {
        return unsupported("structural Unit control type has an empty identity");
    }
    let mut ordered = plans.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.identity.cmp(&right.identity));
    if ordered
        .windows(2)
        .any(|pair| pair[0].identity == pair[1].identity)
    {
        return unsupported("structural Unit control types contain duplicate identities");
    }
    let type_ids = ordered
        .iter()
        .enumerate()
        .map(|(index, plan)| {
            Ok((
                plan.identity.clone(),
                structural_type_id(dense_identity(index)?),
            ))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let mut next_field = 1_u64;
    let declarations = ordered
        .into_iter()
        .map(|plan| {
            let shape =
                match &plan.shape {
                    CheckedUnitStructuralTypeShape::Record { fields } => {
                        let mut identities = BTreeSet::new();
                        let fields = fields.iter().map(|field| {
                    if field.identity.is_empty() || !identities.insert(field.identity.as_str()) {
                        return Err(LoweringError::Unsupported(
                            "structural Unit control type has duplicate field identities",
                        ));
                    }
                    let field_type = match &field.field_type {
                        CheckedUnitStructuralFieldType::Scalar(primitive) => {
                            StructuralFieldType::Scalar(terminal_scalar_type(*primitive)?)
                        }
                        CheckedUnitStructuralFieldType::Structural { type_identity } => {
                            StructuralFieldType::Structural(lookup_type_id(
                                &type_ids,
                                type_identity,
                            )?)
                        }
                        CheckedUnitStructuralFieldType::Erased { type_identity } => {
                            StructuralFieldType::Erased {
                                type_identity: type_identity.clone(),
                            }
                        }
                    };
                    Ok(StructuralFieldDeclaration {
                        id: structural_field_id(allocate_dense(&mut next_field)?),
                        identity: field.identity.clone(),
                        relevance: field.relevance,
                        field_type,
                    })
                    }).collect::<Result<Vec<_>, LoweringError>>()?;
                        StructuralTypeShape::Record { fields }
                    }
                    CheckedUnitStructuralTypeShape::FixedArray {
                        element_type_identity,
                        length,
                    } => StructuralTypeShape::FixedArray {
                        element: lookup_type_id(&type_ids, element_type_identity)?,
                        length: *length,
                    },
                };
            Ok(StructuralTypeDeclaration {
                id: lookup_type_id(&type_ids, &plan.identity)?,
                identity: plan.identity.clone(),
                shape,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    Ok((declarations, type_ids))
}

fn lookup_type_id(
    ids: &[(String, StructuralTypeId)],
    identity: &str,
) -> Result<StructuralTypeId, LoweringError> {
    ids.iter()
        .find_map(|(candidate, id)| (candidate == identity).then_some(*id))
        .ok_or(LoweringError::Unsupported(
            "Unit closure references an unlowered structural type",
        ))
}

fn lookup_domain_id(
    ids: &[(psi_language_semantics::SemanticDomainId, StructuralDomainId)],
    source: psi_language_semantics::SemanticDomainId,
) -> Result<StructuralDomainId, LoweringError> {
    ids.iter()
        .find_map(|(candidate, id)| (*candidate == source).then_some(*id))
        .ok_or(LoweringError::Unsupported(
            "Unit closure references an unlowered structural domain",
        ))
}

fn lookup_service_id(
    ids: &[(ServiceReachId, ServiceId)],
    source: ServiceReachId,
) -> Result<ServiceId, LoweringError> {
    ids.iter()
        .find_map(|(candidate, id)| (*candidate == source).then_some(*id))
        .ok_or(LoweringError::Unsupported(
            "Unit closure references an unlowered checked service",
        ))
}

fn lookup_machine_id(
    ids: &[(psi_symbols::SymbolHandle, MachineId)],
    source: psi_symbols::SymbolHandle,
) -> Result<MachineId, LoweringError> {
    ids.iter()
        .find_map(|(candidate, id)| (*candidate == source).then_some(*id))
        .ok_or(LoweringError::Unsupported(
            "Unit call target is absent from the lowered closure",
        ))
}

fn lookup_claim_id(
    ids: &[(PermissionClaimIdentity, ClaimId)],
    source: PermissionClaimIdentity,
) -> Result<ClaimId, LoweringError> {
    ids.iter()
        .find_map(|(candidate, id)| (*candidate == source).then_some(*id))
        .ok_or(LoweringError::Unsupported(
            "Unit claim transfer references a non-entry caller claim",
        ))
}

fn dense_identity(index: usize) -> Result<u64, LoweringError> {
    u64::try_from(index)
        .map_err(|_| LoweringError::Unsupported("terminal Unit identity count exceeds u64"))?
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "terminal Unit identity count exceeds u64",
        ))
}

fn allocate_dense(next: &mut u64) -> Result<u64, LoweringError> {
    let current = *next;
    *next = next.checked_add(1).ok_or(LoweringError::Unsupported(
        "terminal Unit identity count exceeds u64",
    ))?;
    Ok(current)
}

fn checked_scalar_call_closure(
    checked: &CheckedTrees,
    entry: psi_symbols::SymbolHandle,
) -> Result<Vec<psi_symbols::SymbolHandle>, LoweringError> {
    let mut closure = vec![entry];
    let mut next = 0usize;
    while let Some(machine) = closure.get(next).copied() {
        next += 1;
        let selection = checked
            .facts
            .flow
            .terminal_machines
            .machines
            .iter()
            .find(|selection| selection.machine == machine)
            .ok_or(LoweringError::Unsupported(
                "direct scalar call target has no checked terminal selection",
            ))?;
        if selection.signature != CheckedTerminalSignatureEligibility::Eligible {
            return unsupported("direct scalar call target has an unsupported terminal signature");
        }
        let graph = checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(machine)
            .ok_or(LoweringError::Unsupported(
                "direct scalar call target has no checked scalar graph",
            ))?;
        for target in graph.states.iter().flat_map(|state| {
            state.bindings.iter().filter_map(|binding| {
                let CheckedScalarBindingValue::DirectCall { target_machine, .. } = &binding.value
                else {
                    return None;
                };
                Some(*target_machine)
            })
        }) {
            if !closure.contains(&target) {
                closure.push(target);
            }
        }
    }
    Ok(closure)
}

fn lower_scalar_call_closure(
    checked: &CheckedTrees,
    closure: &[psi_symbols::SymbolHandle],
) -> Result<LoweredTerminalPsi, LoweringError> {
    let prepared = closure
        .iter()
        .map(|machine| {
            let graph = checked
                .facts
                .flow
                .terminal_scalar_graphs
                .for_machine(*machine)
                .ok_or(LoweringError::Unsupported(
                    "terminal call-closure machine has no checked scalar graph",
                ))?;
            prepare_scalar_graph_machine(checked, *machine, graph)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if prepared.iter().any(|machine| {
        !machine.identity_reshuffles.structural_places.is_empty()
            || !machine.identity_reshuffles.entry_claims.is_empty()
            || !machine.identity_reshuffles.reshuffles.is_empty()
            || !machine.partition_compositions.structural_places.is_empty()
            || !machine.partition_compositions.compositions.is_empty()
    }) {
        return unsupported(
            "structural/content call effects require the terminal content-call slice",
        );
    }
    let machine_ids = prepared
        .iter()
        .enumerate()
        .map(|(index, machine)| {
            Ok((
                machine.source_machine,
                machine_id(
                    u64::try_from(index)
                        .map_err(|_| {
                            LoweringError::Unsupported("terminal call closure exceeds u64")
                        })?
                        .checked_add(1)
                        .expect("terminal machine identities are one-based"),
                ),
            ))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let requirement_counts = prepared
        .iter()
        .map(|machine| {
            (
                machine.source_machine,
                usize::from(machine.contract_value.is_some()),
            )
        })
        .collect::<Vec<_>>();
    let mut machines = Vec::with_capacity(prepared.len());
    let mut evidence = Vec::new();
    for (index, machine) in prepared.into_iter().enumerate() {
        let terminal_machine = machine_ids[index].1;
        let identity_base = u64::try_from(index)
            .map_err(|_| LoweringError::Unsupported("terminal call closure exceeds u64"))?
            .checked_mul(TERMINAL_MACHINE_IDENTITY_STRIDE)
            .ok_or(LoweringError::Unsupported(
                "terminal call closure identity range overflows",
            ))?;
        let mut lowered = build_scalar_graph_module(
            &machine.states,
            machine.result_type,
            machine.contract_value,
            machine.crash_routes,
            machine.identity_reshuffles,
            machine.partition_compositions,
            terminal_machine,
            identity_base,
            &machine_ids,
            &requirement_counts,
        )?;
        let [terminal_machine] = lowered.semantic_module.machines.as_slice() else {
            unreachable!("one prepared scalar graph emits one terminal machine")
        };
        machines.push(terminal_machine.clone());
        evidence.append(&mut lowered.proof_bundle.evidence);
    }
    let mut lowered = LoweredTerminalPsi {
        semantic_module: TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine_id(1),
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            evidence_package_invocations: Vec::new(),
            machines,
        },
        proof_bundle: ProofBundle {
            evidence_producers: Vec::new(),
            evidence,
        },
        debug_map: None,
    };
    finalize_operation_proofs(&mut lowered)?;
    Ok(lowered)
}

fn lower_checked_crash_frontier(
    frontier: &[PermissionClaimIdentity],
    source_claims: &[(PermissionClaimIdentity, ClaimId)],
) -> Result<Vec<ClaimId>, LoweringError> {
    let mut lowered = frontier
        .iter()
        .map(|identity| {
            source_claims
                .iter()
                .find_map(|(source, claim)| (source == identity).then_some(*claim))
                .ok_or(LoweringError::CrashFrontierClaimNotLowered(*identity))
        })
        .collect::<Result<Vec<_>, _>>()?;
    lowered.sort();
    lowered.dedup();
    Ok(lowered)
}

fn lower_checked_crash_routes(
    checked: &CheckedTrees,
    machine: psi_symbols::SymbolHandle,
) -> Result<Vec<psi_checked_trees::CrashRouteBucket>, LoweringError> {
    checked
        .facts
        .contract_plans
        .for_machine(machine)
        .map(|contract| {
            contract
                .crash
                .published()
                .iter()
                .map(|bucket| {
                    if bucket.alternative_guards().iter().any(|guard| {
                        matches!(guard, psi_checked_trees::CrashRouteGuard::Predicate(predicate)
                            if predicate.scalar_expression().is_none())
                    }) {
                        return unsupported(
                            "guarded crash route is outside structured scalar predicate lowering",
                        );
                    }
                    Ok(bucket.clone())
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .unwrap_or_else(|| Ok(Vec::new()))
}

fn lower_checked_crash_exit(
    checked: &CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    state: psi_symbols::SymbolHandle,
    statement_ordinal: u32,
    source_claims: &[(PermissionClaimIdentity, ClaimId)],
) -> Result<LoweredCrashExit, LoweringError> {
    let Some(crash_plan) = checked
        .facts
        .contract_plans
        .for_machine(machine)
        .map(|contract| &contract.crash)
    else {
        return unsupported("explicit crash has no checked machine-contract plan");
    };
    let Some(checked_site) = crash_plan.checked_site_at(state, statement_ordinal) else {
        return unsupported("explicit crash has no body-derived checked crash-site row");
    };
    let matching_contracts = crash_plan
        .covering_buckets_for_site(checked_site)
        .map(|(_, bucket)| bucket)
        .collect::<Vec<_>>();
    let [covering_bucket] = matching_contracts.as_slice() else {
        return unsupported(
            "an explicit crash in the terminal-Psi source slice requires exactly one prechecked covering route bucket",
        );
    };
    let site_identities = checked_site
        .path_guard_conjuncts()
        .iter()
        .chain(checked_site.path_guard_consequences())
        .collect::<BTreeSet<_>>();
    let site_guard = covering_bucket
        .alternative_guards()
        .iter()
        .filter_map(|guard| match guard {
            psi_checked_trees::CrashRouteGuard::Truth => None,
            psi_checked_trees::CrashRouteGuard::Predicate(predicate)
                if site_identities.contains(predicate) =>
            {
                Some(
                    predicate
                        .scalar_expression()
                        .cloned()
                        .ok_or(LoweringError::Unsupported(
                            "guarded crash site is outside structured scalar predicate lowering",
                        )),
                )
            }
            psi_checked_trees::CrashRouteGuard::Predicate(_) => None,
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !covering_bucket
        .alternative_guards()
        .contains(&psi_checked_trees::CrashRouteGuard::Truth)
        && site_guard.is_empty()
    {
        return unsupported("guarded crash site has no structured covering predicate");
    }
    Ok(LoweredCrashExit {
        cause: match checked_site.cause() {
            psi_checked_trees::CrashCause::Trap => TerminalCrashCause::Trap,
            psi_checked_trees::CrashCause::Abort => TerminalCrashCause::Abort,
        },
        site_guard,
        frontier_lower_bound: lower_checked_crash_frontier(
            checked_site.frontier_lower_bound(),
            source_claims,
        )?,
    })
}

fn merge_known_parameters<T: Copy + Eq>(
    current: &mut Option<Vec<Option<T>>>,
    incoming: Vec<Option<T>>,
) {
    if let Some(current) = current {
        assert_eq!(current.len(), incoming.len());
        for (current, incoming) in current.iter_mut().zip(incoming) {
            if *current != incoming {
                *current = None;
            }
        }
    } else {
        *current = Some(incoming);
    }
}

fn acyclic_topological_order(successors: &[Vec<usize>]) -> Vec<usize> {
    let mut indegree = vec![0_usize; successors.len()];
    for targets in successors {
        for target in targets {
            indegree[*target] = indegree[*target]
                .checked_add(1)
                .expect("source state count fits usize");
        }
    }
    let mut ready = indegree
        .iter()
        .enumerate()
        .filter_map(|(index, indegree)| (*indegree == 0).then_some(index))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(successors.len());
    while let Some(state) = ready.iter().next().copied() {
        ready.remove(&state);
        order.push(state);
        for target in &successors[state] {
            indegree[*target] = indegree[*target]
                .checked_sub(1)
                .expect("graph indegree is positive before traversal");
            if indegree[*target] == 0 {
                ready.insert(*target);
            }
        }
    }
    assert_eq!(order.len(), successors.len());
    order
}

fn evaluate_known_scalar_graph(states: &[LoweredScalarBranchState]) -> Option<KnownDirectScalar> {
    let successors = states
        .iter()
        .map(|state| match &state.terminator {
            LoweredScalarBranchTerminator::Jump { target, .. } => vec![*target],
            LoweredScalarBranchTerminator::Conditional {
                when_true_target,
                when_false_target,
                ..
            } => vec![*when_true_target, *when_false_target],
            LoweredScalarBranchTerminator::Return { .. }
            | LoweredScalarBranchTerminator::Crash(_) => Vec::new(),
        })
        .collect::<Vec<_>>();
    let topological_order = acyclic_topological_order(&successors);
    let mut known_parameters = vec![None; states.len()];
    known_parameters[0] = Some(vec![None; states[0].parameter_types.len()]);
    let mut return_values = Vec::new();
    let mut reachable_crash = false;
    for state_index in topological_order {
        let Some(mut values) = known_parameters[state_index].clone() else {
            continue;
        };
        for binding in &states[state_index].bindings {
            let value = match binding {
                LoweredScalarBinding::Expression(expression) => {
                    evaluate_direct_expression(expression, &values)
                }
                LoweredScalarBinding::DirectCall(_) => None,
            };
            values.push(value);
        }
        let evaluate_arguments = |arguments: &[LoweredDirectExpression]| {
            arguments
                .iter()
                .map(|argument| evaluate_direct_expression(argument, &values))
                .collect::<Vec<_>>()
        };
        match &states[state_index].terminator {
            LoweredScalarBranchTerminator::Jump { target, arguments } => {
                merge_known_parameters(
                    &mut known_parameters[*target],
                    evaluate_arguments(arguments),
                );
            }
            LoweredScalarBranchTerminator::Conditional {
                condition,
                when_true_target,
                when_true_arguments,
                when_false_target,
                when_false_arguments,
            } => match evaluate_compile_known_boolean_expression(condition, &values) {
                Some(true) => merge_known_parameters(
                    &mut known_parameters[*when_true_target],
                    evaluate_arguments(when_true_arguments),
                ),
                Some(false) => merge_known_parameters(
                    &mut known_parameters[*when_false_target],
                    evaluate_arguments(when_false_arguments),
                ),
                None => {
                    merge_known_parameters(
                        &mut known_parameters[*when_true_target],
                        evaluate_arguments(when_true_arguments),
                    );
                    merge_known_parameters(
                        &mut known_parameters[*when_false_target],
                        evaluate_arguments(when_false_arguments),
                    );
                }
            },
            LoweredScalarBranchTerminator::Return { expression } => {
                return_values.push(evaluate_direct_expression(expression, &values));
            }
            LoweredScalarBranchTerminator::Crash(_) => reachable_crash = true,
        }
    }

    if reachable_crash {
        return None;
    }
    let expected = return_values.first().copied().flatten()?;
    return_values
        .into_iter()
        .all(|value| value == Some(expected))
        .then_some(expected)
}

fn lower_scalar_graph_machine(
    checked: &CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    graph: &CheckedScalarMachineGraph,
) -> Result<LoweredTerminalPsi, LoweringError> {
    let prepared = prepare_scalar_graph_machine(checked, machine, graph)?;
    let machine_ids = [(machine, machine_id(1))];
    let requirement_counts = [(machine, usize::from(prepared.contract_value.is_some()))];
    let mut lowered = build_scalar_graph_module(
        &prepared.states,
        prepared.result_type,
        prepared.contract_value,
        prepared.crash_routes,
        prepared.identity_reshuffles,
        prepared.partition_compositions,
        machine_id(1),
        0,
        &machine_ids,
        &requirement_counts,
    )?;
    finalize_operation_proofs(&mut lowered)?;
    Ok(lowered)
}

fn prepare_scalar_graph_machine(
    checked: &CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    graph: &CheckedScalarMachineGraph,
) -> Result<PreparedScalarMachine, LoweringError> {
    let states = &graph.states;
    let entry_state = states.first().ok_or(LoweringError::Unsupported(
        "checked scalar control plan must contain an entry state",
    ))?;
    let result_type = terminal_scalar_type(entry_state.result_type)?;
    let (identity_reshuffles, partition_compositions) =
        lower_content_evidence(checked, machine, entry_state.state)?;
    let mut lowered_states = Vec::with_capacity(states.len());
    let mut successors = vec![Vec::new(); states.len()];
    let mut indegree = vec![0usize; states.len()];

    for (state_index, state) in states.iter().enumerate() {
        if terminal_scalar_type(state.result_type)? != result_type {
            return unsupported("scalar graph state result types must match exactly");
        }
        let parameter_types = state
            .parameter_types
            .iter()
            .copied()
            .map(terminal_scalar_type)
            .collect::<Result<Vec<_>, _>>()?;
        let mut value_types = parameter_types.clone();
        let mut bindings = Vec::with_capacity(state.bindings.len());
        for (binding_index, binding) in state.bindings.iter().enumerate() {
            let binding_ordinal = u32::try_from(binding_index)
                .map_err(|_| LoweringError::Unsupported("scalar local count exceeds u32"))?;
            let binding_type = terminal_scalar_type(binding.primitive_type)?;
            let lowered = match &binding.value {
                CheckedScalarBindingValue::Expression => {
                    let expression = lower_checked_scalar_expression_at(
                        checked,
                        state.state,
                        binding.statement_ordinal,
                        CheckedScalarExpressionRole::LocalInitializer { binding_ordinal },
                    )?;
                    if expression.scalar_type() != binding_type {
                        return unsupported(
                            "checked scalar local initializer type must match its binding",
                        );
                    }
                    validate_direct_parameter_types(&expression, &value_types)?;
                    LoweredScalarBinding::Expression(expression)
                }
                CheckedScalarBindingValue::DirectCall {
                    target_machine,
                    target_state,
                    call_ordinal,
                    argument_count,
                } => LoweredScalarBinding::DirectCall(lower_checked_direct_call_binding(
                    checked,
                    machine,
                    state.state,
                    binding.statement_ordinal,
                    binding_ordinal,
                    *target_machine,
                    *target_state,
                    *call_ordinal,
                    *argument_count,
                    binding_type,
                    &value_types,
                )?),
            };
            bindings.push(lowered);
            value_types.push(binding_type);
        }
        let terminator = match &state.terminator {
            CheckedScalarStateTerminator::Return { statement_ordinal } => {
                let expression = lower_checked_scalar_expression_at(
                    checked,
                    state.state,
                    *statement_ordinal,
                    CheckedScalarExpressionRole::Return,
                )?;
                if expression.scalar_type() != result_type {
                    return unsupported("checked scalar return type must match the machine result");
                }
                validate_direct_parameter_types(&expression, &value_types)?;
                LoweredScalarBranchTerminator::Return { expression }
            }
            CheckedScalarStateTerminator::Crash { statement_ordinal } => {
                LoweredScalarBranchTerminator::Crash(lower_checked_crash_exit(
                    checked,
                    machine,
                    state.state,
                    *statement_ordinal,
                    &identity_reshuffles.source_claims,
                )?)
            }
            CheckedScalarStateTerminator::Conditional {
                guard_statement_ordinal,
                when_true,
                when_false,
            } => {
                let LoweredDirectExpression::Boolean {
                    expression: condition,
                } = lower_checked_scalar_expression_at(
                    checked,
                    state.state,
                    *guard_statement_ordinal,
                    CheckedScalarExpressionRole::Guard,
                )?
                else {
                    return unsupported("checked scalar graph guard must be Boolean");
                };
                let condition = *condition;
                validate_short_circuit_expression(&condition)?;
                validate_boolean_parameter_types(&condition, &value_types)?;

                let (when_true_target, when_true_arguments) = lower_scalar_graph_successor(
                    checked,
                    states,
                    state.state,
                    &value_types,
                    when_true,
                )?;
                let (when_false_target, when_false_arguments) = lower_scalar_graph_successor(
                    checked,
                    states,
                    state.state,
                    &value_types,
                    when_false,
                )?;
                successors[state_index] = vec![when_true_target, when_false_target];
                indegree[when_true_target] = indegree[when_true_target]
                    .checked_add(1)
                    .expect("source state count fits usize");
                indegree[when_false_target] = indegree[when_false_target]
                    .checked_add(1)
                    .expect("source state count fits usize");
                LoweredScalarBranchTerminator::Conditional {
                    condition,
                    when_true_target,
                    when_true_arguments,
                    when_false_target,
                    when_false_arguments,
                }
            }
            CheckedScalarStateTerminator::Jump(successor) => {
                let (target, arguments) = lower_scalar_graph_successor(
                    checked,
                    states,
                    state.state,
                    &value_types,
                    successor,
                )?;
                successors[state_index] = vec![target];
                indegree[target] = indegree[target]
                    .checked_add(1)
                    .expect("source state count fits usize");
                LoweredScalarBranchTerminator::Jump { target, arguments }
            }
        };
        lowered_states.push(LoweredScalarBranchState {
            parameter_types,
            bindings,
            terminator,
        });
    }

    if indegree[0] != 0 || indegree[1..].contains(&0) {
        return unsupported(
            "scalar graph control must be rooted at the machine entry and reach every state",
        );
    }
    let mut visited = vec![false; states.len()];
    let mut active = vec![false; states.len()];
    validate_scalar_graph(0, &successors, &mut visited, &mut active)?;
    if visited.iter().any(|visited| !*visited) {
        return unsupported("scalar graph control contains an unreachable state");
    }

    let has_crash = lowered_states.iter().any(|state| {
        matches!(&state.terminator, LoweredScalarBranchTerminator::Crash(_))
            || state.bindings.iter().any(|binding| {
                matches!(binding, LoweredScalarBinding::DirectCall(call)
                        if !call.crash_continuations.is_empty())
            })
    });
    let has_return = lowered_states.iter().any(|state| {
        matches!(
            &state.terminator,
            LoweredScalarBranchTerminator::Return { .. }
        )
    });
    let expected_value = evaluate_known_scalar_graph(&lowered_states);
    let contract_value = if has_return {
        Some(validate_closed_scalar_contract(
            checked,
            machine,
            result_type,
            expected_value,
            has_crash,
        )?)
    } else {
        let contract = closed_scalar_contract_plan(checked, machine)?;
        if contract.has_other_clauses()
            || !contract.requires().is_empty()
            || !contract.ensures().is_empty()
        {
            return unsupported("an all-crash scalar graph cannot declare a value contract");
        }
        None
    };
    Ok(PreparedScalarMachine {
        source_machine: machine,
        states: lowered_states,
        result_type,
        contract_value,
        crash_routes: lower_checked_crash_routes(checked, machine)?,
        identity_reshuffles,
        partition_compositions,
    })
}

#[allow(clippy::too_many_arguments)]
fn lower_checked_direct_call_binding(
    checked: &CheckedTrees,
    caller_machine: psi_symbols::SymbolHandle,
    caller_state: psi_symbols::SymbolHandle,
    statement_ordinal: u32,
    binding_ordinal: u32,
    target_machine: psi_symbols::SymbolHandle,
    target_state: psi_symbols::SymbolHandle,
    call_ordinal: u32,
    argument_count: u32,
    result_type: ScalarType,
    caller_value_types: &[ScalarType],
) -> Result<LoweredDirectCallBinding, LoweringError> {
    let target_graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(target_machine)
        .ok_or(LoweringError::Unsupported(
            "direct scalar call target has no source-independent checked graph",
        ))?;
    let target_entry = target_graph
        .states
        .first()
        .ok_or(LoweringError::Unsupported(
            "direct scalar call target has no checked entry state",
        ))?;
    if target_entry.state != target_state {
        return unsupported("direct scalar call must target the callee entry state");
    }
    if terminal_scalar_type(target_entry.result_type)? != result_type {
        return unsupported("direct scalar call result type must match its local binding");
    }
    if usize::try_from(argument_count).ok() != Some(target_entry.parameter_types.len()) {
        return unsupported("direct scalar call argument count must match the callee signature");
    }
    let arguments = target_entry
        .parameter_types
        .iter()
        .enumerate()
        .map(|(argument_index, target_type)| {
            let argument_ordinal = u32::try_from(argument_index).map_err(|_| {
                LoweringError::Unsupported("scalar call argument count exceeds u32")
            })?;
            let expression = lower_checked_scalar_expression_at(
                checked,
                caller_state,
                statement_ordinal,
                CheckedScalarExpressionRole::CallArgument {
                    binding_ordinal,
                    argument_ordinal,
                },
            )?;
            let target_type = terminal_scalar_type(*target_type)?;
            if expression.scalar_type() != target_type {
                return unsupported(
                    "checked scalar call argument type must match its callee parameter",
                );
            }
            validate_direct_parameter_types(&expression, caller_value_types)?;
            Ok(expression)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let checked_call = checked
        .facts
        .contract_plans
        .for_machine(caller_machine)
        .and_then(|plan| {
            plan.crash
                .checked_call_at(caller_state, statement_ordinal, call_ordinal)
        })
        .ok_or(LoweringError::Unsupported(
            "direct scalar call has no matching checked crash-refinement row",
        ))?;
    if checked_call.target_machine() != target_machine
        || checked_call.target_state() != target_state
    {
        return unsupported("checked scalar call target disagrees with crash refinement");
    }
    let target_contract = checked
        .facts
        .contract_plans
        .for_machine(target_machine)
        .ok_or(LoweringError::Unsupported(
            "direct scalar call target has no checked contract plan",
        ))?;
    if checked_call.target_contract_fingerprint() != target_contract.fingerprint {
        return unsupported("checked scalar call target contract fingerprint disagrees");
    }
    if checked_call.surviving_buckets().iter().any(|bucket| {
        bucket.alternative_guards().iter().any(|guard| {
            matches!(guard, psi_checked_trees::CrashRouteGuard::Predicate(predicate)
                if predicate.scalar_expression().is_none())
        })
    }) {
        return unsupported("direct scalar call crash continuation lacks a checked scalar term");
    }
    Ok(LoweredDirectCallBinding {
        target_machine,
        result_type,
        arguments,
        crash_continuations: checked_call.surviving_buckets().to_vec(),
        parameter_relative_crash_routes: target_contract.crash.published().to_vec(),
    })
}

fn lower_scalar_graph_successor(
    checked: &CheckedTrees,
    states: &[psi_checked_trees::CheckedScalarStateGraph],
    source_state: psi_symbols::SymbolHandle,
    source_value_types: &[ScalarType],
    successor: &CheckedScalarSuccessor,
) -> Result<(usize, Vec<LoweredDirectExpression>), LoweringError> {
    let target = states
        .iter()
        .position(|candidate| candidate.state == successor.target)
        .ok_or(LoweringError::Unsupported(
            "scalar graph successor must belong to the selected machine",
        ))?;
    let target_parameter_types = &states[target].parameter_types;
    if usize::try_from(successor.argument_count).ok() != Some(target_parameter_types.len()) {
        return unsupported(
            "scalar graph successor bindings must match the target parameter count",
        );
    }
    let arguments = (0..successor.argument_count)
        .zip(target_parameter_types)
        .map(|(argument_ordinal, target_type)| {
            let target_type = terminal_scalar_type(*target_type)?;
            let expression = lower_checked_scalar_expression_at(
                checked,
                source_state,
                successor.statement_ordinal,
                CheckedScalarExpressionRole::TransitionArgument { argument_ordinal },
            )?;
            validate_direct_parameter_types(&expression, source_value_types)?;
            (expression.scalar_type() == target_type)
                .then_some(expression)
                .ok_or(LoweringError::Unsupported(
                    "checked scalar successor expression type must match its target",
                ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((target, arguments))
}

fn lower_checked_scalar_expression_at(
    checked: &CheckedTrees,
    state: psi_symbols::SymbolHandle,
    statement_ordinal: u32,
    role: CheckedScalarExpressionRole,
) -> Result<LoweredDirectExpression, LoweringError> {
    let expression = checked
        .facts
        .values
        .scalar_expressions
        .expression_at(state, statement_ordinal, role)
        .ok_or(LoweringError::Unsupported(
            "scalar expression has no source-independent checked value plan",
        ))?;
    lower_checked_scalar_expression(expression)
}

fn lower_checked_scalar_expression(
    expression: &CheckedScalarExpression,
) -> Result<LoweredDirectExpression, LoweringError> {
    match expression {
        CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        } => Ok(LoweredDirectExpression::Parameter {
            position: *position,
            scalar_type: terminal_scalar_type(*primitive_type)?,
        }),
        CheckedScalarExpression::Local {
            position,
            primitive_type,
        } => Ok(LoweredDirectExpression::Local {
            position: *position,
            scalar_type: terminal_scalar_type(*primitive_type)?,
        }),
        CheckedScalarExpression::StructuralParameterField { .. } => unsupported(
            "structural parameter fields are retained only inside structural crash predicates",
        ),
        CheckedScalarExpression::IntegerLiteral { literal } => {
            let scalar_type = integer_landing_scalar_type(literal)?;
            Ok(LoweredDirectExpression::IntegerLiteral {
                value: integer_value(literal, scalar_type)?,
                scalar_type,
            })
        }
        CheckedScalarExpression::IntegerBinary {
            kind,
            primitive_type,
            left,
            right,
        } => Ok(LoweredDirectExpression::IntegerBinary {
            kind: match kind {
                CheckedIntegerBinaryKind::ExactAdd => LoweredIntegerBinaryKind::ExactAdd,
                CheckedIntegerBinaryKind::ExactSubtract => LoweredIntegerBinaryKind::ExactSubtract,
                CheckedIntegerBinaryKind::ExactMultiply => LoweredIntegerBinaryKind::ExactMultiply,
                CheckedIntegerBinaryKind::ExactDivide => LoweredIntegerBinaryKind::ExactDivide,
                CheckedIntegerBinaryKind::ExactRemainder => {
                    LoweredIntegerBinaryKind::ExactRemainder
                }
                CheckedIntegerBinaryKind::WrappingDivide => {
                    LoweredIntegerBinaryKind::WrappingDivide
                }
                CheckedIntegerBinaryKind::WrappingRemainder => {
                    LoweredIntegerBinaryKind::WrappingRemainder
                }
                CheckedIntegerBinaryKind::SaturatingDivide => {
                    LoweredIntegerBinaryKind::SaturatingDivide
                }
                CheckedIntegerBinaryKind::SaturatingRemainder => {
                    LoweredIntegerBinaryKind::SaturatingRemainder
                }
                CheckedIntegerBinaryKind::WrappingAdd => LoweredIntegerBinaryKind::WrappingAdd,
                CheckedIntegerBinaryKind::SaturatingAdd => LoweredIntegerBinaryKind::SaturatingAdd,
                CheckedIntegerBinaryKind::WrappingSubtract => {
                    LoweredIntegerBinaryKind::WrappingSubtract
                }
                CheckedIntegerBinaryKind::SaturatingSubtract => {
                    LoweredIntegerBinaryKind::SaturatingSubtract
                }
                CheckedIntegerBinaryKind::WrappingMultiply => {
                    LoweredIntegerBinaryKind::WrappingMultiply
                }
                CheckedIntegerBinaryKind::SaturatingMultiply => {
                    LoweredIntegerBinaryKind::SaturatingMultiply
                }
                CheckedIntegerBinaryKind::BitwiseAnd => LoweredIntegerBinaryKind::BitwiseAnd,
                CheckedIntegerBinaryKind::BitwiseOr => LoweredIntegerBinaryKind::BitwiseOr,
                CheckedIntegerBinaryKind::BitwiseXor => LoweredIntegerBinaryKind::BitwiseXor,
                CheckedIntegerBinaryKind::WrappingShiftLeft => {
                    LoweredIntegerBinaryKind::WrappingShiftLeft
                }
                CheckedIntegerBinaryKind::WrappingShiftRight => {
                    LoweredIntegerBinaryKind::WrappingShiftRight
                }
                CheckedIntegerBinaryKind::ExactShiftLeft => {
                    LoweredIntegerBinaryKind::ExactShiftLeft
                }
                CheckedIntegerBinaryKind::ExactShiftRight => {
                    LoweredIntegerBinaryKind::ExactShiftRight
                }
            },
            scalar_type: terminal_scalar_type(*primitive_type)?,
            left: Box::new(lower_checked_scalar_expression(left)?),
            right: Box::new(lower_checked_scalar_expression(right)?),
        }),
        CheckedScalarExpression::IntegerBitwiseNot {
            primitive_type,
            operand,
        } => Ok(LoweredDirectExpression::IntegerBitwiseNot {
            scalar_type: terminal_scalar_type(*primitive_type)?,
            operand: Box::new(lower_checked_scalar_expression(operand)?),
        }),
        CheckedScalarExpression::IntegerWiden {
            primitive_type,
            operand,
        } => Ok(LoweredDirectExpression::IntegerWiden {
            scalar_type: terminal_scalar_type(*primitive_type)?,
            operand: Box::new(lower_checked_scalar_expression(operand)?),
        }),
        CheckedScalarExpression::IntegerExactCast {
            primitive_type,
            operand,
            ..
        } => Ok(LoweredDirectExpression::IntegerExactCast {
            scalar_type: terminal_scalar_type(*primitive_type)?,
            operand: Box::new(lower_checked_scalar_expression(operand)?),
        }),
        CheckedScalarExpression::Boolean(expression) => Ok(LoweredDirectExpression::Boolean {
            expression: Box::new(lower_checked_boolean_expression(expression)?),
        }),
    }
}

fn lower_checked_boolean_expression(
    expression: &CheckedBooleanExpression,
) -> Result<LoweredBooleanReturnExpression, LoweringError> {
    Ok(match expression {
        CheckedBooleanExpression::Constant(value) => {
            LoweredBooleanReturnExpression::Constant { value: *value }
        }
        CheckedBooleanExpression::Parameter { position } => {
            LoweredBooleanReturnExpression::Parameter {
                position: *position,
            }
        }
        CheckedBooleanExpression::Local { position } => LoweredBooleanReturnExpression::Local {
            position: *position,
        },
        CheckedBooleanExpression::StructuralParameterField {
            parameter_position,
            path,
        } => LoweredBooleanReturnExpression::UnresolvedStructuralParameterField {
            parameter_position: *parameter_position,
            path: path.clone(),
        },
        CheckedBooleanExpression::Not(operand) => LoweredBooleanReturnExpression::Not {
            operand: Box::new(lower_checked_boolean_expression(operand)?),
        },
        CheckedBooleanExpression::Equal { left, right } => LoweredBooleanReturnExpression::Equal {
            left: Box::new(lower_checked_boolean_expression(left)?),
            right: Box::new(lower_checked_boolean_expression(right)?),
        },
        CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
            LoweredBooleanReturnExpression::IntegerComparison {
                kind: match kind {
                    CheckedIntegerComparisonKind::Equal => LoweredIntegerComparisonKind::Equal,
                    CheckedIntegerComparisonKind::LessThan => {
                        LoweredIntegerComparisonKind::LessThan
                    }
                    CheckedIntegerComparisonKind::LessOrEqual => {
                        LoweredIntegerComparisonKind::LessOrEqual
                    }
                },
                left: Box::new(lower_checked_scalar_expression(left)?),
                right: Box::new(lower_checked_scalar_expression(right)?),
            }
        }
        CheckedBooleanExpression::And { left, right } => LoweredBooleanReturnExpression::And {
            left: Box::new(lower_checked_boolean_expression(left)?),
            right: Box::new(lower_checked_boolean_expression(right)?),
        },
        CheckedBooleanExpression::Or { left, right } => LoweredBooleanReturnExpression::Or {
            left: Box::new(lower_checked_boolean_expression(left)?),
            right: Box::new(lower_checked_boolean_expression(right)?),
        },
    })
}

fn validate_boolean_parameter_types(
    expression: &LoweredBooleanReturnExpression,
    parameter_types: &[ScalarType],
) -> Result<(), LoweringError> {
    match expression {
        LoweredBooleanReturnExpression::Constant { .. } => Ok(()),
        LoweredBooleanReturnExpression::StructuralField { .. } => Ok(()),
        LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. } => {
            unsupported("unresolved structural field crossed Boolean type validation")
        }
        LoweredBooleanReturnExpression::Parameter { position }
        | LoweredBooleanReturnExpression::Local { position } => {
            if parameter_types.get(*position) == Some(&ScalarType::Boolean) {
                Ok(())
            } else {
                unsupported("scalar graph guard parameters must be Boolean")
            }
        }
        LoweredBooleanReturnExpression::Not { operand } => {
            validate_boolean_parameter_types(operand, parameter_types)
        }
        LoweredBooleanReturnExpression::Equal { left, right }
        | LoweredBooleanReturnExpression::And { left, right }
        | LoweredBooleanReturnExpression::Or { left, right } => {
            validate_boolean_parameter_types(left, parameter_types)?;
            validate_boolean_parameter_types(right, parameter_types)
        }
        LoweredBooleanReturnExpression::IntegerComparison { left, right, .. } => {
            validate_direct_parameter_types(left, parameter_types)?;
            validate_direct_parameter_types(right, parameter_types)
        }
    }
}

fn validate_direct_parameter_types(
    expression: &LoweredDirectExpression,
    parameter_types: &[ScalarType],
) -> Result<(), LoweringError> {
    match expression {
        LoweredDirectExpression::Parameter {
            position,
            scalar_type,
        }
        | LoweredDirectExpression::Local {
            position,
            scalar_type,
        } => {
            if parameter_types.get(*position) == Some(scalar_type) {
                Ok(())
            } else {
                unsupported("scalar graph integer guard parameter type does not match")
            }
        }
        LoweredDirectExpression::IntegerLiteral { .. } => Ok(()),
        LoweredDirectExpression::IntegerBinary { left, right, .. } => {
            validate_direct_parameter_types(left, parameter_types)?;
            validate_direct_parameter_types(right, parameter_types)
        }
        LoweredDirectExpression::IntegerBitwiseNot { operand, .. } => {
            validate_direct_parameter_types(operand, parameter_types)
        }
        LoweredDirectExpression::IntegerWiden { operand, .. } => {
            validate_direct_parameter_types(operand, parameter_types)
        }
        LoweredDirectExpression::IntegerExactCast { operand, .. } => {
            validate_direct_parameter_types(operand, parameter_types)
        }
        LoweredDirectExpression::Boolean { expression } => {
            validate_boolean_parameter_types(expression, parameter_types)
        }
    }
}

fn validate_scalar_graph(
    state: usize,
    successors: &[Vec<usize>],
    visited: &mut [bool],
    active: &mut [bool],
) -> Result<(), LoweringError> {
    if active[state] {
        return unsupported("scalar graph control must be acyclic");
    }
    if visited[state] {
        return Ok(());
    }
    active[state] = true;
    for successor in &successors[state] {
        validate_scalar_graph(*successor, successors, visited, active)?;
    }
    active[state] = false;
    visited[state] = true;
    Ok(())
}

fn contains_short_circuit(expression: &LoweredBooleanReturnExpression) -> bool {
    match expression {
        LoweredBooleanReturnExpression::Constant { .. }
        | LoweredBooleanReturnExpression::Parameter { .. }
        | LoweredBooleanReturnExpression::Local { .. }
        | LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. }
        | LoweredBooleanReturnExpression::StructuralField { .. }
        | LoweredBooleanReturnExpression::IntegerComparison { .. } => false,
        LoweredBooleanReturnExpression::Not { operand } => contains_short_circuit(operand),
        LoweredBooleanReturnExpression::Equal { left, right } => {
            contains_short_circuit(left) || contains_short_circuit(right)
        }
        LoweredBooleanReturnExpression::And { .. } | LoweredBooleanReturnExpression::Or { .. } => {
            true
        }
    }
}

fn direct_expression_contains_short_circuit(expression: &LoweredDirectExpression) -> bool {
    matches!(
        expression,
        LoweredDirectExpression::Boolean { expression }
            if contains_short_circuit(expression)
    )
}

fn scalar_binding_contains_short_circuit(binding: &LoweredScalarBinding) -> bool {
    match binding {
        LoweredScalarBinding::Expression(expression) => {
            direct_expression_contains_short_circuit(expression)
        }
        LoweredScalarBinding::DirectCall(call) => call
            .arguments
            .iter()
            .any(direct_expression_contains_short_circuit),
    }
}

fn staged_short_circuit_bindings_terminator(
    bindings: &[LoweredScalarBinding],
    terminator: &LoweredScalarBranchTerminator,
) -> Option<(Vec<LoweredScalarBinding>, LoweredScalarBranchTerminator)> {
    if !bindings.iter().any(scalar_binding_contains_short_circuit) {
        return None;
    }
    Some((bindings.to_vec(), terminator.clone()))
}

fn validate_short_circuit_expression(
    expression: &LoweredBooleanReturnExpression,
) -> Result<(), LoweringError> {
    match expression {
        LoweredBooleanReturnExpression::Constant { .. }
        | LoweredBooleanReturnExpression::Parameter { .. }
        | LoweredBooleanReturnExpression::StructuralField { .. }
        | LoweredBooleanReturnExpression::Local { .. }
        | LoweredBooleanReturnExpression::IntegerComparison { .. } => Ok(()),
        LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. } => {
            unsupported("unresolved structural field crossed Boolean validation")
        }
        LoweredBooleanReturnExpression::Not { operand } => {
            validate_short_circuit_expression(operand)
        }
        LoweredBooleanReturnExpression::Equal { left, right } => {
            validate_short_circuit_expression(left)?;
            validate_short_circuit_expression(right)
        }
        LoweredBooleanReturnExpression::And { left, right }
        | LoweredBooleanReturnExpression::Or { left, right } => {
            validate_short_circuit_expression(left)?;
            validate_short_circuit_expression(right)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KnownDirectScalar {
    Boolean(bool),
    Integer(IntegerValue),
}

fn evaluate_direct_expression(
    expression: &LoweredDirectExpression,
    parameters: &[Option<KnownDirectScalar>],
) -> Option<KnownDirectScalar> {
    match expression {
        LoweredDirectExpression::Parameter { position, .. }
        | LoweredDirectExpression::Local { position, .. } => {
            parameters.get(*position).copied().flatten()
        }
        LoweredDirectExpression::IntegerLiteral { value, .. } => {
            Some(KnownDirectScalar::Integer(*value))
        }
        LoweredDirectExpression::IntegerBinary {
            kind,
            scalar_type,
            left,
            right,
        } => {
            let count_type = right.scalar_type();
            let KnownDirectScalar::Integer(left) = evaluate_direct_expression(left, parameters)?
            else {
                return None;
            };
            let KnownDirectScalar::Integer(right) = evaluate_direct_expression(right, parameters)?
            else {
                return None;
            };
            evaluate_lowered_integer_binary(*kind, *scalar_type, count_type, left, right)
                .map(KnownDirectScalar::Integer)
        }
        LoweredDirectExpression::IntegerBitwiseNot {
            scalar_type,
            operand,
        } => {
            let ScalarType::Integer(integer_type) = scalar_type else {
                return None;
            };
            let KnownDirectScalar::Integer(operand) =
                evaluate_direct_expression(operand, parameters)?
            else {
                return None;
            };
            integer_type
                .bitwise_not(operand)
                .map(KnownDirectScalar::Integer)
        }
        LoweredDirectExpression::IntegerWiden {
            scalar_type,
            operand,
        } => {
            let ScalarType::Integer(source_type) = operand.scalar_type() else {
                return None;
            };
            let ScalarType::Integer(target_type) = scalar_type else {
                return None;
            };
            let KnownDirectScalar::Integer(value) =
                evaluate_direct_expression(operand, parameters)?
            else {
                return None;
            };
            source_type
                .widen_value_to(*target_type, value)
                .map(KnownDirectScalar::Integer)
        }
        LoweredDirectExpression::IntegerExactCast {
            scalar_type,
            operand,
        } => {
            let ScalarType::Integer(source_type) = operand.scalar_type() else {
                return None;
            };
            let ScalarType::Integer(target_type) = scalar_type else {
                return None;
            };
            let KnownDirectScalar::Integer(value) =
                evaluate_direct_expression(operand, parameters)?
            else {
                return None;
            };
            source_type
                .exact_cast_value_to(*target_type, value)
                .map(KnownDirectScalar::Integer)
        }
        LoweredDirectExpression::Boolean { expression } => {
            evaluate_compile_known_boolean_expression(expression, parameters)
                .map(KnownDirectScalar::Boolean)
        }
    }
}

fn evaluate_integer_direct_expression(
    expression: &LoweredDirectExpression,
    parameters: &[Option<KnownDirectScalar>],
) -> Option<IntegerValue> {
    let KnownDirectScalar::Integer(value) = evaluate_direct_expression(expression, parameters)?
    else {
        return None;
    };
    Some(value)
}

fn evaluate_lowered_integer_binary(
    kind: LoweredIntegerBinaryKind,
    scalar_type: ScalarType,
    count_type: ScalarType,
    left: IntegerValue,
    right: IntegerValue,
) -> Option<IntegerValue> {
    let ScalarType::Integer(integer_type) = scalar_type else {
        return None;
    };
    match kind {
        LoweredIntegerBinaryKind::BitwiseAnd => integer_type.bitwise_and(left, right),
        LoweredIntegerBinaryKind::BitwiseOr => integer_type.bitwise_or(left, right),
        LoweredIntegerBinaryKind::BitwiseXor => integer_type.bitwise_xor(left, right),
        LoweredIntegerBinaryKind::WrappingShiftLeft => {
            let ScalarType::Integer(count_type) = count_type else {
                return None;
            };
            integer_type.wrapping_shift_left(left, count_type, right)
        }
        LoweredIntegerBinaryKind::WrappingShiftRight => {
            let ScalarType::Integer(count_type) = count_type else {
                return None;
            };
            integer_type.wrapping_shift_right(left, count_type, right)
        }
        LoweredIntegerBinaryKind::ExactShiftLeft => {
            let ScalarType::Integer(count_type) = count_type else {
                return None;
            };
            integer_type.exact_shift_left(left, count_type, right)
        }
        LoweredIntegerBinaryKind::ExactShiftRight => {
            let ScalarType::Integer(count_type) = count_type else {
                return None;
            };
            integer_type.exact_shift_right(left, count_type, right)
        }
        LoweredIntegerBinaryKind::ExactAdd => integer_type.exact_add(left, right),
        LoweredIntegerBinaryKind::ExactSubtract => integer_type.exact_sub(left, right),
        LoweredIntegerBinaryKind::ExactMultiply => integer_type.exact_mul(left, right),
        LoweredIntegerBinaryKind::ExactDivide => integer_type.exact_div(left, right),
        LoweredIntegerBinaryKind::ExactRemainder => integer_type.exact_rem(left, right),
        LoweredIntegerBinaryKind::WrappingDivide => integer_type.wrapping_div(left, right),
        LoweredIntegerBinaryKind::WrappingRemainder => integer_type.wrapping_rem(left, right),
        LoweredIntegerBinaryKind::SaturatingDivide => integer_type.saturating_div(left, right),
        LoweredIntegerBinaryKind::SaturatingRemainder => integer_type.saturating_rem(left, right),
        LoweredIntegerBinaryKind::WrappingAdd => integer_type.wrapping_add(left, right),
        LoweredIntegerBinaryKind::SaturatingAdd => integer_type.saturating_add(left, right),
        LoweredIntegerBinaryKind::WrappingSubtract => integer_type.wrapping_sub(left, right),
        LoweredIntegerBinaryKind::SaturatingSubtract => integer_type.saturating_sub(left, right),
        LoweredIntegerBinaryKind::WrappingMultiply => integer_type.wrapping_mul(left, right),
        LoweredIntegerBinaryKind::SaturatingMultiply => integer_type.saturating_mul(left, right),
    }
}

fn evaluate_compile_known_boolean_expression(
    expression: &LoweredBooleanReturnExpression,
    parameters: &[Option<KnownDirectScalar>],
) -> Option<bool> {
    match expression {
        LoweredBooleanReturnExpression::Constant { value } => Some(*value),
        LoweredBooleanReturnExpression::Parameter { position }
        | LoweredBooleanReturnExpression::Local { position } => {
            let KnownDirectScalar::Boolean(value) = parameters.get(*position).copied().flatten()?
            else {
                return None;
            };
            Some(value)
        }
        LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. }
        | LoweredBooleanReturnExpression::StructuralField { .. } => None,
        LoweredBooleanReturnExpression::Not { operand } => Some(
            !evaluate_compile_known_boolean_expression(operand, parameters)?,
        ),
        LoweredBooleanReturnExpression::Equal { left, right } => Some(
            evaluate_compile_known_boolean_expression(left, parameters)?
                == evaluate_compile_known_boolean_expression(right, parameters)?,
        ),
        LoweredBooleanReturnExpression::IntegerComparison { kind, left, right } => {
            let ScalarType::Integer(integer_type) = left.scalar_type() else {
                return None;
            };
            let left = evaluate_integer_direct_expression(left, parameters)?;
            let right = evaluate_integer_direct_expression(right, parameters)?;
            match kind {
                LoweredIntegerComparisonKind::Equal => Some(left == right),
                LoweredIntegerComparisonKind::LessThan => {
                    Some(integer_type.compare(left, right)?.is_lt())
                }
                LoweredIntegerComparisonKind::LessOrEqual => {
                    Some(!integer_type.compare(left, right)?.is_gt())
                }
            }
        }
        LoweredBooleanReturnExpression::And { left, right } => {
            let left = evaluate_compile_known_boolean_expression(left, parameters)?;
            if left {
                evaluate_compile_known_boolean_expression(right, parameters)
            } else {
                Some(false)
            }
        }
        LoweredBooleanReturnExpression::Or { left, right } => {
            let left = evaluate_compile_known_boolean_expression(left, parameters)?;
            if left {
                Some(true)
            } else {
                evaluate_compile_known_boolean_expression(right, parameters)
            }
        }
    }
}

fn lower_content_evidence(
    checked: &CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    state: psi_symbols::SymbolHandle,
) -> Result<
    (
        LoweredContentIdentityReshuffles,
        LoweredContentPartitionCompositions,
    ),
    LoweringError,
> {
    let identity_facts = checked
        .facts
        .qualifications
        .content
        .identity_reshuffles
        .iter()
        .filter(|fact| fact.machine_symbol == machine && fact.state_symbol == state)
        .cloned()
        .collect::<Vec<_>>();
    let mut identity_reshuffles = lower_content_identity_reshuffles(&identity_facts)?;
    let partition_facts = checked
        .facts
        .qualifications
        .content
        .partition_compositions
        .iter()
        .filter(|fact| fact.machine_symbol == machine && fact.state_symbol == state)
        .cloned()
        .collect::<Vec<_>>();
    let partition_compositions =
        lower_content_partition_compositions(&partition_facts, &mut identity_reshuffles)?;
    Ok((identity_reshuffles, partition_compositions))
}

fn closed_scalar_contract_plan(
    checked: &CheckedTrees,
    machine: psi_symbols::SymbolHandle,
) -> Result<&ClosedScalarValueContractPlan, LoweringError> {
    checked
        .facts
        .contract_plans
        .for_machine(machine)
        .map(|plan| &plan.closed_scalar_values)
        .ok_or(LoweringError::Unsupported(
            "machine has no source-independent checked contract plan",
        ))
}

fn validate_closed_scalar_contract(
    checked: &CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    result_type: ScalarType,
    expected_value: Option<KnownDirectScalar>,
    allow_crash_contracts: bool,
) -> Result<KnownDirectScalar, LoweringError> {
    let contract = closed_scalar_contract_plan(checked, machine)?;
    let ([Some(requires)], [Some(ensures)]) = (contract.requires(), contract.ensures()) else {
        return unsupported("machine must have exactly one requires and one ensures clause");
    };
    if contract.has_other_clauses() || (!allow_crash_contracts && contract.has_crash_clauses()) {
        return unsupported("machine must have exactly one requires and one ensures clause");
    }
    let (requires, ensures) = match (result_type, requires, ensures) {
        (
            ScalarType::Boolean,
            ClosedScalarContractValue::Boolean(requires),
            ClosedScalarContractValue::Boolean(ensures),
        ) => (
            KnownDirectScalar::Boolean(*requires),
            KnownDirectScalar::Boolean(*ensures),
        ),
        (
            ScalarType::Integer(_),
            ClosedScalarContractValue::Integer(requires),
            ClosedScalarContractValue::Integer(ensures),
        ) => (
            KnownDirectScalar::Integer(integer_value(requires, result_type)?),
            KnownDirectScalar::Integer(integer_value(ensures, result_type)?),
        ),
        _ => return unsupported("contract scalar type must match the machine result type"),
    };
    if requires != ensures {
        return unsupported("requires and ensures must carry the same closed equality");
    }
    if expected_value.is_some_and(|expected| expected != requires) {
        return match result_type {
            ScalarType::Boolean => {
                unsupported("Boolean contract literal must match the compile-known result")
            }
            ScalarType::Integer(_) => {
                unsupported("contract literals must equal the executed literal")
            }
        };
    }
    Ok(requires)
}

fn integer_scalar_type(primitive: PrimitiveType) -> Result<ScalarType, LoweringError> {
    if primitive == PrimitiveType::Addr {
        return IntegerType::address(64)
            .map(ScalarType::Integer)
            .map_err(|_| LoweringError::InvalidPsiIntegerType);
    }
    let (sign, bits) = match primitive {
        PrimitiveType::I8 => (IntegerSign::Signed, 8),
        PrimitiveType::I16 => (IntegerSign::Signed, 16),
        PrimitiveType::I32 => (IntegerSign::Signed, 32),
        PrimitiveType::I64 => (IntegerSign::Signed, 64),
        PrimitiveType::U8 => (IntegerSign::Unsigned, 8),
        PrimitiveType::U16 => (IntegerSign::Unsigned, 16),
        PrimitiveType::U32 => (IntegerSign::Unsigned, 32),
        PrimitiveType::U64 => (IntegerSign::Unsigned, 64),
        PrimitiveType::Addr => unreachable!("address carrier handled above"),
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => {
            return unsupported("only primitive integers are supported");
        }
    };
    IntegerType::new(sign, bits)
        .map(ScalarType::Integer)
        .map_err(|_| LoweringError::InvalidPsiIntegerType)
}

fn terminal_scalar_type(primitive: PrimitiveType) -> Result<ScalarType, LoweringError> {
    match primitive {
        PrimitiveType::Bool => Ok(ScalarType::Boolean),
        primitive => integer_scalar_type(primitive),
    }
}

fn integer_landing_scalar_type(
    literal: &psi_numerics::literals::IntegerLiteral,
) -> Result<ScalarType, LoweringError> {
    use psi_numerics::literals::LandedIntegerType;

    let primitive = match literal
        .landing()
        .ok_or(LoweringError::UnlandedIntegerLiteral)?
        .landed_type
    {
        LandedIntegerType::I8 => PrimitiveType::I8,
        LandedIntegerType::I16 => PrimitiveType::I16,
        LandedIntegerType::I32 => PrimitiveType::I32,
        LandedIntegerType::I64 => PrimitiveType::I64,
        LandedIntegerType::U8 => PrimitiveType::U8,
        LandedIntegerType::U16 => PrimitiveType::U16,
        LandedIntegerType::U32 => PrimitiveType::U32,
        LandedIntegerType::U64 => PrimitiveType::U64,
        LandedIntegerType::Addr => PrimitiveType::Addr,
    };
    integer_scalar_type(primitive)
}

fn integer_value(
    literal: &psi_numerics::literals::IntegerLiteral,
    scalar_type: ScalarType,
) -> Result<IntegerValue, LoweringError> {
    let ScalarType::Integer(integer_type) = scalar_type else {
        return Err(LoweringError::InvalidPsiIntegerType);
    };
    let landing = literal
        .landing()
        .ok_or(LoweringError::UnlandedIntegerLiteral)?;
    if landing.landed_type.bit_width() != u32::from(integer_type.bits())
        || landing.landed_type.is_signed() != (integer_type.sign() == IntegerSign::Signed)
    {
        return Err(LoweringError::IntegerLandingMismatch);
    }
    let value = match integer_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(
            literal
                .value_i64()
                .map(i128::from)
                .ok_or(LoweringError::IntegerLiteralOutsideSupportedMagnitude)?,
        ),
        IntegerSign::Unsigned => IntegerValue::Unsigned(
            literal
                .value_u64()
                .map(u128::from)
                .ok_or(LoweringError::IntegerLiteralOutsideSupportedMagnitude)?,
        ),
    };
    if !integer_type.admits(value) {
        return Err(LoweringError::IntegerLiteralOutsidePsiType);
    }
    Ok(value)
}

#[allow(clippy::too_many_arguments)]
fn bind_boolean_decision<F>(
    decision: LoweredBooleanDecision,
    continuation: &F,
) -> LoweredBooleanDecision
where
    F: Fn(&LoweredBooleanReturnExpression) -> LoweredBooleanDecision,
{
    match decision {
        LoweredBooleanDecision::Value(expression) => continuation(&expression),
        LoweredBooleanDecision::Test {
            condition,
            when_true,
            when_false,
        } => LoweredBooleanDecision::Test {
            condition,
            when_true: Box::new(bind_boolean_decision(*when_true, continuation)),
            when_false: Box::new(bind_boolean_decision(*when_false, continuation)),
        },
    }
}

fn branch_boolean_decision(
    decision: LoweredBooleanDecision,
    when_true: LoweredBooleanDecision,
    when_false: LoweredBooleanDecision,
) -> LoweredBooleanDecision {
    match decision {
        LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant { value }) => {
            if value {
                when_true
            } else {
                when_false
            }
        }
        LoweredBooleanDecision::Value(condition) => LoweredBooleanDecision::Test {
            condition,
            when_true: Box::new(when_true),
            when_false: Box::new(when_false),
        },
        LoweredBooleanDecision::Test {
            condition,
            when_true: nested_true,
            when_false: nested_false,
        } => LoweredBooleanDecision::Test {
            condition,
            when_true: Box::new(branch_boolean_decision(
                *nested_true,
                when_true.clone(),
                when_false.clone(),
            )),
            when_false: Box::new(branch_boolean_decision(
                *nested_false,
                when_true,
                when_false,
            )),
        },
    }
}

fn lower_boolean_control_decision(
    expression: &LoweredBooleanReturnExpression,
    when_true: LoweredBooleanDecision,
    when_false: LoweredBooleanDecision,
) -> LoweredBooleanDecision {
    match expression {
        LoweredBooleanReturnExpression::And { left, right } => {
            let right = lower_boolean_control_decision(right, when_true, when_false.clone());
            lower_boolean_control_decision(left, right, when_false)
        }
        LoweredBooleanReturnExpression::Or { left, right } => {
            let right = lower_boolean_control_decision(right, when_true.clone(), when_false);
            lower_boolean_control_decision(left, when_true, right)
        }
        LoweredBooleanReturnExpression::Not { operand } if contains_short_circuit(operand) => {
            lower_boolean_control_decision(operand, when_false, when_true)
        }
        expression if contains_short_circuit(expression) => branch_boolean_decision(
            lower_boolean_value_decision(expression),
            when_true,
            when_false,
        ),
        expression => LoweredBooleanDecision::Test {
            condition: expression.clone(),
            when_true: Box::new(when_true),
            when_false: Box::new(when_false),
        },
    }
}

fn lower_boolean_value_decision(
    expression: &LoweredBooleanReturnExpression,
) -> LoweredBooleanDecision {
    if !contains_short_circuit(expression) {
        return LoweredBooleanDecision::Value(expression.clone());
    }
    match expression {
        LoweredBooleanReturnExpression::And { .. } | LoweredBooleanReturnExpression::Or { .. } => {
            lower_boolean_control_decision(
                expression,
                LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant {
                    value: true,
                }),
                LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant {
                    value: false,
                }),
            )
        }
        LoweredBooleanReturnExpression::Not { operand } => {
            bind_boolean_decision(lower_boolean_value_decision(operand), &|operand| {
                LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Not {
                    operand: Box::new(operand.clone()),
                })
            })
        }
        LoweredBooleanReturnExpression::Equal { left, right } => {
            bind_boolean_decision(lower_boolean_value_decision(left), &|left| {
                bind_boolean_decision(lower_boolean_value_decision(right), &|right| {
                    LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Equal {
                        left: Box::new(left.clone()),
                        right: Box::new(right.clone()),
                    })
                })
            })
        }
        LoweredBooleanReturnExpression::Constant { .. }
        | LoweredBooleanReturnExpression::Parameter { .. }
        | LoweredBooleanReturnExpression::Local { .. }
        | LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. }
        | LoweredBooleanReturnExpression::StructuralField { .. }
        | LoweredBooleanReturnExpression::IntegerComparison { .. } => {
            unreachable!("non-short-circuit expressions return above")
        }
    }
}

fn boolean_decision_block_count(decision: &LoweredBooleanDecision) -> usize {
    match decision {
        LoweredBooleanDecision::Value(_) => 1,
        LoweredBooleanDecision::Test {
            when_true,
            when_false,
            ..
        } => 1 + boolean_decision_block_count(when_true) + boolean_decision_block_count(when_false),
    }
}

fn boolean_decision_test_count(decision: &LoweredBooleanDecision) -> usize {
    match decision {
        LoweredBooleanDecision::Value(_) => 0,
        LoweredBooleanDecision::Test {
            when_true,
            when_false,
            ..
        } => 1 + boolean_decision_test_count(when_true) + boolean_decision_test_count(when_false),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoweredBooleanDecisionTarget {
    block: BlockId,
    arguments: Vec<ValueId>,
}

#[allow(clippy::too_many_arguments)]
fn emit_reserved_boolean_guard_decision_blocks(
    decision: &LoweredBooleanDecision,
    parameters: &[ValueDeclaration],
    block_parameters: Vec<ValueDeclaration>,
    when_true_target: &LoweredBooleanDecisionTarget,
    when_false_target: &LoweredBooleanDecisionTarget,
    first_block_identity: u64,
    next_value_identity: &mut u64,
    next_edge_identity: &mut u64,
    all_operations: &mut OperationBuffer,
    blocks: &mut Vec<Option<Block>>,
) -> LoweredBooleanDecisionTarget {
    match decision {
        LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant { value: true }) => {
            when_true_target.clone()
        }
        LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant {
            value: false,
        }) => when_false_target.clone(),
        LoweredBooleanDecision::Value(_) => {
            unreachable!("guard control decisions end in canonical Boolean choices")
        }
        LoweredBooleanDecision::Test {
            condition,
            when_true,
            when_false,
        } => {
            let block_index = blocks.len();
            let block = block_id(
                first_block_identity
                    .checked_add(
                        u64::try_from(block_index)
                            .expect("reserved guard block count fits a semantic identity"),
                    )
                    .expect("reserved guard block identity advances"),
            );
            blocks.push(None);
            let operation_start = all_operations.len();
            let condition =
                emit_boolean_expression(condition, parameters, next_value_identity, all_operations);
            let operation_end = all_operations.len();
            let true_edge = edge_id(*next_edge_identity);
            let false_edge = edge_id(
                next_edge_identity
                    .checked_add(1)
                    .expect("reserved guard false edge identity advances"),
            );
            *next_edge_identity = next_edge_identity
                .checked_add(2)
                .expect("reserved guard decision edge identities advance");
            let when_true = emit_reserved_boolean_guard_decision_blocks(
                when_true,
                parameters,
                Vec::new(),
                when_true_target,
                when_false_target,
                first_block_identity,
                next_value_identity,
                next_edge_identity,
                all_operations,
                blocks,
            );
            let when_false = emit_reserved_boolean_guard_decision_blocks(
                when_false,
                parameters,
                Vec::new(),
                when_true_target,
                when_false_target,
                first_block_identity,
                next_value_identity,
                next_edge_identity,
                all_operations,
                blocks,
            );
            blocks[block_index] = Some(Block {
                id: block,
                parameters: block_parameters,
                operations: all_operations[operation_start..operation_end].to_vec(),
                terminator: Terminator::Conditional {
                    condition,
                    when_true: SuccessorEdge {
                        edge: true_edge,
                        target: when_true.block,
                        arguments: when_true.arguments,
                        trivial_affine_discards: Vec::new(),
                    },
                    when_false: SuccessorEdge {
                        edge: false_edge,
                        target: when_false.block,
                        arguments: when_false.arguments,
                        trivial_affine_discards: Vec::new(),
                    },
                },
            });
            LoweredBooleanDecisionTarget {
                block,
                arguments: Vec::new(),
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_reserved_boolean_value_blocks(
    decision: &LoweredBooleanDecision,
    parameters: &[ValueDeclaration],
    block_parameters: Vec<ValueDeclaration>,
    exit: LoweredBooleanDecisionExit,
    first_block_identity: u64,
    next_value_identity: &mut u64,
    next_edge_identity: &mut u64,
    all_operations: &mut OperationBuffer,
    blocks: &mut Vec<Option<Block>>,
) -> BlockId {
    let block_index = blocks.len();
    let block = block_id(
        first_block_identity
            .checked_add(
                u64::try_from(block_index)
                    .expect("reserved Boolean return block count fits a semantic identity"),
            )
            .expect("reserved Boolean return block identity advances"),
    );
    blocks.push(None);
    let operation_start = all_operations.len();
    let (terminator, operation_end) = match decision {
        LoweredBooleanDecision::Value(expression) => {
            let value = emit_boolean_expression(
                expression,
                parameters,
                next_value_identity,
                all_operations,
            );
            let edge = edge_id(*next_edge_identity);
            *next_edge_identity = next_edge_identity
                .checked_add(1)
                .expect("reserved Boolean return edge identity advances");
            let terminator = match exit {
                LoweredBooleanDecisionExit::Return => Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge,
                    value,
                },
                LoweredBooleanDecisionExit::Jump { target } => Terminator::Jump {
                    edge,
                    target,
                    arguments: vec![value],
                    trivial_affine_discards: Vec::new(),
                },
            };
            (terminator, all_operations.len())
        }
        LoweredBooleanDecision::Test {
            condition,
            when_true,
            when_false,
        } => {
            let condition =
                emit_boolean_expression(condition, parameters, next_value_identity, all_operations);
            let operation_end = all_operations.len();
            let true_edge = edge_id(*next_edge_identity);
            let false_edge = edge_id(
                next_edge_identity
                    .checked_add(1)
                    .expect("reserved Boolean return false edge identity advances"),
            );
            *next_edge_identity = next_edge_identity
                .checked_add(2)
                .expect("reserved Boolean return decision edges advance");
            let when_true = emit_reserved_boolean_value_blocks(
                when_true,
                parameters,
                Vec::new(),
                exit,
                first_block_identity,
                next_value_identity,
                next_edge_identity,
                all_operations,
                blocks,
            );
            let when_false = emit_reserved_boolean_value_blocks(
                when_false,
                parameters,
                Vec::new(),
                exit,
                first_block_identity,
                next_value_identity,
                next_edge_identity,
                all_operations,
                blocks,
            );
            (
                Terminator::Conditional {
                    condition,
                    when_true: SuccessorEdge {
                        edge: true_edge,
                        target: when_true,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                    when_false: SuccessorEdge {
                        edge: false_edge,
                        target: when_false,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
                operation_end,
            )
        }
    };
    blocks[block_index] = Some(Block {
        id: block,
        parameters: block_parameters,
        operations: all_operations[operation_start..operation_end].to_vec(),
        terminator,
    });
    block
}

#[allow(clippy::too_many_arguments)]
fn emit_inlined_boolean_value_blocks(
    decision: &LoweredBooleanDecision,
    parameters: &[ValueDeclaration],
    block_parameters: Vec<ValueDeclaration>,
    exit: LoweredBooleanDecisionExit,
    source_block: BlockId,
    first_synthetic_block: BlockId,
    next_value_identity: &mut u64,
    next_edge_identity: &mut u64,
    all_operations: &mut OperationBuffer,
) -> (Block, Vec<Block>) {
    let first_reserved_identity = first_synthetic_block
        .get()
        .checked_sub(1)
        .expect("synthetic Boolean blocks follow source blocks");
    let mut reserved = Vec::new();
    let entry = emit_reserved_boolean_value_blocks(
        decision,
        parameters,
        block_parameters,
        exit,
        first_reserved_identity,
        next_value_identity,
        next_edge_identity,
        all_operations,
        &mut reserved,
    );
    assert_eq!(entry.get(), first_reserved_identity);
    let mut reserved = reserved
        .into_iter()
        .map(|block| block.expect("every inlined Boolean value block is finalized"));
    let mut root = reserved
        .next()
        .expect("short-circuit Boolean value has a decision root");
    root.id = source_block;
    (root, reserved.collect())
}

#[allow(clippy::too_many_arguments)]
fn emit_inlined_boolean_guard_blocks(
    decision: &LoweredBooleanDecision,
    parameters: &[ValueDeclaration],
    block_parameters: Vec<ValueDeclaration>,
    when_true_target: &LoweredBooleanDecisionTarget,
    when_false_target: &LoweredBooleanDecisionTarget,
    source_block: BlockId,
    first_synthetic_block: BlockId,
    next_value_identity: &mut u64,
    next_edge_identity: &mut u64,
    all_operations: &mut OperationBuffer,
) -> (Block, Vec<Block>) {
    let first_reserved_identity = first_synthetic_block
        .get()
        .checked_sub(1)
        .expect("synthetic Boolean blocks follow source blocks");
    let mut reserved = Vec::new();
    let entry = emit_reserved_boolean_guard_decision_blocks(
        decision,
        parameters,
        block_parameters,
        when_true_target,
        when_false_target,
        first_reserved_identity,
        next_value_identity,
        next_edge_identity,
        all_operations,
        &mut reserved,
    );
    assert_eq!(entry.block.get(), first_reserved_identity);
    assert!(entry.arguments.is_empty());
    let mut reserved = reserved
        .into_iter()
        .map(|block| block.expect("every inlined Boolean guard block is finalized"));
    let mut root = reserved
        .next()
        .expect("short-circuit Boolean guard has a decision root");
    root.id = source_block;
    (root, reserved.collect())
}

#[allow(clippy::too_many_arguments)]
fn emit_reserved_boolean_tuple_stage_blocks(
    decision: &LoweredBooleanDecision,
    parameters: &[ValueDeclaration],
    block_parameters: Vec<ValueDeclaration>,
    next_stage: BlockId,
    carried_arguments: &[ValueId],
    first_block_identity: u64,
    next_value_identity: &mut u64,
    next_edge_identity: &mut u64,
    all_operations: &mut OperationBuffer,
    blocks: &mut Vec<Option<Block>>,
) -> BlockId {
    let block_index = blocks.len();
    let block = block_id(
        first_block_identity
            .checked_add(
                u64::try_from(block_index)
                    .expect("reserved Boolean tuple block count fits a semantic identity"),
            )
            .expect("reserved Boolean tuple block identity advances"),
    );
    blocks.push(None);
    let operation_start = all_operations.len();
    let (terminator, operation_end) = match decision {
        LoweredBooleanDecision::Value(expression) => {
            let value = emit_boolean_expression(
                expression,
                parameters,
                next_value_identity,
                all_operations,
            );
            let edge = edge_id(*next_edge_identity);
            *next_edge_identity = next_edge_identity
                .checked_add(1)
                .expect("reserved Boolean tuple value edge identity advances");
            let mut arguments = carried_arguments.to_vec();
            arguments.push(value);
            (
                Terminator::Jump {
                    edge,
                    target: next_stage,
                    arguments,
                    trivial_affine_discards: Vec::new(),
                },
                all_operations.len(),
            )
        }
        LoweredBooleanDecision::Test {
            condition,
            when_true,
            when_false,
        } => {
            let condition =
                emit_boolean_expression(condition, parameters, next_value_identity, all_operations);
            let operation_end = all_operations.len();
            let true_edge = edge_id(*next_edge_identity);
            let false_edge = edge_id(
                next_edge_identity
                    .checked_add(1)
                    .expect("reserved Boolean tuple false edge identity advances"),
            );
            *next_edge_identity = next_edge_identity
                .checked_add(2)
                .expect("reserved Boolean tuple decision edges advance");
            let when_true = emit_reserved_boolean_tuple_stage_blocks(
                when_true,
                parameters,
                Vec::new(),
                next_stage,
                carried_arguments,
                first_block_identity,
                next_value_identity,
                next_edge_identity,
                all_operations,
                blocks,
            );
            let when_false = emit_reserved_boolean_tuple_stage_blocks(
                when_false,
                parameters,
                Vec::new(),
                next_stage,
                carried_arguments,
                first_block_identity,
                next_value_identity,
                next_edge_identity,
                all_operations,
                blocks,
            );
            (
                Terminator::Conditional {
                    condition,
                    when_true: SuccessorEdge {
                        edge: true_edge,
                        target: when_true,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                    when_false: SuccessorEdge {
                        edge: false_edge,
                        target: when_false,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
                operation_end,
            )
        }
    };
    blocks[block_index] = Some(Block {
        id: block,
        parameters: block_parameters,
        operations: all_operations[operation_start..operation_end].to_vec(),
        terminator,
    });
    block
}

#[allow(clippy::too_many_arguments)]
fn build_scalar_conditional_target(
    target: usize,
    arguments: &[LoweredDirectExpression],
    current_parameters: &[ValueDeclaration],
    current_parameter_types: &[ScalarType],
    next_block_identity: &mut u64,
    next_value_identity: &mut u64,
    pending_blocks: &mut Vec<PendingNestedBlockGroup>,
    identity_base: u64,
) -> LoweredBooleanDecisionTarget {
    let direct_arguments = arguments
        .iter()
        .map(|argument| match argument {
            LoweredDirectExpression::Parameter { position, .. }
            | LoweredDirectExpression::Local { position, .. } => {
                Some(current_parameters[*position].id)
            }
            LoweredDirectExpression::Boolean { expression } => match expression.as_ref() {
                LoweredBooleanReturnExpression::Parameter { position }
                | LoweredBooleanReturnExpression::Local { position } => {
                    Some(current_parameters[*position].id)
                }
                _ => None,
            },
            LoweredDirectExpression::IntegerLiteral { .. }
            | LoweredDirectExpression::IntegerBinary { .. }
            | LoweredDirectExpression::IntegerBitwiseNot { .. }
            | LoweredDirectExpression::IntegerWiden { .. }
            | LoweredDirectExpression::IntegerExactCast { .. } => None,
        })
        .collect::<Option<Vec<_>>>();
    if let Some(arguments) = direct_arguments {
        return LoweredBooleanDecisionTarget {
            block: scalar_source_block(identity_base, target),
            arguments,
        };
    }

    if arguments
        .iter()
        .any(direct_expression_contains_short_circuit)
    {
        let first_id = block_id(*next_block_identity);
        let reserved_block_count = arguments
            .iter()
            .map(|argument| match argument {
                LoweredDirectExpression::Boolean { expression }
                    if contains_short_circuit(expression) =>
                {
                    boolean_decision_block_count(&lower_boolean_value_decision(expression))
                }
                _ => 1,
            })
            .sum::<usize>()
            .checked_add(1)
            .expect("mixed tuple convergence block count advances");
        *next_block_identity = next_block_identity
            .checked_add(
                u64::try_from(reserved_block_count)
                    .expect("mixed tuple block count fits a semantic identity"),
            )
            .expect("mixed tuple block identities advance");
        let stage_parameters = (0..=arguments.len())
            .map(|completed_argument_count| {
                let mut scalar_types = current_parameter_types.to_vec();
                scalar_types.extend(
                    arguments[..completed_argument_count]
                        .iter()
                        .map(LoweredDirectExpression::scalar_type),
                );
                scalar_types
                    .into_iter()
                    .map(|scalar_type| {
                        let parameter = ValueDeclaration {
                            id: value_id(*next_value_identity),
                            scalar_type,
                        };
                        *next_value_identity = next_value_identity
                            .checked_add(1)
                            .expect("mixed tuple parameter identities advance");
                        parameter
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        pending_blocks.push(PendingNestedBlockGroup::TupleBinding(
            PendingMixedTupleBindingBlocks {
                first_id,
                original_parameter_count: current_parameters.len(),
                arguments: arguments.to_vec(),
                stage_parameters,
                target: scalar_source_block(identity_base, target),
            },
        ));
        return LoweredBooleanDecisionTarget {
            block: first_id,
            arguments: current_parameters
                .iter()
                .map(|parameter| parameter.id)
                .collect(),
        };
    }

    let id = block_id(*next_block_identity);
    *next_block_identity = next_block_identity
        .checked_add(1)
        .expect("conditional binding block identities advance");
    let parameters = current_parameter_types
        .iter()
        .map(|scalar_type| {
            let parameter = ValueDeclaration {
                id: value_id(*next_value_identity),
                scalar_type: *scalar_type,
            };
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("conditional binding parameter identities advance");
            parameter
        })
        .collect::<Vec<_>>();
    pending_blocks.push(PendingNestedBlockGroup::ConditionalBinding(
        PendingConditionalBindingBlock {
            id,
            parameters,
            target: scalar_source_block(identity_base, target),
            arguments: arguments.to_vec(),
        },
    ));
    LoweredBooleanDecisionTarget {
        block: id,
        arguments: current_parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect(),
    }
}

fn scalar_source_block(identity_base: u64, state: usize) -> BlockId {
    block_id(
        identity_base
            .checked_add(u64::try_from(state).expect("state index fits a semantic identity"))
            .and_then(|identity| identity.checked_add(1))
            .expect("state block identity fits the machine namespace"),
    )
}

fn lower_checked_crash_route_buckets(
    buckets: &[psi_checked_trees::CrashRouteBucket],
    parameters: &[ValueDeclaration],
) -> Result<Vec<psi_terminal::CrashRouteBucket>, LoweringError> {
    buckets
        .iter()
        .map(|bucket| {
            let mut alternatives = bucket
                .alternative_guards()
                .iter()
                .map(|guard| match guard {
                    psi_checked_trees::CrashRouteGuard::Truth => {
                        Ok(psi_terminal::CrashRouteGuard::Truth)
                    }
                    psi_checked_trees::CrashRouteGuard::Predicate(predicate) => {
                        let expression = predicate.scalar_expression().ok_or(
                            LoweringError::Unsupported(
                                "guarded crash route is outside structured scalar predicate lowering",
                            ),
                        )?;
                        Ok(psi_terminal::CrashRouteGuard::Predicate(
                            psi_terminal::CrashPredicateTerm::new(
                                checked_boolean_proposition(expression, parameters)?,
                            ),
                        ))
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            alternatives.sort();
            alternatives.dedup();
            Ok(psi_terminal::CrashRouteBucket {
                cause: match bucket.cause() {
                    psi_checked_trees::CrashCause::Trap => TerminalCrashCause::Trap,
                    psi_checked_trees::CrashCause::Abort => TerminalCrashCause::Abort,
                },
                alternatives,
            })
        })
        .collect()
}

fn lower_structural_member_term(
    parameter_position: u32,
    path: &[String],
    expected: ScalarType,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<ScalarTerm, LoweringError> {
    if path.is_empty() {
        return unsupported("structural scalar contract has an empty member path");
    }
    let parameter = parameters
        .iter()
        .find(|parameter| parameter.position == parameter_position)
        .ok_or(LoweringError::Unsupported(
            "structural scalar contract names a non-structural parameter",
        ))?;
    let mut structural_type = parameter.structural_type;
    let mut terminal_path = Vec::with_capacity(path.len());
    for (index, identity) in path.iter().enumerate() {
        let declaration = structural_types
            .iter()
            .find(|declaration| declaration.id == structural_type)
            .ok_or(LoweringError::Unsupported(
                "structural scalar contract path type is absent",
            ))?;
        let StructuralTypeShape::Record { fields } = &declaration.shape else {
            return unsupported("structural scalar contract path receiver is not a record");
        };
        let field = fields
            .iter()
            .find(|candidate| candidate.identity == *identity)
            .filter(|field| !field.relevance.is_erased())
            .ok_or(LoweringError::Unsupported(
                "structural scalar contract path field is absent or erased",
            ))?;
        terminal_path.push(CanonicalStructuralPathSegment::Field(field.id));
        let is_last = index + 1 == path.len();
        match (&field.field_type, is_last) {
            (StructuralFieldType::Structural(next), false) => structural_type = *next,
            (StructuralFieldType::Scalar(actual), true) if *actual == expected => {}
            _ => {
                return unsupported(
                    "structural scalar contract path does not end at the retained scalar type",
                );
            }
        }
    }
    Ok(match expected {
        ScalarType::Boolean => ScalarTerm::boolean_field_path(parameter.place, terminal_path),
        ScalarType::Integer(integer_type) => {
            ScalarTerm::integer_field_path(parameter.place, terminal_path, integer_type)
        }
    })
}

fn lower_structural_runtime_requirement(
    expression: &CheckedBooleanExpression,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<Proposition, LoweringError> {
    fn integer_term(
        expression: &CheckedScalarExpression,
        parameters: &[StructuralParameterDeclaration],
        structural_types: &[StructuralTypeDeclaration],
    ) -> Result<ScalarTerm, LoweringError> {
        match expression {
            CheckedScalarExpression::StructuralParameterField {
                parameter_position,
                path,
                primitive_type,
            } => {
                let ScalarType::Integer(integer_type) = integer_scalar_type(*primitive_type)?
                else {
                    return unsupported("structural runtime requirement member is not an integer");
                };
                lower_structural_member_term(
                    *parameter_position,
                    path,
                    ScalarType::Integer(integer_type),
                    parameters,
                    structural_types,
                )
            }
            CheckedScalarExpression::IntegerLiteral { literal } => {
                let scalar_type = integer_landing_scalar_type(literal)?;
                let ScalarType::Integer(integer_type) = scalar_type else {
                    return unsupported("structural runtime requirement literal is not an integer");
                };
                ScalarTerm::integer(integer_type, integer_value(literal, scalar_type)?)
                    .map_err(LoweringError::InvalidCrashPredicate)
            }
            _ => unsupported(
                "structural runtime requirements currently admit only integer members and literals",
            ),
        }
    }

    let CheckedBooleanExpression::IntegerComparison { kind, left, right } = expression else {
        return unsupported(
            "structural runtime divisor evidence must be an integer comparison requirement",
        );
    };
    let left = integer_term(left, parameters, structural_types)?;
    let right = integer_term(right, parameters, structural_types)?;
    match kind {
        CheckedIntegerComparisonKind::Equal => Ok(Proposition::Equal(left, right)),
        CheckedIntegerComparisonKind::LessThan => Ok(Proposition::LessThan(left, right)),
        CheckedIntegerComparisonKind::LessOrEqual => Ok(Proposition::LessOrEqual(left, right)),
    }
}

fn safe_exact_structural_divisor(
    integer_type: IntegerType,
    dividend: &ScalarTerm,
    divisor: &ScalarTerm,
    requirements: &[Proposition],
) -> bool {
    match divisor {
        ScalarTerm::Integer {
            scalar_type,
            value: IntegerValue::Unsigned(value),
        } => return *scalar_type == integer_type && *value != 0,
        ScalarTerm::Integer {
            scalar_type,
            value: IntegerValue::Signed(value),
        } => return *scalar_type == integer_type && *value != 0 && *value != -1,
        _ => {}
    }

    let one = match integer_type.sign() {
        IntegerSign::Unsigned => IntegerValue::Unsigned(1),
        IntegerSign::Signed => IntegerValue::Signed(1),
    };
    if let Ok(one) = ScalarTerm::integer(integer_type, one)
        && requirements.contains(&Proposition::LessOrEqual(one, divisor.clone()))
    {
        return true;
    }
    if integer_type.sign() != IntegerSign::Signed {
        return false;
    }
    if let Ok(negative_two) = ScalarTerm::integer(integer_type, IntegerValue::Signed(-2))
        && requirements.contains(&Proposition::LessOrEqual(divisor.clone(), negative_two))
    {
        return true;
    }
    let Ok(negative_one) = ScalarTerm::integer(integer_type, IntegerValue::Signed(-1)) else {
        return false;
    };
    if !requirements.contains(&Proposition::LessOrEqual(divisor.clone(), negative_one)) {
        return false;
    }
    let IntegerValue::Signed(minimum) = integer_type.minimum_value() else {
        unreachable!("signed fixed integer has a signed minimum")
    };
    ScalarTerm::integer(
        integer_type,
        IntegerValue::Signed(minimum.checked_add(1).expect("minimum has a successor")),
    )
    .is_ok_and(|minimum_plus_one| {
        requirements.contains(&Proposition::LessOrEqual(
            minimum_plus_one,
            dividend.clone(),
        ))
    })
}

fn safe_policy_structural_divisor(
    integer_type: IntegerType,
    divisor: &ScalarTerm,
    requirements: &[Proposition],
) -> bool {
    match divisor {
        ScalarTerm::Integer {
            scalar_type,
            value: IntegerValue::Unsigned(value),
        } => return *scalar_type == integer_type && *value != 0,
        ScalarTerm::Integer {
            scalar_type,
            value: IntegerValue::Signed(value),
        } => return *scalar_type == integer_type && *value != 0,
        _ => {}
    }

    let one = match integer_type.sign() {
        IntegerSign::Unsigned => IntegerValue::Unsigned(1),
        IntegerSign::Signed => IntegerValue::Signed(1),
    };
    if ScalarTerm::integer(integer_type, one)
        .is_ok_and(|one| requirements.contains(&Proposition::LessOrEqual(one, divisor.clone())))
    {
        return true;
    }
    if integer_type.sign() != IntegerSign::Signed {
        return false;
    }
    [IntegerValue::Signed(-1), IntegerValue::Signed(-2)]
        .into_iter()
        .filter_map(|bound| ScalarTerm::integer(integer_type, bound).ok())
        .any(|bound| requirements.contains(&Proposition::LessOrEqual(divisor.clone(), bound)))
}

fn nonnegative_shift_count(value: IntegerValue) -> Option<u32> {
    match value {
        IntegerValue::Unsigned(value) => u32::try_from(value).ok(),
        IntegerValue::Signed(value) => u32::try_from(value).ok(),
    }
}

fn exact_structural_shift_maximum_count(
    value_type: IntegerType,
    count_type: IntegerType,
    count: &ScalarTerm,
    requirements: &[Proposition],
) -> Option<u32> {
    if count.scalar_type() != ScalarType::Integer(count_type) {
        return None;
    }
    if let Some((literal_type, literal)) = count.integer_value() {
        let literal = nonnegative_shift_count(literal)?;
        return (literal_type == count_type && literal < u32::from(value_type.bits()))
            .then_some(literal);
    }

    if count_type.sign() == IntegerSign::Signed {
        let zero = ScalarTerm::integer(count_type, IntegerValue::Signed(0)).ok()?;
        if !requirements.contains(&Proposition::LessOrEqual(zero, count.clone())) {
            return None;
        }
    }

    let width = u32::from(value_type.bits());
    let intrinsic_maximum = nonnegative_shift_count(count_type.maximum_value())?;
    if intrinsic_maximum < width {
        return Some(intrinsic_maximum);
    }
    requirements
        .iter()
        .filter_map(|requirement| match requirement {
            Proposition::LessOrEqual(left, right) if left == count => {
                let (right_type, right) = right.integer_value()?;
                let right = nonnegative_shift_count(right)?;
                (right_type == count_type && right < width).then_some(right)
            }
            Proposition::LessThan(left, right) if left == count => {
                let (right_type, right) = right.integer_value()?;
                let right = nonnegative_shift_count(right)?;
                (right_type == count_type && right > 0 && right <= width).then_some(right - 1)
            }
            _ => None,
        })
        .min()
}

fn safe_exact_structural_shift(
    left_shift: bool,
    value_type: IntegerType,
    count_type: IntegerType,
    value: &ScalarTerm,
    count: &ScalarTerm,
    requirements: &[Proposition],
) -> bool {
    if value.scalar_type() != ScalarType::Integer(value_type) {
        return false;
    }
    let Some(maximum_count) =
        exact_structural_shift_maximum_count(value_type, count_type, count, requirements)
    else {
        return false;
    };
    if !left_shift || maximum_count == 0 {
        return true;
    }
    if let Some((literal_type, literal)) = value.integer_value() {
        let maximum_count_value = match count_type.sign() {
            IntegerSign::Signed => IntegerValue::Signed(i128::from(maximum_count)),
            IntegerSign::Unsigned => IntegerValue::Unsigned(u128::from(maximum_count)),
        };
        return literal_type == value_type
            && value_type
                .exact_shift_left(literal, count_type, maximum_count_value)
                .is_some();
    }
    match value_type.sign() {
        IntegerSign::Unsigned => {
            let IntegerValue::Unsigned(maximum) = value_type.maximum_value() else {
                unreachable!("unsigned fixed integer has an unsigned maximum")
            };
            ScalarTerm::integer(value_type, IntegerValue::Unsigned(maximum >> maximum_count))
                .is_ok_and(|maximum| {
                    requirements.contains(&Proposition::LessOrEqual(value.clone(), maximum))
                })
        }
        IntegerSign::Signed => {
            let (IntegerValue::Signed(minimum), IntegerValue::Signed(maximum)) =
                (value_type.minimum_value(), value_type.maximum_value())
            else {
                unreachable!("signed fixed integer has signed bounds")
            };
            let minimum =
                ScalarTerm::integer(value_type, IntegerValue::Signed(minimum >> maximum_count));
            let maximum =
                ScalarTerm::integer(value_type, IntegerValue::Signed(maximum >> maximum_count));
            minimum.is_ok_and(|minimum| {
                requirements.contains(&Proposition::LessOrEqual(minimum, value.clone()))
            }) && maximum.is_ok_and(|maximum| {
                requirements.contains(&Proposition::LessOrEqual(value.clone(), maximum))
            })
        }
    }
}

fn lower_structural_crash_route_buckets(
    buckets: &[psi_checked_trees::CrashRouteBucket],
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
    runtime_requirements: &[Proposition],
) -> Result<Vec<psi_terminal::CrashRouteBucket>, LoweringError> {
    fn checked_member_path(
        expression: &psi_checked_trees::CrashPredicateExpression,
        path: &mut Vec<String>,
    ) -> Option<u32> {
        match expression {
            psi_checked_trees::CrashPredicateExpression::Parameter(position) => Some(*position),
            psi_checked_trees::CrashPredicateExpression::Member { receiver, member } => {
                let parameter = checked_member_path(receiver, path)?;
                path.push(member.clone());
                Some(parameter)
            }
            _ => None,
        }
    }

    fn lower_term(
        expression: &CheckedBooleanExpression,
        parameters: &[StructuralParameterDeclaration],
        structural_types: &[StructuralTypeDeclaration],
        runtime_requirements: &[Proposition],
    ) -> Result<ScalarTerm, LoweringError> {
        fn lower_integer_term(
            expression: &CheckedScalarExpression,
            parameters: &[StructuralParameterDeclaration],
            structural_types: &[StructuralTypeDeclaration],
            runtime_requirements: &[Proposition],
        ) -> Result<ScalarTerm, LoweringError> {
            match expression {
                CheckedScalarExpression::StructuralParameterField {
                    parameter_position,
                    path,
                    primitive_type,
                } => {
                    let ScalarType::Integer(integer_type) = integer_scalar_type(*primitive_type)?
                    else {
                        return unsupported(
                            "structural crash integer member has a non-integer type",
                        );
                    };
                    lower_structural_member_term(
                        *parameter_position,
                        path,
                        ScalarType::Integer(integer_type),
                        parameters,
                        structural_types,
                    )
                }
                CheckedScalarExpression::IntegerLiteral { literal } => {
                    let scalar_type = integer_landing_scalar_type(literal)?;
                    let ScalarType::Integer(integer_type) = scalar_type else {
                        return unsupported(
                            "structural crash integer literal is not fixed-integer",
                        );
                    };
                    ScalarTerm::integer(integer_type, integer_value(literal, scalar_type)?)
                        .map_err(LoweringError::InvalidCrashPredicate)
                }
                CheckedScalarExpression::IntegerBitwiseNot {
                    primitive_type,
                    operand,
                } => {
                    let ScalarType::Integer(integer_type) = integer_scalar_type(*primitive_type)?
                    else {
                        return unsupported("structural crash bitwise-not has a non-integer type");
                    };
                    let operand = lower_integer_term(
                        operand,
                        parameters,
                        structural_types,
                        runtime_requirements,
                    )?;
                    ScalarTerm::integer_bitwise_not(integer_type, operand)
                        .map_err(LoweringError::InvalidCrashPredicate)
                }
                CheckedScalarExpression::IntegerBinary {
                    kind,
                    primitive_type,
                    left,
                    right,
                } if matches!(
                    kind,
                    CheckedIntegerBinaryKind::BitwiseAnd
                        | CheckedIntegerBinaryKind::BitwiseOr
                        | CheckedIntegerBinaryKind::BitwiseXor
                ) =>
                {
                    let ScalarType::Integer(integer_type) = integer_scalar_type(*primitive_type)?
                    else {
                        return unsupported(
                            "structural crash bitwise expression has a non-integer type",
                        );
                    };
                    let left = lower_integer_term(
                        left,
                        parameters,
                        structural_types,
                        runtime_requirements,
                    )?;
                    let right = lower_integer_term(
                        right,
                        parameters,
                        structural_types,
                        runtime_requirements,
                    )?;
                    match kind {
                        CheckedIntegerBinaryKind::BitwiseAnd => {
                            ScalarTerm::integer_bitwise_and(integer_type, left, right)
                        }
                        CheckedIntegerBinaryKind::BitwiseOr => {
                            ScalarTerm::integer_bitwise_or(integer_type, left, right)
                        }
                        CheckedIntegerBinaryKind::BitwiseXor => {
                            ScalarTerm::integer_bitwise_xor(integer_type, left, right)
                        }
                        _ => unreachable!("guarded bitwise kind"),
                    }
                    .map_err(LoweringError::InvalidCrashPredicate)
                }
                CheckedScalarExpression::IntegerBinary {
                    kind,
                    primitive_type,
                    left,
                    right,
                } if matches!(
                    kind,
                    CheckedIntegerBinaryKind::WrappingShiftLeft
                        | CheckedIntegerBinaryKind::WrappingShiftRight
                        | CheckedIntegerBinaryKind::ExactShiftLeft
                        | CheckedIntegerBinaryKind::ExactShiftRight
                ) =>
                {
                    let ScalarType::Integer(value_type) = integer_scalar_type(*primitive_type)?
                    else {
                        return unsupported("structural crash shift has a non-integer value type");
                    };
                    let value = lower_integer_term(
                        left,
                        parameters,
                        structural_types,
                        runtime_requirements,
                    )?;
                    let count = lower_integer_term(
                        right,
                        parameters,
                        structural_types,
                        runtime_requirements,
                    )?;
                    if value.scalar_type() != ScalarType::Integer(value_type) {
                        return unsupported(
                            "structural crash shift value does not match its integer type",
                        );
                    }
                    let ScalarType::Integer(count_type) = count.scalar_type() else {
                        return unsupported("structural crash shift count is not an integer");
                    };
                    if matches!(
                        kind,
                        CheckedIntegerBinaryKind::ExactShiftLeft
                            | CheckedIntegerBinaryKind::ExactShiftRight
                    ) && !safe_exact_structural_shift(
                        matches!(kind, CheckedIntegerBinaryKind::ExactShiftLeft),
                        value_type,
                        count_type,
                        &value,
                        &count,
                        runtime_requirements,
                    ) {
                        return unsupported(
                            "structural crash Exact shift requires explicit terminal count and overflow safety evidence",
                        );
                    }
                    match kind {
                        CheckedIntegerBinaryKind::WrappingShiftLeft => {
                            ScalarTerm::wrapping_integer_shift_left(
                                value_type, count_type, value, count,
                            )
                        }
                        CheckedIntegerBinaryKind::WrappingShiftRight => {
                            ScalarTerm::wrapping_integer_shift_right(
                                value_type, count_type, value, count,
                            )
                        }
                        CheckedIntegerBinaryKind::ExactShiftLeft => {
                            ScalarTerm::exact_integer_shift_left(
                                value_type, count_type, value, count,
                            )
                        }
                        CheckedIntegerBinaryKind::ExactShiftRight => {
                            ScalarTerm::exact_integer_shift_right(
                                value_type, count_type, value, count,
                            )
                        }
                        _ => unreachable!("guarded structural shift kind"),
                    }
                    .map_err(LoweringError::InvalidCrashPredicate)
                }
                CheckedScalarExpression::IntegerBinary {
                    kind,
                    primitive_type,
                    left,
                    right,
                } if matches!(
                    kind,
                    CheckedIntegerBinaryKind::ExactAdd
                        | CheckedIntegerBinaryKind::ExactSubtract
                        | CheckedIntegerBinaryKind::ExactMultiply
                        | CheckedIntegerBinaryKind::ExactDivide
                        | CheckedIntegerBinaryKind::ExactRemainder
                        | CheckedIntegerBinaryKind::WrappingAdd
                        | CheckedIntegerBinaryKind::SaturatingAdd
                        | CheckedIntegerBinaryKind::WrappingSubtract
                        | CheckedIntegerBinaryKind::SaturatingSubtract
                        | CheckedIntegerBinaryKind::WrappingMultiply
                        | CheckedIntegerBinaryKind::SaturatingMultiply
                        | CheckedIntegerBinaryKind::WrappingDivide
                        | CheckedIntegerBinaryKind::WrappingRemainder
                        | CheckedIntegerBinaryKind::SaturatingDivide
                        | CheckedIntegerBinaryKind::SaturatingRemainder
                ) =>
                {
                    let ScalarType::Integer(integer_type) = integer_scalar_type(*primitive_type)?
                    else {
                        return unsupported("structural crash arithmetic has a non-integer type");
                    };
                    let left = Box::new(lower_integer_term(
                        left,
                        parameters,
                        structural_types,
                        runtime_requirements,
                    )?);
                    let right = Box::new(lower_integer_term(
                        right,
                        parameters,
                        structural_types,
                        runtime_requirements,
                    )?);
                    if left.scalar_type() != ScalarType::Integer(integer_type)
                        || right.scalar_type() != ScalarType::Integer(integer_type)
                    {
                        return unsupported(
                            "structural crash arithmetic operands do not match its integer type",
                        );
                    }
                    if matches!(
                        kind,
                        CheckedIntegerBinaryKind::ExactDivide
                            | CheckedIntegerBinaryKind::ExactRemainder
                    ) && !safe_exact_structural_divisor(
                        integer_type,
                        &left,
                        &right,
                        runtime_requirements,
                    ) {
                        return unsupported(
                            "structural crash exact division requires explicit terminal divisor safety evidence",
                        );
                    }
                    if matches!(
                        kind,
                        CheckedIntegerBinaryKind::WrappingDivide
                            | CheckedIntegerBinaryKind::WrappingRemainder
                            | CheckedIntegerBinaryKind::SaturatingDivide
                            | CheckedIntegerBinaryKind::SaturatingRemainder
                    ) && !safe_policy_structural_divisor(
                        integer_type,
                        &right,
                        runtime_requirements,
                    ) {
                        return unsupported(
                            "structural crash policy division requires explicit terminal nonzero-divisor evidence",
                        );
                    }
                    Ok(match kind {
                        CheckedIntegerBinaryKind::ExactAdd => ScalarTerm::ExactIntegerAdd {
                            scalar_type: integer_type,
                            left,
                            right,
                        },
                        CheckedIntegerBinaryKind::ExactSubtract => {
                            ScalarTerm::ExactIntegerSubtract {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::ExactMultiply => {
                            ScalarTerm::ExactIntegerMultiply {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::ExactDivide => ScalarTerm::ExactIntegerDivide {
                            scalar_type: integer_type,
                            left,
                            right,
                        },
                        CheckedIntegerBinaryKind::ExactRemainder => {
                            ScalarTerm::ExactIntegerRemainder {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::WrappingAdd => ScalarTerm::WrappingIntegerAdd {
                            scalar_type: integer_type,
                            left,
                            right,
                        },
                        CheckedIntegerBinaryKind::SaturatingAdd => {
                            ScalarTerm::SaturatingIntegerAdd {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::WrappingSubtract => {
                            ScalarTerm::WrappingIntegerSubtract {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::SaturatingSubtract => {
                            ScalarTerm::SaturatingIntegerSubtract {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::WrappingMultiply => {
                            ScalarTerm::WrappingIntegerMultiply {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::SaturatingMultiply => {
                            ScalarTerm::SaturatingIntegerMultiply {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::WrappingDivide => {
                            ScalarTerm::WrappingIntegerDivide {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::WrappingRemainder => {
                            ScalarTerm::WrappingIntegerRemainder {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::SaturatingDivide => {
                            ScalarTerm::SaturatingIntegerDivide {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::SaturatingRemainder => {
                            ScalarTerm::SaturatingIntegerRemainder {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        _ => unreachable!("guarded structural arithmetic kind"),
                    })
                }
                _ => unsupported(
                    "structural crash integer predicate contains an unsupported operand",
                ),
            }
        }

        match expression {
            CheckedBooleanExpression::Constant(value) => Ok(ScalarTerm::boolean(*value)),
            CheckedBooleanExpression::StructuralParameterField {
                parameter_position,
                path,
            } => lower_structural_member_term(
                *parameter_position,
                path,
                ScalarType::Boolean,
                parameters,
                structural_types,
            ),
            CheckedBooleanExpression::Not(operand) => ScalarTerm::boolean_not(lower_term(
                operand,
                parameters,
                structural_types,
                runtime_requirements,
            )?)
            .map_err(LoweringError::InvalidCrashPredicate),
            CheckedBooleanExpression::Equal { left, right } => ScalarTerm::boolean_equal(
                lower_term(left, parameters, structural_types, runtime_requirements)?,
                lower_term(right, parameters, structural_types, runtime_requirements)?,
            )
            .map_err(LoweringError::InvalidCrashPredicate),
            CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
                let left =
                    lower_integer_term(left, parameters, structural_types, runtime_requirements)?;
                let right =
                    lower_integer_term(right, parameters, structural_types, runtime_requirements)?;
                let ScalarType::Integer(integer_type) = left.scalar_type() else {
                    return unsupported("structural crash comparison operand is not an integer");
                };
                match kind {
                    CheckedIntegerComparisonKind::Equal => {
                        ScalarTerm::integer_equal(integer_type, left, right)
                    }
                    CheckedIntegerComparisonKind::LessThan => {
                        ScalarTerm::integer_less_than(integer_type, left, right)
                    }
                    CheckedIntegerComparisonKind::LessOrEqual => {
                        ScalarTerm::integer_less_or_equal(integer_type, left, right)
                    }
                }
                .map_err(LoweringError::InvalidCrashPredicate)
            }
            CheckedBooleanExpression::Parameter { .. }
            | CheckedBooleanExpression::Local { .. }
            | CheckedBooleanExpression::And { .. }
            | CheckedBooleanExpression::Or { .. } => {
                unsupported("structural crash route contains an unsupported Boolean term")
            }
        }
    }

    fn lower_proposition(
        expression: &CheckedBooleanExpression,
        parameters: &[StructuralParameterDeclaration],
        structural_types: &[StructuralTypeDeclaration],
        runtime_requirements: &[Proposition],
    ) -> Result<Proposition, LoweringError> {
        if let CheckedBooleanExpression::And { left, right }
        | CheckedBooleanExpression::Or { left, right } = expression
        {
            let conjunction = matches!(expression, CheckedBooleanExpression::And { .. });
            let mut leaves = Vec::new();
            flatten_checked_boolean_connective(left, conjunction, &mut leaves);
            flatten_checked_boolean_connective(right, conjunction, &mut leaves);
            let propositions = leaves
                .into_iter()
                .map(|leaf| {
                    lower_proposition(leaf, parameters, structural_types, runtime_requirements)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let mut keyed = propositions
                .into_iter()
                .map(|proposition| {
                    psi_terminal_codec::canonical_proposition_order_key(&proposition)
                        .map(|key| (key, proposition))
                        .map_err(|_| {
                            LoweringError::Unsupported(
                                "structural crash connective is not canonically encodable",
                            )
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            keyed.sort_by(|left, right| left.0.cmp(&right.0));
            keyed.dedup_by(|left, right| left.0 == right.0);
            if keyed.len() < 2 {
                return unsupported(
                    "structural crash connective must retain at least two distinct predicates",
                );
            }
            let propositions = keyed
                .into_iter()
                .map(|(_, proposition)| proposition)
                .collect();
            return Ok(if conjunction {
                Proposition::Conjunction(propositions)
            } else {
                Proposition::Disjunction(propositions)
            });
        }
        Ok(Proposition::Equal(
            ScalarTerm::boolean(true),
            lower_term(
                expression,
                parameters,
                structural_types,
                runtime_requirements,
            )?,
        ))
    }

    buckets
        .iter()
        .map(|bucket| {
            let mut alternatives = bucket
                .alternative_guards()
                .iter()
                .map(|guard| match guard {
                    psi_checked_trees::CrashRouteGuard::Truth => {
                        Ok(psi_terminal::CrashRouteGuard::Truth)
                    }
                    psi_checked_trees::CrashRouteGuard::Predicate(predicate) => {
                        let proposition = if let Some(expression) = predicate.scalar_expression() {
                            lower_proposition(
                                expression,
                                parameters,
                                structural_types,
                                runtime_requirements,
                            )?
                        } else {
                            let mut path = Vec::new();
                            let parameter_position = predicate
                                .expression()
                                .and_then(|expression| checked_member_path(expression, &mut path))
                                .ok_or(LoweringError::Unsupported(
                                    "structural crash route is outside checked Boolean member lowering",
                                ))?;
                            Proposition::Equal(
                                ScalarTerm::boolean(true),
                                lower_structural_member_term(
                                    parameter_position,
                                    &path,
                                    ScalarType::Boolean,
                                    parameters,
                                    structural_types,
                                )?,
                            )
                        };
                        Ok(psi_terminal::CrashRouteGuard::Predicate(
                            psi_terminal::CrashPredicateTerm::new(proposition),
                        ))
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            alternatives.sort();
            alternatives.dedup();
            Ok(psi_terminal::CrashRouteBucket {
                cause: match bucket.cause() {
                    psi_checked_trees::CrashCause::Trap => TerminalCrashCause::Trap,
                    psi_checked_trees::CrashCause::Abort => TerminalCrashCause::Abort,
                },
                alternatives,
            })
        })
        .collect()
}

fn substitute_structural_crash_route_roots(
    buckets: &mut [psi_terminal::CrashRouteBucket],
    substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
) -> Result<(), LoweringError> {
    fn substitute_term(
        term: &mut ScalarTerm,
        substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
    ) -> Result<(), LoweringError> {
        match term {
            ScalarTerm::BooleanField { root, path }
            | ScalarTerm::IntegerField { root, path, .. } => {
                let Some((replacement, prefix)) = substitutions.get(root) else {
                    return Ok(());
                };
                *root = *replacement;
                if !prefix.is_empty() {
                    let mut rebased = Vec::with_capacity(prefix.len() + path.len());
                    rebased.extend(prefix);
                    rebased.append(path);
                    *path = rebased;
                }
            }
            ScalarTerm::BooleanNot { operand } => substitute_term(operand, substitutions)?,
            ScalarTerm::IntegerBitwiseNot { operand, .. } => {
                substitute_term(operand, substitutions)?
            }
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
            | ScalarTerm::WrappingIntegerAdd { left, right, .. }
            | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
            | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
            | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
            | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
            | ScalarTerm::SaturatingIntegerMultiply { left, right, .. }
            | ScalarTerm::WrappingIntegerDivide { left, right, .. }
            | ScalarTerm::WrappingIntegerRemainder { left, right, .. }
            | ScalarTerm::SaturatingIntegerDivide { left, right, .. }
            | ScalarTerm::SaturatingIntegerRemainder { left, right, .. } => {
                substitute_term(left, substitutions)?;
                substitute_term(right, substitutions)?;
            }
            ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
            | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
            | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
            | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
                substitute_term(value, substitutions)?;
                substitute_term(count, substitutions)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn substitute_proposition(
        proposition: &mut Proposition,
        substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
    ) -> Result<(), LoweringError> {
        match proposition {
            Proposition::Equal(left, right) => {
                substitute_term(left, substitutions)?;
                substitute_term(right, substitutions)?;
            }
            Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
                for proposition in propositions.iter_mut() {
                    substitute_proposition(proposition, substitutions)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    for bucket in buckets {
        for alternative in &mut bucket.alternatives {
            let psi_terminal::CrashRouteGuard::Predicate(predicate) = alternative else {
                continue;
            };
            let mut proposition = predicate.proposition().clone();
            substitute_proposition(&mut proposition, substitutions)?;
            *predicate = psi_terminal::CrashPredicateTerm::new(proposition);
        }
    }
    Ok(())
}

fn substitute_structural_requirement_roots(
    proposition: &mut Proposition,
    substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
) -> Result<(), LoweringError> {
    fn substitute_term(
        term: &mut ScalarTerm,
        substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
    ) {
        match term {
            ScalarTerm::BooleanField { root, path }
            | ScalarTerm::IntegerField { root, path, .. } => {
                let Some((replacement, prefix)) = substitutions.get(root) else {
                    return;
                };
                *root = *replacement;
                if !prefix.is_empty() {
                    let mut rebased = Vec::with_capacity(prefix.len() + path.len());
                    rebased.extend(prefix);
                    rebased.append(path);
                    *path = rebased;
                }
            }
            ScalarTerm::BooleanNot { operand }
            | ScalarTerm::IntegerBitwiseNot { operand, .. }
            | ScalarTerm::IntegerWiden { operand, .. }
            | ScalarTerm::IntegerExactCast { operand, .. } => {
                substitute_term(operand, substitutions);
            }
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
                substitute_term(left, substitutions);
                substitute_term(right, substitutions);
            }
            ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
            | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
            | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
            | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
                substitute_term(value, substitutions);
                substitute_term(count, substitutions);
            }
            ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => {}
        }
    }

    match proposition {
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            substitute_term(left, substitutions);
            substitute_term(right, substitutions);
        }
        Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
            for proposition in propositions {
                substitute_structural_requirement_roots(proposition, substitutions)?;
            }
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            substitute_structural_requirement_roots(premise, substitutions)?;
            substitute_structural_requirement_roots(conclusion, substitutions)?;
        }
        Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => {}
        Proposition::ContentConservation(_) => {
            return unsupported(
                "runtime structural requirements cannot carry content conservation",
            );
        }
    }
    Ok(())
}

fn structural_crash_route_argument_prefix(
    argument: &StructuralArgument,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<Vec<CanonicalStructuralPathSegment>, LoweringError> {
    let mut structural_type = parameters
        .iter()
        .find(|parameter| parameter.place == argument.place)
        .map(|parameter| parameter.structural_type)
        .ok_or(LoweringError::Unsupported(
            "structural crash route argument has no caller parameter",
        ))?;
    let mut prefix = Vec::with_capacity(argument.path.len());
    for segment in &argument.path {
        let declaration = structural_types
            .iter()
            .find(|declaration| declaration.id == structural_type)
            .ok_or(LoweringError::Unsupported(
                "structural crash route argument path type is absent",
            ))?;
        match segment {
            StructuralPathSegment::Field(identity) => {
                let StructuralTypeShape::Record { fields } = &declaration.shape else {
                    return unsupported(
                        "structural crash route argument path receiver is not a record",
                    );
                };
                let field = fields
                    .iter()
                    .find(|field| field.identity == *identity)
                    .filter(|field| !field.relevance.is_erased())
                    .ok_or(LoweringError::Unsupported(
                        "structural crash route argument field is absent or erased",
                    ))?;
                let StructuralFieldType::Structural(next) = field.field_type else {
                    return unsupported("structural crash route argument field is not structural");
                };
                prefix.push(CanonicalStructuralPathSegment::Field(field.id));
                structural_type = next;
            }
            StructuralPathSegment::FixedIndex(index) => {
                let StructuralTypeShape::FixedArray { element, length } = declaration.shape else {
                    return unsupported(
                        "structural crash route argument fixed index receiver is not an array",
                    );
                };
                if *index >= length {
                    return unsupported(
                        "structural crash route argument fixed index is out of bounds",
                    );
                }
                prefix.push(CanonicalStructuralPathSegment::FixedIndex(*index));
                structural_type = element;
            }
        }
    }
    Ok(prefix)
}

fn lower_checked_crash_predicates(
    predicates: &[CheckedBooleanExpression],
    values: &[ValueDeclaration],
) -> Result<Vec<psi_terminal::CrashPredicateTerm>, LoweringError> {
    let mut predicates = predicates
        .iter()
        .map(|predicate| {
            checked_boolean_proposition(predicate, values)
                .map(psi_terminal::CrashPredicateTerm::new)
        })
        .collect::<Result<Vec<_>, _>>()?;
    predicates.sort();
    predicates.dedup();
    Ok(predicates)
}

fn flatten_checked_boolean_connective<'expression>(
    expression: &'expression CheckedBooleanExpression,
    conjunction: bool,
    output: &mut Vec<&'expression CheckedBooleanExpression>,
) {
    match expression {
        CheckedBooleanExpression::And { left, right } if conjunction => {
            flatten_checked_boolean_connective(left, conjunction, output);
            flatten_checked_boolean_connective(right, conjunction, output);
        }
        CheckedBooleanExpression::Or { left, right } if !conjunction => {
            flatten_checked_boolean_connective(left, conjunction, output);
            flatten_checked_boolean_connective(right, conjunction, output);
        }
        expression => output.push(expression),
    }
}

fn checked_boolean_proposition(
    expression: &CheckedBooleanExpression,
    values: &[ValueDeclaration],
) -> Result<Proposition, LoweringError> {
    match expression {
        CheckedBooleanExpression::Constant(_) => {
            unsupported("constant crash predicates must normalize before terminal lowering")
        }
        CheckedBooleanExpression::And { left, right }
        | CheckedBooleanExpression::Or { left, right } => {
            let conjunction = matches!(expression, CheckedBooleanExpression::And { .. });
            let mut leaves = Vec::new();
            flatten_checked_boolean_connective(left, conjunction, &mut leaves);
            flatten_checked_boolean_connective(right, conjunction, &mut leaves);
            let mut propositions = leaves
                .into_iter()
                .map(|leaf| checked_boolean_proposition(leaf, values))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .map(|proposition| {
                    psi_terminal_codec::canonical_proposition_order_key(&proposition)
                        .map(|key| (key, proposition))
                        .map_err(|_| {
                            LoweringError::Unsupported(
                                "scalar crash connective is not canonically encodable",
                            )
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            propositions.sort_by(|left, right| left.0.cmp(&right.0));
            propositions.dedup_by(|left, right| left.0 == right.0);
            if propositions.len() < 2 {
                return unsupported(
                    "scalar crash connective must retain at least two distinct predicates",
                );
            }
            let propositions = propositions
                .into_iter()
                .map(|(_, proposition)| proposition)
                .collect();
            Ok(if conjunction {
                Proposition::Conjunction(propositions)
            } else {
                Proposition::Disjunction(propositions)
            })
        }
        expression => {
            let mut left = checked_boolean_scalar_term(expression, values)?;
            let mut right = ScalarTerm::boolean(true);
            if left > right {
                std::mem::swap(&mut left, &mut right);
            }
            Ok(Proposition::Equal(left, right))
        }
    }
}

fn checked_boolean_scalar_term(
    expression: &CheckedBooleanExpression,
    values: &[ValueDeclaration],
) -> Result<ScalarTerm, LoweringError> {
    Ok(match expression {
        CheckedBooleanExpression::Constant(value) => ScalarTerm::boolean(*value),
        CheckedBooleanExpression::Parameter { position }
        | CheckedBooleanExpression::Local { position } => {
            let value = values.get(*position).ok_or(LoweringError::Unsupported(
                "crash predicate value position is outside the selected scalar namespace",
            ))?;
            if value.scalar_type != ScalarType::Boolean {
                return unsupported("crash predicate Boolean value has a non-Boolean type");
            }
            ScalarTerm::value(value.id, value.scalar_type)
        }
        CheckedBooleanExpression::StructuralParameterField { .. } => {
            return unsupported(
                "structural crash predicates require structural signature lowering",
            );
        }
        CheckedBooleanExpression::Not(operand) => {
            ScalarTerm::boolean_not(checked_boolean_scalar_term(operand, values)?)
                .map_err(LoweringError::InvalidCrashPredicate)?
        }
        CheckedBooleanExpression::Equal { left, right } => ScalarTerm::boolean_equal(
            checked_boolean_scalar_term(left, values)?,
            checked_boolean_scalar_term(right, values)?,
        )
        .map_err(LoweringError::InvalidCrashPredicate)?,
        CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
            let left = checked_scalar_term(left, values)?;
            let right = checked_scalar_term(right, values)?;
            let ScalarType::Integer(integer_type) = left.scalar_type() else {
                return unsupported("crash comparison operand is not an integer");
            };
            match kind {
                CheckedIntegerComparisonKind::Equal => {
                    ScalarTerm::integer_equal(integer_type, left, right)
                }
                CheckedIntegerComparisonKind::LessThan => {
                    ScalarTerm::integer_less_than(integer_type, left, right)
                }
                CheckedIntegerComparisonKind::LessOrEqual => {
                    ScalarTerm::integer_less_or_equal(integer_type, left, right)
                }
            }
            .map_err(LoweringError::InvalidCrashPredicate)?
        }
        CheckedBooleanExpression::And { .. } | CheckedBooleanExpression::Or { .. } => {
            return unsupported(
                "short-circuit Boolean crash predicate is not one scalar terminal term",
            );
        }
    })
}

fn checked_scalar_term(
    expression: &CheckedScalarExpression,
    values: &[ValueDeclaration],
) -> Result<ScalarTerm, LoweringError> {
    let expression = lower_checked_scalar_expression(expression)?;
    lowered_direct_scalar_term(&expression, values)
}

fn lowered_direct_scalar_term(
    expression: &LoweredDirectExpression,
    values: &[ValueDeclaration],
) -> Result<ScalarTerm, LoweringError> {
    Ok(match expression {
        LoweredDirectExpression::Parameter {
            position,
            scalar_type,
        }
        | LoweredDirectExpression::Local {
            position,
            scalar_type,
        } => {
            let value = values.get(*position).ok_or(LoweringError::Unsupported(
                "crash predicate value position is outside the selected scalar namespace",
            ))?;
            if value.scalar_type != *scalar_type {
                return unsupported("crash predicate value type does not match its checked plan");
            }
            ScalarTerm::value(value.id, *scalar_type)
        }
        LoweredDirectExpression::IntegerLiteral { value, scalar_type } => {
            let ScalarType::Integer(integer_type) = scalar_type else {
                return unsupported("crash predicate integer literal has a non-integer type");
            };
            ScalarTerm::integer(*integer_type, *value)
                .map_err(LoweringError::InvalidCrashPredicate)?
        }
        LoweredDirectExpression::IntegerBinary {
            kind,
            scalar_type,
            left,
            right,
        } => {
            let ScalarType::Integer(integer_type) = scalar_type else {
                return unsupported("crash predicate arithmetic has a non-integer type");
            };
            let left = Box::new(lowered_direct_scalar_term(left, values)?);
            let right = Box::new(lowered_direct_scalar_term(right, values)?);
            match kind {
                LoweredIntegerBinaryKind::BitwiseAnd => ScalarTerm::IntegerBitwiseAnd {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::BitwiseOr => ScalarTerm::IntegerBitwiseOr {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::BitwiseXor => ScalarTerm::IntegerBitwiseXor {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::WrappingShiftLeft => {
                    let ScalarType::Integer(count_type) = right.scalar_type() else {
                        return unsupported("crash shift count is not an integer");
                    };
                    ScalarTerm::WrappingIntegerShiftLeft {
                        value_type: *integer_type,
                        count_type,
                        value: left,
                        count: right,
                    }
                }
                LoweredIntegerBinaryKind::WrappingShiftRight => {
                    let ScalarType::Integer(count_type) = right.scalar_type() else {
                        return unsupported("crash shift count is not an integer");
                    };
                    ScalarTerm::WrappingIntegerShiftRight {
                        value_type: *integer_type,
                        count_type,
                        value: left,
                        count: right,
                    }
                }
                LoweredIntegerBinaryKind::ExactShiftLeft => {
                    let ScalarType::Integer(count_type) = right.scalar_type() else {
                        return unsupported("crash shift count is not an integer");
                    };
                    ScalarTerm::ExactIntegerShiftLeft {
                        value_type: *integer_type,
                        count_type,
                        value: left,
                        count: right,
                    }
                }
                LoweredIntegerBinaryKind::ExactShiftRight => {
                    let ScalarType::Integer(count_type) = right.scalar_type() else {
                        return unsupported("crash shift count is not an integer");
                    };
                    ScalarTerm::ExactIntegerShiftRight {
                        value_type: *integer_type,
                        count_type,
                        value: left,
                        count: right,
                    }
                }
                LoweredIntegerBinaryKind::ExactAdd => ScalarTerm::ExactIntegerAdd {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::ExactSubtract => ScalarTerm::ExactIntegerSubtract {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::ExactMultiply => ScalarTerm::ExactIntegerMultiply {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::ExactDivide => ScalarTerm::ExactIntegerDivide {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::ExactRemainder => ScalarTerm::ExactIntegerRemainder {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::WrappingDivide => ScalarTerm::WrappingIntegerDivide {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::WrappingRemainder => {
                    ScalarTerm::WrappingIntegerRemainder {
                        scalar_type: *integer_type,
                        left,
                        right,
                    }
                }
                LoweredIntegerBinaryKind::SaturatingDivide => ScalarTerm::SaturatingIntegerDivide {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::SaturatingRemainder => {
                    ScalarTerm::SaturatingIntegerRemainder {
                        scalar_type: *integer_type,
                        left,
                        right,
                    }
                }
                LoweredIntegerBinaryKind::WrappingAdd => ScalarTerm::WrappingIntegerAdd {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::SaturatingAdd => ScalarTerm::SaturatingIntegerAdd {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::WrappingSubtract => ScalarTerm::WrappingIntegerSubtract {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::SaturatingSubtract => {
                    ScalarTerm::SaturatingIntegerSubtract {
                        scalar_type: *integer_type,
                        left,
                        right,
                    }
                }
                LoweredIntegerBinaryKind::WrappingMultiply => ScalarTerm::WrappingIntegerMultiply {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::SaturatingMultiply => {
                    ScalarTerm::SaturatingIntegerMultiply {
                        scalar_type: *integer_type,
                        left,
                        right,
                    }
                }
            }
        }
        LoweredDirectExpression::IntegerBitwiseNot {
            scalar_type,
            operand,
        } => {
            let ScalarType::Integer(integer_type) = scalar_type else {
                return unsupported("crash predicate bitwise-not has a non-integer type");
            };
            ScalarTerm::IntegerBitwiseNot {
                scalar_type: *integer_type,
                operand: Box::new(lowered_direct_scalar_term(operand, values)?),
            }
        }
        LoweredDirectExpression::IntegerWiden {
            scalar_type,
            operand,
        } => {
            let ScalarType::Integer(target_type) = scalar_type else {
                return unsupported("crash predicate widen has a non-integer target");
            };
            let operand = lowered_direct_scalar_term(operand, values)?;
            let ScalarType::Integer(source_type) = operand.scalar_type() else {
                return unsupported("crash predicate widen has a non-integer operand");
            };
            ScalarTerm::IntegerWiden {
                source_type,
                target_type: *target_type,
                operand: Box::new(operand),
            }
        }
        LoweredDirectExpression::IntegerExactCast {
            scalar_type,
            operand,
        } => {
            let ScalarType::Integer(target_type) = scalar_type else {
                return unsupported("crash predicate cast has a non-integer target");
            };
            let operand = lowered_direct_scalar_term(operand, values)?;
            let ScalarType::Integer(source_type) = operand.scalar_type() else {
                return unsupported("crash predicate cast has a non-integer operand");
            };
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type: *target_type,
                operand: Box::new(operand),
            }
        }
        LoweredDirectExpression::Boolean { expression } => {
            return checked_boolean_scalar_term_from_lowered(expression, values);
        }
    })
}

fn checked_boolean_scalar_term_from_lowered(
    expression: &LoweredBooleanReturnExpression,
    values: &[ValueDeclaration],
) -> Result<ScalarTerm, LoweringError> {
    match expression {
        LoweredBooleanReturnExpression::Constant { value } => Ok(ScalarTerm::boolean(*value)),
        LoweredBooleanReturnExpression::Parameter { position }
        | LoweredBooleanReturnExpression::Local { position } => {
            let value = values.get(*position).ok_or(LoweringError::Unsupported(
                "crash predicate value position is outside the selected scalar namespace",
            ))?;
            (value.scalar_type == ScalarType::Boolean)
                .then(|| ScalarTerm::value(value.id, value.scalar_type))
                .ok_or(LoweringError::Unsupported(
                    "crash predicate Boolean value has a non-Boolean type",
                ))
        }
        LoweredBooleanReturnExpression::StructuralField { source, field } => {
            Ok(ScalarTerm::boolean_field(*source, *field))
        }
        LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. } => {
            unsupported("unresolved structural field crossed crash-predicate lowering")
        }
        LoweredBooleanReturnExpression::Not { operand } => {
            ScalarTerm::boolean_not(checked_boolean_scalar_term_from_lowered(operand, values)?)
                .map_err(LoweringError::InvalidCrashPredicate)
        }
        LoweredBooleanReturnExpression::Equal { left, right } => ScalarTerm::boolean_equal(
            checked_boolean_scalar_term_from_lowered(left, values)?,
            checked_boolean_scalar_term_from_lowered(right, values)?,
        )
        .map_err(LoweringError::InvalidCrashPredicate),
        LoweredBooleanReturnExpression::IntegerComparison { kind, left, right } => {
            let left = lowered_direct_scalar_term(left, values)?;
            let right = lowered_direct_scalar_term(right, values)?;
            let ScalarType::Integer(integer_type) = left.scalar_type() else {
                return unsupported("crash comparison operand is not an integer");
            };
            match kind {
                LoweredIntegerComparisonKind::Equal => {
                    ScalarTerm::integer_equal(integer_type, left, right)
                }
                LoweredIntegerComparisonKind::LessThan => {
                    ScalarTerm::integer_less_than(integer_type, left, right)
                }
                LoweredIntegerComparisonKind::LessOrEqual => {
                    ScalarTerm::integer_less_or_equal(integer_type, left, right)
                }
            }
            .map_err(LoweringError::InvalidCrashPredicate)
        }
        LoweredBooleanReturnExpression::And { .. } | LoweredBooleanReturnExpression::Or { .. } => {
            unsupported("short-circuit Boolean crash predicate is not one scalar terminal term")
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_scalar_graph_module(
    states: &[LoweredScalarBranchState],
    result_type: ScalarType,
    contract_value: Option<KnownDirectScalar>,
    crash_routes: Vec<psi_checked_trees::CrashRouteBucket>,
    identity_reshuffles: LoweredContentIdentityReshuffles,
    partition_compositions: LoweredContentPartitionCompositions,
    terminal_machine: MachineId,
    identity_base: u64,
    machine_ids: &[(psi_symbols::SymbolHandle, MachineId)],
    requirement_counts: &[(psi_symbols::SymbolHandle, usize)],
) -> Result<LoweredTerminalPsi, LoweringError> {
    let parameters = states[0]
        .parameter_types
        .iter()
        .enumerate()
        .map(|(index, scalar_type)| ValueDeclaration {
            id: value_id(
                identity_base
                    .checked_add(
                        u64::try_from(index).expect("parameter index fits a semantic identity"),
                    )
                    .expect("parameter identity base admits the parameter index")
                    .checked_add(1)
                    .expect("parameter identity is nonzero"),
            ),
            scalar_type: *scalar_type,
        })
        .collect::<Vec<_>>();
    let crash_routes = lower_checked_crash_route_buckets(&crash_routes, &parameters)?;
    let mut next_value_identity = identity_base
        .checked_add(
            u64::try_from(parameters.len()).expect("parameter count fits a semantic identity"),
        )
        .expect("parameter count fits the machine identity namespace")
        .checked_add(1)
        .expect("generated identities follow parameter identities");
    let mut state_parameters = Vec::with_capacity(states.len());
    state_parameters.push(parameters.clone());
    for state in &states[1..] {
        state_parameters.push(
            state
                .parameter_types
                .iter()
                .map(|scalar_type| {
                    let parameter = ValueDeclaration {
                        id: value_id(next_value_identity),
                        scalar_type: *scalar_type,
                    };
                    next_value_identity = next_value_identity
                        .checked_add(1)
                        .expect("scalar graph block parameter identities advance");
                    parameter
                })
                .collect(),
        );
    }

    let mut all_operations = OperationBuffer::new(identity_base);
    let call_obligation_base = identity_base
        .checked_add(TERMINAL_MACHINE_IDENTITY_STRIDE / 2)
        .expect("call obligation range fits the machine identity namespace");
    let mut call_emission = CallEmissionContext {
        machine_ids,
        requirement_counts,
        next_obligation_identity: call_obligation_base,
        obligation_limit: identity_base
            .checked_add(TERMINAL_MACHINE_IDENTITY_STRIDE)
            .expect("machine identity namespace has a finite upper bound"),
    };
    let mut next_edge_identity = identity_base
        .checked_add(1)
        .expect("edge identity base admits one-based identities");
    let mut next_block_identity = identity_base
        .checked_add(u64::try_from(states.len()).expect("state count fits a semantic identity"))
        .expect("state count fits the machine identity namespace")
        .checked_add(1)
        .expect("conditional binding blocks follow source blocks");
    let mut pending_blocks = Vec::new();
    let mut inlined_blocks = Vec::new();
    let mut blocks = Vec::with_capacity(states.len());
    for (index, state) in states.iter().enumerate() {
        let operation_start = all_operations.len();
        let current_parameters = &state_parameters[index];
        let source_block = block_id(
            identity_base
                .checked_add(u64::try_from(index).expect("state index fits a semantic identity"))
                .expect("state index fits the machine identity namespace")
                .checked_add(1)
                .expect("block identity is nonzero"),
        );
        let source_block_parameters = if index == 0 {
            Vec::new()
        } else {
            current_parameters.clone()
        };
        let staged_short_circuit_terminator =
            staged_short_circuit_bindings_terminator(&state.bindings, &state.terminator);
        let mut current_values = current_parameters.clone();
        let mut current_value_types = state.parameter_types.clone();
        if let Some((binding_plans, continuation_plan)) = staged_short_circuit_terminator {
            let mut stage_block = source_block;
            let mut stage_parameters = current_parameters.clone();
            let mut stage_parameter_types = state.parameter_types.clone();
            let mut stage_block_parameters = source_block_parameters;
            for (binding_index, binding) in binding_plans.iter().enumerate() {
                let mut next_stage_types = stage_parameter_types.clone();
                next_stage_types.push(binding.scalar_type());
                let next_stage_parameters = next_stage_types
                    .iter()
                    .copied()
                    .map(|scalar_type| {
                        let parameter = ValueDeclaration {
                            id: value_id(next_value_identity),
                            scalar_type,
                        };
                        next_value_identity = next_value_identity
                            .checked_add(1)
                            .expect("staged local parameter identities advance");
                        parameter
                    })
                    .collect::<Vec<_>>();
                let next_stage =
                    if let LoweredScalarBinding::Expression(LoweredDirectExpression::Boolean {
                        expression,
                    }) = binding
                        && contains_short_circuit(expression)
                    {
                        let decision = lower_boolean_value_decision(expression);
                        let decision_block_count = boolean_decision_block_count(&decision);
                        let first_child_identity = next_block_identity;
                        let next_stage =
                            block_id(
                                next_block_identity
                                    .checked_add(u64::try_from(decision_block_count - 1).expect(
                                        "staged Boolean child count fits a semantic identity",
                                    ))
                                    .expect("staged Boolean continuation identity advances"),
                            );
                        next_block_identity = next_stage
                            .get()
                            .checked_add(1)
                            .expect("staged Boolean block identities advance");
                        let carried_arguments = stage_parameters
                            .iter()
                            .map(|parameter| parameter.id)
                            .collect::<Vec<_>>();
                        let first_reserved_identity = if binding_index == 0 {
                            first_child_identity
                                .checked_sub(1)
                                .expect("staged Boolean blocks follow source blocks")
                        } else {
                            stage_block.get()
                        };
                        let mut decision_blocks = Vec::with_capacity(decision_block_count);
                        let entry = emit_reserved_boolean_tuple_stage_blocks(
                            &decision,
                            &stage_parameters,
                            stage_block_parameters,
                            next_stage,
                            &carried_arguments,
                            first_reserved_identity,
                            &mut next_value_identity,
                            &mut next_edge_identity,
                            &mut all_operations,
                            &mut decision_blocks,
                        );
                        assert_eq!(entry.get(), first_reserved_identity);
                        let mut decision_blocks = decision_blocks
                            .into_iter()
                            .map(|block| block.expect("every staged Boolean block is finalized"));
                        let mut root = decision_blocks
                            .next()
                            .expect("staged short-circuit Boolean has a decision root");
                        if binding_index == 0 {
                            root.id = source_block;
                            blocks.push(root);
                        } else {
                            inlined_blocks.push(root);
                        }
                        inlined_blocks.extend(decision_blocks);
                        next_stage
                    } else if let LoweredScalarBinding::DirectCall(call) = binding
                        && call
                            .arguments
                            .iter()
                            .any(direct_expression_contains_short_circuit)
                    {
                        let (next_stage, mut call_blocks) = emit_staged_scalar_call_binding(
                            call,
                            &stage_parameters,
                            &stage_parameter_types,
                            stage_block_parameters,
                            stage_block,
                            &mut next_block_identity,
                            &mut next_value_identity,
                            &mut next_edge_identity,
                            &mut all_operations,
                            &mut call_emission,
                        )?;
                        let root = call_blocks
                            .drain(..1)
                            .next()
                            .expect("a staged scalar call has an argument root");
                        if binding_index == 0 {
                            blocks.push(root);
                        } else {
                            inlined_blocks.push(root);
                        }
                        inlined_blocks.extend(call_blocks);
                        next_stage
                    } else {
                        let next_stage = block_id(next_block_identity);
                        next_block_identity = next_block_identity
                            .checked_add(1)
                            .expect("staged direct-local block identities advance");
                        let stage_operation_start = all_operations.len();
                        let value = emit_scalar_binding(
                            binding,
                            &stage_parameters,
                            &mut next_value_identity,
                            &mut all_operations,
                            &mut call_emission,
                        )?;
                        let mut arguments = stage_parameters
                            .iter()
                            .map(|parameter| parameter.id)
                            .collect::<Vec<_>>();
                        arguments.push(value);
                        let edge = edge_id(next_edge_identity);
                        next_edge_identity = next_edge_identity
                            .checked_add(1)
                            .expect("staged direct-local edge identity advances");
                        let block = Block {
                            id: stage_block,
                            parameters: stage_block_parameters,
                            operations: all_operations[stage_operation_start..].to_vec(),
                            terminator: Terminator::Jump {
                                edge,
                                target: next_stage,
                                arguments,
                                trivial_affine_discards: Vec::new(),
                            },
                        };
                        if binding_index == 0 {
                            blocks.push(block);
                        } else {
                            inlined_blocks.push(block);
                        }
                        next_stage
                    };
                stage_block = next_stage;
                stage_parameters = next_stage_parameters;
                stage_parameter_types = next_stage_types;
                stage_block_parameters = stage_parameters.clone();
            }

            if let LoweredScalarBranchTerminator::Return {
                expression: LoweredDirectExpression::Boolean { expression },
            } = &continuation_plan
                && contains_short_circuit(expression)
            {
                let decision = lower_boolean_value_decision(expression);
                let block_count = boolean_decision_block_count(&decision);
                let first_synthetic_block = block_id(next_block_identity);
                next_block_identity = next_block_identity
                    .checked_add(
                        u64::try_from(block_count - 1)
                            .expect("staged Boolean return child count fits a semantic identity"),
                    )
                    .expect("staged Boolean return block identities advance");
                let (root, children) = emit_inlined_boolean_value_blocks(
                    &decision,
                    &stage_parameters,
                    stage_parameters.clone(),
                    LoweredBooleanDecisionExit::Return,
                    stage_block,
                    first_synthetic_block,
                    &mut next_value_identity,
                    &mut next_edge_identity,
                    &mut all_operations,
                );
                inlined_blocks.push(root);
                inlined_blocks.extend(children);
                continue;
            }
            if let LoweredScalarBranchTerminator::Jump { target, arguments } = &continuation_plan
                && let [LoweredDirectExpression::Boolean { expression }] = arguments.as_slice()
                && contains_short_circuit(expression)
            {
                let decision = lower_boolean_value_decision(expression);
                let block_count = boolean_decision_block_count(&decision);
                let first_synthetic_block = block_id(next_block_identity);
                next_block_identity = next_block_identity
                    .checked_add(
                        u64::try_from(block_count - 1)
                            .expect("staged Boolean jump child count fits a semantic identity"),
                    )
                    .expect("staged Boolean jump block identities advance");
                let target = scalar_source_block(identity_base, *target);
                let (root, children) = emit_inlined_boolean_value_blocks(
                    &decision,
                    &stage_parameters,
                    stage_parameters.clone(),
                    LoweredBooleanDecisionExit::Jump { target },
                    stage_block,
                    first_synthetic_block,
                    &mut next_value_identity,
                    &mut next_edge_identity,
                    &mut all_operations,
                );
                inlined_blocks.push(root);
                inlined_blocks.extend(children);
                continue;
            }
            if let LoweredScalarBranchTerminator::Conditional {
                condition,
                when_true_target,
                when_true_arguments,
                when_false_target,
                when_false_arguments,
            } = &continuation_plan
                && contains_short_circuit(condition)
            {
                let decision = lower_boolean_control_decision(
                    condition,
                    LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant {
                        value: true,
                    }),
                    LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant {
                        value: false,
                    }),
                );
                let decision_block_count = boolean_decision_test_count(&decision);
                debug_assert!(decision_block_count > 0);
                let first_synthetic_block = block_id(next_block_identity);
                next_block_identity = next_block_identity
                    .checked_add(
                        u64::try_from(decision_block_count - 1)
                            .expect("staged Boolean guard child count fits a semantic identity"),
                    )
                    .expect("staged Boolean guard block identities advance");
                let when_true = build_scalar_conditional_target(
                    *when_true_target,
                    when_true_arguments,
                    &stage_parameters,
                    &stage_parameter_types,
                    &mut next_block_identity,
                    &mut next_value_identity,
                    &mut pending_blocks,
                    identity_base,
                );
                let when_false = build_scalar_conditional_target(
                    *when_false_target,
                    when_false_arguments,
                    &stage_parameters,
                    &stage_parameter_types,
                    &mut next_block_identity,
                    &mut next_value_identity,
                    &mut pending_blocks,
                    identity_base,
                );
                let (root, children) = emit_inlined_boolean_guard_blocks(
                    &decision,
                    &stage_parameters,
                    stage_parameters.clone(),
                    &when_true,
                    &when_false,
                    stage_block,
                    first_synthetic_block,
                    &mut next_value_identity,
                    &mut next_edge_identity,
                    &mut all_operations,
                );
                inlined_blocks.push(root);
                inlined_blocks.extend(children);
                continue;
            }

            let operation_start = all_operations.len();
            let terminator = match continuation_plan {
                LoweredScalarBranchTerminator::Return { expression } => {
                    let value = emit_direct_expression(
                        &expression,
                        &stage_parameters,
                        &mut next_value_identity,
                        &mut all_operations,
                    );
                    let edge = edge_id(next_edge_identity);
                    next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("carried Boolean return edge identity advances");
                    Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge,
                        value,
                    }
                }
                LoweredScalarBranchTerminator::Conditional {
                    condition,
                    when_true_target,
                    when_true_arguments,
                    when_false_target,
                    when_false_arguments,
                } => {
                    let condition = emit_boolean_expression(
                        &condition,
                        &stage_parameters,
                        &mut next_value_identity,
                        &mut all_operations,
                    );
                    let when_true = build_scalar_conditional_target(
                        when_true_target,
                        &when_true_arguments,
                        &stage_parameters,
                        &stage_parameter_types,
                        &mut next_block_identity,
                        &mut next_value_identity,
                        &mut pending_blocks,
                        identity_base,
                    );
                    let when_false = build_scalar_conditional_target(
                        when_false_target,
                        &when_false_arguments,
                        &stage_parameters,
                        &stage_parameter_types,
                        &mut next_block_identity,
                        &mut next_value_identity,
                        &mut pending_blocks,
                        identity_base,
                    );
                    let when_true_edge = edge_id(next_edge_identity);
                    next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("carried Boolean true edge identity advances");
                    let when_false_edge = edge_id(next_edge_identity);
                    next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("carried Boolean false edge identity advances");
                    Terminator::Conditional {
                        condition,
                        when_true: SuccessorEdge {
                            edge: when_true_edge,
                            target: when_true.block,
                            arguments: when_true.arguments,
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            edge: when_false_edge,
                            target: when_false.block,
                            arguments: when_false.arguments,
                            trivial_affine_discards: Vec::new(),
                        },
                    }
                }
                LoweredScalarBranchTerminator::Jump { target, arguments } => {
                    let edge = edge_id(next_edge_identity);
                    next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("staged local jump edge identity advances");
                    if arguments
                        .iter()
                        .any(direct_expression_contains_short_circuit)
                    {
                        let target = build_scalar_conditional_target(
                            target,
                            &arguments,
                            &stage_parameters,
                            &stage_parameter_types,
                            &mut next_block_identity,
                            &mut next_value_identity,
                            &mut pending_blocks,
                            identity_base,
                        );
                        Terminator::Jump {
                            edge,
                            target: target.block,
                            arguments: target.arguments,
                            trivial_affine_discards: Vec::new(),
                        }
                    } else {
                        let arguments = arguments
                            .iter()
                            .map(|argument| {
                                emit_direct_expression(
                                    argument,
                                    &stage_parameters,
                                    &mut next_value_identity,
                                    &mut all_operations,
                                )
                            })
                            .collect();
                        Terminator::Jump {
                            edge,
                            target: scalar_source_block(identity_base, target),
                            arguments,
                            trivial_affine_discards: Vec::new(),
                        }
                    }
                }
                LoweredScalarBranchTerminator::Crash(crash) => {
                    let edge = edge_id(next_edge_identity);
                    next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("staged local crash edge identity advances");
                    Terminator::Crash {
                        edge,
                        cause: crash.cause,
                        site_guard: lower_checked_crash_predicates(&crash.site_guard, &parameters)?,
                        frontier_lower_bound: crash.frontier_lower_bound,
                    }
                }
            };
            inlined_blocks.push(Block {
                id: stage_block,
                parameters: stage_parameters,
                operations: all_operations[operation_start..].to_vec(),
                terminator,
            });
            continue;
        }
        for binding in &state.bindings {
            let id = emit_scalar_binding(
                binding,
                &current_values,
                &mut next_value_identity,
                &mut all_operations,
                &mut call_emission,
            )?;
            current_values.push(ValueDeclaration {
                id,
                scalar_type: binding.scalar_type(),
            });
            current_value_types.push(binding.scalar_type());
        }
        let terminator_operation_start = all_operations.len();
        let terminator = match &state.terminator {
            LoweredScalarBranchTerminator::Jump { target, arguments } => {
                if let [LoweredDirectExpression::Boolean { expression }] = arguments.as_slice()
                    && contains_short_circuit(expression)
                {
                    let decision = lower_boolean_value_decision(expression);
                    let block_count = boolean_decision_block_count(&decision);
                    let first_synthetic_block = block_id(next_block_identity);
                    next_block_identity = next_block_identity
                        .checked_add(
                            u64::try_from(block_count - 1)
                                .expect("Boolean binding child count fits a semantic identity"),
                        )
                        .expect("Boolean binding block identities advance");
                    let target = scalar_source_block(identity_base, *target);
                    let (root, children) = emit_inlined_boolean_value_blocks(
                        &decision,
                        &current_values,
                        source_block_parameters,
                        LoweredBooleanDecisionExit::Jump { target },
                        source_block,
                        first_synthetic_block,
                        &mut next_value_identity,
                        &mut next_edge_identity,
                        &mut all_operations,
                    );
                    let mut root = root;
                    root.operations.splice(
                        0..0,
                        all_operations[operation_start..terminator_operation_start]
                            .iter()
                            .cloned(),
                    );
                    blocks.push(root);
                    inlined_blocks.extend(children);
                    continue;
                } else if arguments
                    .iter()
                    .any(direct_expression_contains_short_circuit)
                {
                    let target = build_scalar_conditional_target(
                        *target,
                        arguments,
                        &current_values,
                        &current_value_types,
                        &mut next_block_identity,
                        &mut next_value_identity,
                        &mut pending_blocks,
                        identity_base,
                    );
                    let edge = edge_id(next_edge_identity);
                    next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("mixed tuple entry edge identity advances");
                    Terminator::Jump {
                        edge,
                        target: target.block,
                        arguments: target.arguments,
                        trivial_affine_discards: Vec::new(),
                    }
                } else {
                    let arguments = arguments
                        .iter()
                        .map(|argument| {
                            emit_direct_expression(
                                argument,
                                &current_values,
                                &mut next_value_identity,
                                &mut all_operations,
                            )
                        })
                        .collect();
                    let edge = edge_id(next_edge_identity);
                    next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("scalar graph jump edge identities advance");
                    Terminator::Jump {
                        edge,
                        target: scalar_source_block(identity_base, *target),
                        arguments,
                        trivial_affine_discards: Vec::new(),
                    }
                }
            }
            LoweredScalarBranchTerminator::Conditional {
                condition,
                when_true_target,
                when_true_arguments,
                when_false_target,
                when_false_arguments,
            } => {
                if contains_short_circuit(condition) {
                    let decision = lower_boolean_control_decision(
                        condition,
                        LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant {
                            value: true,
                        }),
                        LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant {
                            value: false,
                        }),
                    );
                    let decision_block_count = boolean_decision_test_count(&decision);
                    debug_assert!(decision_block_count > 0);
                    let first_synthetic_block = block_id(next_block_identity);
                    next_block_identity = next_block_identity
                        .checked_add(
                            u64::try_from(decision_block_count - 1)
                                .expect("scalar graph guard child count fits a semantic identity"),
                        )
                        .expect("scalar graph guard block identities advance");
                    let when_true = build_scalar_conditional_target(
                        *when_true_target,
                        when_true_arguments,
                        &current_values,
                        &current_value_types,
                        &mut next_block_identity,
                        &mut next_value_identity,
                        &mut pending_blocks,
                        identity_base,
                    );
                    let when_false = build_scalar_conditional_target(
                        *when_false_target,
                        when_false_arguments,
                        &current_values,
                        &current_value_types,
                        &mut next_block_identity,
                        &mut next_value_identity,
                        &mut pending_blocks,
                        identity_base,
                    );
                    let (root, children) = emit_inlined_boolean_guard_blocks(
                        &decision,
                        &current_values,
                        source_block_parameters,
                        &when_true,
                        &when_false,
                        source_block,
                        first_synthetic_block,
                        &mut next_value_identity,
                        &mut next_edge_identity,
                        &mut all_operations,
                    );
                    let mut root = root;
                    root.operations.splice(
                        0..0,
                        all_operations[operation_start..terminator_operation_start]
                            .iter()
                            .cloned(),
                    );
                    blocks.push(root);
                    inlined_blocks.extend(children);
                    continue;
                } else {
                    let condition = emit_boolean_expression(
                        condition,
                        &current_values,
                        &mut next_value_identity,
                        &mut all_operations,
                    );
                    let when_true_edge = edge_id(next_edge_identity);
                    next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("scalar graph edge identities advance");
                    let when_false_edge = edge_id(next_edge_identity);
                    next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("scalar graph edge identities advance");
                    let when_true = build_scalar_conditional_target(
                        *when_true_target,
                        when_true_arguments,
                        &current_values,
                        &current_value_types,
                        &mut next_block_identity,
                        &mut next_value_identity,
                        &mut pending_blocks,
                        identity_base,
                    );
                    let when_false = build_scalar_conditional_target(
                        *when_false_target,
                        when_false_arguments,
                        &current_values,
                        &current_value_types,
                        &mut next_block_identity,
                        &mut next_value_identity,
                        &mut pending_blocks,
                        identity_base,
                    );
                    Terminator::Conditional {
                        condition,
                        when_true: SuccessorEdge {
                            edge: when_true_edge,
                            target: when_true.block,
                            arguments: when_true.arguments,
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            edge: when_false_edge,
                            target: when_false.block,
                            arguments: when_false.arguments,
                            trivial_affine_discards: Vec::new(),
                        },
                    }
                }
            }
            LoweredScalarBranchTerminator::Return { expression } => {
                if let LoweredDirectExpression::Boolean { expression } = expression
                    && contains_short_circuit(expression)
                {
                    let decision = lower_boolean_value_decision(expression);
                    let block_count = boolean_decision_block_count(&decision);
                    let first_synthetic_block = block_id(next_block_identity);
                    next_block_identity = next_block_identity
                        .checked_add(
                            u64::try_from(block_count - 1)
                                .expect("scalar return child count fits a semantic identity"),
                        )
                        .expect("scalar return block identities advance");
                    let (root, children) = emit_inlined_boolean_value_blocks(
                        &decision,
                        &current_values,
                        source_block_parameters,
                        LoweredBooleanDecisionExit::Return,
                        source_block,
                        first_synthetic_block,
                        &mut next_value_identity,
                        &mut next_edge_identity,
                        &mut all_operations,
                    );
                    let mut root = root;
                    root.operations.splice(
                        0..0,
                        all_operations[operation_start..terminator_operation_start]
                            .iter()
                            .cloned(),
                    );
                    blocks.push(root);
                    inlined_blocks.extend(children);
                    continue;
                } else {
                    let value = emit_direct_expression(
                        expression,
                        &current_values,
                        &mut next_value_identity,
                        &mut all_operations,
                    );
                    let edge = edge_id(next_edge_identity);
                    next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("scalar graph return edge identities advance");
                    Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge,
                        value,
                    }
                }
            }
            LoweredScalarBranchTerminator::Crash(crash) => {
                let edge = edge_id(next_edge_identity);
                next_edge_identity = next_edge_identity
                    .checked_add(1)
                    .expect("nested crash edge identities advance");
                Terminator::Crash {
                    edge,
                    cause: crash.cause,
                    site_guard: lower_checked_crash_predicates(&crash.site_guard, &parameters)?,
                    frontier_lower_bound: crash.frontier_lower_bound.clone(),
                }
            }
        };
        blocks.push(Block {
            id: source_block,
            parameters: source_block_parameters,
            operations: all_operations[operation_start..].to_vec(),
            terminator,
        });
    }
    blocks.extend(inlined_blocks);
    pending_blocks.sort_by_key(PendingNestedBlockGroup::first_id);
    for pending in pending_blocks {
        match pending {
            PendingNestedBlockGroup::ConditionalBinding(pending) => {
                let operation_start = all_operations.len();
                let arguments = pending
                    .arguments
                    .iter()
                    .map(|argument| {
                        emit_direct_expression(
                            argument,
                            &pending.parameters,
                            &mut next_value_identity,
                            &mut all_operations,
                        )
                    })
                    .collect();
                let edge = edge_id(next_edge_identity);
                next_edge_identity = next_edge_identity
                    .checked_add(1)
                    .expect("conditional binding jump edge identities advance");
                blocks.push(Block {
                    id: pending.id,
                    parameters: pending.parameters,
                    operations: all_operations[operation_start..].to_vec(),
                    terminator: Terminator::Jump {
                        edge,
                        target: pending.target,
                        arguments,
                        trivial_affine_discards: Vec::new(),
                    },
                });
            }
            PendingNestedBlockGroup::TupleBinding(pending) => {
                let mut pending_stage_blocks = Vec::new();
                let mut next_stage_identity = pending.first_id.get();
                for (index, argument) in pending.arguments.iter().enumerate() {
                    let parameters = &pending.stage_parameters[index];
                    let carried_arguments = parameters
                        .iter()
                        .map(|parameter| parameter.id)
                        .collect::<Vec<_>>();
                    if let LoweredDirectExpression::Boolean { expression } = argument
                        && contains_short_circuit(expression)
                    {
                        let decision = lower_boolean_value_decision(expression);
                        let stage_block_count = boolean_decision_block_count(&decision);
                        let next_stage = block_id(
                            next_stage_identity
                                .checked_add(
                                    u64::try_from(stage_block_count)
                                        .expect("mixed tuple stage count fits a semantic identity"),
                                )
                                .expect("mixed tuple stage block identities advance"),
                        );
                        let mut stage_blocks = Vec::with_capacity(stage_block_count);
                        let entry = emit_reserved_boolean_tuple_stage_blocks(
                            &decision,
                            parameters,
                            parameters.clone(),
                            next_stage,
                            &carried_arguments,
                            next_stage_identity,
                            &mut next_value_identity,
                            &mut next_edge_identity,
                            &mut all_operations,
                            &mut stage_blocks,
                        );
                        assert_eq!(entry.get(), next_stage_identity);
                        pending_stage_blocks.extend(stage_blocks);
                        next_stage_identity = next_stage.get();
                    } else {
                        let operation_start = all_operations.len();
                        let value = emit_direct_expression(
                            argument,
                            parameters,
                            &mut next_value_identity,
                            &mut all_operations,
                        );
                        let mut arguments = carried_arguments;
                        arguments.push(value);
                        let next_stage = block_id(
                            next_stage_identity
                                .checked_add(1)
                                .expect("mixed tuple stage block identity advances"),
                        );
                        let edge = edge_id(next_edge_identity);
                        next_edge_identity = next_edge_identity
                            .checked_add(1)
                            .expect("mixed tuple stage edge identity advances");
                        pending_stage_blocks.push(Some(Block {
                            id: block_id(next_stage_identity),
                            parameters: parameters.clone(),
                            operations: all_operations[operation_start..].to_vec(),
                            terminator: Terminator::Jump {
                                edge,
                                target: next_stage,
                                arguments,
                                trivial_affine_discards: Vec::new(),
                            },
                        }));
                        next_stage_identity = next_stage.get();
                    }
                }
                let parameters = pending
                    .stage_parameters
                    .last()
                    .expect("mixed tuple has a convergence parameter set");
                let edge = edge_id(next_edge_identity);
                next_edge_identity = next_edge_identity
                    .checked_add(1)
                    .expect("mixed tuple convergence edge identity advances");
                pending_stage_blocks.push(Some(Block {
                    id: block_id(next_stage_identity),
                    parameters: parameters.clone(),
                    operations: Vec::new(),
                    terminator: Terminator::Jump {
                        edge,
                        target: pending.target,
                        arguments: parameters[pending.original_parameter_count..]
                            .iter()
                            .map(|parameter| parameter.id)
                            .collect(),
                        trivial_affine_discards: Vec::new(),
                    },
                }));
                blocks.extend(
                    pending_stage_blocks
                        .into_iter()
                        .map(|block| block.expect("every reserved mixed tuple block is finalized")),
                );
            }
        }
    }
    blocks.sort_by_key(|block| block.id);
    let result = ValueDeclaration {
        id: value_id(next_value_identity),
        scalar_type: result_type,
    };
    let (requires, ensures, evidence) = match (result_type, contract_value) {
        (ScalarType::Boolean, Some(KnownDirectScalar::Boolean(value))) => {
            let literal = ScalarTerm::boolean(value);
            let goal = Proposition::Equal(literal.clone(), literal);
            let obligation = obligation_id(
                identity_base
                    .checked_add(1)
                    .expect("contract obligation identity is one-based"),
            );
            (
                vec![goal.clone()],
                vec![ContractClause {
                    obligation,
                    proposition: goal,
                }],
                vec![ObligationEvidence {
                    obligation,
                    route: EvidenceRoute::KernelDerived(PrimitiveJudgment::ReflexiveEquality),
                }],
            )
        }
        (ScalarType::Integer(integer_type), Some(KnownDirectScalar::Integer(value))) => {
            let literal = ScalarTerm::integer(integer_type, value)
                .expect("validated source contract fits the result type");
            let goal = Proposition::Equal(literal.clone(), literal);
            let obligation = obligation_id(
                identity_base
                    .checked_add(1)
                    .expect("contract obligation identity is one-based"),
            );
            (
                vec![goal.clone()],
                vec![ContractClause {
                    obligation,
                    proposition: goal,
                }],
                vec![ObligationEvidence {
                    obligation,
                    route: EvidenceRoute::KernelDerived(PrimitiveJudgment::ClosedIntegerRelation),
                }],
            )
        }
        (_, None) => (Vec::new(), Vec::new(), Vec::new()),
        _ => unreachable!("validated scalar contract matches the machine result type"),
    };
    let mut structural_places = identity_reshuffles
        .structural_places
        .into_iter()
        .map(|place| (place.id, place.kind))
        .collect::<BTreeMap<_, _>>();
    for place in partition_compositions.structural_places {
        merge_content_place_declaration(&mut structural_places, place)
            .expect("checked lowering rejects conflicting structural places");
    }
    Ok(LoweredTerminalPsi {
        semantic_module: TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: terminal_machine,
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            evidence_package_invocations: Vec::new(),
            machines: vec![TerminalMachine {
                id: terminal_machine,
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters,
                result: TerminalMachineResult::Scalar(result),
                structural_places: structural_places
                    .into_iter()
                    .map(|(id, kind)| StructuralPlaceDeclaration { id, kind })
                    .collect(),
                content_entry_claims: identity_reshuffles.entry_claims,
                content_identity_reshuffles: identity_reshuffles.reshuffles,
                content_partition_compositions: partition_compositions.compositions,
                entry: block_id(
                    identity_base
                        .checked_add(1)
                        .expect("machine entry block identity is one-based"),
                ),
                blocks,
                contract: MachineContract {
                    id: contract_id(terminal_machine.get()),
                    crash_routes,
                    requires,
                    ensures,
                },
            }],
        },
        proof_bundle: ProofBundle {
            evidence_producers: Vec::new(),
            evidence,
        },
        debug_map: None,
    })
}

fn finalize_operation_proofs(lowered: &mut LoweredTerminalPsi) -> Result<(), LoweringError> {
    for site in reconstruct_operation_obligations(&lowered.semantic_module)
        .map_err(LoweringError::InvalidTerminalModule)?
    {
        // Some closure builders have already supplied source-derived evidence
        // for contextual call/cleanup obligations. Reconstruct every site,
        // but synthesize only obligations that remain undispatched; the final
        // verifier still checks the retained evidence against the exact goal.
        if lowered
            .proof_bundle
            .evidence
            .iter()
            .any(|evidence| evidence.obligation == site.obligation.id)
        {
            continue;
        }
        let assumptions = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| {
                machine.blocks.iter().any(|block| {
                    block.operations.iter().any(|operation| {
                        matches!(
                            operation.kind,
                            OperationKind::IntegerExactCast { obligation, .. }
                                | OperationKind::ExactIntegerAdd { obligation, .. }
                                | OperationKind::ExactIntegerSubtract { obligation, .. }
                                | OperationKind::ExactIntegerMultiply { obligation, .. }
                                | OperationKind::ExactIntegerShiftRight { obligation, .. }
                                | OperationKind::ExactIntegerShiftLeft { obligation, .. }
                                | OperationKind::ExactIntegerDivide { obligation, .. }
                                | OperationKind::ExactIntegerRemainder { obligation, .. }
                                if obligation == site.obligation.id
                        )
                    })
                })
            })
            .map(|machine| machine.contract.requires.as_slice())
            .unwrap_or_default();
        let proof = proof_from_available_facts(
            &site.obligation.proposition,
            assumptions,
            &site.semantic_axioms,
        );
        let proof = proof.ok_or(LoweringError::ExactIntegerCastProofUnavailable(
            site.obligation.id,
        ))?;
        lowered.proof_bundle.evidence.push(ObligationEvidence {
            obligation: site.obligation.id,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(site.obligation.id.get())
                    .expect("terminal obligations have nonzero identities"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof,
            }),
        });
    }
    lowered
        .proof_bundle
        .evidence
        .sort_by_key(|evidence| evidence.obligation);
    Ok(())
}

fn proof_from_available_facts(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    if goal == &Proposition::Truth {
        return Some(ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
        });
    }
    if matches!(goal, Proposition::Equal(left, right) if left == right) {
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
        });
    }
    if let Some(index) = assumptions.iter().position(|assumption| assumption == goal) {
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::Assumption { index },
        });
    }
    if let Some(index) = semantic_axioms.iter().position(|axiom| axiom == goal) {
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::SemanticAxiom { index },
        });
    }
    let Proposition::Conjunction(conjuncts) = goal else {
        return None;
    };
    let proofs = conjuncts
        .iter()
        .map(|conjunct| proof_from_available_facts(conjunct, assumptions, semantic_axioms))
        .collect::<Option<Vec<_>>>()?;
    Some(ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::ConjunctionIntroduction(proofs),
    })
}

fn emit_boolean_expression(
    expression: &LoweredBooleanReturnExpression,
    parameters: &[ValueDeclaration],
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
) -> ValueId {
    match expression {
        LoweredBooleanReturnExpression::Constant { value } => {
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after a Boolean literal");
            let operation = operations.allocate();
            operations.push(Operation {
                id: operation,
                result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
                    id,
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::BooleanConstant { value: *value },
            });
            id
        }
        LoweredBooleanReturnExpression::IntegerComparison { kind, left, right } => {
            let left = emit_direct_expression(left, parameters, next_value_identity, operations);
            let right = emit_direct_expression(right, parameters, next_value_identity, operations);
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after integer comparison");
            let operation = operations.allocate();
            operations.push(Operation {
                id: operation,
                result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
                    id,
                    scalar_type: ScalarType::Boolean,
                }),
                kind: kind.operation(left, right),
            });
            id
        }
        LoweredBooleanReturnExpression::Parameter { position }
        | LoweredBooleanReturnExpression::Local { position } => parameters[*position].id,
        LoweredBooleanReturnExpression::StructuralField { source, field } => {
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after a structural Boolean load");
            let operation = operations.allocate();
            operations.push(Operation {
                id: operation,
                result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
                    id,
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::BooleanStructuralField {
                    source: *source,
                    field: *field,
                },
            });
            id
        }
        LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. } => {
            unreachable!("shared Boolean members resolve before terminal operation emission")
        }
        LoweredBooleanReturnExpression::Not { operand } => {
            let operand =
                emit_boolean_expression(operand, parameters, next_value_identity, operations);
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after Boolean negation");
            let operation = operations.allocate();
            operations.push(Operation {
                id: operation,
                result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
                    id,
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::BooleanNot { operand },
            });
            id
        }
        LoweredBooleanReturnExpression::Equal { left, right } => {
            let left = emit_boolean_expression(left, parameters, next_value_identity, operations);
            let right = emit_boolean_expression(right, parameters, next_value_identity, operations);
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after Boolean equality");
            let operation = operations.allocate();
            operations.push(Operation {
                id: operation,
                result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
                    id,
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::BooleanEqual { left, right },
            });
            id
        }
        LoweredBooleanReturnExpression::And { .. } | LoweredBooleanReturnExpression::Or { .. } => {
            unreachable!("short-circuit Boolean expressions lower through terminal control")
        }
    }
}

fn emit_scalar_binding(
    binding: &LoweredScalarBinding,
    parameters: &[ValueDeclaration],
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
    call_emission: &mut CallEmissionContext<'_>,
) -> Result<ValueId, LoweringError> {
    let LoweredScalarBinding::DirectCall(call) = binding else {
        let LoweredScalarBinding::Expression(expression) = binding else {
            unreachable!()
        };
        return Ok(emit_direct_expression(
            expression,
            parameters,
            next_value_identity,
            operations,
        ));
    };
    let arguments = call
        .arguments
        .iter()
        .map(|argument| ValueDeclaration {
            id: emit_direct_expression(argument, parameters, next_value_identity, operations),
            scalar_type: argument.scalar_type(),
        })
        .collect::<Vec<_>>();
    emit_direct_call_operation(
        call,
        &call.crash_continuations,
        parameters,
        &arguments,
        next_value_identity,
        operations,
        call_emission,
    )
}

#[allow(clippy::too_many_arguments)]
fn emit_staged_scalar_call_binding(
    call: &LoweredDirectCallBinding,
    stage_parameters: &[ValueDeclaration],
    stage_parameter_types: &[ScalarType],
    stage_block_parameters: Vec<ValueDeclaration>,
    stage_block: BlockId,
    next_block_identity: &mut u64,
    next_value_identity: &mut u64,
    next_edge_identity: &mut u64,
    operations: &mut OperationBuffer,
    call_emission: &mut CallEmissionContext<'_>,
) -> Result<(BlockId, Vec<Block>), LoweringError> {
    debug_assert!(
        call.arguments
            .iter()
            .any(direct_expression_contains_short_circuit)
    );
    let caller_value_count = stage_parameters.len();
    let mut current_block = stage_block;
    let mut current_parameters = stage_parameters.to_vec();
    let mut current_block_parameters = stage_block_parameters;
    let mut blocks = Vec::new();

    for (argument_index, argument) in call.arguments.iter().enumerate() {
        let mut next_stage_types = stage_parameter_types.to_vec();
        next_stage_types.extend(
            call.arguments[..=argument_index]
                .iter()
                .map(LoweredDirectExpression::scalar_type),
        );
        let next_stage_parameters = next_stage_types
            .into_iter()
            .map(|scalar_type| {
                let parameter = ValueDeclaration {
                    id: value_id(*next_value_identity),
                    scalar_type,
                };
                *next_value_identity = next_value_identity
                    .checked_add(1)
                    .expect("staged call-argument parameter identities advance");
                parameter
            })
            .collect::<Vec<_>>();
        let carried_arguments = current_parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect::<Vec<_>>();

        let next_stage = if let LoweredDirectExpression::Boolean { expression } = argument
            && contains_short_circuit(expression)
        {
            let decision = lower_boolean_value_decision(expression);
            let decision_block_count = boolean_decision_block_count(&decision);
            let first_child_identity = *next_block_identity;
            let next_stage = block_id(
                first_child_identity
                    .checked_add(
                        u64::try_from(decision_block_count - 1)
                            .expect("staged call decision count fits a semantic identity"),
                    )
                    .expect("staged call decision block identities advance"),
            );
            *next_block_identity = next_stage
                .get()
                .checked_add(1)
                .expect("staged call argument blocks advance");
            let first_reserved_identity = first_child_identity
                .checked_sub(1)
                .expect("staged call decision blocks follow their root");
            let mut decision_blocks = Vec::with_capacity(decision_block_count);
            let entry = emit_reserved_boolean_tuple_stage_blocks(
                &decision,
                &current_parameters,
                current_block_parameters,
                next_stage,
                &carried_arguments,
                first_reserved_identity,
                next_value_identity,
                next_edge_identity,
                operations,
                &mut decision_blocks,
            );
            assert_eq!(entry.get(), first_reserved_identity);
            let mut decision_blocks = decision_blocks
                .into_iter()
                .map(|block| block.expect("every staged call decision block is finalized"));
            let mut root = decision_blocks
                .next()
                .expect("a short-circuit call argument has a decision root");
            root.id = current_block;
            blocks.push(root);
            blocks.extend(decision_blocks);
            next_stage
        } else {
            let next_stage = block_id(*next_block_identity);
            *next_block_identity = next_block_identity
                .checked_add(1)
                .expect("staged direct call-argument blocks advance");
            let operation_start = operations.len();
            let value = emit_direct_expression(
                argument,
                &current_parameters,
                next_value_identity,
                operations,
            );
            let mut arguments = carried_arguments;
            arguments.push(value);
            let edge = edge_id(*next_edge_identity);
            *next_edge_identity = next_edge_identity
                .checked_add(1)
                .expect("staged direct call-argument edge identities advance");
            blocks.push(Block {
                id: current_block,
                parameters: current_block_parameters,
                operations: operations[operation_start..].to_vec(),
                terminator: Terminator::Jump {
                    edge,
                    target: next_stage,
                    arguments,
                    trivial_affine_discards: Vec::new(),
                },
            });
            next_stage
        };
        current_block = next_stage;
        current_parameters = next_stage_parameters;
        current_block_parameters = current_parameters.clone();
    }

    let continuation = block_id(*next_block_identity);
    *next_block_identity = next_block_identity
        .checked_add(1)
        .expect("staged call continuation block identities advance");
    let operation_start = operations.len();
    let arguments = current_parameters[caller_value_count..].to_vec();
    let result = emit_direct_call_operation(
        call,
        &call.parameter_relative_crash_routes,
        &arguments,
        &arguments,
        next_value_identity,
        operations,
        call_emission,
    )?;
    let mut continuation_arguments = current_parameters[..caller_value_count]
        .iter()
        .map(|parameter| parameter.id)
        .collect::<Vec<_>>();
    continuation_arguments.push(result);
    let edge = edge_id(*next_edge_identity);
    *next_edge_identity = next_edge_identity
        .checked_add(1)
        .expect("staged call continuation edge identities advance");
    blocks.push(Block {
        id: current_block,
        parameters: current_block_parameters,
        operations: operations[operation_start..].to_vec(),
        terminator: Terminator::Jump {
            edge,
            target: continuation,
            arguments: continuation_arguments,
            trivial_affine_discards: Vec::new(),
        },
    });
    Ok((continuation, blocks))
}

fn emit_direct_call_operation(
    call: &LoweredDirectCallBinding,
    crash_routes: &[psi_checked_trees::CrashRouteBucket],
    crash_values: &[ValueDeclaration],
    arguments: &[ValueDeclaration],
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
    call_emission: &mut CallEmissionContext<'_>,
) -> Result<ValueId, LoweringError> {
    let callee = call_emission
        .machine_ids
        .iter()
        .find_map(|(source, terminal)| (*source == call.target_machine).then_some(*terminal))
        .ok_or(LoweringError::Unsupported(
            "direct scalar call target is absent from the terminal closure",
        ))?;
    let crash_continuations = lower_checked_crash_route_buckets(crash_routes, crash_values)?;
    let requirement_count = call_emission
        .requirement_counts
        .iter()
        .find_map(|(source, count)| (*source == call.target_machine).then_some(*count))
        .ok_or(LoweringError::Unsupported(
            "direct scalar call target has no prepared contract",
        ))?;
    let requirement_obligations = (0..requirement_count)
        .map(|_| call_emission.allocate_requirement())
        .collect::<Result<Vec<_>, _>>()?;
    let result = value_id(*next_value_identity);
    *next_value_identity = next_value_identity
        .checked_add(1)
        .expect("generated value identity advances after a direct call");
    let operation = operations.allocate();
    operations.push(Operation {
        id: operation,
        result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
            id: result,
            scalar_type: call.result_type,
        }),
        kind: OperationKind::Call {
            callee,
            arguments: arguments.iter().map(|argument| argument.id).collect(),
            requirement_obligations,
            crash_continuations,
        },
    });
    Ok(result)
}

fn emit_direct_expression(
    expression: &LoweredDirectExpression,
    parameters: &[ValueDeclaration],
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
) -> ValueId {
    match expression {
        LoweredDirectExpression::Parameter { position, .. }
        | LoweredDirectExpression::Local { position, .. } => parameters[*position].id,
        LoweredDirectExpression::IntegerLiteral { value, scalar_type } => {
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after a literal");
            let operation = operations.allocate();
            operations.push(Operation {
                id: operation,
                result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
                    id,
                    scalar_type: *scalar_type,
                }),
                kind: OperationKind::IntegerConstant { value: *value },
            });
            id
        }
        LoweredDirectExpression::IntegerBinary {
            kind,
            scalar_type,
            left,
            right,
        } => {
            let left = emit_direct_expression(left, parameters, next_value_identity, operations);
            let right = emit_direct_expression(right, parameters, next_value_identity, operations);
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after a binary operation");
            let operation = operations.allocate();
            operations.push(Operation {
                id: operation,
                result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
                    id,
                    scalar_type: *scalar_type,
                }),
                kind: kind.operation(operation, left, right),
            });
            id
        }
        LoweredDirectExpression::IntegerBitwiseNot {
            scalar_type,
            operand,
        } => {
            let operand =
                emit_direct_expression(operand, parameters, next_value_identity, operations);
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after bitwise complement");
            let operation = operations.allocate();
            operations.push(Operation {
                id: operation,
                result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
                    id,
                    scalar_type: *scalar_type,
                }),
                kind: OperationKind::IntegerBitwiseNot { operand },
            });
            id
        }
        LoweredDirectExpression::IntegerWiden {
            scalar_type,
            operand,
        } => {
            let operand =
                emit_direct_expression(operand, parameters, next_value_identity, operations);
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after integer widening");
            let operation = operations.allocate();
            operations.push(Operation {
                id: operation,
                result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
                    id,
                    scalar_type: *scalar_type,
                }),
                kind: OperationKind::IntegerWiden { operand },
            });
            id
        }
        LoweredDirectExpression::IntegerExactCast {
            scalar_type,
            operand,
        } => {
            let operand =
                emit_direct_expression(operand, parameters, next_value_identity, operations);
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after an exact integer cast");
            let operation = operations.allocate();
            operations.push(Operation {
                id: operation,
                result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
                    id,
                    scalar_type: *scalar_type,
                }),
                kind: OperationKind::IntegerExactCast {
                    operand,
                    obligation: obligation_id(
                        operation
                            .get()
                            .checked_add(1)
                            .expect("exact-cast obligation follows its operation identity"),
                    ),
                },
            });
            id
        }
        LoweredDirectExpression::Boolean { expression } => {
            emit_boolean_expression(expression, parameters, next_value_identity, operations)
        }
    }
}

fn build_debug_map(
    plan: &CheckedTerminalMachineDebugPlan,
    module: &TerminalModule,
) -> Result<TerminalDebugMap, LoweringError> {
    let terminal_machine = module
        .machines
        .first()
        .expect("the selected entry machine is first in its terminal call closure");
    let source_states = &plan.states;
    let has_source_file = |span: psi_source::SourceSpan| {
        plan.source_files
            .iter()
            .any(|file| file.source_id == span.source_id)
    };
    let mut subjects = Vec::<(DebugSubject, psi_source::SourceSpan)>::new();
    let mut push = |subject, span| {
        if let Some(span) = span {
            subjects.push((subject, span));
        }
    };

    push(
        DebugSubject::Machine(terminal_machine.id),
        plan.machine_span,
    );
    let contract_span = plan.contract_span;
    push(
        DebugSubject::Contract(terminal_machine.contract.id),
        contract_span,
    );
    for clause in &terminal_machine.contract.ensures {
        push(DebugSubject::Obligation(clause.obligation), contract_span);
    }

    for (index, block) in terminal_machine.blocks.iter().enumerate() {
        let source_state = source_states
            .get(index)
            .or_else(|| source_states.last())
            .expect("an accepted source machine has at least one state");
        push(DebugSubject::Block(block.id), source_state.state_span);
        for (edge_index, edge) in block.terminator.edges().enumerate() {
            let transition_span = source_state
                .transition_spans
                .get(edge_index)
                .or_else(|| {
                    (source_state.transition_spans.len() == 1)
                        .then(|| &source_state.transition_spans[0])
                })
                .copied()
                .filter(|span| *span != psi_source::SourceSpan::default())
                .filter(|span| has_source_file(*span));
            push(
                DebugSubject::Edge(edge),
                transition_span.or(source_state.state_span),
            );
        }
        for (operation_index, operation) in block.operations.iter().enumerate() {
            let source_span = source_state
                .operation_spans
                .get(operation_index)
                .copied()
                .filter(|span| *span != psi_source::SourceSpan::default())
                .filter(|span| has_source_file(*span));
            if let Some(source_span) = source_span {
                push(DebugSubject::Operation(operation.id), Some(source_span));
                push(
                    DebugSubject::Value(operation.result.expect_scalar().id),
                    Some(source_span),
                );
            } else {
                push(
                    DebugSubject::Operation(operation.id),
                    source_state.state_span,
                );
                push(
                    DebugSubject::Value(operation.result.expect_scalar().id),
                    source_state.state_span,
                );
            }
        }
        for (parameter_index, parameter) in block.parameters.iter().enumerate() {
            if let Some(source_span) = source_state
                .parameter_spans
                .get(parameter_index)
                .copied()
                .flatten()
            {
                push(DebugSubject::Value(parameter.id), Some(source_span));
            }
        }
    }

    if let Some(entry_state) = source_states.first() {
        for (parameter_index, parameter) in terminal_machine.parameters.iter().enumerate() {
            if let Some(source_span) = entry_state
                .parameter_spans
                .get(parameter_index)
                .copied()
                .flatten()
            {
                push(DebugSubject::Value(parameter.id), Some(source_span));
            }
        }
    }
    push(
        DebugSubject::Value(
            terminal_machine
                .result
                .scalar()
                .expect("the checked scalar producer emits a scalar result")
                .id,
        ),
        plan.machine_span,
    );

    subjects.sort_by_key(|(subject, _)| *subject);
    subjects.dedup_by_key(|(subject, _)| *subject);
    let mut source_ids = subjects
        .iter()
        .map(|(_, span)| span.source_id.0)
        .collect::<Vec<_>>();
    source_ids.sort_unstable();
    source_ids.dedup();

    let mut files = Vec::with_capacity(source_ids.len());
    for (index, source_id) in source_ids.iter().copied().enumerate() {
        let source_file = plan
            .source_files
            .iter()
            .find(|file| file.source_id == psi_source::SourceId(source_id))
            .ok_or(LoweringError::MissingDebugSourceFile(source_id))?;
        let id = DebugFileId::new(
            u32::try_from(index)
                .map_err(|_| LoweringError::DebugSourceFileCountOverflow)?
                .checked_add(1)
                .ok_or(LoweringError::DebugSourceFileCountOverflow)?,
        )
        .expect("one-based debug file identity is nonzero");
        files.push(DebugSourceFile {
            id,
            origin: match source_file.origin {
                psi_source::SourceOrigin::User => DebugSourceOrigin::User,
                psi_source::SourceOrigin::Toolchain => DebugSourceOrigin::Toolchain,
            },
            byte_len: u64::try_from(source_file.source.len())
                .map_err(|_| LoweringError::DebugSourceLengthOverflow)?,
            digest: source_digest(source_file.source.as_bytes()),
            path: source_file.path.to_string_lossy().into_owned(),
        });
    }

    let sites = subjects
        .into_iter()
        .map(|(subject, span)| {
            let file_index = source_ids
                .binary_search(&span.source_id.0)
                .expect("source identity was collected above");
            let file = DebugFileId::new(
                u32::try_from(file_index)
                    .expect("validated debug file count fits u32")
                    .checked_add(1)
                    .expect("one-based debug file identity fits u32"),
            )
            .expect("one-based debug file identity is nonzero");
            Ok(DebugSite {
                subject,
                span: DebugSourceSpan {
                    file,
                    start: u64::try_from(span.span.start)
                        .map_err(|_| LoweringError::DebugSourceLengthOverflow)?,
                    end: u64::try_from(span.span.end)
                        .map_err(|_| LoweringError::DebugSourceLengthOverflow)?,
                },
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let debug_map = TerminalDebugMap {
        semantic: terminal_psi_identity(module).map_err(LoweringError::DebugSemanticCodec)?,
        files,
        sites,
    };
    validate_debug_map(module, &debug_map).map_err(LoweringError::InvalidDebugMap)?;
    Ok(debug_map)
}
fn unsupported<T>(message: &'static str) -> Result<T, LoweringError> {
    Err(LoweringError::Unsupported(message))
}

macro_rules! id_constructor {
    ($function:ident, $type:ty) => {
        fn $function(raw: u64) -> $type {
            <$type>::new(raw).expect("fixed terminal-Psi identities are nonzero")
        }
    };
}

id_constructor!(value_id, ValueId);
id_constructor!(structural_type_id, StructuralTypeId);
id_constructor!(structural_field_id, StructuralFieldId);
id_constructor!(structural_domain_id, StructuralDomainId);
id_constructor!(service_id, ServiceId);
id_constructor!(boundary_machine_id, BoundaryMachineId);
id_constructor!(machine_id, MachineId);
id_constructor!(block_id, BlockId);
id_constructor!(place_id, PlaceId);
id_constructor!(claim_id, ClaimId);
id_constructor!(operation_id, OperationId);
id_constructor!(edge_id, EdgeId);
id_constructor!(contract_id, ContractId);
id_constructor!(obligation_id, ObligationId);
id_constructor!(proposition_id, PropositionId);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweringError {
    MachineNotFound(String),
    AmbiguousMachineName(String),
    DebugSourceFileCountOverflow,
    DebugSourceLengthOverflow,
    MissingDebugSourceFile(usize),
    DebugSemanticCodec(psi_terminal_codec::CodecError),
    InvalidDebugMap(psi_terminal_codec::DebugMapError),
    InvalidTerminalModule(psi_terminal_verifier::ModuleError),
    ExactIntegerCastProofUnavailable(ObligationId),
    Unsupported(&'static str),
    InvalidPsiIntegerType,
    UnlandedIntegerLiteral,
    IntegerLandingMismatch,
    IntegerLiteralOutsideSupportedMagnitude,
    IntegerLiteralOutsidePsiType,
    ContentConservationFingerprintMismatch {
        expected: u64,
        actual: u64,
    },
    ContentIdentityFactOwnerMismatch,
    ContentPartitionFactOwnerMismatch,
    ContentPartitionNotConservation,
    ContentPartitionInputClaimNotLowered,
    ContentPartitionInputClaimBindingMismatch,
    ContentEntryClaimRequiresEntryPlace,
    ContentEntryClaimHasNoProjection,
    ContentEntryClaimMapsMultiplePlaces,
    DuplicateContentEntryClaimInput,
    OverlappingContentEntryClaimInput,
    DuplicateContentPartitionSubstitution,
    DuplicateContentPartitionComposition,
    ContentPartitionResultRewriteUnsupported,
    ContentPartitionDerivedSourceUnsupported,
    ContentPartitionSubstitutionCoverageMismatch,
    ContentPartitionReplayMismatch,
    UnknownContentClaimIdentity,
    ContentIdentityInputParameterMismatch,
    ContentIdentityNotDirectEquality,
    ContentIdentityProjectionMismatch,
    ContentIdentityDirectionMismatch,
    ContentIdentityRootMismatch,
    ContentIdentityClaimMapsMultiplePlaces,
    DuplicateContentIdentityProjection,
    DuplicateContentIdentityInput,
    DuplicateContentIdentityOutput,
    OverlappingContentIdentityInput,
    OverlappingContentIdentityOutput,
    ContentProjectionAlgebraMismatch(ContentProjectionIdentity),
    CrashFrontierClaimNotLowered(PermissionClaimIdentity),
    InvalidContentDomainIdentity,
    ZeroContentProjectionFingerprint,
    ContentTermNestingTooDeep,
    ConflictingContentPlaceRoot {
        id: PlaceId,
        first: StructuralPlaceKind,
        second: StructuralPlaceKind,
    },
    InvalidContentProposition(PropositionError),
    InvalidCrashPredicate(PropositionError),
}

impl std::fmt::Display for LoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LoweringError {}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_language_semantics::{
        PermissionEventSource, SemanticDomainId,
        content::{
            ContentCaseSegment, ContentConservationEquation, ContentConservationOwnerKind,
            ContentFieldSegment,
        },
    };
    use psi_source_files_to_tokens::Lexer;
    use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use psi_symbols::SymbolHandle;
    use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use psi_tokens_to_syntax_trees::parse_syntax_trees;
    use psi_typed_trees_to_checked_trees::lower_typed_trees;

    #[test]
    fn shared_boolean_comparison_normalization_rejects_two_runtime_sides() {
        let comparison = LoweredBooleanReturnExpression::Equal {
            left: Box::new(LoweredBooleanReturnExpression::Parameter { position: 0 }),
            right: Box::new(LoweredBooleanReturnExpression::Parameter { position: 1 }),
        };
        assert!(normalize_shared_boolean_comparison_leaves(&comparison).is_none());

        let local_comparison = LoweredBooleanReturnExpression::Equal {
            left: Box::new(LoweredBooleanReturnExpression::Local { position: 1 }),
            right: Box::new(LoweredBooleanReturnExpression::Constant { value: false }),
        };
        assert!(normalize_shared_boolean_comparison_leaves(&local_comparison).is_none());
    }

    #[test]
    fn scalar_crash_disjunction_lowers_to_canonical_terminal_propositions() {
        let values = vec![
            ValueDeclaration {
                id: value_id(2),
                scalar_type: ScalarType::Boolean,
            },
            ValueDeclaration {
                id: value_id(1),
                scalar_type: ScalarType::Boolean,
            },
        ];
        let proposition = checked_boolean_proposition(
            &CheckedBooleanExpression::Or {
                left: Box::new(CheckedBooleanExpression::Parameter { position: 0 }),
                right: Box::new(CheckedBooleanExpression::Parameter { position: 1 }),
            },
            &values,
        )
        .expect("scalar disjunction lowers");
        let Proposition::Disjunction(disjuncts) = &proposition else {
            panic!("scalar disjunction retains proposition structure")
        };
        assert_eq!(disjuncts.len(), 2);
        let keys = disjuncts
            .iter()
            .map(|disjunct| psi_terminal_codec::canonical_proposition_order_key(disjunct).unwrap())
            .collect::<Vec<_>>();
        assert!(keys[0] < keys[1]);
        PropositionContext::from_value_types(
            values.iter().map(|value| (value.id, value.scalar_type)),
        )
        .unwrap()
        .validate(&proposition)
        .expect("scalar disjunction is well typed");
    }

    fn unit_claim_at(
        machine: SymbolHandle,
        state: SymbolHandle,
        ordinal: u32,
    ) -> PermissionClaimIdentity {
        PermissionClaimIdentity::Established {
            machine_symbol: machine,
            state_symbol: state,
            source: PermissionEventSource::StateEntry,
            ordinal,
        }
    }

    fn unit_claim(machine: SymbolHandle, state: SymbolHandle) -> PermissionClaimIdentity {
        unit_claim_at(machine, state, 0)
    }

    fn nominal_affine_unit_checked_fixture() -> CheckedTrees {
        let source = r#"
            data Token {}
            machine Token::drop(&mut self) {}
            data Root {}
            machine Root::enter(token: Token) {}
        "#;
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        lower_typed_trees(typed).expect("check")
    }

    fn nominal_affine_wide_scalar_unit_checked_fixture() -> CheckedTrees {
        let source = r#"
            data Token { flag: bool; tag: u8; delta: i16; payload: u64; address: addr; }
            machine Token::drop(&mut self) {}
            data Root {}
            machine Root::enter(token: Token) {}
        "#;
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        lower_typed_trees(typed).expect("check")
    }

    fn ordered_one_executable_nominal_affine_checked_fixture() -> CheckedTrees {
        let source = r#"
            data Helper {}
            machine Helper::touch() {}
            data First {}
            machine First::drop(&mut self) { Helper::touch(); }
            data Second {}
            machine Second::drop(&mut self) {}
            data Root {}
            machine Root::enter(first: First, second: Second) {}
        "#;
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        lower_typed_trees(typed).expect("check")
    }

    #[test]
    fn nominal_affine_unit_cleanup_lowers_exact_target_into_terminal_closure() {
        let checked = nominal_affine_unit_checked_fixture();
        let [plan] = checked
            .facts
            .flow
            .terminal_nominal_affine_unit_cleanups
            .machines
            .as_slice()
        else {
            panic!("expected one checked nominal-cleanup plan")
        };
        let lowered = lower_nominal_affine_unit_cleanup_machine(&checked, plan)
            .expect("strict checked nominal cleanup should lower in memory");
        assert_eq!(
            lowered.semantic_module.machines.len(),
            2,
            "cleanup target must be retained as executable closure work"
        );
        let entry = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == lowered.semantic_module.entry)
            .expect("terminal entry");
        let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator
        else {
            panic!("nominal cleanup requires its distinct terminal return")
        };
        assert_eq!(cleanups[0].place, entry.structural_parameters[0].place);
        assert_eq!(
            cleanups[0].structural_type,
            entry.structural_parameters[0].structural_type
        );
        let target = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == cleanups[0].cleanup_machine)
            .expect("terminal cleanup target");
        assert_eq!(target.attachment, Some(cleanups[0].structural_type));
        assert!(target.structural_parameters.is_empty());
        assert!(target.blocks[0].operations.is_empty());
        assert!(matches!(
            &target.blocks[0].terminator,
            Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } if trivial_affine_discards.is_empty()
        ));
        psi_terminal_verifier::validate_module(&lowered.semantic_module)
            .expect("independent verifier accepts exact nominal cleanup closure");
        let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
            .expect("verified nominal cleanup should encode canonically");
        assert_eq!(
            psi_terminal_codec::decode_module(&bytes)
                .expect("canonical nominal cleanup should decode"),
            lowered.semantic_module
        );

        let entry_name = checked
            .facts
            .flow
            .terminal_machines
            .machines
            .iter()
            .find(|selection| selection.machine == plan.machine.machine)
            .expect("nominal cleanup terminal selection")
            .name
            .clone();
        let public = lower_machine(&checked, &entry_name)
            .expect("source nominal cleanup should cross the public lowering entry");
        assert!(matches!(
            public.semantic_module.machines[0].blocks[0].terminator,
            Terminator::ReturnUnitNominalAffine { .. }
        ));
    }

    #[test]
    fn nominal_affine_wide_scalar_unit_cleanup_retains_exact_field_shape() {
        let checked = nominal_affine_wide_scalar_unit_checked_fixture();
        let [plan] = checked
            .facts
            .flow
            .terminal_nominal_affine_unit_cleanups
            .machines
            .as_slice()
        else {
            panic!("expected one checked wide-scalar nominal-cleanup plan")
        };
        let lowered = lower_nominal_affine_unit_cleanup_machine(&checked, plan)
            .expect("wide flat scalar nominal cleanup should lower");
        let entry = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == lowered.semantic_module.entry)
            .expect("terminal entry");
        let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator
        else {
            panic!("wide scalar nominal cleanup requires its distinct terminal return")
        };
        let cleanup_type = lowered
            .semantic_module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == cleanups[0].structural_type)
            .expect("nominal cleanup structural type");
        let StructuralTypeShape::Record { fields } = &cleanup_type.shape else {
            panic!("nominal scalar cleanup retains a record")
        };
        let [flag, tag, delta, payload, address] = fields.as_slice() else {
            panic!("nominal scalar cleanup retains every flat field")
        };
        for (field, identity, primitive) in [
            (flag, "flag", PrimitiveType::Bool),
            (tag, "tag", PrimitiveType::U8),
            (delta, "delta", PrimitiveType::I16),
            (payload, "payload", PrimitiveType::U64),
            (address, "address", PrimitiveType::Addr),
        ] {
            assert_eq!(field.identity, identity);
            assert!(!field.relevance.is_erased());
            let StructuralFieldType::Scalar(actual) = &field.field_type else {
                panic!("wide nominal cleanup field retains its scalar carrier")
            };
            assert_eq!(
                *actual,
                terminal_scalar_type(primitive).expect("fixture uses terminal-supported fields")
            );
        }

        for bad_field_type in [
            CheckedUnitStructuralFieldType::Scalar(PrimitiveType::F64),
            CheckedUnitStructuralFieldType::Erased {
                type_identity: "named(name(Erased))".to_owned(),
            },
            CheckedUnitStructuralFieldType::Structural {
                type_identity: plan.machine.attachment_type_identity.clone(),
            },
        ] {
            let mut stale = checked.clone();
            let shape = stale
                .facts
                .flow
                .terminal_nominal_affine_unit_cleanups
                .structural_types
                .iter_mut()
                .find(|shape| shape.identity == plan.cleanups[0].type_identity)
                .expect("nominal cleanup shape");
            let CheckedUnitStructuralTypeShape::Record { fields } = &mut shape.shape else {
                panic!("scalar fixture has a record shape")
            };
            fields[0].field_type = bad_field_type;
            let stale_plan = stale
                .facts
                .flow
                .terminal_nominal_affine_unit_cleanups
                .machines[0]
                .clone();
            assert!(matches!(
                lower_nominal_affine_unit_cleanup_machine(&stale, &stale_plan),
                Err(LoweringError::Unsupported(
                    "nominal affine Unit parameter is outside the bounded record shape"
                ))
            ));
        }
    }

    #[test]
    fn nominal_affine_unit_cleanup_lowering_rejects_stale_checked_joins() {
        let checked = nominal_affine_unit_checked_fixture();
        let original = checked
            .facts
            .flow
            .terminal_nominal_affine_unit_cleanups
            .machines[0]
            .clone();

        let mut stale = original.clone();
        stale.cleanups[0].source_parameter_index = 1;
        assert!(matches!(
            lower_nominal_affine_unit_cleanup_machine(&checked, &stale),
            Err(LoweringError::Unsupported(
                "nominal affine Unit cleanup signature or coordinates drifted"
            ))
        ));

        let mut stale = original.clone();
        stale.cleanups[0].cleanup_contract_fingerprint ^= 1;
        assert!(matches!(
            lower_nominal_affine_unit_cleanup_machine(&checked, &stale),
            Err(LoweringError::Unsupported(
                "nominal cleanup target identity or bounded signature drifted"
            ))
        ));

        let mut stale_checked = nominal_affine_unit_checked_fixture();
        let mut stale_plan = stale_checked
            .facts
            .flow
            .terminal_nominal_affine_unit_cleanups
            .machines[0]
            .clone();
        let cleanup_target = stale_checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|machine| machine.machine == stale_plan.cleanups[0].cleanup_machine)
            .expect("cleanup target plan");
        cleanup_target.contract_fingerprint ^= 1;
        stale_plan.cleanups[0].cleanup_contract_fingerprint = cleanup_target.contract_fingerprint;
        assert!(matches!(
            lower_nominal_affine_unit_cleanup_machine(&stale_checked, &stale_plan),
            Err(LoweringError::Unsupported(
                "nominal cleanup target identity or bounded signature drifted"
            ))
        ));

        let mut stale_checked = nominal_affine_unit_checked_fixture();
        let stale_plan = stale_checked
            .facts
            .flow
            .terminal_nominal_affine_unit_cleanups
            .machines[0]
            .clone();
        let cleanup_identity = stale_plan.cleanups[0].type_identity.clone();
        let shape = stale_checked
            .facts
            .flow
            .terminal_nominal_affine_unit_cleanups
            .structural_types
            .iter_mut()
            .find(|shape| shape.identity == cleanup_identity)
            .expect("nominal cleanup shape");
        shape.shape = CheckedUnitStructuralTypeShape::FixedArray {
            element_type_identity: cleanup_identity,
            length: 1,
        };
        assert!(matches!(
            lower_nominal_affine_unit_cleanup_machine(&stale_checked, &stale_plan),
            Err(LoweringError::Unsupported(
                "nominal affine Unit parameter is outside the bounded record shape"
            ))
        ));

        let mut stale_checked = nominal_affine_unit_checked_fixture();
        stale_checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .push(original.machine.clone());
        assert!(matches!(
            lower_nominal_affine_unit_cleanup_machine(&stale_checked, &original),
            Err(LoweringError::Unsupported(
                "nominal affine Unit machine is also published in the trivial lane"
            ))
        ));
    }

    #[test]
    fn ordered_nominal_cleanup_lowering_deduplicates_a_shared_helper_across_two_actions() {
        let mut checked = ordered_one_executable_nominal_affine_checked_fixture();
        let plan = checked
            .facts
            .flow
            .terminal_nominal_affine_unit_cleanups
            .machines[0]
            .clone();
        let [empty_cleanup, executable_cleanup] = plan.cleanups.as_slice() else {
            panic!("fixture has two ordered cleanup actions")
        };
        let executable_operation = checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(executable_cleanup.cleanup_machine)
            .expect("executable cleanup target")
            .operations[0]
            .clone();
        let empty_target = checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|target| target.machine == empty_cleanup.cleanup_machine)
            .expect("empty cleanup target");
        empty_target.operations.insert(0, executable_operation);
        let CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index, ..
        } = empty_target
            .operations
            .last_mut()
            .expect("cleanup target return")
        else {
            panic!("cleanup target ends in Unit return")
        };
        *statement_index = 1;

        let lowered = lower_nominal_affine_unit_cleanup_machine(&checked, &plan)
            .expect("two executable cleanup actions may share one exact helper");
        assert_eq!(
            lowered.semantic_module.machines.len(),
            4,
            "the shared helper appears once in the exact machine closure"
        );
        let entry = &lowered.semantic_module.machines[0];
        let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator
        else {
            panic!("ordered nominal cleanup return")
        };
        let helper_ids = cleanups
            .iter()
            .map(|cleanup| {
                let target = lowered
                    .semantic_module
                    .machines
                    .iter()
                    .find(|machine| machine.id == cleanup.cleanup_machine)
                    .expect("cleanup target");
                let [operation] = target.blocks[0].operations.as_slice() else {
                    panic!("each cleanup target calls one helper")
                };
                let OperationKind::CallUnit { callee, .. } = operation.kind else {
                    panic!("cleanup helper call")
                };
                callee
            })
            .collect::<Vec<_>>();
        assert_eq!(helper_ids[0], helper_ids[1]);

        let mut contextual = plan;
        contextual.caller_requirements.push(
            psi_checked_trees::CheckedUnitNominalAffineCallerRequirementPlan {
                source_parameter_index: 0,
                field_identity: "flag".to_owned(),
                expected: true,
            },
        );
        assert!(matches!(
            lower_nominal_affine_unit_cleanup_machine(&checked, &contextual),
            Err(LoweringError::Unsupported(
                "contextual nominal cleanup requirement field is absent, erased, or non-Boolean"
            ))
        ));
    }

    fn partial_affine_unit_checked_fixture() -> CheckedTrees {
        let source = r#"
            data Token { value: u64; }
            data Quartet { first: Token; second: Token; third: Token; fourth: Token; }
            data Sink {}
            machine Sink::take(token: Token) {}
            data Root {}
            machine Root::enter(value: Quartet) {
                Sink::take(value.third);
                Sink::take(value.first);
            }
        "#;
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        lower_typed_trees(typed).expect("check")
    }

    fn nested_partial_affine_unit_checked_fixture() -> CheckedTrees {
        let source = r#"
            data Token { value: u64; }
            data Deep { low: Token; middle: Token; high: Token; }
            data Branch { head: Token; deep: Deep; tail: Token; }
            data Outer { first: Token; left: Branch; right: Branch; last: Token; }
            data Sink {}
            machine Sink::take(token: Token) {}
            data Root {}
            machine Root::enter(value: Outer) {
                Sink::take(value.left.deep.middle);
                Sink::take(value.right.tail);
                Sink::take(value.first);
            }
        "#;
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        lower_typed_trees(typed).expect("check")
    }

    #[test]
    fn partial_affine_unit_cleanup_lowers_exact_terminal_paths_before_verification() {
        let checked = partial_affine_unit_checked_fixture();
        let [plan] = checked
            .facts
            .flow
            .terminal_partial_affine_unit_cleanups
            .machines
            .as_slice()
        else {
            panic!("expected one checked partial-cleanup plan")
        };
        let lowered = lower_partial_affine_unit_cleanup_machine(&checked, plan)
            .expect("strict checked partial cleanup should lower in memory");
        let entry = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == lowered.semantic_module.entry)
            .expect("terminal entry");
        let [first_call, second_call] = entry.blocks[0].operations.as_slice() else {
            panic!("partial cleanup entry should contain both source-ordered calls")
        };
        let moved_paths = [first_call, second_call]
            .into_iter()
            .map(|call| {
                let OperationKind::CallUnit {
                    structural_arguments,
                    claim_transfers,
                    ..
                } = &call.kind
                else {
                    panic!("partial cleanup entry should call Unit")
                };
                assert!(claim_transfers.is_empty());
                structural_arguments[0].path.clone()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            moved_paths,
            vec![
                vec![StructuralPathSegment::Field("third".to_owned())],
                vec![StructuralPathSegment::Field("first".to_owned())],
            ]
        );
        let Terminator::ReturnUnitPartialAffine {
            trivial_affine_discards,
            residual_affine_discards,
            ..
        } = &entry.blocks[0].terminator
        else {
            panic!("partial cleanup requires its distinct terminal return")
        };
        assert!(trivial_affine_discards.is_empty());
        assert_eq!(residual_affine_discards.len(), 2);
        assert_eq!(
            residual_affine_discards
                .iter()
                .map(|discard| discard.path.clone())
                .collect::<Vec<_>>(),
            vec![
                vec![StructuralPathSegment::Field("fourth".to_owned())],
                vec![StructuralPathSegment::Field("second".to_owned())],
            ]
        );
        for residual in residual_affine_discards {
            assert_eq!(residual.place, entry.structural_parameters[0].place);
            assert!(
                lowered
                    .semantic_module
                    .structural_types
                    .iter()
                    .any(|declaration| declaration.id == residual.structural_type
                        && declaration.identity.contains("Token"))
            );
        }
        psi_terminal_verifier::validate_module(&lowered.semantic_module)
            .expect("independent verifier proves moved field plus residual cleanup exhausts root");
        let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
            .expect("verified partial affine cleanup should encode canonically");
        assert_eq!(
            psi_terminal_codec::decode_module(&bytes)
                .expect("canonical partial affine cleanup should decode"),
            lowered.semantic_module
        );
        let entry_name = checked
            .facts
            .flow
            .terminal_machines
            .machines
            .iter()
            .find(|selection| selection.machine == plan.machine.machine)
            .expect("partial cleanup terminal selection")
            .name
            .clone();
        lower_machine(&checked, &entry_name)
            .expect("verified partial affine cleanup should cross the ordinary lowering entry");
    }

    #[test]
    fn mixed_partial_affine_unit_cleanup_lowers_recursive_maximal_residuals() {
        let checked = nested_partial_affine_unit_checked_fixture();
        let [plan] = checked
            .facts
            .flow
            .terminal_partial_affine_unit_cleanups
            .machines
            .as_slice()
        else {
            panic!("expected one checked mixed partial-cleanup plan")
        };
        let lowered = lower_partial_affine_unit_cleanup_machine(&checked, plan)
            .expect("strict mixed partial cleanup should lower in memory");
        let entry = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == lowered.semantic_module.entry)
            .expect("terminal entry");
        assert_eq!(
            entry.blocks[0]
                .operations
                .iter()
                .map(|operation| match &operation.kind {
                    OperationKind::CallUnit {
                        structural_arguments,
                        ..
                    } => structural_arguments[0].path.clone(),
                    _ => panic!("mixed partial cleanup calls Unit"),
                })
                .collect::<Vec<_>>(),
            vec![
                vec![
                    StructuralPathSegment::Field("left".to_owned()),
                    StructuralPathSegment::Field("deep".to_owned()),
                    StructuralPathSegment::Field("middle".to_owned()),
                ],
                vec![
                    StructuralPathSegment::Field("right".to_owned()),
                    StructuralPathSegment::Field("tail".to_owned()),
                ],
                vec![StructuralPathSegment::Field("first".to_owned())],
            ]
        );
        let Terminator::ReturnUnitPartialAffine {
            residual_affine_discards,
            ..
        } = &entry.blocks[0].terminator
        else {
            panic!("mixed partial cleanup retains its distinct return")
        };
        assert_eq!(
            residual_affine_discards
                .iter()
                .map(|discard| discard.path.clone())
                .collect::<Vec<_>>(),
            vec![
                vec![StructuralPathSegment::Field("last".to_owned())],
                vec![
                    StructuralPathSegment::Field("right".to_owned()),
                    StructuralPathSegment::Field("deep".to_owned()),
                ],
                vec![
                    StructuralPathSegment::Field("right".to_owned()),
                    StructuralPathSegment::Field("head".to_owned()),
                ],
                vec![
                    StructuralPathSegment::Field("left".to_owned()),
                    StructuralPathSegment::Field("tail".to_owned()),
                ],
                vec![
                    StructuralPathSegment::Field("left".to_owned()),
                    StructuralPathSegment::Field("deep".to_owned()),
                    StructuralPathSegment::Field("high".to_owned()),
                ],
                vec![
                    StructuralPathSegment::Field("left".to_owned()),
                    StructuralPathSegment::Field("deep".to_owned()),
                    StructuralPathSegment::Field("low".to_owned()),
                ],
                vec![
                    StructuralPathSegment::Field("left".to_owned()),
                    StructuralPathSegment::Field("head".to_owned()),
                ],
            ]
        );

        let mut stale = plan.clone();
        let CheckedUnitEffectOperationPlan::CallUnit {
            structural_arguments,
            ..
        } = &mut stale.machine.operations[0]
        else {
            unreachable!()
        };
        structural_arguments[0].path[2] =
            CheckedUnitStructuralPathSegment::Field("missing".to_owned());
        assert!(lower_partial_affine_unit_cleanup_machine(&checked, &stale).is_err());

        let mut overlapping = plan.clone();
        let [_, second, _, _] = overlapping.machine.operations.as_mut_slice() else {
            unreachable!()
        };
        let CheckedUnitEffectOperationPlan::CallUnit {
            structural_arguments,
            ..
        } = second
        else {
            unreachable!()
        };
        structural_arguments[0].path =
            vec![CheckedUnitStructuralPathSegment::Field("left".to_owned())];
        assert!(matches!(
            lower_partial_affine_unit_cleanup_machine(&checked, &overlapping),
            Err(LoweringError::Unsupported(
                "partial affine Unit cleanup signature or coordinates drifted"
            ))
        ));
    }

    #[test]
    fn partial_affine_unit_cleanup_lowering_rejects_stale_path_type_and_coordinates() {
        let checked = partial_affine_unit_checked_fixture();
        let original = checked
            .facts
            .flow
            .terminal_partial_affine_unit_cleanups
            .machines[0]
            .clone();

        let mut stale = original.clone();
        stale.residual_affine_discards[0]
            .path
            .push(CheckedUnitStructuralPathSegment::Field("nested".to_owned()));
        assert!(matches!(
            lower_partial_affine_unit_cleanup_machine(&checked, &stale),
            Err(LoweringError::Unsupported(
                "partial affine Unit residual field partition drifted"
            ))
        ));

        let mut stale = original.clone();
        stale.residual_affine_discards[0].type_identity = "stale::Token".to_owned();
        assert!(matches!(
            lower_partial_affine_unit_cleanup_machine(&checked, &stale),
            Err(LoweringError::Unsupported(
                "partial affine Unit residual field partition drifted"
            ))
        ));

        let mut stale = original.clone();
        stale.residual_affine_discards.swap(0, 1);
        assert!(matches!(
            lower_partial_affine_unit_cleanup_machine(&checked, &stale),
            Err(LoweringError::Unsupported(
                "partial affine Unit residual field partition drifted"
            ))
        ));

        let mut stale = original.clone();
        let [first, second, _] = stale.machine.operations.as_mut_slice() else {
            unreachable!()
        };
        let CheckedUnitEffectOperationPlan::CallUnit {
            structural_arguments: first_arguments,
            ..
        } = first
        else {
            unreachable!()
        };
        let first_path = first_arguments[0].path.clone();
        let CheckedUnitEffectOperationPlan::CallUnit {
            structural_arguments: second_arguments,
            ..
        } = second
        else {
            unreachable!()
        };
        second_arguments[0].path = first_path;
        assert!(matches!(
            lower_partial_affine_unit_cleanup_machine(&checked, &stale),
            Err(LoweringError::Unsupported(
                "partial affine Unit cleanup signature or coordinates drifted"
            ))
        ));

        let mut stale_checked = partial_affine_unit_checked_fixture();
        let stale_plan = stale_checked
            .facts
            .flow
            .terminal_partial_affine_unit_cleanups
            .machines[0]
            .clone();
        let source_identity = stale_plan.machine.structural_parameters[0]
            .type_identity
            .clone();
        let shape = stale_checked
            .facts
            .flow
            .terminal_partial_affine_unit_cleanups
            .structural_types
            .iter_mut()
            .find(|shape| shape.identity == source_identity)
            .expect("partial source shape");
        let CheckedUnitStructuralTypeShape::Record { fields } = &mut shape.shape else {
            unreachable!()
        };
        let mut extra = fields[0].clone();
        extra.identity = "extra".to_owned();
        fields.push(extra);
        assert!(matches!(
            lower_partial_affine_unit_cleanup_machine(&stale_checked, &stale_plan),
            Err(LoweringError::Unsupported(
                "partial affine Unit residual field partition drifted"
            ))
        ));

        let mut stale = original;
        let CheckedUnitEffectOperationPlan::CallUnit { coordinate, .. } =
            &mut stale.machine.operations[0]
        else {
            unreachable!()
        };
        coordinate.statement_index = 1;
        assert!(matches!(
            lower_partial_affine_unit_cleanup_machine(&checked, &stale),
            Err(LoweringError::Unsupported(
                "partial affine Unit cleanup signature or coordinates drifted"
            ))
        ));
    }

    fn hard_root_checked_fixture() -> CheckedTrees {
        let root = SymbolHandle::from_arena_index(1);
        let helper = SymbolHandle::from_arena_index(2);
        let boundary = SymbolHandle::from_arena_index(3);
        let root_state = SymbolHandle::from_arena_index(11);
        let helper_state = SymbolHandle::from_arena_index(12);
        let boundary_state = SymbolHandle::from_arena_index(13);
        let port_service_symbol = SymbolHandle::from_arena_index(20);
        let domain = SemanticDomainId(9);

        let mut checked = CheckedTrees::default();
        let port_service = checked
            .facts
            .service_reaches
            .services
            .intern(port_service_symbol, "PortIo");
        let empty_reach = checked.facts.service_reaches.rows.intern(Vec::new());
        assert_eq!(
            empty_reach,
            psi_language_semantics::ServiceReachRowTable::EMPTY_ROW
        );
        let port_reach = checked
            .facts
            .service_reaches
            .rows
            .intern(vec![port_service]);
        let reach = ServiceReachSummary {
            direct: port_reach,
            transitive: port_reach,
        };
        let contract_reach = ServiceReachPlan {
            interface: ServiceReachInterface::PublishedCeiling(port_reach),
            checked_inferred: port_reach,
        };
        checked.facts.flow.terminal_machines =
            psi_checked_trees::CheckedTerminalMachineSelections {
                machines: vec![
                    CheckedTerminalMachineSelection {
                        machine: root,
                        name: "example::Root::enter".to_owned(),
                        signature: CheckedTerminalSignatureEligibility::Attached,
                    },
                    CheckedTerminalMachineSelection {
                        machine: helper,
                        name: "example::Helper::run".to_owned(),
                        signature: CheckedTerminalSignatureEligibility::Attached,
                    },
                    CheckedTerminalMachineSelection {
                        machine: boundary,
                        name: "example::Acknowledgement::settle".to_owned(),
                        signature: CheckedTerminalSignatureEligibility::Attached,
                    },
                ],
            };
        let structural_parameter =
            |position| psi_checked_trees::CheckedUnitStructuralParameterPlan {
                position,
                is_self: false,
                type_identity: "example::Acknowledgement".to_owned(),
                multiplicity: Multiplicity::Linear,
                qualifications: vec![domain],
            };
        let entry_claim = |machine, state| psi_checked_trees::CheckedUnitEntryClaimPlan {
            claim_identity: unit_claim(machine, state),
            parameter_index: 0,
            path: Vec::new(),
            carry: CarryPolicy::STRICT,
        };
        checked.facts.flow.terminal_unit_effects = psi_checked_trees::CheckedUnitEffectPlans {
            structural_types: vec![
                psi_checked_trees::CheckedUnitStructuralTypePlan {
                    identity: "example::Acknowledgement".to_owned(),
                    shape: CheckedUnitStructuralTypeShape::Record {
                        fields: vec![
                            psi_checked_trees::CheckedUnitStructuralFieldPlan {
                                identity: "sequence".to_owned(),
                                relevance: psi_terminal::BindingRelevance::Relevant,
                                field_type: CheckedUnitStructuralFieldType::Scalar(
                                    PrimitiveType::U64,
                                ),
                            },
                            psi_checked_trees::CheckedUnitStructuralFieldPlan {
                                identity: "proof".to_owned(),
                                relevance: psi_terminal::BindingRelevance::Erased,
                                field_type: CheckedUnitStructuralFieldType::Erased {
                                    type_identity: "named(name(example::Evidence))".to_owned(),
                                },
                            },
                        ],
                    },
                },
                psi_checked_trees::CheckedUnitStructuralTypePlan {
                    identity: "example::Helper".to_owned(),
                    shape: CheckedUnitStructuralTypeShape::Record { fields: Vec::new() },
                },
                psi_checked_trees::CheckedUnitStructuralTypePlan {
                    identity: "example::Root".to_owned(),
                    shape: CheckedUnitStructuralTypeShape::Record { fields: Vec::new() },
                },
            ],
            structural_domains: vec![psi_checked_trees::CheckedUnitStructuralDomainPlan {
                domain,
                identity: "example::Acknowledgement::Pending".to_owned(),
                carrier_type_identity: "example::Acknowledgement".to_owned(),
            }],
            boundary_machines: vec![CheckedBoundaryMachinePlan {
                machine: boundary,
                state: boundary_state,
                attachment_type_identity: Some("example::Acknowledgement".to_owned()),
                structural_parameters: vec![
                    psi_checked_trees::CheckedUnitStructuralParameterPlan {
                        is_self: true,
                        ..structural_parameter(0)
                    },
                ],
                result_type: None,
                domain_requirements: vec![
                    psi_checked_trees::CheckedUnitStructuralDomainRequirementPlan {
                        argument_index: 0,
                        domain,
                    },
                ],
                contract_fingerprint: 0x303,
                contract_service_reach: contract_reach,
                service_reach: reach,
            }],
            machines: vec![
                CheckedUnitEffectMachinePlan {
                    machine: root,
                    state: root_state,
                    attachment_type_identity: "example::Root".to_owned(),
                    structural_parameters: vec![structural_parameter(7)],
                    trivial_affine_locals: Vec::new(),
                    entry_claims: vec![entry_claim(root, root_state)],
                    body_qualifications: vec![domain],
                    contract_fingerprint: 0x101,
                    contract_service_reach: contract_reach,
                    service_reach: reach,
                    operations: vec![
                        CheckedUnitEffectOperationPlan::CallUnit {
                            coordinate: psi_checked_trees::CheckedUnitCallCoordinate {
                                statement_index: 0,
                                call_ordinal: 0,
                            },
                            target_machine: helper,
                            target_state: helper_state,
                            target_contract_fingerprint: 0x202,
                            service_reach: reach,
                            structural_arguments: vec![
                                psi_checked_trees::CheckedUnitStructuralArgumentPlan {
                                    source_parameter_index: 0,
                                    type_identity: "example::Acknowledgement".to_owned(),
                                    path: Vec::new(),
                                },
                            ],
                            claim_transfers: vec![
                                psi_checked_trees::CheckedUnitClaimTransferPlan {
                                    claim_identity: unit_claim(root, root_state),
                                    argument_index: 0,
                                },
                            ],
                        },
                        CheckedUnitEffectOperationPlan::ReturnUnit {
                            statement_index: 1,
                            trivial_affine_local_discard_ordinals: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    ],
                },
                CheckedUnitEffectMachinePlan {
                    machine: helper,
                    state: helper_state,
                    attachment_type_identity: "example::Helper".to_owned(),
                    structural_parameters: vec![structural_parameter(3)],
                    trivial_affine_locals: Vec::new(),
                    entry_claims: vec![entry_claim(helper, helper_state)],
                    body_qualifications: vec![domain],
                    contract_fingerprint: 0x202,
                    contract_service_reach: contract_reach,
                    service_reach: reach,
                    operations: vec![
                        CheckedUnitEffectOperationPlan::PortWrite {
                            coordinate: psi_checked_trees::CheckedUnitCallCoordinate {
                                statement_index: 0,
                                call_ordinal: 0,
                            },
                            port: 0x3f8,
                            value: 0x5a,
                            service_reach: reach,
                        },
                        CheckedUnitEffectOperationPlan::BoundaryCall {
                            coordinate: psi_checked_trees::CheckedUnitCallCoordinate {
                                statement_index: 1,
                                call_ordinal: 0,
                            },
                            target_machine: boundary,
                            target_state: boundary_state,
                            target_contract_fingerprint: 0x303,
                            service_reach: reach,
                            structural_arguments: vec![
                                psi_checked_trees::CheckedUnitStructuralArgumentPlan {
                                    source_parameter_index: 0,
                                    type_identity: "example::Acknowledgement".to_owned(),
                                    path: Vec::new(),
                                },
                            ],
                            completion_receipts: vec![
                                psi_checked_trees::CheckedUnitClaimTransferPlan {
                                    claim_identity: unit_claim(helper, helper_state),
                                    argument_index: 0,
                                },
                            ],
                        },
                        CheckedUnitEffectOperationPlan::ReturnUnit {
                            statement_index: 2,
                            trivial_affine_local_discard_ordinals: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    ],
                },
            ],
        };
        checked
    }

    fn install_structural_unit_control_fixture(checked: &mut CheckedTrees) {
        let root = SymbolHandle::from_arena_index(1);
        let entry = SymbolHandle::from_arena_index(11);
        let leaf = SymbolHandle::from_arena_index(14);
        let affine_parameter = |position| psi_checked_trees::CheckedUnitStructuralParameterPlan {
            position,
            is_self: false,
            type_identity: "example::Acknowledgement".to_owned(),
            multiplicity: Multiplicity::Affine,
            qualifications: Vec::new(),
        };
        checked.facts.flow.terminal_structural_unit_controls =
            psi_checked_trees::CheckedStructuralUnitControlPlans {
                structural_types: checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .structural_types
                    .clone(),
                machines: vec![CheckedStructuralUnitControlMachinePlan {
                    machine: root,
                    attachment_type_identity: "example::Root".to_owned(),
                    states: vec![
                        psi_checked_trees::CheckedStructuralUnitControlStatePlan {
                            state: entry,
                            structural_parameters: vec![affine_parameter(0), affine_parameter(1)],
                            scalar_parameters: vec![
                                psi_checked_trees::CheckedStructuralScalarParameterPlan {
                                    source_position: 2,
                                    primitive_type: PrimitiveType::I32,
                                },
                            ],
                            terminator: CheckedStructuralUnitControlTerminatorPlan::Jump {
                                statement_ordinal: 0,
                                target_state: leaf,
                                transfers: vec![
                                    psi_checked_trees::CheckedStructuralControlTransferPlan {
                                        source_parameter_index: 1,
                                        target_parameter_index: 0,
                                    },
                                ],
                                scalar_arguments: vec![
                                    psi_checked_trees::CheckedStructuralScalarArgumentPlan {
                                        argument_ordinal: 1,
                                        source_scalar_parameter_index: 0,
                                        target_scalar_parameter_index: 0,
                                        primitive_type: PrimitiveType::I32,
                                    },
                                ],
                                trivial_affine_discard_parameter_positions: vec![0],
                            },
                        },
                        psi_checked_trees::CheckedStructuralUnitControlStatePlan {
                            state: leaf,
                            structural_parameters: vec![affine_parameter(0)],
                            scalar_parameters: vec![
                                psi_checked_trees::CheckedStructuralScalarParameterPlan {
                                    source_position: 1,
                                    primitive_type: PrimitiveType::I32,
                                },
                            ],
                            terminator: CheckedStructuralUnitControlTerminatorPlan::ReturnUnit {
                                trivial_affine_discard_parameter_positions: vec![0],
                            },
                        },
                    ],
                }],
            };
    }

    fn install_structural_unit_conditional_fixture(checked: &mut CheckedTrees) {
        let root = SymbolHandle::from_arena_index(1);
        let entry = SymbolHandle::from_arena_index(11);
        let true_leaf = SymbolHandle::from_arena_index(12);
        let false_leaf = SymbolHandle::from_arena_index(13);
        let affine_parameter = |position| psi_checked_trees::CheckedUnitStructuralParameterPlan {
            position,
            is_self: false,
            type_identity: "example::Acknowledgement".to_owned(),
            multiplicity: Multiplicity::Affine,
            qualifications: Vec::new(),
        };
        let leaf = |state| psi_checked_trees::CheckedStructuralUnitControlStatePlan {
            state,
            structural_parameters: vec![affine_parameter(0)],
            scalar_parameters: vec![psi_checked_trees::CheckedStructuralScalarParameterPlan {
                source_position: 1,
                primitive_type: PrimitiveType::I32,
            }],
            terminator: CheckedStructuralUnitControlTerminatorPlan::ReturnUnit {
                trivial_affine_discard_parameter_positions: vec![0],
            },
        };
        checked.facts.flow.terminal_structural_unit_controls =
            psi_checked_trees::CheckedStructuralUnitControlPlans {
                structural_types: checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .structural_types
                    .clone(),
                machines: vec![CheckedStructuralUnitControlMachinePlan {
                    machine: root,
                    attachment_type_identity: "example::Root".to_owned(),
                    states: vec![
                        psi_checked_trees::CheckedStructuralUnitControlStatePlan {
                            state: entry,
                            structural_parameters: vec![affine_parameter(0), affine_parameter(1)],
                            scalar_parameters: vec![
                                psi_checked_trees::CheckedStructuralScalarParameterPlan {
                                    source_position: 2,
                                    primitive_type: PrimitiveType::Bool,
                                },
                                psi_checked_trees::CheckedStructuralScalarParameterPlan {
                                    source_position: 3,
                                    primitive_type: PrimitiveType::I32,
                                },
                            ],
                            terminator: CheckedStructuralUnitControlTerminatorPlan::Conditional {
                                guard_scalar_parameter_index: 0,
                                when_true:
                                    psi_checked_trees::CheckedStructuralControlSuccessorPlan {
                                        statement_ordinal: 0,
                                        target_state: true_leaf,
                                        transfers: vec![
                                            psi_checked_trees::CheckedStructuralControlTransferPlan {
                                                source_parameter_index: 0,
                                                target_parameter_index: 0,
                                            },
                                        ],
                                        scalar_arguments: vec![
                                            psi_checked_trees::CheckedStructuralScalarArgumentPlan {
                                                argument_ordinal: 1,
                                                source_scalar_parameter_index: 1,
                                                target_scalar_parameter_index: 0,
                                                primitive_type: PrimitiveType::I32,
                                            },
                                        ],
                                        trivial_affine_discard_parameter_positions: vec![1],
                                    },
                                when_false:
                                    psi_checked_trees::CheckedStructuralControlSuccessorPlan {
                                        statement_ordinal: 1,
                                        target_state: false_leaf,
                                        transfers: vec![
                                            psi_checked_trees::CheckedStructuralControlTransferPlan {
                                                source_parameter_index: 1,
                                                target_parameter_index: 0,
                                            },
                                        ],
                                        scalar_arguments: vec![
                                            psi_checked_trees::CheckedStructuralScalarArgumentPlan {
                                                argument_ordinal: 1,
                                                source_scalar_parameter_index: 1,
                                                target_scalar_parameter_index: 0,
                                                primitive_type: PrimitiveType::I32,
                                            },
                                        ],
                                        trivial_affine_discard_parameter_positions: vec![0],
                                    },
                            },
                        },
                        leaf(true_leaf),
                        leaf(false_leaf),
                    ],
                }],
            };
    }

    fn install_structural_unit_nonentry_conditional_fixture(checked: &mut CheckedTrees) {
        install_structural_unit_conditional_fixture(checked);
        let plan = &mut checked
            .facts
            .flow
            .terminal_structural_unit_controls
            .machines[0];
        let conditional_state = plan.states[0].state;
        let structural_parameters = plan.states[0].structural_parameters.clone();
        let scalar_parameters = plan.states[0].scalar_parameters.clone();
        plan.states.insert(
            0,
            psi_checked_trees::CheckedStructuralUnitControlStatePlan {
                state: SymbolHandle::from_arena_index(14),
                structural_parameters,
                scalar_parameters,
                terminator: CheckedStructuralUnitControlTerminatorPlan::Jump {
                    statement_ordinal: 0,
                    target_state: conditional_state,
                    transfers: vec![
                        psi_checked_trees::CheckedStructuralControlTransferPlan {
                            source_parameter_index: 0,
                            target_parameter_index: 0,
                        },
                        psi_checked_trees::CheckedStructuralControlTransferPlan {
                            source_parameter_index: 1,
                            target_parameter_index: 1,
                        },
                    ],
                    scalar_arguments: vec![
                        psi_checked_trees::CheckedStructuralScalarArgumentPlan {
                            argument_ordinal: 2,
                            source_scalar_parameter_index: 0,
                            target_scalar_parameter_index: 0,
                            primitive_type: PrimitiveType::Bool,
                        },
                        psi_checked_trees::CheckedStructuralScalarArgumentPlan {
                            argument_ordinal: 3,
                            source_scalar_parameter_index: 1,
                            target_scalar_parameter_index: 1,
                            primitive_type: PrimitiveType::I32,
                        },
                    ],
                    trivial_affine_discard_parameter_positions: Vec::new(),
                },
            },
        );
    }

    fn install_structural_unit_two_conditional_fixture(checked: &mut CheckedTrees) {
        install_structural_unit_conditional_fixture(checked);
        let plan = &mut checked
            .facts
            .flow
            .terminal_structural_unit_controls
            .machines[0];
        let nested_state = plan.states[1].state;
        let affine_parameter = |position| psi_checked_trees::CheckedUnitStructuralParameterPlan {
            position,
            is_self: false,
            type_identity: "example::Acknowledgement".to_owned(),
            multiplicity: Multiplicity::Affine,
            qualifications: Vec::new(),
        };
        let CheckedStructuralUnitControlTerminatorPlan::Conditional { when_true, .. } =
            &mut plan.states[0].terminator
        else {
            unreachable!()
        };
        when_true.scalar_arguments = vec![
            psi_checked_trees::CheckedStructuralScalarArgumentPlan {
                argument_ordinal: 1,
                source_scalar_parameter_index: 0,
                target_scalar_parameter_index: 0,
                primitive_type: PrimitiveType::Bool,
            },
            psi_checked_trees::CheckedStructuralScalarArgumentPlan {
                argument_ordinal: 2,
                source_scalar_parameter_index: 1,
                target_scalar_parameter_index: 1,
                primitive_type: PrimitiveType::I32,
            },
        ];
        let nested_true = SymbolHandle::from_arena_index(14);
        let nested_false = SymbolHandle::from_arena_index(15);
        plan.states[1].scalar_parameters = vec![
            psi_checked_trees::CheckedStructuralScalarParameterPlan {
                source_position: 1,
                primitive_type: PrimitiveType::Bool,
            },
            psi_checked_trees::CheckedStructuralScalarParameterPlan {
                source_position: 2,
                primitive_type: PrimitiveType::I32,
            },
        ];
        let nested_successor = |statement_ordinal, target_state| {
            psi_checked_trees::CheckedStructuralControlSuccessorPlan {
                statement_ordinal,
                target_state,
                transfers: vec![psi_checked_trees::CheckedStructuralControlTransferPlan {
                    source_parameter_index: 0,
                    target_parameter_index: 0,
                }],
                scalar_arguments: vec![psi_checked_trees::CheckedStructuralScalarArgumentPlan {
                    argument_ordinal: 1,
                    source_scalar_parameter_index: 1,
                    target_scalar_parameter_index: 0,
                    primitive_type: PrimitiveType::I32,
                }],
                trivial_affine_discard_parameter_positions: Vec::new(),
            }
        };
        plan.states[1].terminator = CheckedStructuralUnitControlTerminatorPlan::Conditional {
            guard_scalar_parameter_index: 0,
            when_true: nested_successor(0, nested_true),
            when_false: nested_successor(1, nested_false),
        };
        let leaf = |state| psi_checked_trees::CheckedStructuralUnitControlStatePlan {
            state,
            structural_parameters: vec![affine_parameter(0)],
            scalar_parameters: vec![psi_checked_trees::CheckedStructuralScalarParameterPlan {
                source_position: 1,
                primitive_type: PrimitiveType::I32,
            }],
            terminator: CheckedStructuralUnitControlTerminatorPlan::ReturnUnit {
                trivial_affine_discard_parameter_positions: vec![0],
            },
        };
        plan.states.push(leaf(nested_true));
        plan.states.push(leaf(nested_false));
        assert_eq!(plan.states[1].state, nested_state);
    }

    fn install_structural_unit_join_fixture(checked: &mut CheckedTrees) {
        install_structural_unit_conditional_fixture(checked);
        let plan = &mut checked
            .facts
            .flow
            .terminal_structural_unit_controls
            .machines[0];
        let affine_parameter = |position| psi_checked_trees::CheckedUnitStructuralParameterPlan {
            position,
            is_self: false,
            type_identity: "example::Acknowledgement".to_owned(),
            multiplicity: Multiplicity::Affine,
            qualifications: Vec::new(),
        };
        plan.states[0].scalar_parameters.push(
            psi_checked_trees::CheckedStructuralScalarParameterPlan {
                source_position: 4,
                primitive_type: PrimitiveType::I32,
            },
        );
        let CheckedStructuralUnitControlTerminatorPlan::Conditional { when_false, .. } =
            &mut plan.states[0].terminator
        else {
            unreachable!()
        };
        when_false.transfers[0].source_parameter_index = 0;
        when_false.scalar_arguments[0].source_scalar_parameter_index = 2;
        when_false.trivial_affine_discard_parameter_positions = vec![1];

        let join = SymbolHandle::from_arena_index(14);
        for state in &mut plan.states[1..3] {
            state.terminator = CheckedStructuralUnitControlTerminatorPlan::Jump {
                statement_ordinal: 0,
                target_state: join,
                transfers: vec![psi_checked_trees::CheckedStructuralControlTransferPlan {
                    source_parameter_index: 0,
                    target_parameter_index: 0,
                }],
                scalar_arguments: vec![psi_checked_trees::CheckedStructuralScalarArgumentPlan {
                    argument_ordinal: 1,
                    source_scalar_parameter_index: 0,
                    target_scalar_parameter_index: 0,
                    primitive_type: PrimitiveType::I32,
                }],
                trivial_affine_discard_parameter_positions: Vec::new(),
            };
        }
        plan.states
            .push(psi_checked_trees::CheckedStructuralUnitControlStatePlan {
                state: join,
                structural_parameters: vec![affine_parameter(0)],
                scalar_parameters: vec![psi_checked_trees::CheckedStructuralScalarParameterPlan {
                    source_position: 1,
                    primitive_type: PrimitiveType::I32,
                }],
                terminator: CheckedStructuralUnitControlTerminatorPlan::ReturnUnit {
                    trivial_affine_discard_parameter_positions: vec![0],
                },
            });
    }

    fn install_structural_scalar_return_fixture(checked: &mut CheckedTrees) {
        let root = SymbolHandle::from_arena_index(1);
        let entry = SymbolHandle::from_arena_index(11);
        let affine_parameter = |position| psi_checked_trees::CheckedUnitStructuralParameterPlan {
            position,
            is_self: false,
            type_identity: "example::Acknowledgement".to_owned(),
            multiplicity: Multiplicity::Affine,
            qualifications: Vec::new(),
        };
        checked.facts.flow.terminal_structural_scalar_returns =
            psi_checked_trees::CheckedStructuralScalarReturnPlans {
                structural_types: checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .structural_types
                    .clone(),
                machines: vec![CheckedStructuralScalarReturnMachinePlan {
                    machine: root,
                    state: entry,
                    attachment_type_identity: "example::Root".to_owned(),
                    structural_parameters: vec![affine_parameter(0), affine_parameter(1)],
                    scalar_parameters: Vec::new(),
                    bindings: Vec::new(),
                    result_type: PrimitiveType::I32,
                    return_statement_ordinal: 0,
                    shared_boolean_convergence: None,
                    caller_requirements: Vec::new(),
                    scalar_requirements: Vec::new(),
                    cleanup_actions: vec![
                        CheckedStructuralScalarReturnCleanupAction::DiscardRoot(1),
                        CheckedStructuralScalarReturnCleanupAction::DiscardRoot(0),
                    ],
                }],
            };
        checked.facts.values.scalar_expressions.expressions.push(
            psi_checked_trees::CheckedLocatedScalarExpression {
                state: entry,
                statement_ordinal: 0,
                role: CheckedScalarExpressionRole::Return,
                expression: CheckedScalarExpression::IntegerLiteral {
                    literal: psi_numerics::literals::IntegerLiteral::from_value(7).with_landing(
                        psi_numerics::literals::IntegerLanding {
                            landed_type: psi_numerics::literals::LandedIntegerType::I32,
                            domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
                        },
                    ),
                },
            },
        );
    }

    #[test]
    fn structural_scalar_return_lowers_value_before_exact_affine_cleanup() {
        let mut checked = hard_root_checked_fixture();
        install_structural_scalar_return_fixture(&mut checked);

        let lowered = lower_machine(&checked, "example::Root::enter")
            .expect("closed scalar return and exact affine cleanup should lower");
        let [machine] = lowered.semantic_module.machines.as_slice() else {
            panic!("structural scalar return lowers one attached machine")
        };
        assert_eq!(machine.structural_parameters.len(), 2);
        assert!(machine.parameters.is_empty());
        assert!(matches!(machine.result, TerminalMachineResult::Scalar(_)));
        let [block] = machine.blocks.as_slice() else {
            panic!("closed structural scalar return lowers one block")
        };
        assert!(matches!(
            &block.terminator,
            Terminator::Return {
                cleanup_actions,
                ..
            } if cleanup_actions == &[
                TerminalAffineCleanupAction::DiscardRoot(place_id(2)),
                TerminalAffineCleanupAction::DiscardRoot(place_id(1)),
            ]
        ));
        assert!(matches!(
            block.operations.as_slice(),
            [Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            }]
        ));
        let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
            .expect("structural scalar return should encode canonically");
        assert_eq!(
            psi_terminal_codec::decode_module(&bytes)
                .expect("canonical structural scalar return bytes should decode"),
            lowered.semantic_module
        );
    }

    #[test]
    fn structural_scalar_return_fails_closed_on_stale_cleanup() {
        let mut checked = hard_root_checked_fixture();
        install_structural_scalar_return_fixture(&mut checked);
        checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .machines[0]
            .cleanup_actions = vec![
            CheckedStructuralScalarReturnCleanupAction::DiscardRoot(0),
            CheckedStructuralScalarReturnCleanupAction::DiscardRoot(1),
        ];

        assert!(matches!(
            lower_machine(&checked, "example::Root::enter"),
            Err(LoweringError::Unsupported(
                "structural scalar return cleanup does not consume its exact frontier"
            ))
        ));
    }

    #[test]
    fn structural_unit_control_lowers_exact_transfer_and_edge_cleanup() {
        let mut checked = hard_root_checked_fixture();
        install_structural_unit_control_fixture(&mut checked);

        let lowered = lower_machine(&checked, "example::Root::enter")
            .expect("exact structural custody chain should lower");
        let [machine] = lowered.semantic_module.machines.as_slice() else {
            panic!("structural control slice lowers one attached machine")
        };
        assert_eq!(machine.structural_parameters.len(), 2);
        assert!(matches!(
            machine.parameters.as_slice(),
            [ValueDeclaration {
                id,
                scalar_type: ScalarType::Integer(_),
            }] if *id == value_id(1)
        ));
        assert_eq!(machine.blocks.len(), 2);
        assert!(
            machine
                .blocks
                .iter()
                .all(|block| block.operations.is_empty())
        );
        assert!(matches!(
            &machine.blocks[0].terminator,
            Terminator::Jump {
                target,
                arguments,
                trivial_affine_discards,
                ..
            } if *target == block_id(2)
                && arguments == &[value_id(1)]
                && trivial_affine_discards == &[place_id(1)]
        ));
        assert!(matches!(
            machine.blocks[1].parameters.as_slice(),
            [ValueDeclaration {
                id,
                scalar_type: ScalarType::Integer(_),
            }] if *id == value_id(2)
        ));
        assert!(matches!(
            &machine.blocks[1].terminator,
            Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } if trivial_affine_discards == &[place_id(2)]
        ));
        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &psi_proof_kernel::AdmissionProfile::default(),
        )
        .expect("structural jump scalar binding and cleanup should verify independently");
        let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
            .expect("structural control slice should encode canonically");
        assert_eq!(
            psi_terminal_codec::decode_module(&bytes)
                .expect("canonical structural control bytes should decode"),
            lowered.semantic_module
        );
    }

    #[test]
    fn structural_unit_conditional_lowers_independent_transfer_cleanup_frontiers() {
        let mut checked = hard_root_checked_fixture();
        install_structural_unit_conditional_fixture(&mut checked);

        let lowered = lower_machine(&checked, "example::Root::enter")
            .expect("exact structural conditional frontiers should lower");
        let [machine] = lowered.semantic_module.machines.as_slice() else {
            panic!("structural conditional slice lowers one attached machine")
        };
        assert!(matches!(
            machine.parameters.as_slice(),
            [
                ValueDeclaration {
                    id: guard,
                    scalar_type: ScalarType::Boolean,
                },
                ValueDeclaration {
                    id: value,
                    scalar_type: ScalarType::Integer(_),
                },
            ] if *guard == value_id(1) && *value == value_id(2)
        ));
        assert_eq!(machine.blocks.len(), 3);
        assert!(matches!(
            &machine.blocks[0].terminator,
            Terminator::Conditional {
                condition,
                when_true: SuccessorEdge {
                    target: true_target,
                    arguments: true_arguments,
                    trivial_affine_discards: true_discards,
                    ..
                },
                when_false: SuccessorEdge {
                    target: false_target,
                    arguments: false_arguments,
                    trivial_affine_discards: false_discards,
                    ..
                },
            } if *condition == value_id(1)
                && *true_target == block_id(2)
                && true_arguments == &[value_id(2)]
                && true_discards == &[place_id(2)]
                && *false_target == block_id(3)
                && false_arguments == &[value_id(2)]
                && false_discards == &[place_id(1)]
        ));
        assert!(matches!(
            machine.blocks[1].parameters.as_slice(),
            [ValueDeclaration {
                id,
                scalar_type: ScalarType::Integer(_),
            }] if *id == value_id(3)
        ));
        assert!(matches!(
            machine.blocks[2].parameters.as_slice(),
            [ValueDeclaration {
                id,
                scalar_type: ScalarType::Integer(_),
            }] if *id == value_id(4)
        ));
        assert!(matches!(
            &machine.blocks[1].terminator,
            Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } if trivial_affine_discards == &[place_id(1)]
        ));
        assert!(matches!(
            &machine.blocks[2].terminator,
            Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } if trivial_affine_discards == &[place_id(2)]
        ));
        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &psi_proof_kernel::AdmissionProfile::default(),
        )
        .expect("structural conditional cleanup should verify independently");
        let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
            .expect("structural conditional should encode canonically");
        assert_eq!(
            psi_terminal_codec::decode_module(&bytes)
                .expect("structural conditional should decode canonically"),
            lowered.semantic_module
        );

        let CheckedStructuralUnitControlTerminatorPlan::Conditional { when_true, .. } =
            &mut checked
                .facts
                .flow
                .terminal_structural_unit_controls
                .machines[0]
                .states[0]
                .terminator
        else {
            unreachable!()
        };
        when_true.scalar_arguments[0].source_scalar_parameter_index = 0;
        assert!(matches!(
            lower_machine(&checked, "example::Root::enter"),
            Err(LoweringError::Unsupported(
                "structural Unit scalar successor map changes its checked signature"
            ))
        ));

        install_structural_unit_conditional_fixture(&mut checked);

        let CheckedStructuralUnitControlTerminatorPlan::Conditional {
            when_true,
            when_false,
            ..
        } = &mut checked
            .facts
            .flow
            .terminal_structural_unit_controls
            .machines[0]
            .states[0]
            .terminator
        else {
            unreachable!()
        };
        std::mem::swap(
            &mut when_true.statement_ordinal,
            &mut when_false.statement_ordinal,
        );
        assert!(matches!(
            lower_machine(&checked, "example::Root::enter"),
            Err(LoweringError::Unsupported(
                "structural Unit conditional successors are not in canonical order"
            ))
        ));
    }

    #[test]
    fn structural_unit_conditional_lowers_after_an_unconditional_prefix() {
        let mut checked = hard_root_checked_fixture();
        install_structural_unit_nonentry_conditional_fixture(&mut checked);

        let lowered = lower_machine(&checked, "example::Root::enter")
            .expect("one structural conditional may follow an unconditional prefix");
        let [machine] = lowered.semantic_module.machines.as_slice() else {
            panic!("prefixed structural conditional lowers one attached machine")
        };
        assert_eq!(machine.blocks.len(), 4);
        assert!(matches!(
            &machine.blocks[0].terminator,
            Terminator::Jump {
                target,
                arguments,
                trivial_affine_discards,
                ..
            } if *target == block_id(2)
                && arguments == &[value_id(1), value_id(2)]
                && trivial_affine_discards.is_empty()
        ));
        assert!(matches!(
            machine.blocks[1].parameters.as_slice(),
            [
                ValueDeclaration {
                    id: guard,
                    scalar_type: ScalarType::Boolean,
                },
                ValueDeclaration {
                    id: value,
                    scalar_type: ScalarType::Integer(_),
                },
            ] if *guard == value_id(3) && *value == value_id(4)
        ));
        assert!(matches!(
            &machine.blocks[1].terminator,
            Terminator::Conditional {
                condition,
                when_true: SuccessorEdge {
                    target: true_target,
                    arguments: true_arguments,
                    trivial_affine_discards: true_discards,
                    ..
                },
                when_false: SuccessorEdge {
                    target: false_target,
                    arguments: false_arguments,
                    trivial_affine_discards: false_discards,
                    ..
                },
            } if *condition == value_id(3)
                && *true_target == block_id(3)
                && true_arguments == &[value_id(4)]
                && true_discards == &[place_id(2)]
                && *false_target == block_id(4)
                && false_arguments == &[value_id(4)]
                && false_discards == &[place_id(1)]
        ));
        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &psi_proof_kernel::AdmissionProfile::default(),
        )
        .expect("prefixed conditional maps should verify independently");
        let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
            .expect("prefixed structural conditional should encode canonically");
        assert_eq!(
            psi_terminal_codec::decode_module(&bytes)
                .expect("prefixed structural conditional should decode canonically"),
            lowered.semantic_module
        );

        let second_conditional = checked
            .facts
            .flow
            .terminal_structural_unit_controls
            .machines[0]
            .states[1]
            .terminator
            .clone();
        checked
            .facts
            .flow
            .terminal_structural_unit_controls
            .machines[0]
            .states[2]
            .terminator = second_conditional.clone();
        checked
            .facts
            .flow
            .terminal_structural_unit_controls
            .machines[0]
            .states[3]
            .terminator = second_conditional;
        assert!(matches!(
            lower_machine(&checked, "example::Root::enter"),
            Err(LoweringError::Unsupported(
                "structural Unit control supports at most two checked conditional states"
            ))
        ));
    }

    #[test]
    fn structural_unit_two_conditional_tree_lowers_exact_edge_maps() {
        let mut checked = hard_root_checked_fixture();
        install_structural_unit_two_conditional_fixture(&mut checked);

        let lowered = lower_machine(&checked, "example::Root::enter")
            .expect("two checked structural conditionals should lower");
        let [machine] = lowered.semantic_module.machines.as_slice() else {
            panic!("two-decision structural tree lowers one attached machine")
        };
        assert_eq!(machine.blocks.len(), 5);
        assert!(matches!(
            &machine.blocks[0].terminator,
            Terminator::Conditional {
                condition,
                when_true: SuccessorEdge {
                    target: true_target,
                    arguments: true_arguments,
                    trivial_affine_discards: true_discards,
                    ..
                },
                when_false: SuccessorEdge {
                    target: false_target,
                    arguments: false_arguments,
                    trivial_affine_discards: false_discards,
                    ..
                },
            } if *condition == value_id(1)
                && *true_target == block_id(2)
                && true_arguments == &[value_id(1), value_id(2)]
                && true_discards == &[place_id(2)]
                && *false_target == block_id(3)
                && false_arguments == &[value_id(2)]
                && false_discards == &[place_id(1)]
        ));
        assert!(matches!(
            machine.blocks[1].parameters.as_slice(),
            [
                ValueDeclaration {
                    id: guard,
                    scalar_type: ScalarType::Boolean,
                },
                ValueDeclaration {
                    id: value,
                    scalar_type: ScalarType::Integer(_),
                },
            ] if *guard == value_id(3) && *value == value_id(4)
        ));
        assert!(matches!(
            &machine.blocks[1].terminator,
            Terminator::Conditional {
                condition,
                when_true: SuccessorEdge {
                    target: true_target,
                    arguments: true_arguments,
                    trivial_affine_discards: true_discards,
                    ..
                },
                when_false: SuccessorEdge {
                    target: false_target,
                    arguments: false_arguments,
                    trivial_affine_discards: false_discards,
                    ..
                },
            } if *condition == value_id(3)
                && *true_target == block_id(4)
                && true_arguments == &[value_id(4)]
                && true_discards.is_empty()
                && *false_target == block_id(5)
                && false_arguments == &[value_id(4)]
                && false_discards.is_empty()
        ));
        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &psi_proof_kernel::AdmissionProfile::default(),
        )
        .expect("two-decision structural maps should verify independently");
        let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
            .expect("two-decision structural tree should encode canonically");
        assert_eq!(
            psi_terminal_codec::decode_module(&bytes)
                .expect("two-decision structural tree should decode canonically"),
            lowered.semantic_module
        );
    }

    #[test]
    fn structural_unit_diamond_requires_one_exact_join_frontier() {
        let mut checked = hard_root_checked_fixture();
        install_structural_unit_join_fixture(&mut checked);

        let lowered = lower_machine(&checked, "example::Root::enter")
            .expect("one exact structural diamond should lower");
        let [machine] = lowered.semantic_module.machines.as_slice() else {
            panic!("structural diamond lowers one attached machine")
        };
        assert_eq!(machine.blocks.len(), 4);
        assert!(matches!(
            &machine.blocks[0].terminator,
            Terminator::Conditional {
                condition,
                when_true: SuccessorEdge {
                    target: true_target,
                    arguments: true_arguments,
                    trivial_affine_discards: true_discards,
                    ..
                },
                when_false: SuccessorEdge {
                    target: false_target,
                    arguments: false_arguments,
                    trivial_affine_discards: false_discards,
                    ..
                },
            } if *condition == value_id(1)
                && *true_target == block_id(2)
                && true_arguments == &[value_id(2)]
                && true_discards == &[place_id(2)]
                && *false_target == block_id(3)
                && false_arguments == &[value_id(3)]
                && false_discards == &[place_id(2)]
        ));
        assert!(matches!(
            &machine.blocks[1].terminator,
            Terminator::Jump {
                target,
                arguments,
                trivial_affine_discards,
                ..
            } if *target == block_id(4)
                && arguments == &[value_id(4)]
                && trivial_affine_discards.is_empty()
        ));
        assert!(matches!(
            &machine.blocks[2].terminator,
            Terminator::Jump {
                target,
                arguments,
                trivial_affine_discards,
                ..
            } if *target == block_id(4)
                && arguments == &[value_id(5)]
                && trivial_affine_discards.is_empty()
        ));
        assert!(matches!(
            machine.blocks[3].parameters.as_slice(),
            [ValueDeclaration {
                id,
                scalar_type: ScalarType::Integer(_),
            }] if *id == value_id(6)
        ));
        assert!(matches!(
            &machine.blocks[3].terminator,
            Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } if trivial_affine_discards == &[place_id(1)]
        ));
        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &psi_proof_kernel::AdmissionProfile::default(),
        )
        .expect("the independent verifier should reconstruct one identical join frontier");
        let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
            .expect("structural diamond should encode canonically");
        assert_eq!(
            psi_terminal_codec::decode_module(&bytes)
                .expect("structural diamond should decode canonically"),
            lowered.semantic_module
        );

        let CheckedStructuralUnitControlTerminatorPlan::Conditional { when_false, .. } =
            &mut checked
                .facts
                .flow
                .terminal_structural_unit_controls
                .machines[0]
                .states[0]
                .terminator
        else {
            unreachable!()
        };
        when_false.transfers[0].source_parameter_index = 1;
        when_false.trivial_affine_discard_parameter_positions = vec![0];
        assert!(matches!(
            lower_machine(&checked, "example::Root::enter"),
            Err(LoweringError::Unsupported(
                "structural Unit join predecessors reconstruct different custody frontiers"
            ))
        ));

        install_structural_unit_join_fixture(&mut checked);
        let entry = checked
            .facts
            .flow
            .terminal_structural_unit_controls
            .machines[0]
            .states[0]
            .state;
        checked
            .facts
            .flow
            .terminal_structural_unit_controls
            .machines[0]
            .states[3]
            .terminator = CheckedStructuralUnitControlTerminatorPlan::Jump {
            statement_ordinal: 0,
            target_state: entry,
            transfers: vec![psi_checked_trees::CheckedStructuralControlTransferPlan {
                source_parameter_index: 0,
                target_parameter_index: 0,
            }],
            scalar_arguments: Vec::new(),
            trivial_affine_discard_parameter_positions: Vec::new(),
        };
        assert!(matches!(
            lower_machine(&checked, "example::Root::enter"),
            Err(LoweringError::Unsupported(
                "structural Unit control entry has an incoming edge"
            ))
        ));
    }

    #[test]
    fn structural_unit_control_fails_closed_on_stale_cleanup_or_signature() {
        let mut checked = hard_root_checked_fixture();
        install_structural_unit_control_fixture(&mut checked);
        let plan = &mut checked
            .facts
            .flow
            .terminal_structural_unit_controls
            .machines[0];
        let CheckedStructuralUnitControlTerminatorPlan::Jump {
            trivial_affine_discard_parameter_positions,
            ..
        } = &mut plan.states[0].terminator
        else {
            unreachable!()
        };
        trivial_affine_discard_parameter_positions.clear();
        assert!(matches!(
            lower_machine(&checked, "example::Root::enter"),
            Err(LoweringError::Unsupported(
                "structural Unit jump transfer and cleanup do not partition its exact frontier"
            ))
        ));

        install_structural_unit_control_fixture(&mut checked);
        let CheckedStructuralUnitControlTerminatorPlan::Jump {
            scalar_arguments, ..
        } = &mut checked
            .facts
            .flow
            .terminal_structural_unit_controls
            .machines[0]
            .states[0]
            .terminator
        else {
            unreachable!()
        };
        scalar_arguments[0].source_scalar_parameter_index = 1;
        assert!(matches!(
            lower_machine(&checked, "example::Root::enter"),
            Err(LoweringError::Unsupported(
                "structural Unit scalar successor map changes its checked signature"
            ))
        ));

        install_structural_unit_control_fixture(&mut checked);
        checked
            .facts
            .flow
            .terminal_structural_unit_controls
            .machines[0]
            .states[1]
            .structural_parameters[0]
            .type_identity = "example::Root".to_owned();
        let stale_signature = lower_machine(&checked, "example::Root::enter");
        assert!(
            matches!(
                &stale_signature,
                Err(LoweringError::Unsupported(
                    "structural Unit transfer changes its checked structural signature"
                ))
            ),
            "unexpected stale-signature result: {stale_signature:?}"
        );
    }

    #[test]
    fn attached_unit_hard_root_lowers_exact_checked_closure_with_dense_identities() {
        let checked = hard_root_checked_fixture();
        let lowered = lower_machine(&checked, "example::Root::enter")
            .expect("complete attached Unit closure should lower");
        let module = &lowered.semantic_module;

        assert_eq!(module.entry, machine_id(1));
        assert_eq!(module.structural_types.len(), 3);
        assert_eq!(
            module
                .structural_types
                .iter()
                .map(|declaration| declaration.id.get())
                .collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert_eq!(module.structural_domains[0].id, structural_domain_id(1));
        let acknowledgement = module
            .structural_types
            .iter()
            .find(|declaration| declaration.identity == "example::Acknowledgement")
            .expect("acknowledgement structural type");
        let StructuralTypeShape::Record { fields } = &acknowledgement.shape else {
            panic!("acknowledgement is a record")
        };
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[1].relevance, psi_terminal::BindingRelevance::Erased);
        assert!(matches!(
            &fields[1].field_type,
            StructuralFieldType::Erased { type_identity }
                if type_identity == "named(name(example::Evidence))"
        ));
        assert_eq!(module.services[0].id, service_id(1));
        assert_eq!(module.services[0].identity, "PortIo");
        assert_eq!(module.boundary_machines[0].id, boundary_machine_id(1));
        assert_eq!(module.boundary_machines[0].requires.len(), 1);
        assert_eq!(module.machines.len(), 2);
        assert_eq!(module.machines[0].id, machine_id(1));
        assert_eq!(module.machines[1].id, machine_id(2));
        assert_eq!(module.machines[0].structural_parameters[0].position, 0);
        assert_eq!(module.machines[1].structural_parameters[0].position, 0);
        assert_eq!(module.machines[0].entry_claims[0].claim, claim_id(1));
        assert_eq!(module.machines[1].entry_claims[0].claim, claim_id(1));

        let [root_call] = module.machines[0].blocks[0].operations.as_slice() else {
            panic!("root emits one call before its Unit return")
        };
        let OperationKind::CallUnit {
            callee,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            ..
        } = &root_call.kind
        else {
            panic!("root operation should be CallUnit")
        };
        assert_eq!(*callee, machine_id(2));
        assert_eq!(structural_arguments[0].place, place_id(2));
        assert_eq!(claim_transfers[0].claim, claim_id(1));
        assert!(requirement_obligations.is_empty());

        let [port, settlement] = module.machines[1].blocks[0].operations.as_slice() else {
            panic!("helper emits port output and boundary settlement")
        };
        assert!(matches!(
            port.kind,
            OperationKind::PortWrite {
                service,
                port: 0x3f8,
                value: 0x5a,
            } if service == service_id(1)
        ));
        let OperationKind::BoundaryCall {
            boundary,
            structural_arguments,
            completion_receipts,
            requirement_obligations,
        } = &settlement.kind
        else {
            panic!("helper settlement should be BoundaryCall")
        };
        assert_eq!(*boundary, boundary_machine_id(1));
        assert_eq!(structural_arguments[0].place, place_id(3));
        assert_eq!(completion_receipts[0].claim, claim_id(1));
        assert!(requirement_obligations.is_empty());
        assert!(matches!(
            module.machines[0].blocks[0].terminator,
            Terminator::ReturnUnit { edge, .. } if edge == edge_id(1)
        ));
        assert!(matches!(
            module.machines[1].blocks[0].terminator,
            Terminator::ReturnUnit { edge, .. } if edge == edge_id(2)
        ));
        assert!(lowered.proof_bundle.evidence.is_empty());
        assert_eq!(
            lower_machine(&checked, "example::Root::enter")
                .expect("repeat lowering")
                .semantic_module,
            *module,
            "canonical identities must be deterministic"
        );
    }

    #[test]
    fn attached_unit_record_field_custody_crosses_call_and_boundary_settlement() {
        let mut checked = hard_root_checked_fixture();
        let plans = &mut checked.facts.flow.terminal_unit_effects;
        plans
            .structural_types
            .push(psi_checked_trees::CheckedUnitStructuralTypePlan {
                identity: "example::Token".to_owned(),
                shape: CheckedUnitStructuralTypeShape::Record { fields: Vec::new() },
            });
        let acknowledgement = plans
            .structural_types
            .iter_mut()
            .find(|shape| shape.identity == "example::Acknowledgement")
            .expect("acknowledgement shape");
        let CheckedUnitStructuralTypeShape::Record { fields } = &mut acknowledgement.shape else {
            panic!("acknowledgement is a record")
        };
        fields[0].identity = "#7".to_owned();
        fields[0].field_type = CheckedUnitStructuralFieldType::Structural {
            type_identity: "example::Token".to_owned(),
        };
        for machine in &mut plans.machines {
            machine.entry_claims[0].path =
                vec![CheckedUnitStructuralPathSegment::Field("#7".to_owned())];
        }

        let lowered = lower_machine(&checked, "example::Root::enter")
            .expect("record-field custody should cross the complete Unit closure");
        assert_eq!(
            lowered.semantic_module.machines[0].entry_claims[0].path,
            [StructuralPathSegment::Field("#7".to_owned())]
        );
        assert_eq!(
            lowered.semantic_module.machines[1].entry_claims[0].path,
            [StructuralPathSegment::Field("#7".to_owned())]
        );
        let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
            .expect("aggregate custody must have a canonical terminal encoding");
        assert_eq!(
            psi_terminal_codec::decode_module(&bytes).expect("canonical aggregate custody bytes"),
            lowered.semantic_module
        );
    }

    #[test]
    fn attached_unit_nested_record_claim_lowers_through_complete_closure() {
        let mut checked = hard_root_checked_fixture();
        let plans = &mut checked.facts.flow.terminal_unit_effects;
        plans.structural_types.extend([
            psi_checked_trees::CheckedUnitStructuralTypePlan {
                identity: "example::Pocket".to_owned(),
                shape: CheckedUnitStructuralTypeShape::Record {
                    fields: vec![psi_checked_trees::CheckedUnitStructuralFieldPlan {
                        identity: "#9".to_owned(),
                        relevance: psi_terminal::BindingRelevance::Relevant,
                        field_type: CheckedUnitStructuralFieldType::Structural {
                            type_identity: "example::Token".to_owned(),
                        },
                    }],
                },
            },
            psi_checked_trees::CheckedUnitStructuralTypePlan {
                identity: "example::Token".to_owned(),
                shape: CheckedUnitStructuralTypeShape::Record { fields: Vec::new() },
            },
        ]);
        let acknowledgement = plans
            .structural_types
            .iter_mut()
            .find(|shape| shape.identity == "example::Acknowledgement")
            .expect("acknowledgement shape");
        let CheckedUnitStructuralTypeShape::Record { fields } = &mut acknowledgement.shape else {
            panic!("acknowledgement is a record")
        };
        fields[0].identity = "#7".to_owned();
        fields[0].field_type = CheckedUnitStructuralFieldType::Structural {
            type_identity: "example::Pocket".to_owned(),
        };
        for boundary in &mut plans.boundary_machines {
            boundary.structural_parameters[0].multiplicity = Multiplicity::Affine;
        }
        for machine in &mut plans.machines {
            machine.structural_parameters[0].multiplicity = Multiplicity::Affine;
            machine.entry_claims[0].path = vec![
                CheckedUnitStructuralPathSegment::Field("#7".to_owned()),
                CheckedUnitStructuralPathSegment::Field("#9".to_owned()),
            ];
        }

        let lowered = lower_machine(&checked, "example::Root::enter")
            .expect("nested record custody should cross the complete Unit closure");
        for machine in &lowered.semantic_module.machines {
            assert_eq!(
                machine.structural_parameters[0].multiplicity,
                StructuralMultiplicity::Affine
            );
            assert_eq!(machine.entry_claims.len(), 1);
            assert_eq!(
                machine.entry_claims[0].path,
                [
                    StructuralPathSegment::Field("#7".to_owned()),
                    StructuralPathSegment::Field("#9".to_owned()),
                ]
            );
        }
        let acknowledgement = lowered
            .semantic_module
            .structural_types
            .iter()
            .find(|shape| shape.identity == "example::Acknowledgement")
            .expect("lowered acknowledgement shape");
        let StructuralTypeShape::Record { fields } = &acknowledgement.shape else {
            panic!("acknowledgement is a record")
        };
        assert!(matches!(
            &fields[0].field_type,
            StructuralFieldType::Structural(structural_type)
                if lowered.semantic_module.structural_types.iter().any(|shape| {
                    shape.id == *structural_type && shape.identity == "example::Pocket"
                })
        ));
    }

    #[test]
    fn attached_unit_disjoint_sibling_claims_lower_as_one_aggregate_transfer() {
        let mut checked = hard_root_checked_fixture();
        let plans = &mut checked.facts.flow.terminal_unit_effects;
        plans
            .structural_types
            .push(psi_checked_trees::CheckedUnitStructuralTypePlan {
                identity: "example::Token".to_owned(),
                shape: CheckedUnitStructuralTypeShape::Record { fields: Vec::new() },
            });
        let acknowledgement = plans
            .structural_types
            .iter_mut()
            .find(|shape| shape.identity == "example::Acknowledgement")
            .expect("acknowledgement shape");
        let CheckedUnitStructuralTypeShape::Record { fields } = &mut acknowledgement.shape else {
            panic!("acknowledgement is a record")
        };
        fields[0].identity = "#7".to_owned();
        fields[0].field_type = CheckedUnitStructuralFieldType::Structural {
            type_identity: "example::Token".to_owned(),
        };
        fields.insert(
            1,
            psi_checked_trees::CheckedUnitStructuralFieldPlan {
                identity: "#9".to_owned(),
                relevance: psi_terminal::BindingRelevance::Relevant,
                field_type: CheckedUnitStructuralFieldType::Structural {
                    type_identity: "example::Token".to_owned(),
                },
            },
        );
        for boundary in &mut plans.boundary_machines {
            boundary.structural_parameters[0].multiplicity = Multiplicity::Affine;
        }
        for machine in &mut plans.machines {
            machine.structural_parameters[0].multiplicity = Multiplicity::Affine;
            machine.entry_claims[0].path =
                vec![CheckedUnitStructuralPathSegment::Field("#7".to_owned())];
            let mut sibling = machine.entry_claims[0].clone();
            sibling.claim_identity = unit_claim_at(machine.machine, machine.state, 1);
            sibling.path = vec![CheckedUnitStructuralPathSegment::Field("#9".to_owned())];
            machine.entry_claims.push(sibling);
        }
        let root = plans.machines[0].machine;
        let root_state = plans.machines[0].state;
        let CheckedUnitEffectOperationPlan::CallUnit {
            claim_transfers, ..
        } = &mut plans.machines[0].operations[0]
        else {
            unreachable!()
        };
        claim_transfers.push(psi_checked_trees::CheckedUnitClaimTransferPlan {
            claim_identity: unit_claim_at(root, root_state, 1),
            argument_index: 0,
        });
        let helper = plans.machines[1].machine;
        let helper_state = plans.machines[1].state;
        let CheckedUnitEffectOperationPlan::BoundaryCall {
            completion_receipts,
            ..
        } = &mut plans.machines[1].operations[1]
        else {
            unreachable!()
        };
        completion_receipts.push(psi_checked_trees::CheckedUnitClaimTransferPlan {
            claim_identity: unit_claim_at(helper, helper_state, 1),
            argument_index: 0,
        });

        let lowered = lower_machine(&checked, "example::Root::enter")
            .expect("both sibling resources should cross the complete Unit closure");
        for machine in &lowered.semantic_module.machines {
            assert_eq!(
                machine.structural_parameters[0].multiplicity,
                StructuralMultiplicity::Affine
            );
            assert_eq!(machine.entry_claims.len(), 2);
            assert_eq!(machine.entry_claims[0].claim, claim_id(1));
            assert_eq!(
                machine.entry_claims[0].path,
                [StructuralPathSegment::Field("#7".to_owned())]
            );
            assert_eq!(machine.entry_claims[1].claim, claim_id(2));
            assert_eq!(
                machine.entry_claims[1].path,
                [StructuralPathSegment::Field("#9".to_owned())]
            );
        }
        let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
            .expect("multi-field custody must have a canonical terminal encoding");
        assert_eq!(
            psi_terminal_codec::decode_module(&bytes).expect("canonical aggregate custody bytes"),
            lowered.semantic_module
        );
    }

    #[test]
    fn attached_unit_affine_argument_lowers_as_an_owned_transfer_without_a_claim_row() {
        let mut checked = hard_root_checked_fixture();
        let plans = &mut checked.facts.flow.terminal_unit_effects.machines;
        for plan in plans.iter_mut() {
            plan.structural_parameters[0].multiplicity = Multiplicity::Affine;
            plan.entry_claims.clear();
        }
        let CheckedUnitEffectOperationPlan::CallUnit {
            claim_transfers, ..
        } = &mut plans[0].operations[0]
        else {
            unreachable!()
        };
        claim_transfers.clear();
        plans[1].operations.retain(|operation| {
            !matches!(
                operation,
                CheckedUnitEffectOperationPlan::BoundaryCall { .. }
            )
        });
        let CheckedUnitEffectOperationPlan::ReturnUnit {
            trivial_affine_discards,
            ..
        } = plans[1].operations.last_mut().unwrap()
        else {
            unreachable!()
        };
        *trivial_affine_discards = vec![0];

        let lowered = lower_machine(&checked, "example::Root::enter")
            .expect("the checked affine Unit transfer should lower and verify");
        assert_eq!(
            lowered.semantic_module.machines[0].structural_parameters[0].multiplicity,
            StructuralMultiplicity::Affine
        );
        let OperationKind::CallUnit {
            claim_transfers, ..
        } = &lowered.semantic_module.machines[0].blocks[0].operations[0].kind
        else {
            unreachable!()
        };
        assert!(claim_transfers.is_empty());
    }

    #[test]
    fn attached_unit_affine_return_lowers_exact_no_code_discard() {
        let mut checked = hard_root_checked_fixture();
        let root = &mut checked.facts.flow.terminal_unit_effects.machines[0];
        root.structural_parameters[0].multiplicity = Multiplicity::Affine;
        root.entry_claims.clear();
        root.operations = vec![CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index: 0,
            trivial_affine_local_discard_ordinals: Vec::new(),
            trivial_affine_discards: vec![0],
        }];

        let lowered = lower_machine(&checked, "example::Root::enter")
            .expect("checked affine discard should lower as explicit return-edge cleanup");
        let [machine] = lowered.semantic_module.machines.as_slice() else {
            panic!("the no-call closure should contain only its root")
        };
        let [block] = machine.blocks.as_slice() else {
            panic!("the no-call root should contain one block")
        };
        let Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } = &block.terminator
        else {
            panic!("affine cleanup should remain attached to the Unit return")
        };
        assert_eq!(
            trivial_affine_discards,
            &[machine.structural_parameters[0].place]
        );
        let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
            .expect("affine discard must have a canonical terminal encoding");
        assert_eq!(
            psi_terminal_codec::decode_module(&bytes).expect("canonical affine discard bytes"),
            lowered.semantic_module
        );
    }

    #[test]
    fn attached_unit_hard_root_fails_closed_on_missing_transitive_member() {
        let mut checked = hard_root_checked_fixture();
        checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .retain(|machine| machine.contract_fingerprint != 0x202);

        assert!(matches!(
            lower_machine(&checked, "example::Root::enter"),
            Err(LoweringError::Unsupported(
                "attached Unit closure is missing a checked transitive machine plan"
            ))
        ));
    }

    #[test]
    fn attached_unit_port_write_requires_exact_direct_checked_port_service() {
        let mut checked = hard_root_checked_fixture();
        let empty = psi_language_semantics::ServiceReachRowTable::EMPTY_ROW;
        let helper = checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|machine| machine.contract_fingerprint == 0x202)
            .expect("helper plan");
        let CheckedUnitEffectOperationPlan::PortWrite { service_reach, .. } =
            &mut helper.operations[0]
        else {
            panic!("fixture begins helper with port output")
        };
        service_reach.direct = empty;

        assert!(matches!(
            lower_machine(&checked, "example::Root::enter"),
            Err(LoweringError::Unsupported(
                "port output does not carry the unique exact checked PortIo service"
            ))
        ));
    }

    fn source_projection(
        version: CheckedContentPlaceVersion,
        root: CheckedContentPlaceRoot,
        fields: &[(&str, u32)],
        semantic_domain: SemanticDomainId,
    ) -> CheckedContentConservationTerm {
        CheckedContentConservationTerm::Projection {
            domain: SymbolHandle::from_arena_index(70),
            semantic_domain,
            projection_machine: SymbolHandle::from_arena_index(71),
            projection_fingerprint: 0xfeed,
            subject: CheckedContentStructuralPlace {
                version,
                root,
                segments: fields
                    .iter()
                    .map(|(name, symbol)| {
                        CheckedContentPlaceSegment::Field(ContentFieldSegment {
                            symbol: SymbolHandle::from_arena_index(*symbol),
                            name: (*name).to_owned(),
                        })
                    })
                    .collect(),
            },
        }
    }

    #[test]
    fn scalar_machine_builder_uses_a_disjoint_module_identity_namespace() {
        let identity_base = TERMINAL_MACHINE_IDENTITY_STRIDE;
        let lowered = build_scalar_graph_module(
            &[LoweredScalarBranchState {
                parameter_types: vec![ScalarType::Boolean],
                bindings: Vec::new(),
                terminator: LoweredScalarBranchTerminator::Return {
                    expression: LoweredDirectExpression::Boolean {
                        expression: Box::new(LoweredBooleanReturnExpression::Parameter {
                            position: 0,
                        }),
                    },
                },
            }],
            ScalarType::Boolean,
            None,
            Vec::new(),
            LoweredContentIdentityReshuffles {
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                reshuffles: Vec::new(),
                source_claims: Vec::new(),
            },
            LoweredContentPartitionCompositions {
                structural_places: Vec::new(),
                compositions: Vec::new(),
            },
            machine_id(2),
            identity_base,
            &[],
            &[],
        )
        .expect("a nonentry machine should lower in its disjoint identity range");

        let [machine] = lowered.semantic_module.machines.as_slice() else {
            panic!("the isolated builder emits one machine")
        };
        assert_eq!(machine.id, machine_id(2));
        assert_eq!(machine.contract.id, contract_id(2));
        assert_eq!(machine.entry, block_id(identity_base + 1));
        assert_eq!(machine.parameters[0].id, value_id(identity_base + 1));
        assert_eq!(
            machine
                .result
                .scalar()
                .expect("the scalar fixture has a result")
                .id,
            value_id(identity_base + 2)
        );
        let Terminator::Return { edge, value, .. } = machine.blocks[0].terminator else {
            panic!("the fixture should retain its scalar return")
        };
        assert_eq!(edge, edge_id(identity_base + 1));
        assert_eq!(value, value_id(identity_base + 1));
    }

    #[test]
    fn primitive_scalar_source_jump_emits_empty_affine_cleanup() {
        let identity_base = TERMINAL_MACHINE_IDENTITY_STRIDE;
        let parameter_expression = || LoweredDirectExpression::Boolean {
            expression: Box::new(LoweredBooleanReturnExpression::Parameter { position: 0 }),
        };
        let lowered = build_scalar_graph_module(
            &[
                LoweredScalarBranchState {
                    parameter_types: vec![ScalarType::Boolean],
                    bindings: Vec::new(),
                    terminator: LoweredScalarBranchTerminator::Jump {
                        target: 1,
                        arguments: vec![parameter_expression()],
                    },
                },
                LoweredScalarBranchState {
                    parameter_types: vec![ScalarType::Boolean],
                    bindings: Vec::new(),
                    terminator: LoweredScalarBranchTerminator::Return {
                        expression: parameter_expression(),
                    },
                },
            ],
            ScalarType::Boolean,
            None,
            Vec::new(),
            LoweredContentIdentityReshuffles {
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                reshuffles: Vec::new(),
                source_claims: Vec::new(),
            },
            LoweredContentPartitionCompositions {
                structural_places: Vec::new(),
                compositions: Vec::new(),
            },
            machine_id(2),
            identity_base,
            &[],
            &[],
        )
        .expect("primitive scalar jump should lower");

        let Terminator::Jump {
            trivial_affine_discards,
            ..
        } = &lowered.semantic_module.machines[0].blocks[0].terminator
        else {
            panic!("first scalar block should jump")
        };
        assert!(trivial_affine_discards.is_empty());
    }

    #[test]
    fn primitive_scalar_source_conditional_emits_empty_affine_cleanup() {
        let identity_base = TERMINAL_MACHINE_IDENTITY_STRIDE;
        let parameter_expression = || LoweredDirectExpression::Boolean {
            expression: Box::new(LoweredBooleanReturnExpression::Parameter { position: 0 }),
        };
        let states = [
            LoweredScalarBranchState {
                parameter_types: vec![ScalarType::Boolean],
                bindings: Vec::new(),
                terminator: LoweredScalarBranchTerminator::Conditional {
                    condition: LoweredBooleanReturnExpression::Parameter { position: 0 },
                    when_true_target: 1,
                    when_true_arguments: vec![parameter_expression()],
                    when_false_target: 2,
                    when_false_arguments: vec![parameter_expression()],
                },
            },
            LoweredScalarBranchState {
                parameter_types: vec![ScalarType::Boolean],
                bindings: Vec::new(),
                terminator: LoweredScalarBranchTerminator::Return {
                    expression: parameter_expression(),
                },
            },
            LoweredScalarBranchState {
                parameter_types: vec![ScalarType::Boolean],
                bindings: Vec::new(),
                terminator: LoweredScalarBranchTerminator::Return {
                    expression: parameter_expression(),
                },
            },
        ];
        let lowered = build_scalar_graph_module(
            &states,
            ScalarType::Boolean,
            None,
            Vec::new(),
            LoweredContentIdentityReshuffles {
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                reshuffles: Vec::new(),
                source_claims: Vec::new(),
            },
            LoweredContentPartitionCompositions {
                structural_places: Vec::new(),
                compositions: Vec::new(),
            },
            machine_id(2),
            identity_base,
            &[],
            &[],
        )
        .expect("primitive scalar conditional should lower");

        let Terminator::Conditional {
            when_true,
            when_false,
            ..
        } = &lowered.semantic_module.machines[0].blocks[0].terminator
        else {
            panic!("first scalar block should branch")
        };
        assert!(when_true.trivial_affine_discards.is_empty());
        assert!(when_false.trivial_affine_discards.is_empty());
    }

    fn source_plan_with_domain(semantic_domain: SemanticDomainId) -> ContentConservationPlan {
        let entry = source_projection(
            CheckedContentPlaceVersion::Entry,
            CheckedContentPlaceRoot::Parameter {
                position: 0,
                symbol: SymbolHandle::from_arena_index(10),
                name: "extent".to_owned(),
                is_self: false,
            },
            &[],
            semantic_domain,
        );
        let left = source_projection(
            CheckedContentPlaceVersion::Current,
            CheckedContentPlaceRoot::Result,
            &[("left", 11)],
            semantic_domain,
        );
        let right = source_projection(
            CheckedContentPlaceVersion::Current,
            CheckedContentPlaceRoot::Result,
            &[("right", 12)],
            semantic_domain,
        );
        let algebra = CheckedContentAlgebraIdentity::IntervalSet {
            coordinate_space: "Address".to_owned(),
        };
        let equation = ContentConservationEquation::new(
            entry,
            CheckedContentConservationTerm::separate([right, left]),
        );
        let fingerprint = conservation_fingerprint(&algebra, &equation);
        ContentConservationPlan {
            owner_kind: ContentConservationOwnerKind::Machine,
            owner: SymbolHandle::from_arena_index(20),
            callable: SymbolHandle::from_arena_index(21),
            algebra,
            equation,
            fingerprint,
        }
    }

    fn source_plan() -> ContentConservationPlan {
        source_plan_with_domain(SemanticDomainId(9))
    }

    fn direct_source_plan(
        semantic_domain: SemanticDomainId,
        output_field: &str,
    ) -> ContentConservationPlan {
        let entry = source_projection(
            CheckedContentPlaceVersion::Entry,
            CheckedContentPlaceRoot::Parameter {
                position: 0,
                symbol: SymbolHandle::from_arena_index(10),
                name: "extent".to_owned(),
                is_self: false,
            },
            &[],
            semantic_domain,
        );
        let output = source_projection(
            CheckedContentPlaceVersion::Current,
            CheckedContentPlaceRoot::Result,
            &[(output_field, 11)],
            semantic_domain,
        );
        let algebra = CheckedContentAlgebraIdentity::IntervalSet {
            coordinate_space: "Address".to_owned(),
        };
        let equation = ContentConservationEquation::new(entry, output);
        let fingerprint = conservation_fingerprint(&algebra, &equation);
        ContentConservationPlan {
            owner_kind: ContentConservationOwnerKind::Machine,
            owner: SymbolHandle::from_arena_index(20),
            callable: SymbolHandle::from_arena_index(21),
            algebra,
            equation,
            fingerprint,
        }
    }

    fn case_direct_source_plan(semantic_domain: SemanticDomainId) -> ContentConservationPlan {
        let segments = || {
            vec![
                CheckedContentPlaceSegment::Case(ContentCaseSegment {
                    symbol: SymbolHandle::from_arena_index(30),
                    name: "Present".to_owned(),
                }),
                CheckedContentPlaceSegment::Field(ContentFieldSegment {
                    symbol: SymbolHandle::from_arena_index(31),
                    name: "region".to_owned(),
                }),
            ]
        };
        let projection = |version, root| CheckedContentConservationTerm::Projection {
            domain: SymbolHandle::from_arena_index(70),
            semantic_domain,
            projection_machine: SymbolHandle::from_arena_index(71),
            projection_fingerprint: 0xfeed,
            subject: CheckedContentStructuralPlace {
                version,
                root,
                segments: segments(),
            },
        };
        let equation = ContentConservationEquation::new(
            projection(
                CheckedContentPlaceVersion::Entry,
                CheckedContentPlaceRoot::Parameter {
                    position: 0,
                    symbol: SymbolHandle::from_arena_index(10),
                    name: "envelope".to_owned(),
                    is_self: false,
                },
            ),
            projection(
                CheckedContentPlaceVersion::Current,
                CheckedContentPlaceRoot::Result,
            ),
        );
        let algebra = CheckedContentAlgebraIdentity::IntervalSet {
            coordinate_space: "Address".to_owned(),
        };
        let fingerprint = conservation_fingerprint(&algebra, &equation);
        ContentConservationPlan {
            owner_kind: ContentConservationOwnerKind::Machine,
            owner: SymbolHandle::from_arena_index(20),
            callable: SymbolHandle::from_arena_index(21),
            algebra,
            equation,
            fingerprint,
        }
    }

    fn identity_fact(
        semantic_domain: SemanticDomainId,
        output_field: &str,
        ordinal: u32,
    ) -> ContentIdentityReshuffleFact {
        ContentIdentityReshuffleFact {
            machine_symbol: SymbolHandle::from_arena_index(20),
            state_symbol: SymbolHandle::from_arena_index(21),
            claim_identity: PermissionClaimIdentity::Established {
                machine_symbol: SymbolHandle::from_arena_index(20),
                state_symbol: SymbolHandle::from_arena_index(21),
                source: PermissionEventSource::StateEntry,
                ordinal,
            },
            input_parameter_symbol: SymbolHandle::from_arena_index(10),
            input_segments: Default::default(),
            output_segments: Default::default(),
            plan: direct_source_plan(semantic_domain, output_field),
        }
    }

    fn partition_composition_fact() -> ContentPartitionCompositionFact {
        fn subjects(
            term: &CheckedContentConservationTerm,
            output: &mut Vec<CheckedContentStructuralPlace>,
        ) {
            match term {
                CheckedContentConservationTerm::Projection { subject, .. } => {
                    if !output.contains(subject) {
                        output.push(subject.clone());
                    }
                }
                CheckedContentConservationTerm::Separate(terms) => {
                    for term in terms {
                        subjects(term, output);
                    }
                }
            }
        }

        let mut source_plan = source_plan();
        source_plan.owner = SymbolHandle::from_arena_index(30);
        source_plan.callable = SymbolHandle::from_arena_index(31);
        let mut plan = source_plan.clone();
        plan.owner = SymbolHandle::from_arena_index(20);
        plan.callable = SymbolHandle::from_arena_index(21);
        let mut places = Vec::new();
        subjects(source_plan.equation.left(), &mut places);
        subjects(source_plan.equation.right(), &mut places);
        let claim_identity = identity_fact(SemanticDomainId(9), "left", 1).claim_identity;
        let CheckedContentConservationTerm::Projection { subject, .. } =
            source_plan.equation.left()
        else {
            panic!("fixture source input is a projection")
        };
        let entry_place = subject.clone();
        ContentPartitionCompositionFact {
            machine_symbol: plan.owner,
            state_symbol: plan.callable,
            source_callable: source_plan.callable,
            source_fingerprint: source_plan.fingerprint,
            source_derivation_depth: 0,
            source_plan,
            statement_index: 4,
            call_ordinal: 2,
            input_claim_identities: vec![claim_identity],
            input_claim_bindings: vec![psi_checked_trees::ContentPartitionInputClaimBinding {
                claim_identity,
                entry_place,
            }],
            result_rewrites: Vec::new(),
            substitutions: places
                .into_iter()
                .map(
                    |place| psi_checked_trees::ContentPartitionPlaceSubstitution {
                        source: place.clone(),
                        target: place,
                    },
                )
                .collect(),
            plan,
        }
    }

    #[test]
    fn checked_content_plan_lowers_without_arena_local_identity() {
        let plan = source_plan();
        let lowered = lower_content_conservation_plan(&plan).expect("lowered conservation");

        assert_eq!(lowered.source_fingerprint, plan.fingerprint);
        assert_eq!(
            lowered.structural_places,
            vec![
                StructuralPlaceDeclaration {
                    id: PlaceId::new(1).expect("parameter place"),
                    kind: StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: false,
                    },
                },
                StructuralPlaceDeclaration {
                    id: PlaceId::new(RESULT_STRUCTURAL_PLACE_ID).expect("result place"),
                    kind: StructuralPlaceKind::Result,
                },
            ]
        );
        let structural_places = lowered
            .structural_places
            .iter()
            .map(|place| (place.id, place.kind))
            .collect();
        let Proposition::ContentConservation(conservation) = &lowered.proposition else {
            panic!("content proposition")
        };
        assert_eq!(
            psi_core::content_conservation_fingerprint(conservation, &structural_places),
            Some(plan.fingerprint),
            "terminal reconstruction must preserve the checked-plan identity preimage"
        );

        let Proposition::ContentConservation(conservation) = lowered.proposition else {
            panic!("content proposition")
        };
        assert_eq!(
            conservation.algebra(),
            &ContentAlgebra {
                kind: ContentAlgebraKind::IntervalSet,
                parameter: "Address".to_owned(),
            }
        );
        let ContentTerm::Projection {
            projection,
            subject,
        } = conservation.left()
        else {
            panic!("entry projection")
        };
        assert_eq!(projection.domain.get(), 9);
        assert_eq!(projection.projection_fingerprint, 0xfeed);
        assert_eq!(subject.version, ContentPlaceVersion::Entry);
        assert_eq!(subject.root.get(), 1);
        assert!(subject.segments.is_empty());
        let ContentTerm::Separate(parts) = conservation.right() else {
            panic!("separated result")
        };
        assert_eq!(parts.len(), 2);
        assert!(matches!(
            &parts[0],
            ContentTerm::Projection { subject, .. }
                if subject.segments == [ContentPlaceSegment::Field("left".to_owned())]
        ));
        assert!(matches!(
            &parts[1],
            ContentTerm::Projection { subject, .. }
                if subject.segments == [ContentPlaceSegment::Field("right".to_owned())]
        ));
    }

    #[test]
    fn checked_crash_frontier_maps_only_through_dense_terminal_claims() {
        let first = PermissionClaimIdentity::Established {
            machine_symbol: SymbolHandle::from_arena_index(1),
            state_symbol: SymbolHandle::from_arena_index(2),
            source: PermissionEventSource::StateEntry,
            ordinal: 0,
        };
        let second = PermissionClaimIdentity::Established {
            machine_symbol: SymbolHandle::from_arena_index(1),
            state_symbol: SymbolHandle::from_arena_index(2),
            source: PermissionEventSource::Statement { statement_index: 1 },
            ordinal: 1,
        };
        let first_id = ClaimId::new(1).expect("claim");
        let second_id = ClaimId::new(2).expect("claim");
        assert_eq!(
            lower_checked_crash_frontier(
                &[first, second],
                &[(second, second_id), (first, first_id)],
            ),
            Ok(vec![first_id, second_id])
        );

        let missing = PermissionClaimIdentity::Established {
            machine_symbol: SymbolHandle::from_arena_index(1),
            state_symbol: SymbolHandle::from_arena_index(2),
            source: PermissionEventSource::Statement { statement_index: 2 },
            ordinal: 2,
        };
        assert_eq!(
            lower_checked_crash_frontier(&[missing], &[(first, first_id)]),
            Err(LoweringError::CrashFrontierClaimNotLowered(missing)),
            "terminal production must not silently omit a checked abandoned claim"
        );
    }

    #[test]
    fn checked_partition_composition_lowers_with_exact_source_and_dense_claims() {
        let identity = identity_fact(SemanticDomainId(9), "left", 1);
        let mut identities =
            lower_content_identity_reshuffles(&[identity]).expect("identity fact lowers");
        let fact = partition_composition_fact();
        let lowered =
            lower_content_partition_compositions(std::slice::from_ref(&fact), &mut identities)
                .expect("exact theorem substitution lowers");

        assert_eq!(lowered.compositions.len(), 1);
        let row = &lowered.compositions[0];
        assert_eq!(row.source_fingerprint, fact.source_fingerprint);
        assert_eq!(row.input_claims, vec![ClaimId::new(1).expect("claim")]);
        assert_eq!(row.substitutions.len(), 3);
        assert_eq!(row.source, row.derived);

        let mut staged = fact.clone();
        let source = staged.substitutions[0].source.clone();
        let target = staged.substitutions[0].target.clone();
        staged
            .result_rewrites
            .push(psi_checked_trees::ContentPartitionResultRewrite {
                claim_identity: identity_fact(SemanticDomainId(9), "left", 2).claim_identity,
                source,
                target,
            });
        let identities_before_error = identities.clone();
        assert_eq!(
            lower_content_partition_compositions(&[staged], &mut identities),
            Err(LoweringError::ContentPartitionResultRewriteUnsupported)
        );
        assert_eq!(identities, identities_before_error);

        let mut derived_source = fact.clone();
        derived_source.source_derivation_depth = 1;
        assert_eq!(
            lower_content_partition_compositions(&[derived_source], &mut identities),
            Err(LoweringError::ContentPartitionDerivedSourceUnsupported)
        );

        let mut drifted = fact;
        let projection = drifted.plan.equation.left().clone();
        drifted.plan.equation = ContentConservationEquation::new(
            projection.clone(),
            CheckedContentConservationTerm::separate([projection.clone(), projection]),
        );
        assert_eq!(
            lower_content_partition_compositions(&[drifted], &mut identities),
            Err(LoweringError::ContentPartitionReplayMismatch)
        );
    }

    #[test]
    fn checked_partition_composition_lowers_a_partition_only_entry_claim() {
        let fact = partition_composition_fact();
        let mut identities =
            lower_content_identity_reshuffles(&[]).expect("empty identity evidence lowers");
        let lowered =
            lower_content_partition_compositions(std::slice::from_ref(&fact), &mut identities)
                .expect("partition input binding lowers independently of output equality");

        assert!(identities.reshuffles.is_empty());
        assert_eq!(identities.entry_claims.len(), 1);
        assert_eq!(
            identities.entry_claims[0].claim,
            ClaimId::new(1).expect("dense claim")
        );
        assert_eq!(
            identities.entry_claims[0].input.version,
            ContentPlaceVersion::Entry
        );
        assert_eq!(
            lowered.compositions[0].input_claims,
            vec![ClaimId::new(1).expect("dense claim")]
        );
    }

    #[test]
    fn checked_content_plan_fails_closed_on_corrupt_identity() {
        let mut plan = source_plan();
        plan.fingerprint ^= 1;
        assert!(matches!(
            lower_content_conservation_plan(&plan),
            Err(LoweringError::ContentConservationFingerprintMismatch { .. })
        ));

        let plan = source_plan_with_domain(SemanticDomainId::NULL);
        assert_eq!(
            lower_content_conservation_plan(&plan),
            Err(LoweringError::InvalidContentDomainIdentity)
        );
    }

    #[test]
    fn checked_identity_facts_group_exact_projections_into_canonical_terminal_rows() {
        let first = identity_fact(SemanticDomainId(9), "payload", 0);
        let second = identity_fact(SemanticDomainId(10), "payload", 0);
        let lowered = lower_content_identity_reshuffles(&[second.clone(), first.clone()])
            .expect("exact checked identity facts lower");
        let reordered = lower_content_identity_reshuffles(&[first, second])
            .expect("source fact order is irrelevant");

        assert_eq!(lowered, reordered);
        assert_eq!(lowered.structural_places.len(), 2);
        assert_eq!(lowered.reshuffles.len(), 1);
        let row = &lowered.reshuffles[0];
        assert_eq!(row.claim, ClaimId::new(1).expect("dense claim"));
        assert_eq!(row.input.version, ContentPlaceVersion::Entry);
        assert_eq!(row.input.root, PlaceId::new(1).expect("parameter root"));
        assert_eq!(row.output.version, ContentPlaceVersion::Current);
        assert_eq!(
            row.output.root,
            PlaceId::new(RESULT_STRUCTURAL_PLACE_ID).expect("result root")
        );
        assert_eq!(
            row.output.segments,
            [ContentPlaceSegment::Field("payload".to_owned())]
        );
        assert_eq!(
            row.projections
                .iter()
                .map(|projection| projection.projection.domain.get())
                .collect::<Vec<_>>(),
            vec![9, 10]
        );
        assert_eq!(row.inferred_propositions().count(), 2);
    }

    #[test]
    fn checked_identity_fact_lowers_stable_sum_case_paths_without_arena_identity() {
        let mut fact = identity_fact(SemanticDomainId(9), "unused", 0);
        fact.plan = case_direct_source_plan(SemanticDomainId(9));

        let lowered =
            lower_content_identity_reshuffles(&[fact]).expect("sum-case identity fact lowers");
        let [row] = lowered.reshuffles.as_slice() else {
            panic!("one terminal reshuffle row");
        };
        let expected = [
            ContentPlaceSegment::Case("Present".to_owned()),
            ContentPlaceSegment::Field("region".to_owned()),
        ];
        assert_eq!(row.input.segments, expected);
        assert_eq!(row.output.segments, expected);
    }

    #[test]
    fn checked_identity_fact_lowering_revalidates_claim_and_equation_shape() {
        let mut unknown = identity_fact(SemanticDomainId(9), "payload", 0);
        unknown.claim_identity = PermissionClaimIdentity::Unknown;
        assert_eq!(
            lower_content_identity_reshuffles(&[unknown]),
            Err(LoweringError::UnknownContentClaimIdentity)
        );

        let mut partition = identity_fact(SemanticDomainId(9), "payload", 0);
        partition.plan = source_plan();
        assert_eq!(
            lower_content_identity_reshuffles(&[partition]),
            Err(LoweringError::ContentIdentityNotDirectEquality)
        );

        let mut moved_twice = identity_fact(SemanticDomainId(9), "left", 0);
        let second_destination = identity_fact(SemanticDomainId(10), "right", 0);
        assert_eq!(
            lower_content_identity_reshuffles(&[moved_twice.clone(), second_destination]),
            Err(LoweringError::ContentIdentityClaimMapsMultiplePlaces)
        );

        moved_twice.plan.owner = SymbolHandle::from_arena_index(99);
        assert_eq!(
            lower_content_identity_reshuffles(&[moved_twice]),
            Err(LoweringError::ContentIdentityFactOwnerMismatch)
        );
    }

    #[test]
    fn structural_scalar_return_reconstructs_closed_exact_expression_proof() {
        let mut checked = hard_root_checked_fixture();
        install_structural_scalar_return_fixture(&mut checked);
        let landed = |value| {
            psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
                psi_numerics::literals::IntegerLanding {
                    landed_type: psi_numerics::literals::LandedIntegerType::I32,
                    domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
                },
            )
        };
        checked.facts.values.scalar_expressions.expressions[0].expression =
            CheckedScalarExpression::IntegerBinary {
                kind: CheckedIntegerBinaryKind::ExactAdd,
                primitive_type: PrimitiveType::I32,
                left: Box::new(CheckedScalarExpression::IntegerLiteral { literal: landed(3) }),
                right: Box::new(CheckedScalarExpression::IntegerLiteral { literal: landed(4) }),
            };

        let lowered = lower_machine(&checked, "example::Root::enter")
            .expect("closed exact expression should lower with reconstructed proof");
        let operations = &lowered.semantic_module.machines[0].blocks[0].operations;
        assert!(matches!(
            operations.as_slice(),
            [
                Operation {
                    kind: OperationKind::IntegerConstant { .. },
                    ..
                },
                Operation {
                    kind: OperationKind::IntegerConstant { .. },
                    ..
                },
                Operation {
                    kind: OperationKind::ExactIntegerAdd { .. },
                    ..
                }
            ]
        ));
        assert_eq!(lowered.proof_bundle.evidence.len(), 1);
        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &psi_proof_kernel::AdmissionProfile::default(),
        )
        .expect("the reconstructed exact-operation proof should verify canonically");
        let module_bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
            .expect("closed structural expression module should encode canonically");
        assert_eq!(
            psi_terminal_codec::decode_module(&module_bytes)
                .expect("closed structural expression module should decode canonically"),
            lowered.semantic_module
        );
        assert!(matches!(
            &lowered.semantic_module.machines[0].blocks[0].terminator,
            Terminator::Return {
                cleanup_actions,
                ..
            } if cleanup_actions == &[
                TerminalAffineCleanupAction::DiscardRoot(place_id(2)),
                TerminalAffineCleanupAction::DiscardRoot(place_id(1)),
            ]
        ));

        checked.facts.values.scalar_expressions.expressions[0].expression =
            CheckedScalarExpression::Parameter {
                position: 0,
                primitive_type: PrimitiveType::I32,
            };
        assert!(matches!(
            lower_machine(&checked, "example::Root::enter"),
            Err(LoweringError::Unsupported(
                "structural scalar return is outside its checked value/control slice"
            ))
        ));
    }

    #[test]
    fn structural_scalar_return_materializes_branch_free_local_prefix_before_cleanup() {
        let mut checked = hard_root_checked_fixture();
        install_structural_scalar_return_fixture(&mut checked);
        let landed = |value| {
            psi_numerics::literals::IntegerLiteral::from_value(value).with_landing(
                psi_numerics::literals::IntegerLanding {
                    landed_type: psi_numerics::literals::LandedIntegerType::I32,
                    domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
                },
            )
        };
        let plan = &mut checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .machines[0];
        plan.bindings = vec![psi_checked_trees::CheckedScalarBinding {
            statement_ordinal: 0,
            primitive_type: PrimitiveType::I32,
            value: CheckedScalarBindingValue::Expression,
        }];
        plan.return_statement_ordinal = 1;
        checked.facts.values.scalar_expressions.expressions = vec![
            psi_checked_trees::CheckedLocatedScalarExpression {
                state: plan.state,
                statement_ordinal: 0,
                role: CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
                expression: CheckedScalarExpression::IntegerBinary {
                    kind: CheckedIntegerBinaryKind::ExactAdd,
                    primitive_type: PrimitiveType::I32,
                    left: Box::new(CheckedScalarExpression::IntegerLiteral { literal: landed(3) }),
                    right: Box::new(CheckedScalarExpression::IntegerLiteral { literal: landed(4) }),
                },
            },
            psi_checked_trees::CheckedLocatedScalarExpression {
                state: plan.state,
                statement_ordinal: 1,
                role: CheckedScalarExpressionRole::Return,
                expression: CheckedScalarExpression::Local {
                    position: 0,
                    primitive_type: PrimitiveType::I32,
                },
            },
        ];

        let lowered = lower_machine(&checked, "example::Root::enter")
            .expect("checked local prefix should lower before exact affine cleanup");
        let block = &lowered.semantic_module.machines[0].blocks[0];
        assert!(matches!(
            block.operations.as_slice(),
            [
                Operation {
                    kind: OperationKind::IntegerConstant { .. },
                    ..
                },
                Operation {
                    kind: OperationKind::IntegerConstant { .. },
                    ..
                },
                Operation {
                    kind: OperationKind::ExactIntegerAdd { .. },
                    ..
                }
            ]
        ));
        assert!(matches!(
            &block.terminator,
            Terminator::Return {
                value,
                cleanup_actions,
                ..
            } if *value == value_id(3)
                && cleanup_actions == &[
                    TerminalAffineCleanupAction::DiscardRoot(place_id(2)),
                    TerminalAffineCleanupAction::DiscardRoot(place_id(1)),
                ]
        ));
        assert_eq!(lowered.proof_bundle.evidence.len(), 1);
        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &psi_proof_kernel::AdmissionProfile::default(),
        )
        .expect("local-prefix cleanup module should verify");

        checked.facts.values.scalar_expressions.expressions[0].expression =
            CheckedScalarExpression::Local {
                position: 0,
                primitive_type: PrimitiveType::I32,
            };
        assert!(matches!(
            lower_machine(&checked, "example::Root::enter"),
            Err(LoweringError::Unsupported(
                "structural scalar binding is not one branch-free local expression"
            ))
        ));
    }

    #[test]
    fn structural_scalar_return_supports_repeated_carried_short_circuit_local_continuations() {
        let mut checked = hard_root_checked_fixture();
        install_structural_scalar_return_fixture(&mut checked);
        let plan = &mut checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .machines[0];
        plan.bindings = vec![
            psi_checked_trees::CheckedScalarBinding {
                statement_ordinal: 0,
                primitive_type: PrimitiveType::Bool,
                value: CheckedScalarBindingValue::Expression,
            },
            psi_checked_trees::CheckedScalarBinding {
                statement_ordinal: 1,
                primitive_type: PrimitiveType::Bool,
                value: CheckedScalarBindingValue::Expression,
            },
            psi_checked_trees::CheckedScalarBinding {
                statement_ordinal: 2,
                primitive_type: PrimitiveType::Bool,
                value: CheckedScalarBindingValue::Expression,
            },
            psi_checked_trees::CheckedScalarBinding {
                statement_ordinal: 3,
                primitive_type: PrimitiveType::Bool,
                value: CheckedScalarBindingValue::Expression,
            },
            psi_checked_trees::CheckedScalarBinding {
                statement_ordinal: 4,
                primitive_type: PrimitiveType::Bool,
                value: CheckedScalarBindingValue::Expression,
            },
            psi_checked_trees::CheckedScalarBinding {
                statement_ordinal: 5,
                primitive_type: PrimitiveType::Bool,
                value: CheckedScalarBindingValue::Expression,
            },
            psi_checked_trees::CheckedScalarBinding {
                statement_ordinal: 6,
                primitive_type: PrimitiveType::Bool,
                value: CheckedScalarBindingValue::Expression,
            },
        ];
        plan.result_type = PrimitiveType::Bool;
        plan.return_statement_ordinal = 7;
        checked.facts.values.scalar_expressions.expressions = vec![
            psi_checked_trees::CheckedLocatedScalarExpression {
                state: plan.state,
                statement_ordinal: 0,
                role: CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
                expression: CheckedScalarExpression::Boolean(Box::new(
                    CheckedBooleanExpression::Constant(true),
                )),
            },
            psi_checked_trees::CheckedLocatedScalarExpression {
                state: plan.state,
                statement_ordinal: 1,
                role: CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 1 },
                expression: CheckedScalarExpression::Boolean(Box::new(
                    CheckedBooleanExpression::And {
                        left: Box::new(CheckedBooleanExpression::Local { position: 0 }),
                        right: Box::new(CheckedBooleanExpression::Constant(false)),
                    },
                )),
            },
            psi_checked_trees::CheckedLocatedScalarExpression {
                state: plan.state,
                statement_ordinal: 2,
                role: CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 2 },
                expression: CheckedScalarExpression::Boolean(Box::new(
                    CheckedBooleanExpression::Not(Box::new(CheckedBooleanExpression::Local {
                        position: 1,
                    })),
                )),
            },
            psi_checked_trees::CheckedLocatedScalarExpression {
                state: plan.state,
                statement_ordinal: 3,
                role: CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 3 },
                expression: CheckedScalarExpression::Boolean(Box::new(
                    CheckedBooleanExpression::Or {
                        left: Box::new(CheckedBooleanExpression::Local { position: 2 }),
                        right: Box::new(CheckedBooleanExpression::Constant(false)),
                    },
                )),
            },
            psi_checked_trees::CheckedLocatedScalarExpression {
                state: plan.state,
                statement_ordinal: 4,
                role: CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 4 },
                expression: CheckedScalarExpression::Boolean(Box::new(
                    CheckedBooleanExpression::Not(Box::new(CheckedBooleanExpression::Local {
                        position: 3,
                    })),
                )),
            },
            psi_checked_trees::CheckedLocatedScalarExpression {
                state: plan.state,
                statement_ordinal: 5,
                role: CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 5 },
                expression: CheckedScalarExpression::Boolean(Box::new(
                    CheckedBooleanExpression::And {
                        left: Box::new(CheckedBooleanExpression::Local { position: 4 }),
                        right: Box::new(CheckedBooleanExpression::Constant(true)),
                    },
                )),
            },
            psi_checked_trees::CheckedLocatedScalarExpression {
                state: plan.state,
                statement_ordinal: 6,
                role: CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 6 },
                expression: CheckedScalarExpression::Boolean(Box::new(
                    CheckedBooleanExpression::Not(Box::new(CheckedBooleanExpression::Local {
                        position: 5,
                    })),
                )),
            },
            psi_checked_trees::CheckedLocatedScalarExpression {
                state: plan.state,
                statement_ordinal: 7,
                role: CheckedScalarExpressionRole::Return,
                expression: CheckedScalarExpression::Boolean(Box::new(
                    CheckedBooleanExpression::Local { position: 6 },
                )),
            },
        ];

        let lowered = lower_machine(&checked, "example::Root::enter")
            .expect("repeated short-circuit locals should compose through carried continuations");
        let machine = &lowered.semantic_module.machines[0];
        assert_eq!(machine.blocks.len(), 16);
        let second_stage = machine
            .blocks
            .iter()
            .find(|block| block.id == block_id(6))
            .expect("the first short-circuit result enters the second decision stage");
        let third_stage = machine
            .blocks
            .iter()
            .find(|block| block.id == block_id(11))
            .expect("the second short-circuit result enters the third decision stage");
        let continuation = machine
            .blocks
            .iter()
            .find(|block| block.id == block_id(16))
            .expect("the final short-circuit result enters the return continuation");
        assert!(matches!(
            machine.blocks[0].operations.first(),
            Some(Operation {
                kind: OperationKind::BooleanConstant { value: true },
                ..
            })
        ));
        assert!(matches!(
            second_stage.parameters.as_slice(),
            [ValueDeclaration {
                scalar_type: ScalarType::Boolean,
                ..
            }]
        ));
        assert!(matches!(
            second_stage.operations.as_slice(),
            [Operation {
                kind: OperationKind::BooleanNot { .. },
                ..
            }]
        ));
        assert!(matches!(
            third_stage.parameters.as_slice(),
            [ValueDeclaration {
                scalar_type: ScalarType::Boolean,
                ..
            }]
        ));
        assert!(matches!(
            third_stage.operations.as_slice(),
            [Operation {
                kind: OperationKind::BooleanNot { .. },
                ..
            }]
        ));
        assert!(matches!(
            continuation.parameters.as_slice(),
            [ValueDeclaration {
                scalar_type: ScalarType::Boolean,
                ..
            }]
        ));
        assert!(matches!(
            continuation.operations.as_slice(),
            [Operation {
                kind: OperationKind::BooleanNot { .. },
                ..
            }]
        ));
        assert!(matches!(
            &continuation.terminator,
            Terminator::Return {
                cleanup_actions,
                ..
            } if cleanup_actions == &[
                TerminalAffineCleanupAction::DiscardRoot(place_id(2)),
                TerminalAffineCleanupAction::DiscardRoot(place_id(1)),
            ]
        ));
        assert!(
            machine.blocks[..15]
                .iter()
                .all(|block| match &block.terminator {
                    Terminator::Conditional {
                        when_true,
                        when_false,
                        ..
                    } =>
                        when_true.trivial_affine_discards.is_empty()
                            && when_false.trivial_affine_discards.is_empty(),
                    Terminator::Jump {
                        target,
                        trivial_affine_discards,
                        ..
                    } => {
                        matches!(*target, target if target == block_id(6)
                            || target == block_id(11)
                            || target == block_id(16))
                            && trivial_affine_discards.is_empty()
                    }
                    _ => false,
                })
        );
        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &psi_proof_kernel::AdmissionProfile::default(),
        )
        .expect("short-circuit local convergence should preserve the structural frontier");
        let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
            .expect("short-circuit local cleanup should encode canonically");
        assert_eq!(
            psi_terminal_codec::decode_module(&bytes)
                .expect("short-circuit local cleanup should decode canonically"),
            lowered.semantic_module
        );

        checked.facts.values.scalar_expressions.expressions[7].expression =
            CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::And {
                left: Box::new(CheckedBooleanExpression::Local { position: 6 }),
                right: Box::new(CheckedBooleanExpression::Constant(false)),
            }));
        let lowered = lower_machine(&checked, "example::Root::enter")
            .expect("repeated local decisions should feed a final short-circuit return");
        let machine = &lowered.semantic_module.machines[0];
        assert_eq!(machine.blocks.len(), 20);
        let final_decision = &machine.blocks[15..];
        assert!(matches!(
            final_decision[0].parameters.as_slice(),
            [ValueDeclaration {
                scalar_type: ScalarType::Boolean,
                ..
            }]
        ));
        assert!(matches!(
            final_decision[0].operations.as_slice(),
            [Operation {
                kind: OperationKind::BooleanNot { .. },
                ..
            }]
        ));
        assert!(final_decision.iter().all(|block| match &block.terminator {
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                when_true.trivial_affine_discards.is_empty()
                    && when_false.trivial_affine_discards.is_empty()
            }
            Terminator::Return {
                cleanup_actions, ..
            } =>
                cleanup_actions
                    == &[
                        TerminalAffineCleanupAction::DiscardRoot(place_id(2)),
                        TerminalAffineCleanupAction::DiscardRoot(place_id(1)),
                    ],
            _ => false,
        }));
        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &psi_proof_kernel::AdmissionProfile::default(),
        )
        .expect("final short-circuit cleanup should verify after repeated local convergence");
        let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
            .expect("composed final short-circuit cleanup should encode canonically");
        assert_eq!(
            psi_terminal_codec::decode_module(&bytes)
                .expect("composed final short-circuit cleanup should decode canonically"),
            lowered.semantic_module
        );

        checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .machines[0]
            .bindings[5]
            .primitive_type = PrimitiveType::I32;
        assert!(matches!(
            lower_machine(&checked, "example::Root::enter"),
            Err(LoweringError::Unsupported(
                "structural scalar short-circuit binding has a non-Boolean carrier"
            ))
        ));
    }

    #[test]
    fn structural_scalar_return_maps_interleaved_scalar_parameters_before_cleanup() {
        let mut checked = hard_root_checked_fixture();
        install_structural_scalar_return_fixture(&mut checked);
        let plan = &mut checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .machines[0];
        plan.structural_parameters[1].position = 2;
        plan.scalar_parameters = vec![
            psi_checked_trees::CheckedStructuralScalarParameterPlan {
                source_position: 1,
                primitive_type: PrimitiveType::I32,
            },
            psi_checked_trees::CheckedStructuralScalarParameterPlan {
                source_position: 3,
                primitive_type: PrimitiveType::Bool,
            },
        ];
        plan.result_type = PrimitiveType::Bool;
        plan.cleanup_actions = vec![
            CheckedStructuralScalarReturnCleanupAction::DiscardRoot(2),
            CheckedStructuralScalarReturnCleanupAction::DiscardRoot(0),
        ];
        checked.facts.values.scalar_expressions.expressions[0].expression =
            CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::Not(Box::new(
                CheckedBooleanExpression::Parameter { position: 1 },
            ))));

        let lowered = lower_machine(&checked, "example::Root::enter")
            .expect("exact mixed parameter map should lower before affine cleanup");
        let machine = &lowered.semantic_module.machines[0];
        assert!(matches!(
            machine.parameters.as_slice(),
            [
                ValueDeclaration {
                    id,
                    scalar_type: ScalarType::Integer(_),
                },
                ValueDeclaration {
                    id: bool_id,
                    scalar_type: ScalarType::Boolean,
                }
            ] if *id == value_id(1) && *bool_id == value_id(2)
        ));
        assert_eq!(machine.structural_parameters.len(), 2);
        assert!(matches!(
            machine.blocks[0].operations.as_slice(),
            [Operation {
                result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
                    id,
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::BooleanNot { operand },
                ..
            }] if *id == value_id(3) && *operand == value_id(2)
        ));
        assert!(matches!(
            &machine.blocks[0].terminator,
            Terminator::Return {
                value,
                cleanup_actions,
                ..
            } if *value == value_id(3)
                && cleanup_actions == &[
                    TerminalAffineCleanupAction::DiscardRoot(place_id(2)),
                    TerminalAffineCleanupAction::DiscardRoot(place_id(1)),
                ]
        ));
        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &psi_proof_kernel::AdmissionProfile::default(),
        )
        .expect("mixed scalar/structural parameter module should verify");

        checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .machines[0]
            .scalar_parameters[0]
            .source_position = 0;
        assert!(matches!(
            lower_machine(&checked, "example::Root::enter"),
            Err(LoweringError::Unsupported(
                "structural scalar return parameter maps overlap or repeat a source position"
            ))
        ));
    }

    #[test]
    fn structural_scalar_return_emits_boolean_paths_before_cleanup() {
        let mut checked = hard_root_checked_fixture();
        install_structural_scalar_return_fixture(&mut checked);
        checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .machines[0]
            .result_type = PrimitiveType::Bool;
        checked.facts.values.scalar_expressions.expressions[0].expression =
            CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::Not(Box::new(
                CheckedBooleanExpression::Equal {
                    left: Box::new(CheckedBooleanExpression::Constant(true)),
                    right: Box::new(CheckedBooleanExpression::Constant(false)),
                },
            ))));

        let lowered = lower_machine(&checked, "example::Root::enter")
            .expect("closed branch-free Boolean should lower before structural cleanup");
        let machine = &lowered.semantic_module.machines[0];
        assert!(matches!(
            machine.result,
            TerminalMachineResult::Scalar(ValueDeclaration {
                scalar_type: ScalarType::Boolean,
                ..
            })
        ));
        assert!(matches!(
            machine.blocks[0].operations.as_slice(),
            [
                Operation {
                    kind: OperationKind::BooleanConstant { value: true },
                    ..
                },
                Operation {
                    kind: OperationKind::BooleanConstant { value: false },
                    ..
                },
                Operation {
                    kind: OperationKind::BooleanEqual { .. },
                    ..
                },
                Operation {
                    kind: OperationKind::BooleanNot { .. },
                    ..
                }
            ]
        ));
        assert!(matches!(
            &machine.blocks[0].terminator,
            Terminator::Return {
                cleanup_actions,
                ..
            } if cleanup_actions == &[
                TerminalAffineCleanupAction::DiscardRoot(place_id(2)),
                TerminalAffineCleanupAction::DiscardRoot(place_id(1)),
            ]
        ));
        assert!(lowered.proof_bundle.evidence.is_empty());
        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &psi_proof_kernel::AdmissionProfile::default(),
        )
        .expect("closed Boolean return and cleanup should verify");
        let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
            .expect("closed Boolean cleanup module should encode canonically");
        assert_eq!(
            psi_terminal_codec::decode_module(&bytes)
                .expect("closed Boolean cleanup module should decode canonically"),
            lowered.semantic_module
        );

        let plan = &mut checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .machines[0];
        plan.bindings = vec![psi_checked_trees::CheckedScalarBinding {
            statement_ordinal: 0,
            primitive_type: PrimitiveType::Bool,
            value: CheckedScalarBindingValue::Expression,
        }];
        plan.return_statement_ordinal = 1;
        checked.facts.values.scalar_expressions.expressions = vec![
            psi_checked_trees::CheckedLocatedScalarExpression {
                state: plan.state,
                statement_ordinal: 0,
                role: CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
                expression: CheckedScalarExpression::Boolean(Box::new(
                    CheckedBooleanExpression::Constant(true),
                )),
            },
            psi_checked_trees::CheckedLocatedScalarExpression {
                state: plan.state,
                statement_ordinal: 1,
                role: CheckedScalarExpressionRole::Return,
                expression: CheckedScalarExpression::Boolean(Box::new(
                    CheckedBooleanExpression::And {
                        left: Box::new(CheckedBooleanExpression::Local { position: 0 }),
                        right: Box::new(CheckedBooleanExpression::Constant(false)),
                    },
                )),
            },
        ];
        let lowered = lower_machine(&checked, "example::Root::enter")
            .expect("short-circuit Boolean leaves should each perform exact affine cleanup");
        let blocks = &lowered.semantic_module.machines[0].blocks;
        assert_eq!(blocks.len(), 5);
        assert!(matches!(
            blocks[0].operations.first(),
            Some(Operation {
                kind: OperationKind::BooleanConstant { value: true },
                ..
            })
        ));
        let mut return_count = 0;
        for block in blocks {
            match &block.terminator {
                Terminator::Return {
                    cleanup_actions, ..
                } => {
                    return_count += 1;
                    assert_eq!(
                        cleanup_actions,
                        &[
                            TerminalAffineCleanupAction::DiscardRoot(place_id(2)),
                            TerminalAffineCleanupAction::DiscardRoot(place_id(1)),
                        ]
                    );
                }
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    assert!(when_true.trivial_affine_discards.is_empty());
                    assert!(when_false.trivial_affine_discards.is_empty());
                }
                _ => panic!("short-circuit return emits only decisions and scalar leaves"),
            }
        }
        assert_eq!(return_count, 3);
        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &psi_proof_kernel::AdmissionProfile::default(),
        )
        .expect("short-circuit cleanup frontiers should verify on every path");
        let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
            .expect("short-circuit structural cleanup should encode canonically");
        assert_eq!(
            psi_terminal_codec::decode_module(&bytes)
                .expect("short-circuit structural cleanup should decode canonically"),
            lowered.semantic_module
        );
    }
}
