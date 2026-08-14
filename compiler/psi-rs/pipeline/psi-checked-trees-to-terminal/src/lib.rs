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
    CheckedStructuralReturnMachinePlan, CheckedStructuralScalarReturnCleanupAction,
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
    lowered.proof_bundle.evidence_producers =
        lower_evidence_producer_provenance(checked, selection.machine, &evidence_terms.term_ids)?;
    lowered.semantic_module.proposition_declarations = declarations;
    lowered.semantic_module.proposition_applications = applications;
    lowered.semantic_module.evidence_terms = evidence_terms.declarations;
    lowered.semantic_module.evidence_contract_lanes = evidence_contract_lanes;
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
        term_ids[index] = root_ids.get(&root).copied();
    }
    Ok(LoweredEvidenceTermIds { term_ids })
}

fn lower_evidence_terms(
    checked: &CheckedTrees,
    selected_machine: psi_symbols::SymbolHandle,
    declaration_ids: &[(psi_symbols::SymbolHandle, PropositionId)],
    applications: &[PropositionApplicationIdentity],
    term_ids: Vec<Option<EvidenceTermId>>,
) -> Result<LoweredEvidenceTerms, LoweringError> {
    let mut identities_by_id =
        BTreeMap::<EvidenceTermId, (PropositionId, EvidenceInterfaceIdentity)>::new();
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

fn lower_evidence_producer_provenance(
    checked: &CheckedTrees,
    selected_machine: psi_symbols::SymbolHandle,
    term_ids: &[Option<EvidenceTermId>],
) -> Result<Vec<EvidenceProducerProvenance>, LoweringError> {
    let mut producers =
        checked
            .facts
            .proof
            .evidence_forwardings
            .iter()
            .filter_map(|(_, forwarding)| {
                if forwarding.machine_symbol != selected_machine {
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

fn lower_structural_scalar_return_machine(
    checked: &CheckedTrees,
    plan: &CheckedStructuralScalarReturnMachinePlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    if plan.cleanup_actions.iter().any(|action| {
        matches!(
            action,
            CheckedStructuralScalarReturnCleanupAction::InvokeNominal(_)
        )
    }) {
        return lower_nominal_structural_scalar_return_machine(checked, plan);
    }
    let (structural_types, type_ids) = lower_structural_type_plans(
        &checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .structural_types,
    )?;
    if plan.structural_parameters.is_empty() {
        return unsupported("structural scalar return has no structural parameters");
    }
    let mut positions = BTreeSet::new();
    for parameter in &plan.structural_parameters {
        if parameter.is_self
            || parameter.multiplicity != Multiplicity::Affine
            || !parameter.qualifications.is_empty()
            || !positions.insert(parameter.position)
        {
            return unsupported(
                "structural scalar return signature is not claim-free affine custody",
            );
        }
        lookup_type_id(&type_ids, &parameter.type_identity)?;
    }
    for parameter in &plan.scalar_parameters {
        if !positions.insert(parameter.source_position) {
            return unsupported(
                "structural scalar return parameter maps overlap or repeat a source position",
            );
        }
        terminal_scalar_type(parameter.primitive_type)?;
    }
    let parameter_count = plan
        .structural_parameters
        .len()
        .checked_add(plan.scalar_parameters.len())
        .ok_or(LoweringError::Unsupported(
            "structural scalar return parameter count exceeds usize",
        ))?;
    if positions.len() != parameter_count
        || positions
            .iter()
            .copied()
            .enumerate()
            .any(|(index, position)| u32::try_from(index).ok() != Some(position))
    {
        return unsupported(
            "structural scalar return parameter maps do not partition source positions",
        );
    }
    let expected_cleanup = plan
        .structural_parameters
        .iter()
        .rev()
        .map(|parameter| {
            CheckedStructuralScalarReturnCleanupAction::DiscardRoot(parameter.position)
        })
        .collect::<Vec<_>>();
    if plan.cleanup_actions != expected_cleanup {
        return unsupported("structural scalar return cleanup does not consume its exact frontier");
    }
    let expected_return_ordinal = u32::try_from(plan.bindings.len()).map_err(|_| {
        LoweringError::Unsupported("structural scalar return binding count exceeds u32")
    })?;
    if plan.return_statement_ordinal != expected_return_ordinal {
        return unsupported("structural scalar return coordinates are not a contiguous prefix");
    }
    let result_type = terminal_scalar_type(plan.result_type)?;

    let mut next_place = 1_u64;
    let structural_parameters =
        lower_unit_parameters(&plan.structural_parameters, &type_ids, &[], &mut next_place)?;
    let cleanup = plan
        .cleanup_actions
        .iter()
        .map(|action| {
            let CheckedStructuralScalarReturnCleanupAction::DiscardRoot(position) = action else {
                return Err(LoweringError::Unsupported(
                    "structural scalar return trivial lane acquired a nominal cleanup",
                ));
            };
            let parameter_index = plan
                .structural_parameters
                .iter()
                .position(|parameter| parameter.position == *position)
                .ok_or(LoweringError::Unsupported(
                    "structural scalar return cleanup position is absent from its signature",
                ))?;
            structural_parameters
                .get(parameter_index)
                .map(|parameter| parameter.place)
                .ok_or(LoweringError::Unsupported(
                    "structural scalar return cleanup position has no terminal place",
                ))
        })
        .map(|place| place.map(TerminalAffineCleanupAction::DiscardRoot))
        .collect::<Result<Vec<_>, _>>()?;
    let mut operations = OperationBuffer::new(0);
    let mut next_value = 1_u64;
    let scalar_parameters = plan
        .scalar_parameters
        .iter()
        .map(|parameter| {
            let value = ValueDeclaration {
                id: value_id(allocate_dense(&mut next_value)?),
                scalar_type: terminal_scalar_type(parameter.primitive_type)?,
            };
            Ok(value)
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let scalar_parameter_count = scalar_parameters.len();
    let mut scalar_values = Vec::with_capacity(
        scalar_parameter_count
            .checked_add(plan.bindings.len())
            .ok_or(LoweringError::Unsupported(
                "structural scalar value namespace exceeds usize",
            ))?,
    );
    scalar_values.extend_from_slice(&scalar_parameters);
    let mut staged_short_circuit_bindings = Vec::new();
    for (binding_index, binding) in plan.bindings.iter().enumerate() {
        let statement_ordinal = u32::try_from(binding_index).map_err(|_| {
            LoweringError::Unsupported("structural scalar return binding index exceeds u32")
        })?;
        if binding.statement_ordinal != statement_ordinal
            || binding.value != CheckedScalarBindingValue::Expression
        {
            return unsupported(
                "structural scalar return bindings are not a direct expression prefix",
            );
        }
        let expression = lower_checked_scalar_expression_at(
            checked,
            plan.state,
            statement_ordinal,
            CheckedScalarExpressionRole::LocalInitializer {
                binding_ordinal: statement_ordinal,
            },
        )?;
        if let LoweredDirectExpression::Boolean { expression } = expression
            && contains_short_circuit(&expression)
        {
            if binding.primitive_type != PrimitiveType::Bool {
                return unsupported(
                    "structural scalar short-circuit binding has a non-Boolean carrier",
                );
            }
            staged_short_circuit_bindings.push((binding_index, *expression));
        }
    }
    for (binding_index, binding) in plan
        .bindings
        .iter()
        .enumerate()
        .filter(|(binding_index, _)| {
            staged_short_circuit_bindings
                .first()
                .is_none_or(|(staged_index, _)| binding_index < staged_index)
        })
    {
        let statement_ordinal = u32::try_from(binding_index).map_err(|_| {
            LoweringError::Unsupported("structural scalar return binding index exceeds u32")
        })?;
        if binding.statement_ordinal != statement_ordinal
            || binding.value != CheckedScalarBindingValue::Expression
        {
            return unsupported(
                "structural scalar return bindings are not a direct expression prefix",
            );
        }
        let scalar_type = terminal_scalar_type(binding.primitive_type)?;
        let expression = lower_checked_scalar_expression_at(
            checked,
            plan.state,
            statement_ordinal,
            CheckedScalarExpressionRole::LocalInitializer {
                binding_ordinal: statement_ordinal,
            },
        )?;
        if !is_branch_free_structural_scalar_expression(
            &expression,
            scalar_parameter_count,
            binding_index,
        ) {
            return unsupported(
                "structural scalar binding is not one branch-free local expression",
            );
        }
        if expression.scalar_type() != scalar_type {
            return unsupported(
                "structural scalar binding value does not match its checked local type",
            );
        }
        validate_direct_parameter_types(
            &expression,
            &scalar_values
                .iter()
                .map(|value: &ValueDeclaration| value.scalar_type)
                .collect::<Vec<_>>(),
        )?;
        let id = emit_direct_expression(
            &expression,
            &scalar_values,
            &mut next_value,
            &mut operations,
        );
        scalar_values.push(ValueDeclaration { id, scalar_type });
    }
    let expression = lower_checked_scalar_expression_at(
        checked,
        plan.state,
        plan.return_statement_ordinal,
        CheckedScalarExpressionRole::Return,
    )?;
    if !is_structural_scalar_return_expression(
        &expression,
        scalar_parameter_count,
        plan.bindings.len(),
    ) {
        return unsupported("structural scalar return is outside its checked value/control slice");
    }
    if expression.scalar_type() != result_type {
        return unsupported(
            "structural scalar return value does not match its checked result type",
        );
    }
    let blocks = if !staged_short_circuit_bindings.is_empty() {
        let mut next_edge = 1_u64;
        let mut next_block = block_id(1);
        let mut next_block_parameters = Vec::new();
        let mut operation_start = 0;
        let mut blocks = Vec::new();
        for (stage_position, (staged_index, short_circuit_binding)) in
            staged_short_circuit_bindings.iter().enumerate()
        {
            validate_boolean_parameter_types(
                short_circuit_binding,
                &scalar_values
                    .iter()
                    .map(|value| value.scalar_type)
                    .collect::<Vec<_>>(),
            )?;
            let decision = lower_boolean_value_decision(short_circuit_binding);
            let decision_block_count = boolean_decision_block_count(&decision);
            let continuation = block_id(
                next_block
                    .get()
                    .checked_add(u64::try_from(decision_block_count).map_err(|_| {
                        LoweringError::Unsupported(
                            "structural scalar local decision block count exceeds u64",
                        )
                    })?)
                    .ok_or(LoweringError::Unsupported(
                        "structural scalar local continuation identity overflows",
                    ))?,
            );
            let decision_operation_start = operations.operations.len();
            let first_synthetic_block = block_id(next_block.get().checked_add(1).ok_or(
                LoweringError::Unsupported(
                    "structural scalar local decision block identity overflows",
                ),
            )?);
            let (mut root, mut children) = emit_inlined_boolean_value_blocks(
                &decision,
                &scalar_values,
                next_block_parameters,
                LoweredBooleanDecisionExit::Jump {
                    target: continuation,
                },
                next_block,
                first_synthetic_block,
                &mut next_value,
                &mut next_edge,
                &mut operations,
            );
            let mut root_operations =
                operations.operations[operation_start..decision_operation_start].to_vec();
            root_operations.extend(root.operations);
            root.operations = root_operations;
            blocks.push(root);
            blocks.append(&mut children);

            let local = ValueDeclaration {
                id: value_id(allocate_dense(&mut next_value)?),
                scalar_type: ScalarType::Boolean,
            };
            scalar_values.push(local);
            next_block = continuation;
            next_block_parameters = vec![local];
            operation_start = operations.operations.len();

            let next_staged_index = staged_short_circuit_bindings
                .get(stage_position + 1)
                .map_or(plan.bindings.len(), |(binding_index, _)| *binding_index);
            for binding_index in staged_index + 1..next_staged_index {
                let binding = &plan.bindings[binding_index];
                let statement_ordinal = u32::try_from(binding_index).map_err(|_| {
                    LoweringError::Unsupported("structural scalar return binding index exceeds u32")
                })?;
                let scalar_type = terminal_scalar_type(binding.primitive_type)?;
                let continuation_expression = lower_checked_scalar_expression_at(
                    checked,
                    plan.state,
                    statement_ordinal,
                    CheckedScalarExpressionRole::LocalInitializer {
                        binding_ordinal: statement_ordinal,
                    },
                )?;
                if !is_branch_free_structural_scalar_expression(
                    &continuation_expression,
                    scalar_parameter_count,
                    binding_index,
                ) {
                    return unsupported(
                        "structural scalar continuation binding is not branch-free",
                    );
                }
                if continuation_expression.scalar_type() != scalar_type {
                    return unsupported(
                        "structural scalar continuation binding does not match its checked local type",
                    );
                }
                validate_direct_parameter_types(
                    &continuation_expression,
                    &scalar_values
                        .iter()
                        .map(|value| value.scalar_type)
                        .collect::<Vec<_>>(),
                )?;
                let id = emit_direct_expression(
                    &continuation_expression,
                    &scalar_values,
                    &mut next_value,
                    &mut operations,
                );
                scalar_values.push(ValueDeclaration { id, scalar_type });
            }
        }
        if let LoweredDirectExpression::Boolean { expression } = &expression
            && contains_short_circuit(expression)
        {
            validate_boolean_parameter_types(
                expression,
                &scalar_values
                    .iter()
                    .map(|value| value.scalar_type)
                    .collect::<Vec<_>>(),
            )?;
            let decision_operation_start = operations.operations.len();
            let decision = lower_boolean_value_decision(expression);
            let first_synthetic_block = block_id(next_block.get().checked_add(1).ok_or(
                LoweringError::Unsupported(
                    "structural scalar return decision block identity overflows",
                ),
            )?);
            let (mut root, mut children) = emit_inlined_boolean_value_blocks(
                &decision,
                &scalar_values,
                next_block_parameters,
                LoweredBooleanDecisionExit::Return,
                next_block,
                first_synthetic_block,
                &mut next_value,
                &mut next_edge,
                &mut operations,
            );
            let mut root_operations =
                operations.operations[operation_start..decision_operation_start].to_vec();
            root_operations.extend(root.operations);
            root.operations = root_operations;
            let final_decision_start = blocks.len();
            blocks.push(root);
            blocks.append(&mut children);
            for block in &mut blocks[final_decision_start..] {
                if let Terminator::Return {
                    cleanup_actions, ..
                } = &mut block.terminator
                {
                    *cleanup_actions = cleanup.clone();
                }
            }
        } else {
            validate_direct_parameter_types(
                &expression,
                &scalar_values
                    .iter()
                    .map(|value| value.scalar_type)
                    .collect::<Vec<_>>(),
            )?;
            let value = emit_direct_expression(
                &expression,
                &scalar_values,
                &mut next_value,
                &mut operations,
            );
            blocks.push(Block {
                id: next_block,
                parameters: next_block_parameters,
                operations: operations.operations[operation_start..].to_vec(),
                terminator: Terminator::Return {
                    edge: edge_id(next_edge),
                    value,
                    cleanup_actions: cleanup,
                },
            });
        }
        blocks
    } else if let LoweredDirectExpression::Boolean { expression } = &expression
        && contains_short_circuit(expression)
    {
        validate_boolean_parameter_types(
            expression,
            &scalar_values
                .iter()
                .map(|value| value.scalar_type)
                .collect::<Vec<_>>(),
        )?;
        let entry_operation_count = operations.operations.len();
        let decision = lower_boolean_value_decision(expression);
        let mut next_edge = 1_u64;
        let (mut root, mut children) = emit_inlined_boolean_value_blocks(
            &decision,
            &scalar_values,
            Vec::new(),
            LoweredBooleanDecisionExit::Return,
            block_id(1),
            block_id(2),
            &mut next_value,
            &mut next_edge,
            &mut operations,
        );
        let mut entry_operations = operations.operations[..entry_operation_count].to_vec();
        entry_operations.extend(root.operations);
        root.operations = entry_operations;
        let mut blocks = Vec::with_capacity(1_usize.checked_add(children.len()).ok_or(
            LoweringError::Unsupported("structural scalar return block count exceeds usize"),
        )?);
        blocks.push(root);
        blocks.append(&mut children);
        for block in &mut blocks {
            if let Terminator::Return {
                cleanup_actions, ..
            } = &mut block.terminator
            {
                *cleanup_actions = cleanup.clone();
            }
        }
        blocks
    } else {
        validate_direct_parameter_types(
            &expression,
            &scalar_values
                .iter()
                .map(|value| value.scalar_type)
                .collect::<Vec<_>>(),
        )?;
        let value = emit_direct_expression(
            &expression,
            &scalar_values,
            &mut next_value,
            &mut operations,
        );
        vec![Block {
            id: block_id(1),
            parameters: Vec::new(),
            operations: operations.operations,
            terminator: Terminator::Return {
                edge: edge_id(1),
                value,
                cleanup_actions: cleanup,
            },
        }]
    };
    let result = ValueDeclaration {
        id: value_id(next_value),
        scalar_type: result_type,
    };
    let machine = TerminalMachine {
        id: machine_id(1),
        attachment: Some(lookup_type_id(&type_ids, &plan.attachment_type_identity)?),
        parameters: scalar_parameters,
        structural_parameters: structural_parameters.clone(),
        result: TerminalMachineResult::Scalar(result),
        structural_places: structural_parameters
            .iter()
            .map(|parameter| StructuralPlaceDeclaration {
                id: parameter.place,
                kind: StructuralPlaceKind::Parameter {
                    position: parameter.position,
                    is_self: parameter.is_self,
                },
            })
            .collect(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(1),
        blocks,
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
            structural_domains: Vec::new(),
            services: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            machines: vec![machine],
        },
        proof_bundle: ProofBundle::default(),
        debug_map: None,
    };
    finalize_operation_proofs(&mut lowered)?;
    Ok(lowered)
}

/// Reuse the already ratified bounded nominal-Unit closure construction, then
/// replace only its synthetic entry body with the checked scalar computation.
/// This keeps cleanup target/helper retention, dense identities, and ownership
/// proof validation in one implementation while making result materialization
/// precede the cleanup action on every scalar return leaf.
fn lower_nominal_structural_scalar_return_machine(
    checked: &CheckedTrees,
    plan: &CheckedStructuralScalarReturnMachinePlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    if plan.structural_parameters.is_empty()
        || plan.cleanup_actions.len() != plan.structural_parameters.len()
    {
        return unsupported("nominal scalar return exceeds its first bounded slice");
    }
    let expected_return_ordinal = u32::try_from(plan.bindings.len()).map_err(|_| {
        LoweringError::Unsupported("nominal scalar return binding count exceeds u32")
    })?;
    if plan.return_statement_ordinal != expected_return_ordinal {
        return unsupported("nominal scalar return coordinates are not a contiguous prefix");
    }
    let mut positions = BTreeSet::new();
    for parameter in &plan.structural_parameters {
        if parameter.is_self
            || parameter.multiplicity != Multiplicity::Affine
            || !parameter.qualifications.is_empty()
            || !positions.insert(parameter.position)
        {
            return unsupported("nominal scalar return cleanup frontier drifted");
        }
    }
    if plan
        .structural_parameters
        .windows(2)
        .any(|pair| pair[0].position >= pair[1].position)
    {
        return unsupported("nominal scalar return structural parameters are not in source order");
    }
    for parameter in &plan.scalar_parameters {
        if !positions.insert(parameter.source_position) {
            return unsupported(
                "nominal scalar return parameter maps overlap or repeat a source position",
            );
        }
        terminal_scalar_type(parameter.primitive_type)?;
    }
    if plan
        .scalar_parameters
        .windows(2)
        .any(|pair| pair[0].source_position >= pair[1].source_position)
    {
        return unsupported("nominal scalar return scalar parameters are not in source order");
    }
    let parameter_count = plan
        .structural_parameters
        .len()
        .checked_add(plan.scalar_parameters.len())
        .ok_or(LoweringError::Unsupported(
            "nominal scalar return parameter count exceeds usize",
        ))?;
    if positions.len() != parameter_count
        || positions
            .iter()
            .copied()
            .enumerate()
            .any(|(index, position)| u32::try_from(index).ok() != Some(position))
    {
        return unsupported(
            "nominal scalar return parameter maps do not partition source positions",
        );
    }
    for (parameter, cleanup) in plan
        .structural_parameters
        .iter()
        .zip(plan.cleanup_actions.iter().rev())
    {
        match cleanup {
            CheckedStructuralScalarReturnCleanupAction::DiscardRoot(cleanup_position)
                if *cleanup_position == parameter.position => {}
            CheckedStructuralScalarReturnCleanupAction::InvokeNominal(cleanup)
                if cleanup.source_parameter_index == parameter.position
                    && cleanup.type_identity == parameter.type_identity => {}
            _ => return unsupported("nominal scalar return cleanup frontier drifted"),
        }
    }
    let mut nominal_parameters = Vec::new();
    let mut nominal_source_positions = Vec::new();
    for parameter in &plan.structural_parameters {
        if plan.cleanup_actions.iter().any(|action| {
            matches!(
                action,
                CheckedStructuralScalarReturnCleanupAction::InvokeNominal(cleanup)
                    if cleanup.source_parameter_index == parameter.position
            )
        }) {
            let mut normalized = parameter.clone();
            normalized.position = u32::try_from(nominal_parameters.len()).map_err(|_| {
                LoweringError::Unsupported("nominal scalar return root count exceeds u32")
            })?;
            nominal_source_positions.push(parameter.position);
            nominal_parameters.push(normalized);
        }
    }
    let mut nominal_cleanups = plan
        .cleanup_actions
        .iter()
        .filter_map(|action| match action {
            CheckedStructuralScalarReturnCleanupAction::InvokeNominal(cleanup) => {
                Some(cleanup.clone())
            }
            CheckedStructuralScalarReturnCleanupAction::DiscardRoot(_) => None,
        })
        .collect::<Vec<_>>();
    for cleanup in &mut nominal_cleanups {
        cleanup.source_parameter_index = u32::try_from(
            nominal_source_positions
                .iter()
                .position(|position| *position == cleanup.source_parameter_index)
                .ok_or(LoweringError::Unsupported(
                    "nominal scalar return cleanup root is absent",
                ))?,
        )
        .map_err(|_| LoweringError::Unsupported("nominal scalar return root count exceeds u32"))?;
    }
    let nominal_caller_requirements = plan
        .caller_requirements
        .iter()
        .filter_map(|requirement| {
            let compact_position = nominal_source_positions
                .iter()
                .position(|position| *position == requirement.source_parameter_index)?;
            let mut normalized = requirement.clone();
            Some(
                u32::try_from(compact_position)
                    .map(|position| {
                        normalized.source_parameter_index = position;
                        normalized
                    })
                    .map_err(|_| {
                        LoweringError::Unsupported("nominal scalar return root count exceeds u32")
                    }),
            )
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let contract = checked
        .facts
        .contract_plans
        .for_machine(plan.machine)
        .ok_or(LoweringError::Unsupported(
            "nominal scalar return is missing its checked contract identity",
        ))?;
    let flow = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == plan.machine && state.state_symbol == plan.state)
                .then_some(state)
        })
        .ok_or(LoweringError::Unsupported(
            "nominal scalar return is missing its checked flow state",
        ))?;
    let synthetic = CheckedUnitEffectMachinePlan {
        machine: plan.machine,
        state: plan.state,
        attachment_type_identity: plan.attachment_type_identity.clone(),
        structural_parameters: nominal_parameters,
        trivial_affine_locals: Vec::new(),
        entry_claims: Vec::new(),
        body_qualifications: Vec::new(),
        contract_fingerprint: contract.fingerprint,
        contract_service_reach: contract.service_reach,
        service_reach: flow.service_reach,
        operations: vec![CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index: 0,
            trivial_affine_local_discard_ordinals: Vec::new(),
            trivial_affine_discards: Vec::new(),
        }],
    };
    let nominal = CheckedNominalAffineUnitCleanupMachinePlan {
        machine: synthetic,
        caller_requirements: nominal_caller_requirements,
        cleanups: nominal_cleanups,
    };
    let mut staged = checked.clone();
    for shape in &checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .structural_types
    {
        match staged
            .facts
            .flow
            .terminal_nominal_affine_unit_cleanups
            .structural_types
            .iter()
            .find(|candidate| candidate.identity == shape.identity)
        {
            Some(existing) if existing != shape => {
                return unsupported(
                    "nominal scalar return structural type conflicts with its cleanup closure",
                );
            }
            Some(_) => {}
            None => staged
                .facts
                .flow
                .terminal_nominal_affine_unit_cleanups
                .structural_types
                .push(shape.clone()),
        }
    }
    let mut lowered = lower_nominal_affine_unit_cleanup_machine(&staged, &nominal)?;
    let result_type = terminal_scalar_type(plan.result_type)?;
    retain_additional_structural_types(
        &mut lowered.semantic_module,
        &checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .structural_types,
        plan.structural_parameters
            .iter()
            .map(|parameter| parameter.type_identity.clone()),
    )?;
    let operation_identity_base = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .map(|operation| operation.id.get())
        .max()
        .unwrap_or(0)
        .max(
            lowered
                .proof_bundle
                .evidence
                .iter()
                .map(|evidence| evidence.obligation.get())
                .max()
                .unwrap_or(0),
        );
    let type_ids = lowered
        .semantic_module
        .structural_types
        .iter()
        .map(|declaration| (declaration.identity.clone(), declaration.id))
        .collect::<Vec<_>>();
    let mut next_place = 1_u64;
    let structural_parameters =
        lower_unit_parameters(&plan.structural_parameters, &type_ids, &[], &mut next_place)?;
    let structural_parameter_indexes = plan
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| (parameter.position, index))
        .collect::<BTreeMap<_, _>>();
    let entry_index = lowered
        .semantic_module
        .machines
        .iter()
        .position(|machine| machine.id == lowered.semantic_module.entry)
        .ok_or(LoweringError::Unsupported(
            "nominal scalar return entry machine was not retained",
        ))?;
    let compact_parameters = lowered.semantic_module.machines[entry_index]
        .structural_parameters
        .clone();
    let [compact_block] = lowered.semantic_module.machines[entry_index]
        .blocks
        .as_slice()
    else {
        return unsupported("nominal scalar return entry control is not a single block");
    };
    let Terminator::ReturnUnitNominalAffine { edge, cleanups } = &compact_block.terminator else {
        return unsupported("nominal scalar return synthetic cleanup edge drifted");
    };
    let edge = *edge;
    let mut terminal_nominals = cleanups.clone();
    if cleanups.len()
        != plan
            .cleanup_actions
            .iter()
            .filter(|action| {
                matches!(
                    action,
                    CheckedStructuralScalarReturnCleanupAction::InvokeNominal(_)
                )
            })
            .count()
    {
        return unsupported("nominal scalar return synthetic cleanup count drifted");
    }

    let mut caller_place_rebase = BTreeMap::new();
    for compact in &compact_parameters {
        let source_position = nominal_source_positions
            .get(usize::try_from(compact.position).map_err(|_| {
                LoweringError::Unsupported(
                    "nominal scalar return compact root position exceeds usize",
                )
            })?)
            .copied()
            .ok_or(LoweringError::Unsupported(
                "nominal scalar return compact root is absent",
            ))?;
        let full = structural_parameter_indexes
            .get(&source_position)
            .and_then(|index| structural_parameters.get(*index))
            .ok_or(LoweringError::Unsupported(
                "nominal scalar return full root is absent",
            ))?;
        if compact.structural_type != full.structural_type
            || caller_place_rebase
                .insert(compact.place, full.place)
                .is_some()
        {
            return unsupported("nominal scalar return compact root mapping drifted");
        }
    }
    if caller_place_rebase.len() != nominal_source_positions.len() {
        return unsupported("nominal scalar return compact root mapping is incomplete");
    }

    let mut next_proof_root = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.structural_places)
        .map(|place| place.id.get())
        .chain(
            structural_parameters
                .iter()
                .map(|parameter| parameter.place.get()),
        )
        .max()
        .unwrap_or(0);
    let mut receiver_place_rebase = BTreeMap::new();
    for cleanup in &terminal_nominals {
        let Some(receiver) = cleanup.cleanup_receiver else {
            continue;
        };
        if receiver_place_rebase.contains_key(&receiver) {
            continue;
        }
        next_proof_root = next_proof_root
            .checked_add(1)
            .ok_or(LoweringError::Unsupported(
                "contextual nominal scalar proof-root identity space is exhausted",
            ))?;
        receiver_place_rebase.insert(receiver, place_id(next_proof_root));
    }

    for requirement in &mut lowered.semantic_module.machines[entry_index]
        .contract
        .requires
    {
        rebase_direct_boolean_requirement_root(
            requirement,
            &caller_place_rebase,
            "contextual nominal scalar caller requirement root drifted",
        )?;
    }
    let mut full_caller_clauses = plan
        .caller_requirements
        .iter()
        .map(|requirement| {
            let parameter = structural_parameter_indexes
                .get(&requirement.source_parameter_index)
                .and_then(|index| structural_parameters.get(*index))
                .ok_or(LoweringError::Unsupported(
                    "contextual nominal scalar full caller root is absent",
                ))?;
            let structural_type = lowered
                .semantic_module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == parameter.structural_type)
                .ok_or(LoweringError::Unsupported(
                    "contextual nominal scalar full caller type is absent",
                ))?;
            let field = match &structural_type.shape {
                StructuralTypeShape::Record { fields } => fields
                    .iter()
                    .find(|field| field.identity == requirement.field_identity)
                    .filter(|field| {
                        !field.relevance.is_erased()
                            && field.field_type == StructuralFieldType::Scalar(ScalarType::Boolean)
                    })
                    .map(|field| field.id),
                StructuralTypeShape::FixedArray { .. } => None,
            }
            .ok_or(LoweringError::Unsupported(
                "contextual nominal scalar full caller field drifted",
            ))?;
            Ok((
                (requirement.expected, parameter.place, field),
                Proposition::Equal(
                    ScalarTerm::boolean(requirement.expected),
                    ScalarTerm::boolean_field(parameter.place, field),
                ),
            ))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    full_caller_clauses.sort_by_key(|((expected, root, field), _)| {
        (
            *expected,
            root.get().to_le_bytes(),
            field.get().to_le_bytes(),
        )
    });
    if full_caller_clauses
        .windows(2)
        .any(|pair| pair[0].0 == pair[1].0)
    {
        return unsupported("contextual nominal scalar full caller requirements are duplicated");
    }
    let full_caller_requires = full_caller_clauses
        .into_iter()
        .map(|(_, proposition)| proposition)
        .collect::<Vec<_>>();
    let compact_caller_requires = lowered.semantic_module.machines[entry_index]
        .contract
        .requires
        .clone();
    let assumption_rebase = compact_caller_requires
        .iter()
        .map(|requirement| {
            full_caller_requires
                .iter()
                .position(|full| full == requirement)
                .ok_or(LoweringError::Unsupported(
                    "contextual nominal scalar proof assumption is absent from the full caller",
                ))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    lowered.semantic_module.machines[entry_index]
        .contract
        .requires = full_caller_requires;
    let target_receivers = terminal_nominals
        .iter()
        .filter_map(|cleanup| {
            cleanup
                .cleanup_receiver
                .map(|receiver| (cleanup.cleanup_machine, receiver))
        })
        .collect::<BTreeMap<_, _>>();
    for (target, receiver) in target_receivers {
        if terminal_nominals.iter().any(|cleanup| {
            cleanup.cleanup_machine == target && cleanup.cleanup_receiver != Some(receiver)
        }) {
            return unsupported("shared contextual scalar cleanup receiver drifted");
        }
        let target = lowered
            .semantic_module
            .machines
            .iter_mut()
            .find(|machine| machine.id == target)
            .ok_or(LoweringError::Unsupported(
                "contextual nominal scalar cleanup target is absent",
            ))?;
        for requirement in &mut target.contract.requires {
            rebase_direct_boolean_requirement_root(
                requirement,
                &receiver_place_rebase,
                "contextual nominal scalar cleanup receiver drifted",
            )?;
        }
    }
    for evidence in &mut lowered.proof_bundle.evidence {
        let EvidenceRoute::CertificateDerived(certificate) = &mut evidence.route else {
            return unsupported("contextual nominal scalar cleanup evidence route drifted");
        };
        let ProofRule::Assumption { index } = &mut certificate.proof.rule else {
            return unsupported("contextual nominal scalar cleanup proof rule drifted");
        };
        *index = *assumption_rebase
            .get(*index)
            .ok_or(LoweringError::Unsupported(
                "contextual nominal scalar cleanup proof assumption index drifted",
            ))?;
        rebase_direct_boolean_requirement_root(
            &mut certificate.proof.conclusion,
            &caller_place_rebase,
            "contextual nominal scalar cleanup proof conclusion drifted",
        )?;
    }
    for cleanup in &mut terminal_nominals {
        if let Some(receiver) = cleanup.cleanup_receiver {
            cleanup.cleanup_receiver = Some(*receiver_place_rebase.get(&receiver).ok_or(
                LoweringError::Unsupported(
                    "contextual nominal scalar cleanup receiver mapping is absent",
                ),
            )?);
        }
    }

    let mut terminal_nominals = terminal_nominals.into_iter();
    let cleanup_actions = plan
        .cleanup_actions
        .iter()
        .map(|action| {
            let source_position = match action {
                CheckedStructuralScalarReturnCleanupAction::DiscardRoot(position) => *position,
                CheckedStructuralScalarReturnCleanupAction::InvokeNominal(cleanup) => {
                    cleanup.source_parameter_index
                }
            };
            let place = structural_parameter_indexes
                .get(&source_position)
                .and_then(|index| structural_parameters.get(*index))
                .map(|parameter| parameter.place)
                .ok_or(LoweringError::Unsupported(
                    "nominal scalar return cleanup terminal root is absent",
                ))?;
            match action {
                CheckedStructuralScalarReturnCleanupAction::DiscardRoot(_) => {
                    Ok(TerminalAffineCleanupAction::DiscardRoot(place))
                }
                CheckedStructuralScalarReturnCleanupAction::InvokeNominal(_) => {
                    let mut cleanup =
                        terminal_nominals.next().ok_or(LoweringError::Unsupported(
                            "nominal scalar return synthetic cleanup stream is short",
                        ))?;
                    cleanup.place = place;
                    Ok(TerminalAffineCleanupAction::InvokeNominal(cleanup))
                }
            }
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    if terminal_nominals.next().is_some() {
        return unsupported("nominal scalar return synthetic cleanup stream is long");
    }
    let mut next_value = 1_u64;
    let scalar_parameters = plan
        .scalar_parameters
        .iter()
        .map(|parameter| {
            Ok(ValueDeclaration {
                id: value_id(allocate_dense(&mut next_value)?),
                scalar_type: terminal_scalar_type(parameter.primitive_type)?,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let scalar_parameter_count = scalar_parameters.len();
    let mut operations = OperationBuffer::new(operation_identity_base);
    let mut scalar_values = Vec::with_capacity(
        scalar_parameter_count
            .checked_add(plan.bindings.len())
            .ok_or(LoweringError::Unsupported(
                "nominal scalar value namespace exceeds usize",
            ))?,
    );
    scalar_values.extend_from_slice(&scalar_parameters);
    let authored_return_expression = lower_checked_scalar_expression_at(
        checked,
        plan.state,
        plan.return_statement_ordinal,
        CheckedScalarExpressionRole::Return,
    )?;
    let candidate_source_distributed_short_circuit_bindings = plan
        .bindings
        .len()
        .checked_sub(1)
        .and_then(|binding_index| {
            let binding = &plan.bindings[binding_index];
            let statement_ordinal = u32::try_from(binding_index).ok()?;
            if binding.statement_ordinal != statement_ordinal
                || binding.value != CheckedScalarBindingValue::Expression
                || binding.primitive_type != PrimitiveType::Bool
            {
                return None;
            }
            if !(0..binding_index).all(|prior_index| {
                let Ok(prior_ordinal) = u32::try_from(prior_index) else {
                    return false;
                };
                lower_checked_scalar_expression_at(
                    checked,
                    plan.state,
                    prior_ordinal,
                    CheckedScalarExpressionRole::LocalInitializer {
                        binding_ordinal: prior_ordinal,
                    },
                )
                .is_ok_and(|expression| {
                    is_branch_free_structural_scalar_expression(
                        &expression,
                        scalar_parameter_count,
                        prior_index,
                    )
                })
            }) {
                return None;
            }
            let LoweredDirectExpression::Boolean {
                expression: return_expression,
            } = &authored_return_expression
            else {
                return None;
            };
            let local_position = scalar_parameter_count + binding_index;
            if !is_branch_free_structural_boolean_expression(
                return_expression,
                scalar_parameter_count,
                binding_index + 1,
            ) || boolean_local_reference_count(return_expression, local_position) == 0
            {
                return None;
            }
            let expression = lower_checked_scalar_expression_at(
                checked,
                plan.state,
                statement_ordinal,
                CheckedScalarExpressionRole::LocalInitializer {
                    binding_ordinal: statement_ordinal,
                },
            )
            .ok()?;
            let LoweredDirectExpression::Boolean { expression } = expression else {
                return None;
            };
            if !is_structural_short_circuit_boolean_decision(
                &expression,
                scalar_parameter_count,
                binding_index,
            ) {
                return None;
            }
            let decision = source_distribute_boolean_local(
                lower_boolean_value_decision(&expression),
                return_expression,
                local_position,
            );
            Some((binding_index, decision))
        })
        .or_else(|| {
            let final_binding_index = plan.bindings.len().checked_sub(1)?;
            let LoweredDirectExpression::Boolean {
                expression: return_expression,
            } = &authored_return_expression
            else {
                return None;
            };
            let final_binding_position = scalar_parameter_count + final_binding_index;
            if !matches!(return_expression.as_ref(),
                    LoweredBooleanReturnExpression::Local { position }
                        if *position == final_binding_position)
            {
                return None;
            }
            (0..final_binding_index).find_map(|short_circuit_index| {
                if !plan.bindings[short_circuit_index..].iter().enumerate().all(
                    |(offset, binding)| {
                        let index = short_circuit_index + offset;
                        u32::try_from(index)
                            .is_ok_and(|ordinal| binding.statement_ordinal == ordinal)
                            && binding.value == CheckedScalarBindingValue::Expression
                            && binding.primitive_type == PrimitiveType::Bool
                    },
                ) {
                    return None;
                }
                let short_circuit_ordinal = u32::try_from(short_circuit_index).ok()?;
                let short_circuit_expression = lower_checked_scalar_expression_at(
                    checked,
                    plan.state,
                    short_circuit_ordinal,
                    CheckedScalarExpressionRole::LocalInitializer {
                        binding_ordinal: short_circuit_ordinal,
                    },
                )
                .ok()?;
                let LoweredDirectExpression::Boolean {
                    expression: short_circuit_expression,
                } = short_circuit_expression
                else {
                    return None;
                };
                if !is_structural_short_circuit_boolean_decision(
                    &short_circuit_expression,
                    scalar_parameter_count,
                    short_circuit_index,
                ) {
                    return None;
                }
                let mut decision = lower_boolean_value_decision(&short_circuit_expression);
                for continuation_index in short_circuit_index + 1..=final_binding_index {
                    let continuation_ordinal = u32::try_from(continuation_index).ok()?;
                    let continuation_expression = lower_checked_scalar_expression_at(
                        checked,
                        plan.state,
                        continuation_ordinal,
                        CheckedScalarExpressionRole::LocalInitializer {
                            binding_ordinal: continuation_ordinal,
                        },
                    )
                    .ok()?;
                    let LoweredDirectExpression::Boolean {
                        expression: continuation_expression,
                    } = continuation_expression
                    else {
                        return None;
                    };
                    let prior_position = scalar_parameter_count + continuation_index - 1;
                    if !(is_branch_free_structural_boolean_expression(
                        &continuation_expression,
                        scalar_parameter_count,
                        continuation_index,
                    ) || is_structural_short_circuit_boolean_decision(
                        &continuation_expression,
                        scalar_parameter_count,
                        continuation_index,
                    )) || boolean_local_reference_count(&continuation_expression, prior_position)
                        == 0
                    {
                        return None;
                    }
                    decision = source_distribute_boolean_local(
                        decision,
                        &continuation_expression,
                        prior_position,
                    );
                }
                Some((short_circuit_index, decision))
            })
        });
    let source_distributed_short_circuit_bindings = plan
        .shared_boolean_convergence
        .is_none()
        .then_some(candidate_source_distributed_short_circuit_bindings)
        .flatten();
    for (binding_index, binding) in plan.bindings.iter().enumerate() {
        let statement_ordinal = u32::try_from(binding_index).map_err(|_| {
            LoweringError::Unsupported("nominal scalar return binding index exceeds u32")
        })?;
        if binding.statement_ordinal != statement_ordinal
            || binding.value != CheckedScalarBindingValue::Expression
        {
            return unsupported(
                "nominal scalar return bindings are not a direct expression prefix",
            );
        }
        if source_distributed_short_circuit_bindings
            .as_ref()
            .is_some_and(|(first_distributed, _)| binding_index >= *first_distributed)
            || plan.shared_boolean_convergence.is_some_and(|convergence| {
                usize::try_from(convergence.binding_ordinal).ok() == Some(binding_index)
            })
        {
            continue;
        }
        let scalar_type = terminal_scalar_type(binding.primitive_type)?;
        let expression = lower_checked_scalar_expression_at(
            checked,
            plan.state,
            statement_ordinal,
            CheckedScalarExpressionRole::LocalInitializer {
                binding_ordinal: statement_ordinal,
            },
        )?;
        if !is_branch_free_structural_scalar_expression(
            &expression,
            scalar_parameter_count,
            binding_index,
        ) {
            return unsupported("nominal scalar binding is not one branch-free local expression");
        }
        if expression.scalar_type() != scalar_type {
            return unsupported(
                "nominal scalar binding value does not match its checked local type",
            );
        }
        validate_direct_parameter_types(
            &expression,
            &scalar_values
                .iter()
                .map(|value: &ValueDeclaration| value.scalar_type)
                .collect::<Vec<_>>(),
        )?;
        let id = emit_direct_expression(
            &expression,
            &scalar_values,
            &mut next_value,
            &mut operations,
        );
        scalar_values.push(ValueDeclaration { id, scalar_type });
    }
    let expression = authored_return_expression;
    let expression_available_locals = source_distributed_short_circuit_bindings
        .as_ref()
        .map_or(plan.bindings.len(), |(first_distributed, _)| {
            *first_distributed
        });
    let authored_short_circuit_return = matches!(
        &expression,
        LoweredDirectExpression::Boolean { expression }
            if is_structural_short_circuit_boolean_decision(
                expression,
                scalar_parameter_count,
                expression_available_locals,
            )
    );
    let nominal_short_circuit_return =
        source_distributed_short_circuit_bindings.is_some() || authored_short_circuit_return;
    if !is_branch_free_structural_scalar_expression(
        &expression,
        scalar_parameter_count,
        plan.bindings.len(),
    ) && !nominal_short_circuit_return
    {
        return unsupported(
            "nominal scalar return expression is not branch-free or one top-level Boolean decision",
        );
    }
    if expression.scalar_type() != result_type {
        return unsupported("nominal scalar return value does not match its checked result type");
    }
    let first_unused_edge = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| block.terminator.edges())
        .map(|edge| edge.get())
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "nominal scalar return edge identity space is exhausted",
        ))?;
    let first_unused_block = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .map(|block| block.id.get())
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "nominal scalar return block identity space is exhausted",
        ))?;
    let entry = &mut lowered.semantic_module.machines[entry_index];
    let [synthetic_block] = entry.blocks.as_slice() else {
        return unsupported("nominal scalar return entry control is not a single block");
    };
    if synthetic_block.terminator.edge() != edge {
        return unsupported("nominal scalar return synthetic edge drifted");
    }
    let parameter_types = scalar_values
        .iter()
        .map(|value| value.scalar_type)
        .collect::<Vec<_>>();
    if let Some(convergence) = plan.shared_boolean_convergence {
        let binding_index = usize::try_from(convergence.binding_ordinal).map_err(|_| {
            LoweringError::Unsupported("shared Boolean convergence binding exceeds usize")
        })?;
        let decision = lower_checked_scalar_expression_at(
            checked,
            plan.state,
            convergence.binding_ordinal,
            CheckedScalarExpressionRole::LocalInitializer {
                binding_ordinal: convergence.binding_ordinal,
            },
        )?;
        let LoweredDirectExpression::Boolean {
            expression: decision,
        } = decision
        else {
            return unsupported("shared Boolean convergence decision is not Boolean");
        };
        let decision = resolve_shared_boolean_member_fields(
            *decision,
            &structural_parameters,
            &lowered.semantic_module.structural_types,
        )?;
        let decision = normalize_shared_boolean_comparison_leaves(&decision).ok_or(
            LoweringError::Unsupported(
                "shared Boolean convergence contains a non-normalizable comparison leaf",
            ),
        )?;
        if binding_index >= plan.bindings.len()
            || shared_boolean_runtime_parameters(&decision)
                .is_none_or(|inputs| !valid_shared_boolean_runtime_inputs(&inputs))
        {
            return unsupported("shared Boolean convergence has no normalized runtime input");
        }
        validate_boolean_parameter_types(&decision, &parameter_types)?;
    } else if let Some((_, decision)) = &source_distributed_short_circuit_bindings {
        validate_boolean_decision_parameter_types(decision, &parameter_types)?;
    } else {
        validate_direct_parameter_types(&expression, &parameter_types)?;
    }
    let blocks = if let Some(convergence) = plan.shared_boolean_convergence {
        usize::try_from(convergence.binding_ordinal).map_err(|_| {
            LoweringError::Unsupported("shared Boolean convergence binding exceeds usize")
        })?;
        let decision = lower_checked_scalar_expression_at(
            checked,
            plan.state,
            convergence.binding_ordinal,
            CheckedScalarExpressionRole::LocalInitializer {
                binding_ordinal: convergence.binding_ordinal,
            },
        )?;
        let LoweredDirectExpression::Boolean {
            expression: decision,
        } = decision
        else {
            return unsupported("shared Boolean convergence decision is not Boolean");
        };
        let decision = resolve_shared_boolean_member_fields(
            *decision,
            &structural_parameters,
            &lowered.semantic_module.structural_types,
        )?;
        let decision = normalize_shared_boolean_comparison_leaves(&decision).ok_or(
            LoweringError::Unsupported(
                "shared Boolean convergence contains a non-normalizable comparison leaf",
            ),
        )?;
        if shared_boolean_runtime_parameters(&decision)
            .is_none_or(|inputs| !valid_shared_boolean_runtime_inputs(&inputs))
        {
            return unsupported("shared Boolean convergence has no normalized runtime input");
        }
        let decision = lower_boolean_value_decision(&decision);
        let decision_block_count = boolean_decision_block_count(&decision);
        let continuation_block = block_id(
            first_unused_block
                .checked_add(
                    u64::try_from(decision_block_count.saturating_sub(1)).map_err(|_| {
                        LoweringError::Unsupported(
                            "shared Boolean convergence block count exceeds u64",
                        )
                    })?,
                )
                .ok_or(LoweringError::Unsupported(
                    "shared Boolean convergence block identity overflows",
                ))?,
        );
        let entry_operation_count = operations.operations.len();
        let mut next_edge = first_unused_edge;
        let (mut root, mut children) = emit_inlined_boolean_value_blocks(
            &decision,
            &scalar_values,
            Vec::new(),
            LoweredBooleanDecisionExit::Jump {
                target: continuation_block,
            },
            entry.entry,
            block_id(first_unused_block),
            &mut next_value,
            &mut next_edge,
            &mut operations,
        );
        let mut entry_operations = operations.operations[..entry_operation_count].to_vec();
        entry_operations.extend(root.operations);
        root.operations = entry_operations;
        let convergence_value = ValueDeclaration {
            id: value_id(allocate_dense(&mut next_value)?),
            scalar_type: ScalarType::Boolean,
        };
        scalar_values.push(convergence_value);
        validate_direct_parameter_types(
            &expression,
            &scalar_values
                .iter()
                .map(|value| value.scalar_type)
                .collect::<Vec<_>>(),
        )?;
        let continuation_operation_start = operations.operations.len();
        let value = emit_direct_expression(
            &expression,
            &scalar_values,
            &mut next_value,
            &mut operations,
        );
        let return_edge = edge_id(next_edge);
        let mut blocks = Vec::with_capacity(2_usize.checked_add(children.len()).ok_or(
            LoweringError::Unsupported("shared Boolean convergence block count exceeds usize"),
        )?);
        blocks.push(root);
        blocks.append(&mut children);
        blocks.push(Block {
            id: continuation_block,
            parameters: vec![convergence_value],
            operations: operations.operations[continuation_operation_start..].to_vec(),
            terminator: Terminator::Return {
                edge: return_edge,
                value,
                cleanup_actions: cleanup_actions.clone(),
            },
        });
        attach_edge_local_cleanup_proofs(
            &mut blocks,
            &cleanup_actions,
            operations.next_identity,
            &mut lowered.proof_bundle,
        )?;
        blocks
    } else if nominal_short_circuit_return {
        let entry_operation_count = operations.operations.len();
        let decision = if let Some((_, decision)) = source_distributed_short_circuit_bindings {
            decision
        } else {
            let LoweredDirectExpression::Boolean { expression } = &expression else {
                unreachable!("the bounded nominal decision is Boolean")
            };
            lower_boolean_value_decision(expression)
        };
        let mut next_edge = first_unused_edge;
        let (mut root, mut children) = emit_inlined_boolean_value_blocks(
            &decision,
            &scalar_values,
            Vec::new(),
            LoweredBooleanDecisionExit::Return,
            entry.entry,
            block_id(first_unused_block),
            &mut next_value,
            &mut next_edge,
            &mut operations,
        );
        let mut entry_operations = operations.operations[..entry_operation_count].to_vec();
        entry_operations.extend(root.operations);
        root.operations = entry_operations;
        let mut blocks = Vec::with_capacity(1_usize.checked_add(children.len()).ok_or(
            LoweringError::Unsupported("nominal scalar return block count exceeds usize"),
        )?);
        blocks.push(root);
        blocks.append(&mut children);
        attach_edge_local_cleanup_proofs(
            &mut blocks,
            &cleanup_actions,
            operations.next_identity,
            &mut lowered.proof_bundle,
        )?;
        blocks
    } else {
        let value = emit_direct_expression(
            &expression,
            &scalar_values,
            &mut next_value,
            &mut operations,
        );
        vec![Block {
            id: entry.entry,
            parameters: Vec::new(),
            operations: operations.operations,
            terminator: Terminator::Return {
                edge,
                value,
                cleanup_actions,
            },
        }]
    };
    entry.blocks = blocks;
    entry.parameters = scalar_parameters;
    entry.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(next_value),
        scalar_type: result_type,
    });
    entry.structural_parameters = structural_parameters.clone();
    entry.structural_places = structural_parameters
        .iter()
        .map(|parameter| StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::Parameter {
                position: parameter.position,
                is_self: parameter.is_self,
            },
        })
        .collect();
    finalize_operation_proofs(&mut lowered)?;
    Ok(lowered)
}

fn attach_edge_local_cleanup_proofs(
    blocks: &mut [Block],
    cleanup_actions: &[TerminalAffineCleanupAction],
    next_operation_identity: u64,
    proof_bundle: &mut ProofBundle,
) -> Result<(), LoweringError> {
    let mut first_return = true;
    // Cleanup obligations are edge-local semantic events. Keep the first
    // leaf's already-verified stream, then clone its proof for each later leaf
    // under fresh identities beyond every operation-derived goal.
    let mut next_cleanup_obligation =
        next_operation_identity
            .checked_add(1)
            .ok_or(LoweringError::Unsupported(
                "nominal scalar Boolean cleanup obligation identity space is exhausted",
            ))?;
    let original_evidence = proof_bundle
        .evidence
        .iter()
        .map(|evidence| (evidence.obligation, evidence.clone()))
        .collect::<BTreeMap<_, _>>();
    for block in blocks {
        let Terminator::Return {
            cleanup_actions: leaf_cleanup,
            ..
        } = &mut block.terminator
        else {
            continue;
        };
        *leaf_cleanup = cleanup_actions.to_vec();
        if first_return {
            first_return = false;
            continue;
        }
        for action in leaf_cleanup {
            let TerminalAffineCleanupAction::InvokeNominal(cleanup) = action else {
                continue;
            };
            for obligation in &mut cleanup.requirement_obligations {
                let mut evidence = original_evidence.get(obligation).cloned().ok_or(
                    LoweringError::Unsupported("nominal scalar Boolean cleanup evidence is absent"),
                )?;
                let identity = next_cleanup_obligation;
                next_cleanup_obligation =
                    next_cleanup_obligation
                        .checked_add(1)
                        .ok_or(LoweringError::Unsupported(
                            "nominal scalar Boolean cleanup obligation identity space is exhausted",
                        ))?;
                let leaf_obligation = obligation_id(identity);
                evidence.obligation = leaf_obligation;
                let EvidenceRoute::CertificateDerived(certificate) = &mut evidence.route else {
                    return unsupported("nominal scalar Boolean cleanup evidence route drifted");
                };
                certificate.identity =
                    EvidenceIdentity::new(identity).ok_or(LoweringError::Unsupported(
                        "nominal scalar Boolean cleanup evidence identity is invalid",
                    ))?;
                *obligation = leaf_obligation;
                proof_bundle.evidence.push(evidence);
            }
        }
    }
    Ok(())
}

fn rebase_direct_boolean_requirement_root(
    proposition: &mut Proposition,
    places: &BTreeMap<PlaceId, PlaceId>,
    error: &'static str,
) -> Result<(), LoweringError> {
    let Proposition::Equal(ScalarTerm::Boolean(_), ScalarTerm::BooleanField { root, .. }) =
        proposition
    else {
        return unsupported(error);
    };
    *root = *places.get(root).ok_or(LoweringError::Unsupported(error))?;
    Ok(())
}

fn is_structural_scalar_return_expression(
    expression: &LoweredDirectExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        LoweredDirectExpression::Boolean { expression } => {
            is_structural_boolean_return_expression(expression, scalar_parameters, available_locals)
        }
        expression => is_branch_free_structural_integer_expression(
            expression,
            scalar_parameters,
            available_locals,
        ),
    }
}

fn is_structural_boolean_return_expression(
    expression: &LoweredBooleanReturnExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        LoweredBooleanReturnExpression::Constant { .. } => true,
        LoweredBooleanReturnExpression::Not { operand } => {
            is_structural_boolean_return_expression(operand, scalar_parameters, available_locals)
        }
        LoweredBooleanReturnExpression::Equal { left, right }
        | LoweredBooleanReturnExpression::And { left, right }
        | LoweredBooleanReturnExpression::Or { left, right } => {
            is_structural_boolean_return_expression(left, scalar_parameters, available_locals)
                && is_structural_boolean_return_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        LoweredBooleanReturnExpression::IntegerComparison { left, right, .. } => {
            is_branch_free_structural_integer_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_integer_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        LoweredBooleanReturnExpression::Parameter { position } => *position < scalar_parameters,
        LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { path, .. } => {
            path.len() == 1
        }
        LoweredBooleanReturnExpression::StructuralField { .. } => true,
        LoweredBooleanReturnExpression::Local { position } => {
            *position >= scalar_parameters
                && *position < scalar_parameters.saturating_add(available_locals)
        }
    }
}

fn is_branch_free_structural_integer_expression(
    expression: &LoweredDirectExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        LoweredDirectExpression::IntegerLiteral { .. } => true,
        LoweredDirectExpression::IntegerBinary { left, right, .. } => {
            is_branch_free_structural_integer_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_integer_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        LoweredDirectExpression::IntegerBitwiseNot { operand, .. }
        | LoweredDirectExpression::IntegerWiden { operand, .. }
        | LoweredDirectExpression::IntegerExactCast { operand, .. } => {
            is_branch_free_structural_integer_expression(
                operand,
                scalar_parameters,
                available_locals,
            )
        }
        LoweredDirectExpression::Parameter { position, .. } => *position < scalar_parameters,
        LoweredDirectExpression::Local { position, .. } => {
            *position >= scalar_parameters
                && *position < scalar_parameters.saturating_add(available_locals)
        }
        LoweredDirectExpression::Boolean { .. } => false,
    }
}

fn is_branch_free_structural_scalar_expression(
    expression: &LoweredDirectExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        LoweredDirectExpression::Boolean { expression } => {
            is_branch_free_structural_boolean_expression(
                expression,
                scalar_parameters,
                available_locals,
            )
        }
        expression => is_branch_free_structural_integer_expression(
            expression,
            scalar_parameters,
            available_locals,
        ),
    }
}

fn is_branch_free_structural_boolean_expression(
    expression: &LoweredBooleanReturnExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        LoweredBooleanReturnExpression::Constant { .. } => true,
        LoweredBooleanReturnExpression::Not { operand } => {
            is_branch_free_structural_boolean_expression(
                operand,
                scalar_parameters,
                available_locals,
            )
        }
        LoweredBooleanReturnExpression::Equal { left, right } => {
            is_branch_free_structural_boolean_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_boolean_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        LoweredBooleanReturnExpression::IntegerComparison { left, right, .. } => {
            is_branch_free_structural_integer_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_integer_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        LoweredBooleanReturnExpression::Parameter { position } => *position < scalar_parameters,
        LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { path, .. } => {
            path.len() == 1
        }
        LoweredBooleanReturnExpression::StructuralField { .. } => true,
        LoweredBooleanReturnExpression::Local { position } => {
            *position >= scalar_parameters
                && *position < scalar_parameters.saturating_add(available_locals)
        }
        LoweredBooleanReturnExpression::And { .. } | LoweredBooleanReturnExpression::Or { .. } => {
            false
        }
    }
}

fn is_structural_short_circuit_boolean_decision(
    expression: &LoweredBooleanReturnExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    contains_short_circuit(expression)
        && is_structural_boolean_return_expression(expression, scalar_parameters, available_locals)
}

fn boolean_local_reference_count(
    expression: &LoweredBooleanReturnExpression,
    local: usize,
) -> usize {
    match expression {
        LoweredBooleanReturnExpression::Local { position } => usize::from(*position == local),
        LoweredBooleanReturnExpression::Not { operand } => {
            boolean_local_reference_count(operand, local)
        }
        LoweredBooleanReturnExpression::Equal { left, right }
        | LoweredBooleanReturnExpression::And { left, right }
        | LoweredBooleanReturnExpression::Or { left, right } => {
            boolean_local_reference_count(left, local)
                .saturating_add(boolean_local_reference_count(right, local))
        }
        LoweredBooleanReturnExpression::Constant { .. }
        | LoweredBooleanReturnExpression::Parameter { .. }
        | LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. }
        | LoweredBooleanReturnExpression::StructuralField { .. }
        | LoweredBooleanReturnExpression::IntegerComparison { .. } => 0,
    }
}

fn inline_boolean_local(
    expression: &LoweredBooleanReturnExpression,
    local: usize,
    replacement: &LoweredBooleanReturnExpression,
) -> LoweredBooleanReturnExpression {
    match expression {
        LoweredBooleanReturnExpression::Local { position } if *position == local => {
            replacement.clone()
        }
        LoweredBooleanReturnExpression::Not { operand } => LoweredBooleanReturnExpression::Not {
            operand: Box::new(inline_boolean_local(operand, local, replacement)),
        },
        LoweredBooleanReturnExpression::Equal { left, right } => {
            LoweredBooleanReturnExpression::Equal {
                left: Box::new(inline_boolean_local(left, local, replacement)),
                right: Box::new(inline_boolean_local(right, local, replacement)),
            }
        }
        LoweredBooleanReturnExpression::And { left, right } => {
            LoweredBooleanReturnExpression::And {
                left: Box::new(inline_boolean_local(left, local, replacement)),
                right: Box::new(inline_boolean_local(right, local, replacement)),
            }
        }
        LoweredBooleanReturnExpression::Or { left, right } => LoweredBooleanReturnExpression::Or {
            left: Box::new(inline_boolean_local(left, local, replacement)),
            right: Box::new(inline_boolean_local(right, local, replacement)),
        },
        expression => expression.clone(),
    }
}

fn source_distribute_boolean_local(
    decision: LoweredBooleanDecision,
    continuation: &LoweredBooleanReturnExpression,
    local: usize,
) -> LoweredBooleanDecision {
    // Preserve source evaluation exactly once: decide the staged value first,
    // then substitute only its already-computed leaf into each pure
    // continuation copy. Replacing every use with the original decision tree
    // would duplicate both execution and logical fuel.
    bind_boolean_decision(decision, &|value| {
        lower_boolean_value_decision(&inline_boolean_local(continuation, local, value))
    })
}

fn validate_boolean_decision_parameter_types(
    decision: &LoweredBooleanDecision,
    parameter_types: &[ScalarType],
) -> Result<(), LoweringError> {
    match decision {
        LoweredBooleanDecision::Value(expression) => {
            validate_boolean_parameter_types(expression, parameter_types)
        }
        LoweredBooleanDecision::Test {
            condition,
            when_true,
            when_false,
        } => {
            validate_boolean_parameter_types(condition, parameter_types)?;
            validate_boolean_decision_parameter_types(when_true, parameter_types)?;
            validate_boolean_decision_parameter_types(when_false, parameter_types)
        }
    }
}

fn lower_structural_unit_control_machine(
    checked: &CheckedTrees,
    plan: &CheckedStructuralUnitControlMachinePlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    if plan.states.len() < 2 {
        return unsupported("structural Unit control plan must contain multiple states");
    }
    let (structural_types, type_ids) = lower_structural_type_plans(
        &checked
            .facts
            .flow
            .terminal_structural_unit_controls
            .structural_types,
    )?;
    if plan
        .states
        .iter()
        .filter(|state| {
            matches!(
                state.terminator,
                CheckedStructuralUnitControlTerminatorPlan::Conditional { .. }
            )
        })
        .count()
        > 2
    {
        return unsupported(
            "structural Unit control supports at most two checked conditional states",
        );
    }
    for state in &plan.states {
        if state.structural_parameters.is_empty() {
            return unsupported("structural Unit state has no structural parameters");
        }
        let mut positions = BTreeSet::new();
        for parameter in &state.structural_parameters {
            if parameter.is_self
                || parameter.multiplicity != Multiplicity::Affine
                || !parameter.qualifications.is_empty()
                || !positions.insert(parameter.position)
            {
                return unsupported(
                    "structural Unit state signature is not claim-free affine custody",
                );
            }
            lookup_type_id(&type_ids, &parameter.type_identity)?;
        }
        for parameter in &state.scalar_parameters {
            if !positions.insert(parameter.source_position) {
                return unsupported(
                    "structural Unit scalar inputs overlap the authored parameter partition",
                );
            }
            terminal_scalar_type(parameter.primitive_type)?;
        }
        if positions.len() != state.structural_parameters.len() + state.scalar_parameters.len()
            || positions
                .iter()
                .copied()
                .enumerate()
                .any(|(index, position)| u32::try_from(index).ok() != Some(position))
        {
            return unsupported(
                "structural Unit parameter maps do not partition authored positions",
            );
        }
        match &state.terminator {
            CheckedStructuralUnitControlTerminatorPlan::Conditional {
                guard_scalar_parameter_index,
                ..
            } if usize::try_from(*guard_scalar_parameter_index)
                .ok()
                .and_then(|index| state.scalar_parameters.get(index))
                .is_some_and(|parameter| parameter.primitive_type == PrimitiveType::Bool) => {}
            CheckedStructuralUnitControlTerminatorPlan::Conditional { .. } => {
                return unsupported(
                    "structural Unit conditional must select one Boolean scalar state input",
                );
            }
            _ => {}
        }
    }
    let mut next_place = 1_u64;
    let entry_parameters = lower_unit_parameters(
        &plan.states[0].structural_parameters,
        &type_ids,
        &[],
        &mut next_place,
    )?;
    if entry_parameters.is_empty() {
        return unsupported("structural Unit control entry has no structural parameters");
    }
    let mut next_value = 1_u64;
    let state_scalar_parameters = plan
        .states
        .iter()
        .map(|state| {
            state
                .scalar_parameters
                .iter()
                .map(|parameter| {
                    Ok(ValueDeclaration {
                        id: value_id(allocate_dense(&mut next_value)?),
                        scalar_type: terminal_scalar_type(parameter.primitive_type)?,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let entry_places = entry_parameters
        .iter()
        .map(|parameter| parameter.place)
        .collect::<Vec<_>>();
    let entry_place_order = entry_places
        .iter()
        .enumerate()
        .map(|(index, place)| (*place, index))
        .collect::<BTreeMap<_, _>>();
    let state_ids = plan
        .states
        .iter()
        .enumerate()
        .map(|(index, state)| Ok((state.state, block_id(dense_identity(index)?))))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    if state_ids
        .iter()
        .enumerate()
        .any(|(index, (state, _))| state_ids[..index].iter().any(|(other, _)| other == state))
    {
        return unsupported("structural Unit control plan contains duplicate states");
    }

    let mut predecessor_counts = vec![0_usize; plan.states.len()];
    for state in &plan.states {
        let targets = match &state.terminator {
            CheckedStructuralUnitControlTerminatorPlan::ReturnUnit { .. } => Vec::new(),
            CheckedStructuralUnitControlTerminatorPlan::Jump { target_state, .. } => {
                vec![*target_state]
            }
            CheckedStructuralUnitControlTerminatorPlan::Conditional {
                when_true,
                when_false,
                ..
            } => {
                if when_true.target_state == when_false.target_state {
                    return unsupported(
                        "structural Unit conditional successors must remain distinct",
                    );
                }
                vec![when_true.target_state, when_false.target_state]
            }
        };
        for target in targets {
            let target_index = plan
                .states
                .iter()
                .position(|candidate| candidate.state == target)
                .ok_or(LoweringError::Unsupported(
                    "structural Unit jump targets an unknown checked state",
                ))?;
            predecessor_counts[target_index] += 1;
            if predecessor_counts[target_index] > 2 {
                return unsupported("structural Unit join supports exactly two incoming frontiers");
            }
        }
    }
    if predecessor_counts[0] != 0 {
        return unsupported("structural Unit control entry has an incoming edge");
    }
    if predecessor_counts
        .iter()
        .filter(|count| **count == 2)
        .count()
        > 1
    {
        return unsupported("structural Unit control supports at most one join state");
    }

    let mut bindings = vec![None; plan.states.len()];
    bindings[0] = Some(entry_places);
    let mut received_predecessors = vec![0_usize; plan.states.len()];
    let mut completed = BTreeSet::new();
    loop {
        let Some(index) = (0..plan.states.len()).find(|index| {
            bindings[*index].is_some()
                && !completed.contains(index)
                && (*index == 0 || received_predecessors[*index] == predecessor_counts[*index])
        }) else {
            break;
        };
        completed.insert(index);
        let source = bindings[index]
            .as_ref()
            .expect("ready structural state has a binding")
            .clone();
        if source.len() != plan.states[index].structural_parameters.len() {
            return unsupported("structural Unit state binding has the wrong arity");
        }
        let successors = match &plan.states[index].terminator {
            CheckedStructuralUnitControlTerminatorPlan::ReturnUnit {
                trivial_affine_discard_parameter_positions,
            } => {
                let expected = plan.states[index]
                    .structural_parameters
                    .iter()
                    .rev()
                    .map(|parameter| parameter.position)
                    .collect::<Vec<_>>();
                if *trivial_affine_discard_parameter_positions != expected {
                    return unsupported(
                        "structural Unit return cleanup does not consume its exact frontier",
                    );
                }
                continue;
            }
            CheckedStructuralUnitControlTerminatorPlan::Jump {
                target_state,
                transfers,
                scalar_arguments,
                trivial_affine_discard_parameter_positions,
                ..
            } => vec![(
                target_state,
                transfers.as_slice(),
                scalar_arguments.as_slice(),
                trivial_affine_discard_parameter_positions.as_slice(),
            )],
            CheckedStructuralUnitControlTerminatorPlan::Conditional {
                when_true,
                when_false,
                ..
            } => vec![
                (
                    &when_true.target_state,
                    when_true.transfers.as_slice(),
                    when_true.scalar_arguments.as_slice(),
                    when_true
                        .trivial_affine_discard_parameter_positions
                        .as_slice(),
                ),
                (
                    &when_false.target_state,
                    when_false.transfers.as_slice(),
                    when_false.scalar_arguments.as_slice(),
                    when_false
                        .trivial_affine_discard_parameter_positions
                        .as_slice(),
                ),
            ],
        };
        for (target_state, transfers, scalar_arguments, cleanup_positions) in successors {
            let target_state_index = plan
                .states
                .iter()
                .position(|state| state.state == *target_state)
                .ok_or(LoweringError::Unsupported(
                    "structural Unit jump targets an unknown checked state",
                ))?;
            if completed.contains(&target_state_index) {
                return unsupported("structural Unit control graph contains a cycle");
            }
            let target_arity = plan.states[target_state_index].structural_parameters.len();
            if transfers.len() != target_arity {
                return unsupported(
                    "structural Unit transfer map does not fill its target frontier",
                );
            }
            let target_scalar_parameters = &plan.states[target_state_index].scalar_parameters;
            if scalar_arguments.len() != target_scalar_parameters.len() {
                return unsupported(
                    "structural Unit scalar successor map does not fill its target signature",
                );
            }
            for (target_index, (argument, target_parameter)) in scalar_arguments
                .iter()
                .zip(target_scalar_parameters)
                .enumerate()
            {
                let source_index = usize::try_from(argument.source_scalar_parameter_index)
                    .map_err(|_| {
                        LoweringError::Unsupported(
                            "structural Unit scalar successor source exceeds usize",
                        )
                    })?;
                if argument.target_scalar_parameter_index
                    != u32::try_from(target_index).map_err(|_| {
                        LoweringError::Unsupported(
                            "structural Unit scalar successor target exceeds u32",
                        )
                    })?
                    || argument.argument_ordinal != target_parameter.source_position
                    || argument.primitive_type != target_parameter.primitive_type
                    || plan.states[index]
                        .scalar_parameters
                        .get(source_index)
                        .is_none_or(|source| {
                            source.primitive_type != target_parameter.primitive_type
                        })
                {
                    return unsupported(
                        "structural Unit scalar successor map changes its checked signature",
                    );
                }
            }
            let mut target = vec![None; target_arity];
            let mut used_sources = BTreeSet::new();
            for transfer in transfers {
                let source_index =
                    usize::try_from(transfer.source_parameter_index).map_err(|_| {
                        LoweringError::Unsupported("structural Unit source parameter exceeds usize")
                    })?;
                let target_parameter_index = usize::try_from(transfer.target_parameter_index)
                    .map_err(|_| {
                        LoweringError::Unsupported("structural Unit target parameter exceeds usize")
                    })?;
                let place = *source.get(source_index).ok_or(LoweringError::Unsupported(
                    "structural Unit transfer names an unknown source parameter",
                ))?;
                let source_parameter = &plan.states[index].structural_parameters[source_index];
                let target_parameter = plan.states[target_state_index]
                    .structural_parameters
                    .get(target_parameter_index)
                    .ok_or(LoweringError::Unsupported(
                        "structural Unit transfer names an unknown target parameter",
                    ))?;
                if source_parameter.type_identity != target_parameter.type_identity
                    || source_parameter.multiplicity != target_parameter.multiplicity
                    || source_parameter.qualifications != target_parameter.qualifications
                {
                    return unsupported(
                        "structural Unit transfer changes its checked structural signature",
                    );
                }
                let slot =
                    target
                        .get_mut(target_parameter_index)
                        .ok_or(LoweringError::Unsupported(
                            "structural Unit transfer names an unknown target parameter",
                        ))?;
                if slot.replace(place).is_some() || !used_sources.insert(source_index) {
                    return unsupported("structural Unit transfer map is not one-to-one");
                }
            }
            let expected_cleanup = plan.states[index]
                .structural_parameters
                .iter()
                .enumerate()
                .rev()
                .filter_map(|(parameter_index, parameter)| {
                    (!used_sources.contains(&parameter_index)).then_some(parameter.position)
                })
                .collect::<Vec<_>>();
            if *cleanup_positions != expected_cleanup {
                return unsupported(
                    "structural Unit jump transfer and cleanup do not partition its exact frontier",
                );
            }
            let target = target.into_iter().collect::<Option<Vec<_>>>().ok_or(
                LoweringError::Unsupported(
                    "structural Unit transfer map leaves a target parameter unbound",
                ),
            )?;
            if target
                .windows(2)
                .any(|pair| entry_place_order[&pair[0]] >= entry_place_order[&pair[1]])
            {
                return unsupported(
                    "structural Unit target frontier reorders entry custody outside terminal representation",
                );
            }
            if bindings[target_state_index]
                .as_ref()
                .is_some_and(|existing| existing != &target)
            {
                return unsupported(
                    "structural Unit join predecessors reconstruct different custody frontiers",
                );
            }
            bindings[target_state_index].get_or_insert(target);
            received_predecessors[target_state_index] += 1;
        }
    }
    if bindings.iter().any(Option::is_none) || completed.len() != plan.states.len() {
        return unsupported("structural Unit control graph is cyclic or unreachable");
    }

    let mut next_edge = 1_u64;
    let mut blocks = Vec::with_capacity(plan.states.len());
    for (index, state) in plan.states.iter().enumerate() {
        let state_binding = bindings[index]
            .as_ref()
            .expect("every structural state binding was reconstructed");
        let lower_discards = |positions: &[u32]| -> Result<Vec<PlaceId>, LoweringError> {
            positions
                .iter()
                .map(|position| {
                    let parameter_index = state
                        .structural_parameters
                        .iter()
                        .position(|parameter| parameter.position == *position)
                        .ok_or(LoweringError::Unsupported(
                            "structural Unit cleanup position is absent from its state signature",
                        ))?;
                    state_binding
                        .get(parameter_index)
                        .copied()
                        .ok_or(LoweringError::Unsupported(
                            "structural Unit cleanup position has no live entry binding",
                        ))
                })
                .collect()
        };
        let edge = edge_id(allocate_dense(&mut next_edge)?);
        let terminator = match &state.terminator {
            CheckedStructuralUnitControlTerminatorPlan::ReturnUnit {
                trivial_affine_discard_parameter_positions,
            } => Terminator::ReturnUnit {
                edge,
                trivial_affine_discards: lower_discards(
                    trivial_affine_discard_parameter_positions,
                )?,
            },
            CheckedStructuralUnitControlTerminatorPlan::Jump {
                statement_ordinal,
                target_state,
                scalar_arguments,
                trivial_affine_discard_parameter_positions,
                ..
            } => {
                if *statement_ordinal != 0 {
                    return unsupported(
                        "structural Unit jump is not the state's sole checked statement",
                    );
                }
                Terminator::Jump {
                    edge,
                    target: state_ids
                        .iter()
                        .find_map(|(state, block)| (*state == *target_state).then_some(*block))
                        .ok_or(LoweringError::Unsupported(
                            "structural Unit jump target has no terminal block",
                        ))?,
                    arguments: scalar_arguments
                        .iter()
                        .map(|argument| {
                            state_scalar_parameters[index]
                                .get(
                                    usize::try_from(argument.source_scalar_parameter_index)
                                        .map_err(|_| {
                                            LoweringError::Unsupported(
                                                "structural Unit scalar successor source exceeds usize",
                                            )
                                        })?,
                                )
                                .map(|parameter| parameter.id)
                                .ok_or(LoweringError::Unsupported(
                                    "structural Unit scalar successor names an unknown source",
                                ))
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                    trivial_affine_discards: lower_discards(
                        trivial_affine_discard_parameter_positions,
                    )?,
                }
            }
            CheckedStructuralUnitControlTerminatorPlan::Conditional {
                guard_scalar_parameter_index,
                when_true,
                when_false,
            } => {
                if when_true.statement_ordinal != 0 || when_false.statement_ordinal != 1 {
                    return unsupported(
                        "structural Unit conditional successors are not in canonical order",
                    );
                }
                let source_scalar_parameters = &state_scalar_parameters[index];
                let condition = source_scalar_parameters
                    .get(usize::try_from(*guard_scalar_parameter_index).map_err(|_| {
                        LoweringError::Unsupported(
                            "structural Unit guard scalar index exceeds usize",
                        )
                    })?)
                    .ok_or(LoweringError::Unsupported(
                        "structural Unit conditional names an unknown scalar guard",
                    ))?;
                let lower_successor =
                    |successor: &psi_checked_trees::CheckedStructuralControlSuccessorPlan,
                     edge: EdgeId|
                     -> Result<SuccessorEdge, LoweringError> {
                        Ok(SuccessorEdge {
                            edge,
                            target: state_ids
                                .iter()
                                .find_map(|(state, block)| {
                                    (*state == successor.target_state).then_some(*block)
                                })
                                .ok_or(LoweringError::Unsupported(
                                    "structural Unit conditional target has no terminal block",
                                ))?,
                            arguments: successor
                                .scalar_arguments
                                .iter()
                                .map(|argument| {
                                    source_scalar_parameters
                                        .get(
                                            usize::try_from(
                                                argument.source_scalar_parameter_index,
                                            )
                                            .map_err(|_| {
                                                LoweringError::Unsupported(
                                                    "structural Unit scalar successor source exceeds usize",
                                                )
                                            })?,
                                        )
                                        .map(|parameter| parameter.id)
                                        .ok_or(LoweringError::Unsupported(
                                            "structural Unit scalar successor names an unknown source",
                                        ))
                                })
                                .collect::<Result<Vec<_>, _>>()?,
                            trivial_affine_discards: lower_discards(
                                &successor.trivial_affine_discard_parameter_positions,
                            )?,
                        })
                    };
                let false_edge = edge_id(allocate_dense(&mut next_edge)?);
                Terminator::Conditional {
                    condition: condition.id,
                    when_true: lower_successor(when_true, edge)?,
                    when_false: lower_successor(when_false, false_edge)?,
                }
            }
        };
        blocks.push(Block {
            id: state_ids[index].1,
            parameters: if index == 0 {
                Vec::new()
            } else {
                state_scalar_parameters[index].clone()
            },
            operations: Vec::new(),
            terminator,
        });
    }
    let machine = TerminalMachine {
        id: machine_id(1),
        attachment: Some(lookup_type_id(&type_ids, &plan.attachment_type_identity)?),
        parameters: state_scalar_parameters[0].clone(),
        structural_parameters: entry_parameters.clone(),
        result: TerminalMachineResult::Unit,
        structural_places: entry_parameters
            .iter()
            .map(|parameter| StructuralPlaceDeclaration {
                id: parameter.place,
                kind: StructuralPlaceKind::Parameter {
                    position: parameter.position,
                    is_self: parameter.is_self,
                },
            })
            .collect(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: state_ids[0].1,
        blocks,
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
            entry: machine.id,
            structural_types,
            structural_domains: Vec::new(),
            services: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            machines: vec![machine],
        },
        proof_bundle: ProofBundle::default(),
        debug_map: None,
    })
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

fn lower_nominal_affine_unit_cleanup_machine(
    checked: &CheckedTrees,
    nominal: &CheckedNominalAffineUnitCleanupMachinePlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    if nominal.cleanups.len() >= 2 {
        return lower_ordered_nominal_affine_unit_cleanup_machine(checked, nominal);
    }
    let [cleanup] = nominal.cleanups.as_slice() else {
        return unsupported("nominal affine Unit cleanup list must be nonempty");
    };
    let plan = &nominal.machine;
    if checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .any(|candidate| candidate.machine == plan.machine)
    {
        return unsupported("nominal affine Unit machine is also published in the trivial lane");
    }
    let [parameter] = plan.structural_parameters.as_slice() else {
        return unsupported("nominal affine Unit cleanup requires one structural parameter");
    };
    let [
        CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index,
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
        },
    ] = plan.operations.as_slice()
    else {
        return unsupported("nominal affine Unit cleanup operation sequence drifted");
    };
    if parameter.position != 0
        || parameter.is_self
        || parameter.multiplicity != Multiplicity::Affine
        || !parameter.qualifications.is_empty()
        || !plan.trivial_affine_locals.is_empty()
        || !plan.entry_claims.is_empty()
        || !plan.body_qualifications.is_empty()
        || *statement_index != 0
        || !trivial_affine_local_discard_ordinals.is_empty()
        || !trivial_affine_discards.is_empty()
        || cleanup.source_parameter_index != 0
        || cleanup.type_identity != parameter.type_identity
        || cleanup.cleanup_machine == plan.machine
        || cleanup.cleanup_contract_fingerprint == 0
    {
        return unsupported("nominal affine Unit cleanup signature or coordinates drifted");
    }

    let nominal_types = &checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .structural_types;
    if nominal_types
        .iter()
        .any(|candidate| candidate.identity.is_empty())
        || nominal_types.iter().enumerate().any(|(index, candidate)| {
            nominal_types[..index]
                .iter()
                .any(|earlier| earlier.identity == candidate.identity)
        })
    {
        return unsupported("nominal affine Unit structural types are empty or duplicated");
    }
    let attachment_shape = nominal_types
        .iter()
        .find(|candidate| candidate.identity == plan.attachment_type_identity)
        .ok_or(LoweringError::Unsupported(
            "nominal affine Unit attachment type is absent from its checked shapes",
        ))?;
    if !matches!(
        &attachment_shape.shape,
        CheckedUnitStructuralTypeShape::Record { fields } if fields.is_empty()
    ) {
        return unsupported("nominal affine Unit attachment is not an empty record");
    }
    let parameter_shape = nominal_types
        .iter()
        .find(|candidate| candidate.identity == parameter.type_identity)
        .ok_or(LoweringError::Unsupported(
            "nominal affine Unit parameter type is absent from its checked shapes",
        ))?;
    if !is_bounded_nominal_cleanup_record(&parameter_shape.shape) {
        return unsupported("nominal affine Unit parameter is outside the bounded record shape");
    }
    let checked_contextual_field = |field_identity: &str, expected: bool| {
        let CheckedUnitStructuralTypeShape::Record { fields } = &parameter_shape.shape else {
            unreachable!("bounded nominal cleanup receiver is a record")
        };
        fields
            .iter()
            .find(|field| field.identity == field_identity)
            .filter(|field| {
                !field.relevance.is_erased()
                    && field.field_type
                        == CheckedUnitStructuralFieldType::Scalar(PrimitiveType::Bool)
            })
            .map(|field| (field.identity.clone(), expected))
            .ok_or(LoweringError::Unsupported(
                "contextual nominal cleanup requirement field is absent, erased, or non-Boolean",
            ))
    };
    let contextual_requirements = cleanup
        .requirements
        .iter()
        .map(|requirement| {
            checked_contextual_field(&requirement.field_identity, requirement.expected)
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    if contextual_requirements
        .iter()
        .collect::<BTreeSet<_>>()
        .len()
        != contextual_requirements.len()
    {
        return unsupported("contextual nominal cleanup requirements are duplicated");
    }
    let contextual_caller_requirements = nominal
        .caller_requirements
        .iter()
        .map(|requirement| {
            if requirement.source_parameter_index != cleanup.source_parameter_index {
                return Err(LoweringError::Unsupported(
                    "contextual nominal cleanup caller requirement root drifted",
                ));
            }
            checked_contextual_field(&requirement.field_identity, requirement.expected)
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    if contextual_caller_requirements
        .iter()
        .collect::<BTreeSet<_>>()
        .len()
        != contextual_caller_requirements.len()
        || contextual_requirements.iter().any(|required| {
            !contextual_caller_requirements
                .iter()
                .any(|caller| caller == required)
        })
    {
        return unsupported(
            "contextual nominal cleanup caller requirements are duplicated or incomplete",
        );
    }

    let cleanup_target = unique_unit_machine(
        &checked.facts.flow.terminal_unit_effects,
        cleanup.cleanup_machine,
    )?;
    let cleanup_contract = checked
        .facts
        .contract_plans
        .for_machine(cleanup.cleanup_machine)
        .ok_or(LoweringError::Unsupported(
            "nominal cleanup target is missing its checked contract identity",
        ))?;
    let service_summary_is_empty = |summary: ServiceReachSummary| {
        checked
            .facts
            .service_reaches
            .rows
            .services(summary.direct)
            .is_empty()
            && checked
                .facts
                .service_reaches
                .rows
                .services(summary.transitive)
                .is_empty()
    };
    let service_plan_is_empty = |plan: ServiceReachPlan| {
        let published_is_empty = match plan.interface {
            ServiceReachInterface::InternalInferred => true,
            ServiceReachInterface::PublishedCeiling(row) => {
                checked.facts.service_reaches.rows.services(row).is_empty()
            }
        };
        published_is_empty
            && checked
                .facts
                .service_reaches
                .rows
                .services(plan.checked_inferred)
                .is_empty()
    };
    let (cleanup_return, cleanup_calls) =
        cleanup_target
            .operations
            .split_last()
            .ok_or(LoweringError::Unsupported(
                "nominal cleanup target operation sequence is empty",
            ))?;
    let CheckedUnitEffectOperationPlan::ReturnUnit {
        statement_index,
        trivial_affine_local_discard_ordinals,
        trivial_affine_discards,
    } = cleanup_return
    else {
        return unsupported("nominal cleanup target operation sequence drifted");
    };
    if usize::try_from(*statement_index).ok() != Some(cleanup_calls.len())
        || !trivial_affine_local_discard_ordinals.is_empty()
        || !trivial_affine_discards.is_empty()
    {
        return unsupported("nominal cleanup target operation sequence drifted");
    }
    let mut cleanup_helpers = Vec::with_capacity(cleanup_calls.len());
    for (statement_index, operation) in cleanup_calls.iter().enumerate() {
        let CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            target_machine,
            target_state,
            target_contract_fingerprint,
            service_reach,
            structural_arguments,
            claim_transfers,
        } = operation
        else {
            return unsupported("nominal cleanup target operation sequence drifted");
        };
        if usize::try_from(coordinate.statement_index).ok() != Some(statement_index)
            || coordinate.call_ordinal != 0
            || *target_machine == plan.machine
            || *target_machine == cleanup.cleanup_machine
            || cleanup_helpers
                .iter()
                .any(|(helper, _, _)| helper == target_machine)
            || !service_summary_is_empty(*service_reach)
            || !structural_arguments.is_empty()
            || !claim_transfers.is_empty()
        {
            return unsupported("nominal cleanup target operation sequence drifted");
        }
        cleanup_helpers.push((*target_machine, *target_state, *target_contract_fingerprint));
    }
    if cleanup_target.state != cleanup.cleanup_state
        || cleanup_target.contract_fingerprint != cleanup.cleanup_contract_fingerprint
        || cleanup_contract.fingerprint != cleanup.cleanup_contract_fingerprint
        || cleanup_target.attachment_type_identity != cleanup.type_identity
        || !cleanup_target.structural_parameters.is_empty()
        || !cleanup_target.trivial_affine_locals.is_empty()
        || !cleanup_target.entry_claims.is_empty()
        || !cleanup_target.body_qualifications.is_empty()
        || !service_summary_is_empty(cleanup_target.service_reach)
        || !service_plan_is_empty(cleanup_target.contract_service_reach)
    {
        return unsupported("nominal cleanup target identity or bounded signature drifted");
    }

    for &(helper_machine, helper_state, helper_fingerprint) in &cleanup_helpers {
        let helper =
            unique_unit_machine(&checked.facts.flow.terminal_unit_effects, helper_machine)?;
        let helper_contract = checked
            .facts
            .contract_plans
            .for_machine(helper_machine)
            .ok_or(LoweringError::Unsupported(
                "nominal cleanup helper is missing its checked contract identity",
            ))?;
        let helper_shape = checked
            .facts
            .flow
            .terminal_unit_effects
            .structural_types
            .iter()
            .chain(nominal_types)
            .find(|candidate| candidate.identity == helper.attachment_type_identity)
            .ok_or(LoweringError::Unsupported(
                "nominal cleanup helper attachment is missing its checked shape",
            ))?;
        if helper.state != helper_state
            || helper.contract_fingerprint != helper_fingerprint
            || helper_contract.fingerprint != helper_fingerprint
            || !matches!(
                &helper_shape.shape,
                CheckedUnitStructuralTypeShape::Record { fields } if fields.is_empty()
            )
            || !helper.structural_parameters.is_empty()
            || !helper.trivial_affine_locals.is_empty()
            || !helper.entry_claims.is_empty()
            || !helper.body_qualifications.is_empty()
            || !service_summary_is_empty(helper.service_reach)
            || !service_plan_is_empty(helper.contract_service_reach)
            || !matches!(
                helper.operations.as_slice(),
                [CheckedUnitEffectOperationPlan::ReturnUnit {
                    statement_index: 0,
                    trivial_affine_local_discard_ordinals,
                    trivial_affine_discards,
                }] if trivial_affine_local_discard_ordinals.is_empty()
                    && trivial_affine_discards.is_empty()
            )
        {
            return unsupported("nominal cleanup helper is not exact and empty");
        }
    }

    // Cleanup is an explicit additional closure root because it is executable
    // edge work, not a source-authored ordinary call operation.
    let mut staged = checked.clone();
    let staged_unit = &mut staged.facts.flow.terminal_unit_effects;
    for shape in nominal_types {
        match staged_unit
            .structural_types
            .iter()
            .find(|candidate| candidate.identity == shape.identity)
        {
            Some(existing) if existing != shape => {
                return unsupported(
                    "nominal affine Unit structural type conflicts with its cleanup closure",
                );
            }
            Some(_) => {}
            None => staged_unit.structural_types.push(shape.clone()),
        }
    }
    staged_unit.machines.push(plan.clone());
    let closure =
        checked_unit_call_closure_including(&staged, plan.machine, &[cleanup.cleanup_machine])?;
    let mut expected_closure = vec![plan.machine, cleanup.cleanup_machine];
    expected_closure.extend(cleanup_helpers.iter().map(|(helper, _, _)| *helper));
    if closure != expected_closure {
        return unsupported("nominal cleanup closure is not the exact bounded machine graph");
    }
    let cleanup_machine_index = closure
        .iter()
        .position(|candidate| *candidate == cleanup.cleanup_machine)
        .ok_or(LoweringError::Unsupported(
            "nominal cleanup target is absent from its checked closure",
        ))?;
    let cleanup_terminal_id = machine_id(dense_identity(cleanup_machine_index)?);
    let helper_terminal_ids = cleanup_helpers
        .iter()
        .map(|(helper, _, _)| {
            closure
                .iter()
                .position(|candidate| candidate == helper)
                .ok_or(LoweringError::Unsupported(
                    "nominal cleanup helper is absent from its checked closure",
                ))
                .and_then(dense_identity)
                .map(machine_id)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut lowered =
        lower_attached_unit_closure_including(&staged, plan.machine, &[cleanup.cleanup_machine])?;
    let type_ids = lowered
        .semantic_module
        .structural_types
        .iter()
        .map(|declaration| (declaration.identity.clone(), declaration.id))
        .collect::<Vec<_>>();
    let cleanup_type = lookup_type_id(&type_ids, &cleanup.type_identity)?;

    let (cleanup_receiver, requirement_obligations, target_requires, caller_requires, evidence) =
        if contextual_caller_requirements.is_empty() {
            (None, Vec::new(), Vec::new(), Vec::new(), Vec::new())
        } else {
            if !lowered.proof_bundle.evidence.is_empty()
                || lowered.semantic_module.machines.iter().any(|machine| {
                    !machine.contract.requires.is_empty() || !machine.contract.ensures.is_empty()
                })
            {
                return unsupported(
                    "contextual nominal cleanup obligation namespace is not isolated",
                );
            }
            let receiver = if contextual_requirements.is_empty() {
                None
            } else {
                Some(place_id(
                    lowered
                        .semantic_module
                        .machines
                        .iter()
                        .flat_map(|machine| machine.structural_places.iter())
                        .map(|place| place.id.get())
                        .max()
                        .unwrap_or(0)
                        .checked_add(1)
                        .ok_or(LoweringError::Unsupported(
                            "contextual nominal cleanup proof-root identity space is exhausted",
                        ))?,
                ))
            };
            let caller_place = lowered
                .semantic_module
                .machines
                .iter()
                .find(|machine| machine.id == lowered.semantic_module.entry)
                .and_then(|machine| machine.structural_parameters.first())
                .ok_or(LoweringError::Unsupported(
                    "contextual nominal cleanup caller parameter is absent",
                ))?
                .place;
            let terminal_fields = lowered
                .semantic_module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == cleanup_type)
                .and_then(|declaration| match &declaration.shape {
                    StructuralTypeShape::Record { fields } => Some(fields),
                    StructuralTypeShape::FixedArray { .. } => None,
                })
                .ok_or(LoweringError::Unsupported(
                    "contextual nominal cleanup terminal receiver shape drifted",
                ))?;
            let terminal_field = |field_identity: &str| {
                terminal_fields
                    .iter()
                    .find(|field| field.identity == field_identity)
                    .filter(|field| {
                        !field.relevance.is_erased()
                            && field.field_type == StructuralFieldType::Scalar(ScalarType::Boolean)
                    })
                    .map(|field| field.id)
                    .ok_or(LoweringError::Unsupported(
                        "contextual nominal cleanup terminal field identity drifted",
                    ))
            };

            let mut caller_clauses = contextual_caller_requirements
                .iter()
                .map(|(field_identity, expected)| {
                    let field = terminal_field(field_identity)?;
                    Ok((
                        *expected,
                        field,
                        Proposition::Equal(
                            ScalarTerm::boolean(*expected),
                            ScalarTerm::boolean_field(caller_place, field),
                        ),
                    ))
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            // Every proposition in this bounded vocabulary shares the same
            // tags and root. Its canonical codec order is Boolean polarity,
            // then the little-endian byte order of StructuralFieldId. Sort
            // after terminal identities exist rather than trusting checked
            // declaration-identity order.
            caller_clauses
                .sort_by_key(|(expected, field, _)| (*expected, field.get().to_le_bytes()));
            let caller_requires = caller_clauses
                .iter()
                .map(|(_, _, proposition)| proposition.clone())
                .collect::<Vec<_>>();

            let mut target_clauses = contextual_requirements
                .iter()
                .map(|(field_identity, expected)| {
                    let field = terminal_field(field_identity)?;
                    let receiver = receiver.expect(
                        "a nonempty contextual cleanup requirement set has a proof-only receiver",
                    );
                    Ok((
                        *expected,
                        field,
                        Proposition::Equal(
                            ScalarTerm::boolean(*expected),
                            ScalarTerm::boolean_field(receiver, field),
                        ),
                    ))
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            target_clauses
                .sort_by_key(|(expected, field, _)| (*expected, field.get().to_le_bytes()));

            let mut requirement_obligations = Vec::with_capacity(target_clauses.len());
            let mut target_requires = Vec::with_capacity(target_clauses.len());
            let mut evidence = Vec::with_capacity(target_clauses.len());
            for (obligation_index, (expected, field, target_requirement)) in
                target_clauses.into_iter().enumerate()
            {
                let identity = u64::try_from(obligation_index)
                    .ok()
                    .and_then(|index| index.checked_add(1))
                    .ok_or(LoweringError::Unsupported(
                        "contextual nominal cleanup obligation identity space is exhausted",
                    ))?;
                let assumption_index = caller_clauses
                    .iter()
                    .position(|(caller_expected, caller_field, _)| {
                        *caller_expected == expected && *caller_field == field
                    })
                    .ok_or(LoweringError::Unsupported(
                        "contextual nominal cleanup caller requirement is absent",
                    ))?;
                let caller_requirement = caller_requires[assumption_index].clone();
                let obligation = obligation_id(identity);
                requirement_obligations.push(obligation);
                target_requires.push(target_requirement);
                evidence.push(ObligationEvidence {
                    obligation,
                    route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                        identity: EvidenceIdentity::new(identity)
                            .expect("terminal obligation identity is nonzero"),
                        proof_system_marker: ProofSystemMarker::CURRENT,
                        proof: ProofNode {
                            conclusion: caller_requirement,
                            rule: ProofRule::Assumption {
                                index: assumption_index,
                            },
                        },
                    }),
                });
            }
            (
                receiver,
                requirement_obligations,
                target_requires,
                caller_requires,
                evidence,
            )
        };

    let cleanup_terminal = lowered
        .semantic_module
        .machines
        .iter_mut()
        .find(|machine| machine.id == cleanup_terminal_id)
        .ok_or(LoweringError::Unsupported(
            "nominal cleanup target was not retained in the terminal closure",
        ))?;
    cleanup_terminal.contract.requires = target_requires.clone();
    let [cleanup_block] = cleanup_terminal.blocks.as_slice() else {
        return unsupported("nominal cleanup target terminal control drifted");
    };
    let cleanup_operations_are_exact = cleanup_block.operations.len() == helper_terminal_ids.len()
        && cleanup_block
            .operations
            .iter()
            .zip(&helper_terminal_ids)
            .all(|(operation, helper)| {
                operation.result == psi_terminal::OperationResult::Unit
                    && matches!(
                        &operation.kind,
                        OperationKind::CallUnit {
                            callee,
                            structural_arguments,
                            claim_transfers,
                            requirement_obligations,
                            crash_continuations,
                        } if callee == helper
                            && structural_arguments.is_empty()
                            && claim_transfers.is_empty()
                            && requirement_obligations.is_empty()
                            && crash_continuations.is_empty()
                    )
            });
    if cleanup_terminal.attachment != Some(cleanup_type)
        || !cleanup_terminal.parameters.is_empty()
        || !cleanup_terminal.structural_parameters.is_empty()
        || cleanup_terminal.result != TerminalMachineResult::Unit
        || !cleanup_terminal.structural_places.is_empty()
        || !cleanup_terminal.entry_claims.is_empty()
        || !cleanup_terminal.published_service_ceiling.is_empty()
        || !cleanup_terminal.content_entry_claims.is_empty()
        || !cleanup_terminal.content_identity_reshuffles.is_empty()
        || !cleanup_terminal.content_partition_compositions.is_empty()
        || !cleanup_operations_are_exact
        || !matches!(
            &cleanup_block.terminator,
            Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } if trivial_affine_discards.is_empty()
        )
        || !cleanup_terminal.contract.crash_routes.is_empty()
        || cleanup_terminal.contract.requires != target_requires
        || !cleanup_terminal.contract.ensures.is_empty()
    {
        return unsupported("nominal cleanup target terminal machine is not exact and bounded");
    }

    for &helper_id in &helper_terminal_ids {
        let helper = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == helper_id)
            .ok_or(LoweringError::Unsupported(
                "nominal cleanup helper was not retained in the terminal closure",
            ))?;
        let [helper_block] = helper.blocks.as_slice() else {
            return unsupported("nominal cleanup helper terminal control drifted");
        };
        let helper_attachment_is_empty = helper.attachment.is_some_and(|attachment| {
            lowered
                .semantic_module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == attachment)
                .is_some_and(|declaration| {
                    matches!(
                        &declaration.shape,
                        StructuralTypeShape::Record { fields } if fields.is_empty()
                    )
                })
        });
        if helper.id == cleanup_terminal_id
            || helper.id == lowered.semantic_module.entry
            || !helper_attachment_is_empty
            || !helper.parameters.is_empty()
            || !helper.structural_parameters.is_empty()
            || helper.result != TerminalMachineResult::Unit
            || !helper.structural_places.is_empty()
            || !helper.entry_claims.is_empty()
            || !helper.published_service_ceiling.is_empty()
            || !helper.content_entry_claims.is_empty()
            || !helper.content_identity_reshuffles.is_empty()
            || !helper.content_partition_compositions.is_empty()
            || !helper_block.parameters.is_empty()
            || !helper_block.operations.is_empty()
            || !matches!(
                &helper_block.terminator,
                Terminator::ReturnUnit {
                    trivial_affine_discards,
                    ..
                } if trivial_affine_discards.is_empty()
            )
            || !helper.contract.crash_routes.is_empty()
            || !helper.contract.requires.is_empty()
            || !helper.contract.ensures.is_empty()
        {
            return unsupported("nominal cleanup helper terminal machine is not exact and empty");
        }
    }

    let entry = lowered
        .semantic_module
        .machines
        .iter_mut()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .ok_or(LoweringError::Unsupported(
            "nominal affine Unit entry machine was not lowered",
        ))?;
    let [terminal_parameter] = entry.structural_parameters.as_slice() else {
        return unsupported("nominal affine Unit terminal parameter drifted");
    };
    entry.contract.requires = caller_requires.clone();
    if entry.attachment != Some(lookup_type_id(&type_ids, &plan.attachment_type_identity)?)
        || !entry.parameters.is_empty()
        || entry.result != TerminalMachineResult::Unit
        || entry.structural_places.len() != 1
        || !entry.entry_claims.is_empty()
        || !entry.published_service_ceiling.is_empty()
        || !entry.content_entry_claims.is_empty()
        || !entry.content_identity_reshuffles.is_empty()
        || !entry.content_partition_compositions.is_empty()
        || !entry.contract.crash_routes.is_empty()
        || entry.contract.requires != caller_requires
        || !entry.contract.ensures.is_empty()
        || terminal_parameter.structural_type != cleanup_type
        || terminal_parameter.multiplicity != StructuralMultiplicity::Affine
        || !terminal_parameter.qualifications.is_empty()
    {
        return unsupported("nominal affine Unit terminal parameter identity drifted");
    }
    let [block] = entry.blocks.as_mut_slice() else {
        return unsupported("nominal affine Unit terminal control drifted");
    };
    if block.id != entry.entry || !block.parameters.is_empty() || !block.operations.is_empty() {
        return unsupported("nominal affine Unit terminal control is not exact and empty");
    }
    let Terminator::ReturnUnit {
        edge,
        trivial_affine_discards: lowered_trivial_discards,
    } = &block.terminator
    else {
        return unsupported("nominal affine Unit terminal return drifted");
    };
    if !lowered_trivial_discards.is_empty() {
        return unsupported("nominal affine Unit return acquired trivial cleanup");
    }
    block.terminator = Terminator::ReturnUnitNominalAffine {
        edge: *edge,
        cleanups: vec![NominalAffineCleanup {
            place: terminal_parameter.place,
            structural_type: cleanup_type,
            cleanup_machine: cleanup_terminal_id,
            cleanup_receiver,
            requirement_obligations,
        }],
    };
    lowered.proof_bundle.evidence = evidence;
    Ok(lowered)
}

fn lower_ordered_nominal_affine_unit_cleanup_machine(
    checked: &CheckedTrees,
    nominal: &CheckedNominalAffineUnitCleanupMachinePlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    let plan = &nominal.machine;
    let service_summary_is_empty = |summary: ServiceReachSummary| {
        checked
            .facts
            .service_reaches
            .rows
            .services(summary.direct)
            .is_empty()
            && checked
                .facts
                .service_reaches
                .rows
                .services(summary.transitive)
                .is_empty()
    };
    let service_plan_is_empty = |plan: ServiceReachPlan| {
        let published_is_empty = match plan.interface {
            ServiceReachInterface::InternalInferred => true,
            ServiceReachInterface::PublishedCeiling(row) => {
                checked.facts.service_reaches.rows.services(row).is_empty()
            }
        };
        published_is_empty
            && checked
                .facts
                .service_reaches
                .rows
                .services(plan.checked_inferred)
                .is_empty()
    };
    let parameter_count = plan.structural_parameters.len();
    if parameter_count < 2 || nominal.cleanups.len() != parameter_count {
        return unsupported("ordered nominal cleanup requires matched actions");
    }
    let [
        CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index: 0,
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
        },
    ] = plan.operations.as_slice()
    else {
        return unsupported("ordered nominal cleanup caller operation sequence drifted");
    };
    if checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .any(|candidate| candidate.machine == plan.machine)
        || !plan.trivial_affine_locals.is_empty()
        || !plan.entry_claims.is_empty()
        || !plan.body_qualifications.is_empty()
        || !service_summary_is_empty(plan.service_reach)
        || !service_plan_is_empty(plan.contract_service_reach)
        || !trivial_affine_local_discard_ordinals.is_empty()
        || !trivial_affine_discards.is_empty()
    {
        return unsupported("ordered nominal cleanup caller signature drifted");
    }
    for (position, parameter) in plan.structural_parameters.iter().enumerate() {
        let cleanup = &nominal.cleanups[parameter_count - position - 1];
        if usize::try_from(parameter.position).ok() != Some(position)
            || usize::try_from(cleanup.source_parameter_index).ok() != Some(position)
            || parameter.is_self
            || parameter.multiplicity != Multiplicity::Affine
            || !parameter.qualifications.is_empty()
            || cleanup.type_identity != parameter.type_identity
            || cleanup.cleanup_machine == plan.machine
            || cleanup.cleanup_contract_fingerprint == 0
        {
            return unsupported("ordered nominal cleanup parameter join drifted");
        }
    }

    let nominal_types = &checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .structural_types;
    if nominal_types
        .iter()
        .any(|candidate| candidate.identity.is_empty())
        || nominal_types.iter().enumerate().any(|(index, candidate)| {
            nominal_types[..index]
                .iter()
                .any(|earlier| earlier.identity == candidate.identity)
        })
    {
        return unsupported("ordered nominal cleanup structural types are empty or duplicated");
    }
    let attachment_shape = nominal_types
        .iter()
        .find(|candidate| candidate.identity == plan.attachment_type_identity)
        .ok_or(LoweringError::Unsupported(
            "ordered nominal cleanup attachment shape is absent",
        ))?;
    if !matches!(&attachment_shape.shape, CheckedUnitStructuralTypeShape::Record { fields } if fields.is_empty())
    {
        return unsupported("ordered nominal cleanup attachment is not an empty record");
    }
    for parameter in &plan.structural_parameters {
        let shape = nominal_types
            .iter()
            .find(|candidate| candidate.identity == parameter.type_identity)
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup parameter shape is absent",
            ))?;
        if !is_bounded_nominal_cleanup_record(&shape.shape) {
            return unsupported("ordered nominal cleanup parameter shape is outside the bound");
        }
    }

    let checked_contextual_field =
        |source_parameter_index: u32, field_identity: &str, expected: bool| {
            let parameter = plan
                .structural_parameters
                .get(usize::try_from(source_parameter_index).map_err(|_| {
                    LoweringError::Unsupported(
                        "contextual nominal cleanup caller requirement root is out of range",
                    )
                })?)
                .ok_or(LoweringError::Unsupported(
                    "contextual nominal cleanup caller requirement root is absent",
                ))?;
            let shape = nominal_types
                .iter()
                .find(|candidate| candidate.identity == parameter.type_identity)
                .ok_or(LoweringError::Unsupported(
                    "contextual nominal cleanup receiver shape is absent",
                ))?;
            let CheckedUnitStructuralTypeShape::Record { fields } = &shape.shape else {
                unreachable!("bounded nominal cleanup receiver is a record")
            };
            fields
            .iter()
            .find(|field| field.identity == field_identity)
            .filter(|field| {
                !field.relevance.is_erased()
                    && field.field_type
                        == CheckedUnitStructuralFieldType::Scalar(PrimitiveType::Bool)
            })
            .map(|field| (field.identity.clone(), expected))
            .ok_or(LoweringError::Unsupported(
                "contextual nominal cleanup requirement field is absent, erased, or non-Boolean",
            ))
        };
    let contextual_caller_requirements = nominal
        .caller_requirements
        .iter()
        .map(|requirement| {
            checked_contextual_field(
                requirement.source_parameter_index,
                &requirement.field_identity,
                requirement.expected,
            )
            .map(|field| (requirement.source_parameter_index, field))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    if contextual_caller_requirements
        .iter()
        .collect::<BTreeSet<_>>()
        .len()
        != contextual_caller_requirements.len()
    {
        return unsupported("contextual nominal cleanup caller requirements are duplicated");
    }
    let contextual_cleanup_requirements = nominal
        .cleanups
        .iter()
        .map(|cleanup| {
            let requirements = cleanup
                .requirements
                .iter()
                .map(|requirement| {
                    checked_contextual_field(
                        cleanup.source_parameter_index,
                        &requirement.field_identity,
                        requirement.expected,
                    )
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            if requirements.iter().collect::<BTreeSet<_>>().len() != requirements.len()
                || requirements.iter().any(|field| {
                    !contextual_caller_requirements.iter().any(|(root, caller_field)| {
                        *root == cleanup.source_parameter_index && caller_field == field
                    })
                })
            {
                return unsupported(
                    "contextual nominal cleanup requirements are duplicated or lack a caller premise",
                );
            }
            Ok(requirements)
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    for (index, cleanup) in nominal.cleanups.iter().enumerate() {
        if let Some(earlier) = nominal.cleanups[..index]
            .iter()
            .position(|candidate| candidate.cleanup_machine == cleanup.cleanup_machine)
            && contextual_cleanup_requirements[earlier] != contextual_cleanup_requirements[index]
        {
            return unsupported("shared nominal cleanup target requirements drifted");
        }
    }

    let mut roots = Vec::new();
    let mut cleanup_helpers = Vec::new();
    for cleanup in &nominal.cleanups {
        let target = unique_unit_machine(
            &checked.facts.flow.terminal_unit_effects,
            cleanup.cleanup_machine,
        )?;
        let contract = checked
            .facts
            .contract_plans
            .for_machine(cleanup.cleanup_machine)
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup target contract is absent",
            ))?;
        let (target_return, target_calls) =
            target
                .operations
                .split_last()
                .ok_or(LoweringError::Unsupported(
                    "ordered nominal cleanup target operations are empty",
                ))?;
        let CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index,
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
        } = target_return
        else {
            return unsupported("ordered nominal cleanup target does not end in Unit return");
        };
        if usize::try_from(*statement_index).ok() != Some(target_calls.len())
            || !trivial_affine_local_discard_ordinals.is_empty()
            || !trivial_affine_discards.is_empty()
        {
            return unsupported("ordered nominal cleanup target operation sequence drifted");
        }
        let collect_helpers = !roots.contains(&cleanup.cleanup_machine);
        for (statement_index, operation) in target_calls.iter().enumerate() {
            let CheckedUnitEffectOperationPlan::CallUnit {
                coordinate,
                target_machine,
                target_state,
                target_contract_fingerprint,
                service_reach,
                structural_arguments,
                claim_transfers,
            } = operation
            else {
                return unsupported("ordered nominal cleanup target operation sequence drifted");
            };
            if usize::try_from(coordinate.statement_index).ok() != Some(statement_index)
                || coordinate.call_ordinal != 0
                || *target_machine == plan.machine
                || *target_machine == cleanup.cleanup_machine
                || target_calls[..statement_index].iter().any(|earlier| {
                    matches!(
                        earlier,
                        CheckedUnitEffectOperationPlan::CallUnit {
                            target_machine: earlier_target,
                            ..
                        } if earlier_target == target_machine
                    )
                })
                || !service_summary_is_empty(*service_reach)
                || !structural_arguments.is_empty()
                || !claim_transfers.is_empty()
            {
                return unsupported("ordered nominal cleanup helper call is not exact");
            }
            if collect_helpers {
                cleanup_helpers.push((
                    cleanup.cleanup_machine,
                    *target_machine,
                    *target_state,
                    *target_contract_fingerprint,
                ));
            }
        }
        if target.state != cleanup.cleanup_state
            || target.contract_fingerprint != cleanup.cleanup_contract_fingerprint
            || contract.fingerprint != cleanup.cleanup_contract_fingerprint
            || target.attachment_type_identity != cleanup.type_identity
            || !target.structural_parameters.is_empty()
            || !target.trivial_affine_locals.is_empty()
            || !target.entry_claims.is_empty()
            || !target.body_qualifications.is_empty()
            || !service_summary_is_empty(target.service_reach)
            || !service_plan_is_empty(target.contract_service_reach)
        {
            return unsupported("ordered nominal cleanup target is not exact and bounded");
        }
        if !roots.contains(&cleanup.cleanup_machine) {
            roots.push(cleanup.cleanup_machine);
        }
    }
    for &(_, helper_machine, helper_state, helper_fingerprint) in &cleanup_helpers {
        if roots.contains(&helper_machine) {
            return unsupported("ordered nominal cleanup helper overlaps a cleanup target");
        }
        let helper =
            unique_unit_machine(&checked.facts.flow.terminal_unit_effects, helper_machine)?;
        let helper_contract = checked
            .facts
            .contract_plans
            .for_machine(helper_machine)
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup helper contract is absent",
            ))?;
        let helper_shape = checked
            .facts
            .flow
            .terminal_unit_effects
            .structural_types
            .iter()
            .chain(nominal_types)
            .find(|candidate| candidate.identity == helper.attachment_type_identity)
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup helper attachment shape is absent",
            ))?;
        if helper.state != helper_state
            || helper.contract_fingerprint != helper_fingerprint
            || helper_contract.fingerprint != helper_fingerprint
            || !matches!(
                &helper_shape.shape,
                CheckedUnitStructuralTypeShape::Record { fields } if fields.is_empty()
            )
            || !helper.structural_parameters.is_empty()
            || !helper.trivial_affine_locals.is_empty()
            || !helper.entry_claims.is_empty()
            || !helper.body_qualifications.is_empty()
            || !service_summary_is_empty(helper.service_reach)
            || !service_plan_is_empty(helper.contract_service_reach)
            || !matches!(
                helper.operations.as_slice(),
                [CheckedUnitEffectOperationPlan::ReturnUnit {
                    statement_index: 0,
                    trivial_affine_local_discard_ordinals,
                    trivial_affine_discards,
                }] if trivial_affine_local_discard_ordinals.is_empty()
                    && trivial_affine_discards.is_empty()
            )
        {
            return unsupported("ordered nominal cleanup helper is not exact and empty");
        }
    }

    let mut staged = checked.clone();
    for shape in nominal_types {
        match staged
            .facts
            .flow
            .terminal_unit_effects
            .structural_types
            .iter()
            .find(|candidate| candidate.identity == shape.identity)
        {
            Some(existing) if existing != shape => {
                return unsupported("ordered nominal cleanup structural type conflicts");
            }
            Some(_) => {}
            None => staged
                .facts
                .flow
                .terminal_unit_effects
                .structural_types
                .push(shape.clone()),
        }
    }
    staged
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .push(plan.clone());
    let closure = checked_unit_call_closure_including(&staged, plan.machine, &roots)?;
    let mut expected = vec![plan.machine];
    expected.extend(&roots);
    for &(_, helper, _, _) in &cleanup_helpers {
        if !expected.contains(&helper) {
            expected.push(helper);
        }
    }
    if closure != expected {
        return unsupported("ordered nominal cleanup closure is not exact");
    }
    let mut lowered = lower_attached_unit_closure_including(&staged, plan.machine, &roots)?;
    let type_ids = lowered
        .semantic_module
        .structural_types
        .iter()
        .map(|declaration| (declaration.identity.clone(), declaration.id))
        .collect::<Vec<_>>();
    let entry_index = lowered
        .semantic_module
        .machines
        .iter()
        .position(|machine| machine.id == lowered.semantic_module.entry)
        .ok_or(LoweringError::Unsupported(
            "ordered nominal cleanup entry is absent",
        ))?;
    let entry_parameters = lowered.semantic_module.machines[entry_index]
        .structural_parameters
        .clone();
    if !contextual_caller_requirements.is_empty()
        && (!lowered.proof_bundle.evidence.is_empty()
            || lowered.semantic_module.machines.iter().any(|machine| {
                !machine.contract.requires.is_empty() || !machine.contract.ensures.is_empty()
            }))
    {
        return unsupported("contextual nominal cleanup obligation namespace is not isolated");
    }
    let terminal_field =
        |source_parameter_index: u32,
         field_identity: &str|
         -> Result<(PlaceId, StructuralTypeId, StructuralFieldId), LoweringError> {
            let parameter = plan
                .structural_parameters
                .get(usize::try_from(source_parameter_index).map_err(|_| {
                    LoweringError::Unsupported(
                        "contextual nominal cleanup terminal root is out of range",
                    )
                })?)
                .ok_or(LoweringError::Unsupported(
                    "contextual nominal cleanup terminal root is absent",
                ))?;
            let terminal_parameter = entry_parameters
                .iter()
                .find(|candidate| candidate.position == parameter.position)
                .ok_or(LoweringError::Unsupported(
                    "contextual nominal cleanup terminal parameter is absent",
                ))?;
            let structural_type = lookup_type_id(&type_ids, &parameter.type_identity)?;
            let field = lowered
                .semantic_module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == structural_type)
                .and_then(|declaration| match &declaration.shape {
                    StructuralTypeShape::Record { fields } => {
                        fields.iter().find(|field| field.identity == field_identity)
                    }
                    StructuralTypeShape::FixedArray { .. } => None,
                })
                .filter(|field| {
                    !field.relevance.is_erased()
                        && field.field_type == StructuralFieldType::Scalar(ScalarType::Boolean)
                })
                .map(|field| field.id)
                .ok_or(LoweringError::Unsupported(
                    "contextual nominal cleanup terminal field identity drifted",
                ))?;
            Ok((terminal_parameter.place, structural_type, field))
        };
    let mut caller_clauses = contextual_caller_requirements
        .iter()
        .map(|(root, (field_identity, expected))| {
            let (place, _, field) = terminal_field(*root, field_identity)?;
            Ok((
                (*expected, place, field),
                Proposition::Equal(
                    ScalarTerm::boolean(*expected),
                    ScalarTerm::boolean_field(place, field),
                ),
            ))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    caller_clauses.sort_by_key(|((expected, root, field), _)| {
        (
            *expected,
            root.get().to_le_bytes(),
            field.get().to_le_bytes(),
        )
    });
    let caller_requires = caller_clauses
        .iter()
        .map(|(_, proposition)| proposition.clone())
        .collect::<Vec<_>>();

    let mut next_proof_root = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| machine.structural_places.iter())
        .map(|place| place.id.get())
        .max()
        .unwrap_or(0);
    let mut target_contexts = Vec::<(
        psi_symbols::SymbolHandle,
        Option<PlaceId>,
        Vec<(bool, StructuralFieldId, Proposition)>,
    )>::new();
    for (cleanup, requirements) in nominal
        .cleanups
        .iter()
        .zip(&contextual_cleanup_requirements)
    {
        if target_contexts
            .iter()
            .any(|(target, _, _)| *target == cleanup.cleanup_machine)
        {
            continue;
        }
        let receiver = if requirements.is_empty() {
            None
        } else {
            next_proof_root = next_proof_root
                .checked_add(1)
                .ok_or(LoweringError::Unsupported(
                    "contextual nominal cleanup proof-root identity space is exhausted",
                ))?;
            Some(place_id(next_proof_root))
        };
        let mut clauses = requirements
            .iter()
            .map(|(field_identity, expected)| {
                let (_, _, field) = terminal_field(cleanup.source_parameter_index, field_identity)?;
                let receiver = receiver.expect(
                    "a nonempty contextual cleanup requirement set has a proof-only receiver",
                );
                Ok((
                    *expected,
                    field,
                    Proposition::Equal(
                        ScalarTerm::boolean(*expected),
                        ScalarTerm::boolean_field(receiver, field),
                    ),
                ))
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        clauses.sort_by_key(|(expected, field, _)| (*expected, field.get().to_le_bytes()));
        target_contexts.push((cleanup.cleanup_machine, receiver, clauses));
    }
    for (target_symbol, _, clauses) in &target_contexts {
        let target_index = closure
            .iter()
            .position(|candidate| candidate == target_symbol)
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup target was not retained",
            ))?;
        let target_id = machine_id(dense_identity(target_index)?);
        let target = lowered
            .semantic_module
            .machines
            .iter_mut()
            .find(|machine| machine.id == target_id)
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup target was not retained",
            ))?;
        target.contract.requires = clauses
            .iter()
            .map(|(_, _, proposition)| proposition.clone())
            .collect();
    }

    let mut next_obligation_identity = 0_u64;
    let mut evidence = Vec::new();
    let mut terminal_cleanups = Vec::with_capacity(nominal.cleanups.len());
    for cleanup in &nominal.cleanups {
        let parameter = plan
            .structural_parameters
            .get(
                usize::try_from(cleanup.source_parameter_index).map_err(|_| {
                    LoweringError::Unsupported(
                        "ordered nominal cleanup source root is out of range",
                    )
                })?,
            )
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup source root is absent",
            ))?;
        let terminal_parameter = entry_parameters
            .iter()
            .find(|candidate| candidate.position == parameter.position)
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup terminal parameter is absent",
            ))?;
        let machine_index = closure
            .iter()
            .position(|candidate| *candidate == cleanup.cleanup_machine)
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup target is absent",
            ))?;
        let (_, receiver, target_clauses) = target_contexts
            .iter()
            .find(|(target, _, _)| *target == cleanup.cleanup_machine)
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup target context is absent",
            ))?;
        let mut requirement_obligations = Vec::with_capacity(target_clauses.len());
        for (expected, field, _) in target_clauses {
            let assumption_index = caller_clauses
                .iter()
                .position(|((caller_expected, root, caller_field), _)| {
                    caller_expected == expected
                        && *root == terminal_parameter.place
                        && caller_field == field
                })
                .ok_or(LoweringError::Unsupported(
                    "contextual nominal cleanup caller requirement is absent",
                ))?;
            next_obligation_identity =
                next_obligation_identity
                    .checked_add(1)
                    .ok_or(LoweringError::Unsupported(
                        "contextual nominal cleanup obligation identity space is exhausted",
                    ))?;
            let obligation = obligation_id(next_obligation_identity);
            requirement_obligations.push(obligation);
            evidence.push(ObligationEvidence {
                obligation,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(next_obligation_identity)
                        .expect("terminal obligation identity is nonzero"),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        conclusion: caller_requires[assumption_index].clone(),
                        rule: ProofRule::Assumption {
                            index: assumption_index,
                        },
                    },
                }),
            });
        }
        terminal_cleanups.push(NominalAffineCleanup {
            place: terminal_parameter.place,
            structural_type: lookup_type_id(&type_ids, &cleanup.type_identity)?,
            cleanup_machine: machine_id(dense_identity(machine_index)?),
            cleanup_receiver: *receiver,
            requirement_obligations,
        });
    }
    for (cleanup, checked_cleanup) in terminal_cleanups.iter().zip(&nominal.cleanups) {
        let target = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == cleanup.cleanup_machine)
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup target was not retained",
            ))?;
        let [target_block] = target.blocks.as_slice() else {
            return unsupported("ordered nominal cleanup target terminal control drifted");
        };
        let expected_helpers = cleanup_helpers
            .iter()
            .filter(|(owner, _, _, _)| *owner == checked_cleanup.cleanup_machine)
            .map(|(_, helper, _, _)| {
                closure
                    .iter()
                    .position(|candidate| candidate == helper)
                    .ok_or(LoweringError::Unsupported(
                        "ordered nominal cleanup helper was not retained",
                    ))
                    .and_then(dense_identity)
                    .map(machine_id)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let target_operations_are_exact =
            target_block.operations.len() == expected_helpers.len()
                && target_block.operations.iter().zip(&expected_helpers).all(
                    |(operation, helper)| {
                        operation.result == psi_terminal::OperationResult::Unit
                            && matches!(
                                &operation.kind,
                                OperationKind::CallUnit {
                                    callee,
                                    structural_arguments,
                                    claim_transfers,
                                    requirement_obligations,
                                    crash_continuations,
                                } if callee == helper
                                    && structural_arguments.is_empty()
                                    && claim_transfers.is_empty()
                                    && requirement_obligations.is_empty()
                                    && crash_continuations.is_empty()
                            )
                    },
                );
        let expected_target_requires = target_contexts
            .iter()
            .find(|(target_symbol, _, _)| *target_symbol == checked_cleanup.cleanup_machine)
            .map(|(_, _, clauses)| {
                clauses
                    .iter()
                    .map(|(_, _, proposition)| proposition.clone())
                    .collect::<Vec<_>>()
            })
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup target context is absent",
            ))?;
        if target.attachment != Some(cleanup.structural_type)
            || !target.parameters.is_empty()
            || !target.structural_parameters.is_empty()
            || target.result != TerminalMachineResult::Unit
            || !target.structural_places.is_empty()
            || !target.entry_claims.is_empty()
            || !target.published_service_ceiling.is_empty()
            || !target.content_entry_claims.is_empty()
            || !target.content_identity_reshuffles.is_empty()
            || !target.content_partition_compositions.is_empty()
            || target_block.id != target.entry
            || !target_block.parameters.is_empty()
            || !target_operations_are_exact
            || !matches!(
                &target_block.terminator,
                Terminator::ReturnUnit {
                    trivial_affine_discards,
                    ..
                } if trivial_affine_discards.is_empty()
            )
            || !target.contract.crash_routes.is_empty()
            || target.contract.requires != expected_target_requires
            || !target.contract.ensures.is_empty()
        {
            return unsupported("ordered nominal cleanup target terminal machine is not exact");
        }
    }
    for &(_, helper_symbol, _, _) in &cleanup_helpers {
        let helper_index = closure
            .iter()
            .position(|candidate| *candidate == helper_symbol)
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup helper was not retained",
            ))?;
        let helper_id = machine_id(dense_identity(helper_index)?);
        let helper = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == helper_id)
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup helper terminal machine is absent",
            ))?;
        let [helper_block] = helper.blocks.as_slice() else {
            return unsupported("ordered nominal cleanup helper terminal control drifted");
        };
        let helper_attachment_is_empty = helper.attachment.is_some_and(|attachment| {
            lowered
                .semantic_module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == attachment)
                .is_some_and(|declaration| {
                    matches!(
                        &declaration.shape,
                        StructuralTypeShape::Record { fields } if fields.is_empty()
                    )
                })
        });
        if !helper_attachment_is_empty
            || !helper.parameters.is_empty()
            || !helper.structural_parameters.is_empty()
            || helper.result != TerminalMachineResult::Unit
            || !helper.structural_places.is_empty()
            || !helper.entry_claims.is_empty()
            || !helper.published_service_ceiling.is_empty()
            || !helper.content_entry_claims.is_empty()
            || !helper.content_identity_reshuffles.is_empty()
            || !helper.content_partition_compositions.is_empty()
            || helper_block.id != helper.entry
            || !helper_block.parameters.is_empty()
            || !helper_block.operations.is_empty()
            || !matches!(
                &helper_block.terminator,
                Terminator::ReturnUnit {
                    trivial_affine_discards,
                    ..
                } if trivial_affine_discards.is_empty()
            )
            || !helper.contract.crash_routes.is_empty()
            || !helper.contract.requires.is_empty()
            || !helper.contract.ensures.is_empty()
        {
            return unsupported("ordered nominal cleanup helper terminal machine is not exact");
        }
    }
    let entry = &mut lowered.semantic_module.machines[entry_index];
    entry.contract.requires = caller_requires.clone();
    let [block] = entry.blocks.as_mut_slice() else {
        return unsupported("ordered nominal cleanup entry control drifted");
    };
    let Terminator::ReturnUnit {
        edge,
        trivial_affine_discards,
    } = &block.terminator
    else {
        return unsupported("ordered nominal cleanup entry return drifted");
    };
    if entry.structural_parameters.len() != parameter_count
        || entry.structural_places.len() != parameter_count
        || !entry.parameters.is_empty()
        || entry.result != TerminalMachineResult::Unit
        || !entry.entry_claims.is_empty()
        || !entry.published_service_ceiling.is_empty()
        || !entry.content_entry_claims.is_empty()
        || !entry.content_identity_reshuffles.is_empty()
        || !entry.content_partition_compositions.is_empty()
        || block.id != entry.entry
        || !block.parameters.is_empty()
        || !block.operations.is_empty()
        || !trivial_affine_discards.is_empty()
        || !entry.contract.crash_routes.is_empty()
        || entry.contract.requires != caller_requires
        || !entry.contract.ensures.is_empty()
    {
        return unsupported("ordered nominal cleanup terminal caller is not exact");
    }
    block.terminator = Terminator::ReturnUnitNominalAffine {
        edge: *edge,
        cleanups: terminal_cleanups,
    };
    lowered.proof_bundle.evidence = evidence;
    Ok(lowered)
}

fn is_bounded_nominal_cleanup_record(shape: &CheckedUnitStructuralTypeShape) -> bool {
    match shape {
        CheckedUnitStructuralTypeShape::Record { fields } => fields.iter().all(|field| {
            !field.relevance.is_erased()
                && matches!(
                    &field.field_type,
                    CheckedUnitStructuralFieldType::Scalar(
                        PrimitiveType::Bool
                            | PrimitiveType::I8
                            | PrimitiveType::I16
                            | PrimitiveType::I32
                            | PrimitiveType::I64
                            | PrimitiveType::U8
                            | PrimitiveType::U16
                            | PrimitiveType::U32
                            | PrimitiveType::U64
                            | PrimitiveType::Addr
                    )
                )
        }),
        CheckedUnitStructuralTypeShape::FixedArray { .. } => false,
    }
}

fn lower_partial_affine_unit_cleanup_machine(
    checked: &CheckedTrees,
    partial: &CheckedPartialAffineUnitCleanupMachinePlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    let plan = &partial.machine;
    if checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .any(|candidate| candidate.machine == plan.machine)
    {
        return unsupported("partial affine Unit machine is also published in the root-only lane");
    }
    let [parameter] = plan.structural_parameters.as_slice() else {
        return unsupported("partial affine Unit cleanup requires one structural parameter");
    };
    if partial.residual_affine_discards.is_empty() {
        return unsupported("partial affine Unit cleanup requires residual actions");
    }
    let Some((return_operation, call_operations)) = plan.operations.split_last() else {
        return unsupported("partial affine Unit cleanup operation sequence drifted");
    };
    let CheckedUnitEffectOperationPlan::ReturnUnit {
        statement_index,
        trivial_affine_local_discard_ordinals,
        trivial_affine_discards,
    } = return_operation
    else {
        return unsupported("partial affine Unit cleanup operation sequence drifted");
    };
    if call_operations.is_empty() {
        return unsupported("partial affine Unit cleanup requires projected calls");
    }
    let mut moved_paths = Vec::<(
        &[CheckedUnitStructuralPathSegment],
        &str,
        psi_symbols::SymbolHandle,
    )>::new();
    for (operation_ordinal, operation) in call_operations.iter().enumerate() {
        let CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            target_machine,
            structural_arguments,
            claim_transfers,
            ..
        } = operation
        else {
            return unsupported("partial affine Unit cleanup operation sequence drifted");
        };
        let [argument] = structural_arguments.as_slice() else {
            return unsupported("partial affine Unit cleanup requires one structural argument");
        };
        if argument.path.is_empty()
            || argument
                .path
                .iter()
                .any(|segment| !matches!(segment, CheckedUnitStructuralPathSegment::Field(_)))
        {
            return unsupported("partial affine Unit transfer is not an exact field path");
        }
        if coordinate.statement_index
            != u32::try_from(operation_ordinal)
                .map_err(|_| LoweringError::Unsupported("partial affine call count exceeds u32"))?
            || coordinate.call_ordinal != 0
            || !claim_transfers.is_empty()
            || argument.source_parameter_index != 0
            || moved_paths.iter().any(|(earlier, _, _)| {
                earlier.starts_with(&argument.path) || argument.path.starts_with(earlier)
            })
        {
            return unsupported("partial affine Unit cleanup signature or coordinates drifted");
        }
        moved_paths.push((
            argument.path.as_slice(),
            argument.type_identity.as_str(),
            *target_machine,
        ));
    }
    if partial.residual_affine_discards.iter().any(|residual| {
        residual.path.is_empty()
            || residual
                .path
                .iter()
                .any(|segment| !matches!(segment, CheckedUnitStructuralPathSegment::Field(_)))
    }) {
        return unsupported("partial affine Unit cleanup is not an exact field path");
    }
    if parameter.position != 0
        || parameter.is_self
        || parameter.multiplicity != Multiplicity::Affine
        || !parameter.qualifications.is_empty()
        || !plan.trivial_affine_locals.is_empty()
        || !plan.entry_claims.is_empty()
        || !plan.body_qualifications.is_empty()
        || usize::try_from(*statement_index).ok() != Some(call_operations.len())
        || partial
            .residual_affine_discards
            .iter()
            .any(|residual| residual.source_parameter_index != 0)
        || !trivial_affine_local_discard_ordinals.is_empty()
        || !trivial_affine_discards.is_empty()
    {
        return unsupported("partial affine Unit cleanup signature or coordinates drifted");
    }

    let partial_plans = &checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .structural_types;
    if partial_plans
        .iter()
        .any(|candidate| candidate.identity.is_empty())
        || partial_plans.iter().enumerate().any(|(index, candidate)| {
            partial_plans[..index]
                .iter()
                .any(|earlier| earlier.identity == candidate.identity)
        })
    {
        return unsupported("partial affine Unit structural types are empty or duplicated");
    }
    let source_shape = partial_plans
        .iter()
        .find(|candidate| candidate.identity == parameter.type_identity)
        .ok_or(LoweringError::Unsupported(
            "partial affine Unit parameter type is absent from its checked shapes",
        ))?;
    let CheckedUnitStructuralTypeShape::Record { fields } = &source_shape.shape else {
        return unsupported("partial affine Unit parameter is not a record");
    };
    if fields.len() < 2 {
        return unsupported("partial affine Unit record has fewer than two fields");
    }
    if fields.iter().enumerate().any(|(index, field)| {
        field.relevance.is_erased()
            || !matches!(
                field.field_type,
                CheckedUnitStructuralFieldType::Structural { .. }
            )
            || fields[..index]
                .iter()
                .any(|earlier| earlier.identity == field.identity)
    }) {
        return unsupported("partial affine Unit field path or type identity drifted");
    }
    let expected_residuals = checked_partial_affine_residuals(
        partial_plans,
        &parameter.type_identity,
        &moved_paths
            .iter()
            .map(|(path, moved_type, _)| (*path, *moved_type))
            .collect::<Vec<_>>(),
    )
    .ok_or(LoweringError::Unsupported(
        "partial affine Unit field path or type identity drifted",
    ))?;
    if partial.residual_affine_discards != expected_residuals {
        return unsupported("partial affine Unit residual field partition drifted");
    }
    for (_, moved_type, target_machine) in &moved_paths {
        let target =
            unique_unit_machine(&checked.facts.flow.terminal_unit_effects, *target_machine)?;
        let [target_parameter] = target.structural_parameters.as_slice() else {
            return unsupported("partial affine Unit target signature drifted");
        };
        if target_parameter.type_identity != *moved_type
            || target_parameter.is_self
            || target_parameter.multiplicity != Multiplicity::Affine
            || !target_parameter.qualifications.is_empty()
        {
            return unsupported("partial affine Unit target parameter drifted");
        }
    }

    // Reuse the ordinary closure lowerer only after validating the separate
    // checked lane. The staged copy is local producer state; no compatibility
    // or alternate artifact path escapes this function.
    let mut staged = checked.clone();
    let staged_unit = &mut staged.facts.flow.terminal_unit_effects;
    for shape in partial_plans {
        match staged_unit
            .structural_types
            .iter()
            .find(|candidate| candidate.identity == shape.identity)
        {
            Some(existing) if existing != shape => {
                return unsupported(
                    "partial affine Unit structural type conflicts with its closure",
                );
            }
            Some(_) => {}
            None => staged_unit.structural_types.push(shape.clone()),
        }
    }
    staged_unit.machines.push(plan.clone());
    let mut lowered = lower_attached_unit_closure(&staged, plan.machine)?;
    let entry = lowered
        .semantic_module
        .machines
        .iter_mut()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .ok_or(LoweringError::Unsupported(
            "partial affine Unit entry machine was not lowered",
        ))?;
    let [terminal_parameter] = entry.structural_parameters.as_slice() else {
        return unsupported("partial affine Unit terminal parameter drifted");
    };
    let [block] = entry.blocks.as_mut_slice() else {
        return unsupported("partial affine Unit terminal control drifted");
    };
    let Terminator::ReturnUnit {
        edge,
        trivial_affine_discards: lowered_trivial_discards,
    } = &block.terminator
    else {
        return unsupported("partial affine Unit terminal return drifted");
    };
    if !lowered_trivial_discards.is_empty() {
        return unsupported("partial affine Unit return acquired root-only cleanup");
    }
    let terminal_type_ids = lowered
        .semantic_module
        .structural_types
        .iter()
        .map(|declaration| (declaration.identity.clone(), declaration.id))
        .collect::<Vec<_>>();
    let residual_affine_discards = partial
        .residual_affine_discards
        .iter()
        .map(|residual| {
            Ok(StructuralAffineDiscard {
                place: terminal_parameter.place,
                path: lower_structural_path(&residual.path),
                structural_type: lookup_type_id(&terminal_type_ids, &residual.type_identity)?,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    block.terminator = Terminator::ReturnUnitPartialAffine {
        edge: *edge,
        trivial_affine_discards: Vec::new(),
        residual_affine_discards,
    };
    Ok(lowered)
}

fn checked_partial_affine_residuals(
    types: &[CheckedUnitStructuralTypePlan],
    root_type: &str,
    moved_paths: &[(&[CheckedUnitStructuralPathSegment], &str)],
) -> Option<Vec<CheckedUnitPartialAffineDiscardPlan>> {
    fn visit(
        types: &[CheckedUnitStructuralTypePlan],
        current_type: &str,
        moved_paths: &[(&[CheckedUnitStructuralPathSegment], &str)],
        prefix: &mut Vec<CheckedUnitStructuralPathSegment>,
        residuals: &mut Vec<CheckedUnitPartialAffineDiscardPlan>,
    ) -> Option<()> {
        if moved_paths.is_empty()
            || moved_paths.iter().any(|(path, _)| {
                !matches!(
                    path.first(),
                    Some(CheckedUnitStructuralPathSegment::Field(_))
                )
            })
        {
            return None;
        }
        let declaration = types
            .iter()
            .find(|declaration| declaration.identity == current_type)?;
        let CheckedUnitStructuralTypeShape::Record { fields } = &declaration.shape else {
            return None;
        };
        if fields.is_empty()
            || fields.iter().enumerate().any(|(index, field)| {
                field.relevance.is_erased()
                    || !matches!(
                        field.field_type,
                        CheckedUnitStructuralFieldType::Structural { .. }
                    )
                    || fields[..index]
                        .iter()
                        .any(|earlier| earlier.identity == field.identity)
            })
        {
            return None;
        }
        for field in fields.iter().rev() {
            let CheckedUnitStructuralFieldType::Structural { type_identity } = &field.field_type
            else {
                return None;
            };
            let matching = moved_paths
                .iter()
                .filter(|(path, _)| {
                    matches!(path.first(), Some(CheckedUnitStructuralPathSegment::Field(identity))
                        if identity == &field.identity)
                })
                .copied()
                .collect::<Vec<_>>();
            prefix.push(CheckedUnitStructuralPathSegment::Field(
                field.identity.clone(),
            ));
            if matching.is_empty() {
                residuals.push(CheckedUnitPartialAffineDiscardPlan {
                    source_parameter_index: 0,
                    path: prefix.clone(),
                    type_identity: type_identity.clone(),
                });
                prefix.pop();
                continue;
            }
            let whole = matching
                .iter()
                .filter(|(path, _)| path.len() == 1)
                .collect::<Vec<_>>();
            if !whole.is_empty() {
                if whole.len() != 1 || matching.len() != 1 || whole[0].1 != type_identity {
                    return None;
                }
                prefix.pop();
                continue;
            }
            let nested = matching
                .iter()
                .map(|(path, moved_type)| (&path[1..], *moved_type))
                .collect::<Vec<_>>();
            visit(types, type_identity, &nested, prefix, residuals)?;
            prefix.pop();
        }
        Some(())
    }

    if moved_paths.is_empty() {
        return None;
    }
    let mut residuals = Vec::new();
    visit(
        types,
        root_type,
        moved_paths,
        &mut Vec::new(),
        &mut residuals,
    )?;
    Some(residuals)
}

fn lower_attached_unit_closure(
    checked: &CheckedTrees,
    entry: psi_symbols::SymbolHandle,
) -> Result<LoweredTerminalPsi, LoweringError> {
    lower_attached_unit_closure_including(checked, entry, &[])
}

fn lower_attached_unit_closure_including(
    checked: &CheckedTrees,
    entry: psi_symbols::SymbolHandle,
    additional_roots: &[psi_symbols::SymbolHandle],
) -> Result<LoweredTerminalPsi, LoweringError> {
    let plans = &checked.facts.flow.terminal_unit_effects;
    let mut retained_roots = additional_roots.to_vec();
    let (closure, provider_candidate_plans) = loop {
        let closure = checked_unit_call_closure_including(checked, entry, &retained_roots)?;
        let candidates = checked_unit_provider_candidates(checked, &closure)?;
        let new_roots = candidates
            .iter()
            .map(|candidate| candidate.candidate)
            .filter(|candidate| !retained_roots.contains(candidate) && *candidate != entry)
            .collect::<Vec<_>>();
        if new_roots.is_empty() {
            break (closure, candidates);
        }
        retained_roots.extend(new_roots);
    };
    reject_recursive_unit_closure(plans, &closure)?;

    let mut boundaries = Vec::<(&CheckedBoundaryMachinePlan, String)>::new();
    for machine_symbol in &closure {
        let machine = unique_unit_machine(plans, *machine_symbol)?;
        if machine.contract_fingerprint == 0 {
            return unsupported("Unit closure contains a null checked contract fingerprint");
        }
        validate_unit_operation_sequence(machine)?;
        for operation in &machine.operations {
            match operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    target_machine,
                    target_state,
                    target_contract_fingerprint,
                    service_reach,
                    ..
                } => {
                    let target = unique_unit_machine(plans, *target_machine)?;
                    if target.state != *target_state
                        || target.contract_fingerprint != *target_contract_fingerprint
                        || !checked_unit_target_reach_matches(
                            *service_reach,
                            target.contract_service_reach,
                        )
                    {
                        return unsupported(
                            "Unit call does not match the exact checked target state, contract, and reach",
                        );
                    }
                }
                CheckedUnitEffectOperationPlan::BoundaryCall {
                    target_machine,
                    target_state,
                    target_contract_fingerprint,
                    service_reach,
                    ..
                } => {
                    let target = unique_unit_boundary(plans, *target_machine)?;
                    if target.contract_fingerprint == 0 {
                        return unsupported(
                            "Unit boundary target has a null checked contract fingerprint",
                        );
                    }
                    if target.state != *target_state
                        || target.contract_fingerprint != *target_contract_fingerprint
                        || !checked_unit_target_reach_matches(
                            *service_reach,
                            target.contract_service_reach,
                        )
                    {
                        return unsupported(
                            "boundary Unit call does not match the exact checked target state, contract, and reach",
                        );
                    }
                    if !boundaries
                        .iter()
                        .any(|(candidate, _)| candidate.machine == target.machine)
                    {
                        boundaries.push((
                            target,
                            checked_unit_boundary_identity(checked, target.machine)?,
                        ));
                    }
                }
                CheckedUnitEffectOperationPlan::PortWrite { .. }
                | CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal { .. }
                | CheckedUnitEffectOperationPlan::ReturnUnit { .. } => {}
            }
        }
    }
    boundaries.sort_by(|left, right| left.1.cmp(&right.1));
    if boundaries.windows(2).any(|pair| pair[0].1 == pair[1].1) {
        return unsupported("boundary Unit closure contains duplicate canonical identities");
    }

    let (structural_types, type_ids) = lower_unit_structural_types(checked, &closure, &boundaries)?;
    let (structural_domains, domain_ids) =
        lower_unit_structural_domains(checked, &closure, &boundaries, &type_ids)?;
    let (services, service_ids) =
        lower_unit_services(checked, &closure, &boundaries, &provider_candidate_plans)?;

    let mut next_place = 1_u64;
    let mut lowered_boundary_parameters = Vec::with_capacity(boundaries.len());
    let mut boundary_machines = Vec::with_capacity(boundaries.len());
    for (index, (plan, identity)) in boundaries.iter().enumerate() {
        let parameters = lower_unit_parameters(
            &plan.structural_parameters,
            &type_ids,
            &domain_ids,
            &mut next_place,
        )?;
        let mut requires = plan
            .domain_requirements
            .iter()
            .map(|requirement| {
                if usize::try_from(requirement.argument_index)
                    .ok()
                    .map_or(true, |index| index >= parameters.len())
                {
                    return Err(LoweringError::Unsupported(
                        "boundary structural requirement has an invalid argument index",
                    ));
                }
                Ok(StructuralDomainRequirement {
                    argument_index: requirement.argument_index,
                    domain: lookup_domain_id(&domain_ids, requirement.domain)?,
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        requires.sort();
        let original_requirement_count = requires.len();
        requires.dedup();
        if requires.len() != original_requirement_count {
            return unsupported("boundary structural requirements contain duplicates");
        }
        let published_service_ceiling = lower_published_service_ceiling(
            &checked.facts.service_reaches.rows,
            plan.contract_service_reach,
            plan.service_reach,
            &service_ids,
        )?;
        let id = boundary_machine_id(dense_identity(index)?);
        boundary_machines.push(BoundaryMachineDeclaration {
            id,
            identity: identity.clone(),
            attachment: plan
                .attachment_type_identity
                .as_ref()
                .map(|identity| lookup_type_id(&type_ids, identity))
                .transpose()?,
            structural_parameters: parameters.clone(),
            result: plan.result_type.map(terminal_scalar_type).transpose()?,
            requires,
            published_service_ceiling,
        });
        lowered_boundary_parameters.push((plan.machine, id, parameters));
    }

    let mut lowered_machine_parameters = Vec::with_capacity(closure.len());
    let mut lowered_claims = Vec::with_capacity(closure.len());
    for machine_symbol in &closure {
        let plan = unique_unit_machine(plans, *machine_symbol)?;
        if plan.body_qualifications.iter().any(|domain| {
            !plan
                .structural_parameters
                .iter()
                .any(|parameter| parameter.qualifications.contains(domain))
        }) {
            return unsupported(
                "Unit body qualification is not represented by an exact structural parameter precondition",
            );
        }
        let parameters = lower_unit_parameters(
            &plan.structural_parameters,
            &type_ids,
            &domain_ids,
            &mut next_place,
        )?;
        let mut claims = Vec::with_capacity(plan.entry_claims.len());
        let mut claim_bindings = Vec::with_capacity(plan.entry_claims.len());
        // ClaimId is machine-local; unrelated closure members must not shift
        // this machine's canonical claim namespace.
        let mut next_claim = 1_u64;
        for claim in &plan.entry_claims {
            if claim.carry != CarryPolicy::STRICT {
                return unsupported("Unit entry claim has a non-default carry policy");
            }
            let parameter = parameters
                .get(usize::try_from(claim.parameter_index).map_err(|_| {
                    LoweringError::Unsupported("Unit entry claim parameter index exceeds usize")
                })?)
                .ok_or(LoweringError::Unsupported(
                    "Unit entry claim has an invalid parameter index",
                ))?;
            let PermissionClaimIdentity::Established {
                machine_symbol,
                state_symbol,
                source: psi_language_semantics::PermissionEventSource::StateEntry,
                ..
            } = claim.claim_identity
            else {
                return unsupported("Unit entry claim is not an exact checked state-entry claim");
            };
            if machine_symbol != plan.machine || state_symbol != plan.state {
                return unsupported("Unit entry claim belongs to another checked state");
            }
            if claim_bindings
                .iter()
                .any(|(identity, _)| *identity == claim.claim_identity)
            {
                return unsupported("Unit entry claim identity is duplicated");
            }
            let id = claim_id(allocate_dense(&mut next_claim)?);
            claims.push(EntryClaim {
                claim: id,
                input: parameter.place,
                path: lower_structural_path(&claim.path),
            });
            claim_bindings.push((claim.claim_identity, id));
        }
        lowered_machine_parameters.push((*machine_symbol, parameters));
        lowered_claims.push((*machine_symbol, claims, claim_bindings));
    }

    let lowered_machine_runtime_requirements = closure
        .iter()
        .map(|machine_symbol| {
            let Some(contract) = checked.facts.contract_plans.for_machine(*machine_symbol) else {
                return Ok((*machine_symbol, Vec::new()));
            };
            let requirements = if contract.crash.uses_structural_proof_gated_arithmetic() {
                let checked_requirements = contract.crash.structural_runtime_requirements().ok_or(
                    LoweringError::Unsupported(
                        "proof-gated structural arithmetic lacks a complete checked requirement package",
                    ),
                )?;
                let parameters = lowered_machine_parameters
                    .iter()
                    .find_map(|(symbol, parameters)| {
                        (*symbol == *machine_symbol).then_some(parameters)
                    })
                    .expect("every closure machine has lowered parameters");
                let requirements = checked_requirements
                    .iter()
                    .map(|requirement| {
                        lower_structural_runtime_requirement(
                            requirement,
                            parameters,
                            &structural_types,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let mut keyed = requirements
                    .into_iter()
                    .map(|requirement| {
                        psi_terminal_codec::canonical_proposition_order_key(&requirement)
                            .map(|key| (key, requirement))
                            .map_err(|_| {
                                LoweringError::Unsupported(
                                    "structural runtime requirement is not canonically encodable",
                                )
                            })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                keyed.sort_by(|left, right| left.0.cmp(&right.0));
                keyed.dedup_by(|left, right| left.0 == right.0);
                keyed
                    .into_iter()
                    .map(|(_, requirement)| requirement)
                    .collect()
            } else {
                Vec::new()
            };
            Ok((*machine_symbol, requirements))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;

    let machine_ids = closure
        .iter()
        .enumerate()
        .map(|(index, symbol)| Ok((*symbol, machine_id(dense_identity(index)?))))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let mut next_operation = 1_u64;
    let mut next_edge = 1_u64;
    let mut next_block = 1_u64;
    let mut next_call_obligation = TERMINAL_UNIT_CALL_OBLIGATION_BASE;
    let mut call_evidence = Vec::new();
    let mut machines = Vec::with_capacity(closure.len());

    for machine_symbol in &closure {
        let plan = unique_unit_machine(plans, *machine_symbol)?;
        let terminal_machine = lookup_machine_id(&machine_ids, plan.machine)?;
        let parameters = lowered_machine_parameters
            .iter()
            .find_map(|(symbol, parameters)| (*symbol == plan.machine).then_some(parameters))
            .expect("every closure machine has lowered parameters");
        let runtime_requirements = lowered_machine_runtime_requirements
            .iter()
            .find_map(|(symbol, requirements)| (*symbol == plan.machine).then_some(requirements))
            .expect("every closure machine has lowered runtime requirements");
        let (_, entry_claims, claim_bindings) = lowered_claims
            .iter()
            .find(|(symbol, _, _)| *symbol == plan.machine)
            .expect("every closure machine has lowered entry claims");
        let local_places = plan
            .trivial_affine_locals
            .iter()
            .map(|local| {
                Ok(StructuralPlaceDeclaration {
                    id: place_id(allocate_dense(&mut next_place)?),
                    kind: StructuralPlaceKind::TrivialAffineLocal {
                        declaration_ordinal: local.declaration_ordinal,
                        structural_type: lookup_type_id(&type_ids, &local.type_identity)?,
                    },
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let mut operations = Vec::with_capacity(plan.operations.len().saturating_sub(1));
        for operation in &plan.operations[..plan.operations.len() - 1] {
            let kind = match operation {
                CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                    declaration_ordinal,
                    type_identity,
                    ..
                } => {
                    let local = local_places
                        .get(usize::try_from(*declaration_ordinal).map_err(|_| {
                            LoweringError::Unsupported("Unit local ordinal exceeds usize")
                        })?)
                        .ok_or(LoweringError::Unsupported(
                            "Unit local ordinal is not dense",
                        ))?;
                    if !matches!(
                        local.kind,
                        StructuralPlaceKind::TrivialAffineLocal {
                            declaration_ordinal: ordinal,
                            structural_type,
                        } if ordinal == *declaration_ordinal
                            && structural_type == lookup_type_id(&type_ids, type_identity)?
                    ) {
                        return unsupported("Unit local declaration drifted from checked custody");
                    }
                    OperationKind::EstablishTrivialAffineLocal {
                        destination: local.id,
                    }
                }
                CheckedUnitEffectOperationPlan::CallUnit {
                    target_machine,
                    structural_arguments,
                    claim_transfers,
                    ..
                } => {
                    let target = unique_unit_machine(plans, *target_machine)?;
                    validate_transfer_shape(
                        structural_arguments,
                        claim_transfers,
                        parameters,
                        &target.structural_parameters,
                        &type_ids,
                        &target
                            .entry_claims
                            .iter()
                            .map(|claim| claim.parameter_index)
                            .collect::<Vec<_>>(),
                    )?;
                    let terminal_arguments =
                        lower_structural_arguments(structural_arguments, parameters)?;
                    let target_parameters = lowered_machine_parameters
                        .iter()
                        .find_map(|(symbol, parameters)| {
                            (*symbol == *target_machine).then_some(parameters)
                        })
                        .expect("every closure target has lowered parameters");
                    let mut crash_continuations = if let Some(target_contract) =
                        checked.facts.contract_plans.for_machine(*target_machine)
                    {
                        lower_structural_crash_route_buckets(
                            target_contract.crash.published(),
                            target_parameters,
                            &structural_types,
                            lowered_machine_runtime_requirements
                                .iter()
                                .find_map(|(symbol, requirements)| {
                                    (*symbol == *target_machine).then_some(requirements.as_slice())
                                })
                                .expect("every closure target has lowered runtime requirements"),
                        )?
                    } else {
                        Vec::new()
                    };
                    let substitutions = target_parameters
                        .iter()
                        .zip(&terminal_arguments)
                        .map(|(parameter, argument)| {
                            Ok((
                                parameter.place,
                                (
                                    argument.place,
                                    structural_crash_route_argument_prefix(
                                        argument,
                                        parameters,
                                        &structural_types,
                                    )?,
                                ),
                            ))
                        })
                        .collect::<Result<BTreeMap<_, _>, LoweringError>>()?;
                    substitute_structural_crash_route_roots(
                        &mut crash_continuations,
                        &substitutions,
                    )?;
                    let target_runtime_requirements = lowered_machine_runtime_requirements
                        .iter()
                        .find_map(|(symbol, requirements)| {
                            (*symbol == *target_machine).then_some(requirements)
                        })
                        .expect("every closure target has lowered runtime requirements");
                    let requirement_obligations = target_runtime_requirements
                        .iter()
                        .map(|requirement| {
                            let mut goal = requirement.clone();
                            substitute_structural_requirement_roots(&mut goal, &substitutions)?;
                            let assumption_index = runtime_requirements
                                .iter()
                                .position(|assumption| assumption == &goal)
                                .ok_or(LoweringError::Unsupported(
                                    "runtime structural call requirement is not an exact caller premise",
                                ))?;
                            let obligation = obligation_id(next_call_obligation);
                            next_call_obligation = next_call_obligation
                                .checked_add(1)
                                .ok_or(LoweringError::Unsupported(
                                    "runtime structural call obligation identity space is exhausted",
                                ))?;
                            call_evidence.push(ObligationEvidence {
                                obligation,
                                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                                    identity: EvidenceIdentity::new(obligation.get())
                                        .expect("terminal obligation identity is nonzero"),
                                    proof_system_marker: ProofSystemMarker::CURRENT,
                                    proof: ProofNode {
                                        conclusion: goal,
                                        rule: ProofRule::Assumption {
                                            index: assumption_index,
                                        },
                                    },
                                }),
                            });
                            Ok(obligation)
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    OperationKind::CallUnit {
                        callee: lookup_machine_id(&machine_ids, *target_machine)?,
                        structural_arguments: terminal_arguments,
                        claim_transfers: claim_transfers
                            .iter()
                            .map(|transfer| {
                                Ok(ClaimTransfer {
                                    claim: lookup_claim_id(
                                        claim_bindings,
                                        transfer.claim_identity,
                                    )?,
                                    argument_index: transfer.argument_index,
                                })
                            })
                            .collect::<Result<Vec<_>, LoweringError>>()?,
                        requirement_obligations,
                        crash_continuations,
                    }
                }
                CheckedUnitEffectOperationPlan::BoundaryCall {
                    target_machine,
                    structural_arguments,
                    completion_receipts,
                    ..
                } => {
                    let target = unique_unit_boundary(plans, *target_machine)?;
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
                                        LoweringError::Unsupported(
                                            "boundary Unit argument index exceeds u32",
                                        )
                                    })
                                })
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    validate_transfer_shape(
                        structural_arguments,
                        completion_receipts,
                        parameters,
                        &target.structural_parameters,
                        &type_ids,
                        &expected_claim_arguments,
                    )?;
                    let (_, boundary, _) = lowered_boundary_parameters
                        .iter()
                        .find(|(symbol, _, _)| *symbol == *target_machine)
                        .ok_or(LoweringError::Unsupported(
                            "boundary Unit call target is absent from the lowered closure",
                        ))?;
                    OperationKind::BoundaryCall {
                        boundary: *boundary,
                        structural_arguments: lower_structural_arguments(
                            structural_arguments,
                            parameters,
                        )?,
                        completion_receipts: completion_receipts
                            .iter()
                            .map(|settlement| {
                                Ok(CompletionReceipt {
                                    claim: lookup_claim_id(
                                        claim_bindings,
                                        settlement.claim_identity,
                                    )?,
                                    argument_index: settlement.argument_index,
                                })
                            })
                            .collect::<Result<Vec<_>, LoweringError>>()?,
                        requirement_obligations: Vec::new(),
                    }
                }
                CheckedUnitEffectOperationPlan::PortWrite {
                    service_reach,
                    port,
                    value,
                    ..
                } => {
                    let direct = checked
                        .facts
                        .service_reaches
                        .rows
                        .services(service_reach.direct);
                    let [port_service] = direct else {
                        return unsupported(
                            "port output does not carry the unique exact checked PortIo service",
                        );
                    };
                    if !checked
                        .facts
                        .service_reaches
                        .rows
                        .services(service_reach.transitive)
                        .contains(port_service)
                    {
                        return unsupported(
                            "port output does not carry the unique exact checked PortIo service",
                        );
                    }
                    OperationKind::PortWrite {
                        // `CheckedUnitEffectOperationPlan::PortWrite` is minted only for the
                        // exact checked asm-port-out builtin. Its singleton direct row is
                        // therefore the symbol-backed PortIo authority; no spelling lookup is
                        // repeated here.
                        service: lookup_service_id(&service_ids, *port_service)?,
                        port: *port,
                        value: *value,
                    }
                }
                CheckedUnitEffectOperationPlan::ReturnUnit { .. } => {
                    return unsupported("Unit return is not the final checked operation");
                }
            };
            operations.push(Operation {
                id: operation_id(allocate_dense(&mut next_operation)?),
                result: psi_terminal::OperationResult::Unit,
                kind,
            });
        }
        let CheckedUnitEffectOperationPlan::ReturnUnit {
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
            ..
        } = plan.operations.last().expect("Unit sequence was validated")
        else {
            unreachable!()
        };
        let trivial_affine_discards = trivial_affine_local_discard_ordinals
            .iter()
            .map(|ordinal| {
                local_places
                    .get(usize::try_from(*ordinal).map_err(|_| {
                        LoweringError::Unsupported("Unit local cleanup ordinal exceeds usize")
                    })?)
                    .map(|local| local.id)
                    .ok_or(LoweringError::Unsupported(
                        "Unit local cleanup ordinal is not dense",
                    ))
            })
            .chain(trivial_affine_discards.iter().map(|parameter_index| {
                parameters
                    .get(usize::try_from(*parameter_index).map_err(|_| {
                        LoweringError::Unsupported(
                            "Unit affine discard parameter index exceeds usize",
                        )
                    })?)
                    .map(|parameter| parameter.place)
                    .ok_or(LoweringError::Unsupported(
                        "Unit affine discard has an invalid parameter index",
                    ))
            }))
            .collect::<Result<Vec<_>, _>>()?;
        let block = block_id(allocate_dense(&mut next_block)?);
        let edge = edge_id(allocate_dense(&mut next_edge)?);
        let crash_routes =
            if let Some(contract_plan) = checked.facts.contract_plans.for_machine(plan.machine) {
                lower_structural_crash_route_buckets(
                    contract_plan.crash.published(),
                    parameters,
                    &structural_types,
                    runtime_requirements,
                )?
            } else {
                Vec::new()
            };
        machines.push(TerminalMachine {
            id: terminal_machine,
            attachment: Some(lookup_type_id(&type_ids, &plan.attachment_type_identity)?),
            parameters: Vec::new(),
            structural_parameters: parameters.clone(),
            result: TerminalMachineResult::Unit,
            structural_places: parameters
                .iter()
                .map(|parameter| StructuralPlaceDeclaration {
                    id: parameter.place,
                    kind: StructuralPlaceKind::Parameter {
                        position: parameter.position,
                        is_self: parameter.is_self,
                    },
                })
                .chain(local_places.iter().cloned())
                .collect(),
            entry_claims: entry_claims.clone(),
            published_service_ceiling: if let Some(provider) = provider_candidate_plans
                .iter()
                .find(|candidate| candidate.candidate == plan.machine)
            {
                lower_provider_candidate_service_ceiling(
                    checked,
                    plans,
                    provider,
                    plan,
                    &service_ids,
                )?
            } else {
                lower_published_service_ceiling(
                    &checked.facts.service_reaches.rows,
                    plan.contract_service_reach,
                    plan.service_reach,
                    &service_ids,
                )?
            },
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block,
            blocks: vec![Block {
                id: block,
                parameters: Vec::new(),
                operations,
                terminator: Terminator::ReturnUnit {
                    edge,
                    trivial_affine_discards,
                },
            }],
            contract: MachineContract {
                id: contract_id(terminal_machine.get()),
                crash_routes,
                requires: runtime_requirements.clone(),
                ensures: Vec::new(),
            },
        });
    }

    let mut provider_candidates = provider_candidate_plans
        .iter()
        .map(|candidate| {
            let (_, boundary, parameters) = lowered_boundary_parameters
                .iter()
                .find(|(symbol, _, _)| *symbol == candidate.boundary)
                .ok_or(LoweringError::Unsupported(
                    "provider candidate references an unlowered Unit boundary requirement",
                ))?;
            let terminal_candidate = lookup_machine_id(&machine_ids, candidate.candidate)?;
            let realized = machines
                .iter()
                .find(|machine| machine.id == terminal_candidate)
                .expect("provider candidate root was lowered as an ordinary terminal machine");
            Ok(ProviderCandidateConformance {
                boundary: *boundary,
                requirement_identity: candidate.requirement_identity.clone(),
                provider_identity: candidate.provider_identity.clone(),
                candidate_identity: candidate.candidate_identity.clone(),
                candidate: terminal_candidate,
                signature: ProviderUnitSignature {
                    parameters: parameters
                        .iter()
                        .map(|parameter| ProviderSignatureParameter {
                            position: parameter.position,
                            is_self: parameter.is_self,
                            structural_type: parameter.structural_type,
                            multiplicity: parameter.multiplicity,
                            qualifications: parameter.qualifications.clone(),
                        })
                        .collect(),
                },
                refinement: ProviderUnitRefinement {
                    positional_parameters: (0..parameters.len())
                        .map(|index| {
                            let index = u32::try_from(index).map_err(|_| {
                                LoweringError::Unsupported("provider signature arity exceeds u32")
                            })?;
                            Ok(ProviderParameterRefinement {
                                boundary_index: index,
                                candidate_index: index,
                            })
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?,
                    required_domains: boundary_machines
                        .iter()
                        .find(|declaration| declaration.id == *boundary)
                        .expect("lowered provider boundary declaration exists")
                        .requires
                        .clone(),
                    realized_service_ceiling: realized.published_service_ceiling.clone(),
                },
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    provider_candidates.sort_by(|left, right| {
        (
            left.boundary,
            &left.provider_identity,
            &left.candidate_identity,
            left.candidate,
        )
            .cmp(&(
                right.boundary,
                &right.provider_identity,
                &right.candidate_identity,
                right.candidate,
            ))
    });

    Ok(LoweredTerminalPsi {
        semantic_module: TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine_id(1),
            structural_types,
            structural_domains,
            services,
            boundary_machines,
            provider_candidates,
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            machines,
        },
        proof_bundle: ProofBundle {
            evidence_producers: Vec::new(),
            evidence: call_evidence,
        },
        debug_map: None,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckedUnitProviderCandidate {
    boundary: psi_symbols::SymbolHandle,
    candidate: psi_symbols::SymbolHandle,
    requirement_identity: String,
    provider_identity: String,
    candidate_identity: String,
}

fn checked_unit_provider_candidates(
    checked: &CheckedTrees,
    closure: &[psi_symbols::SymbolHandle],
) -> Result<Vec<CheckedUnitProviderCandidate>, LoweringError> {
    let plans = &checked.facts.flow.terminal_unit_effects;
    let mut boundary_symbols = closure
        .iter()
        .flat_map(|symbol| {
            plans
                .for_machine(*symbol)
                .into_iter()
                .flat_map(|plan| &plan.operations)
        })
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::BoundaryCall { target_machine, .. } => {
                Some(*target_machine)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    boundary_symbols.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
    boundary_symbols.dedup();
    let mut output = Vec::new();
    for boundary_symbol in boundary_symbols {
        let boundary_plan =
            plans
                .boundary_for_machine(boundary_symbol)
                .ok_or(LoweringError::Unsupported(
                    "Unit provider catalog references an unknown checked boundary plan",
                ))?;
        let exact_requirements = checked
            .typed
            .traits()
            .iter()
            .filter(|definition| definition.is_boundary)
            .flat_map(|definition| {
                checked
                    .typed
                    .trait_machine_signatures(definition)
                    .iter()
                    .filter(move |signature| signature.symbol == boundary_symbol)
                    .map(move |signature| (definition, signature))
            })
            .collect::<Vec<_>>();
        let (definition, signature) = match exact_requirements.as_slice() {
            [] => continue,
            [(definition, signature)] => (*definition, *signature),
            _ => {
                return unsupported(
                    "Unit boundary provider catalog requires one exact trait/signature symbol coordinate",
                );
            }
        };
        if !boundary_plan.structural_parameters.is_empty()
            || !boundary_plan.domain_requirements.is_empty()
        {
            return unsupported(
                "checked provider dispatch currently admits only zero-argument Unit boundary requirements",
            );
        }
        let requirement_identity = checked
            .typed
            .normalized_trait_requirement_overload_identity(definition, signature)
            .identity();
        if requirement_identity.is_empty() {
            return unsupported("Unit boundary requirement has an empty overload identity");
        }
        for machine in checked.typed.machines().iter().filter(|machine| {
            machine.supply_mode == psi_language_semantics::MachineSupplyMode::CheckedBody
                && machine.attached_data.is_some()
                && checked
                    .typed
                    .machine_trait_conformances(machine)
                    .iter()
                    .any(|conformance| {
                        conformance.via.is_none()
                            && conformance.symbol == definition.symbol
                            && conformance
                                .requirement
                                .as_ref()
                                .is_some_and(|name| name == &signature.name)
                    })
        }) {
            plans
                .for_machine(machine.symbol)
                .ok_or(LoweringError::Unsupported(
                    "checked Unit provider candidate has no complete terminal body plan",
                ))?;
            output.push(CheckedUnitProviderCandidate {
                boundary: boundary_symbol,
                candidate: machine.symbol,
                requirement_identity: requirement_identity.clone(),
                provider_identity: machine
                    .attached_data
                    .as_ref()
                    .expect("candidate filter requires an attached provider type")
                    .as_str()
                    .to_owned(),
                candidate_identity: checked_terminal_machine_name(checked, machine.symbol)?
                    .to_owned(),
            });
        }
    }
    output.sort_by(|left, right| {
        (
            left.boundary.arena_index(),
            left.boundary.generation(),
            &left.provider_identity,
            left.candidate.arena_index(),
            left.candidate.generation(),
        )
            .cmp(&(
                right.boundary.arena_index(),
                right.boundary.generation(),
                &right.provider_identity,
                right.candidate.arena_index(),
                right.candidate.generation(),
            ))
    });
    if output.windows(2).any(|pair| {
        pair[0].boundary == pair[1].boundary
            && pair[0].provider_identity == pair[1].provider_identity
            && pair[0].candidate == pair[1].candidate
    }) {
        return unsupported("Unit provider catalog contains a duplicate exact candidate");
    }
    Ok(output)
}

fn checked_unit_call_closure_including(
    checked: &CheckedTrees,
    entry: psi_symbols::SymbolHandle,
    additional_roots: &[psi_symbols::SymbolHandle],
) -> Result<Vec<psi_symbols::SymbolHandle>, LoweringError> {
    let plans = &checked.facts.flow.terminal_unit_effects;
    let mut closure = vec![entry];
    for root in additional_roots {
        if closure.contains(root) {
            return unsupported("attached Unit closure contains a duplicate explicit root");
        }
        closure.push(*root);
    }
    let mut next = 0_usize;
    while let Some(machine_symbol) = closure.get(next).copied() {
        next += 1;
        checked_terminal_machine_name(checked, machine_symbol)?;
        let machine = unique_unit_machine(plans, machine_symbol)?;
        for target in machine
            .operations
            .iter()
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit { target_machine, .. } => {
                    Some(*target_machine)
                }
                _ => None,
            })
        {
            if !closure.contains(&target) {
                closure.push(target);
            }
        }
    }
    Ok(closure)
}

fn unique_unit_machine(
    plans: &psi_checked_trees::CheckedUnitEffectPlans,
    symbol: psi_symbols::SymbolHandle,
) -> Result<&CheckedUnitEffectMachinePlan, LoweringError> {
    let mut matches = plans.machines.iter().filter(|plan| plan.machine == symbol);
    let plan = matches.next().ok_or(LoweringError::Unsupported(
        "attached Unit closure is missing a checked transitive machine plan",
    ))?;
    if matches.next().is_some() {
        return unsupported("attached Unit closure contains duplicate checked machine plans");
    }
    Ok(plan)
}

fn unique_unit_boundary(
    plans: &psi_checked_trees::CheckedUnitEffectPlans,
    symbol: psi_symbols::SymbolHandle,
) -> Result<&CheckedBoundaryMachinePlan, LoweringError> {
    let mut matches = plans
        .boundary_machines
        .iter()
        .filter(|plan| plan.machine == symbol);
    let plan = matches.next().ok_or(LoweringError::Unsupported(
        "attached Unit closure is missing a checked boundary machine plan",
    ))?;
    if matches.next().is_some() {
        return unsupported("attached Unit closure contains duplicate boundary machine plans");
    }
    Ok(plan)
}

fn checked_terminal_machine_name(
    checked: &CheckedTrees,
    symbol: psi_symbols::SymbolHandle,
) -> Result<&str, LoweringError> {
    let mut matches = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .filter(|selection| selection.machine == symbol);
    let selection = matches.next().ok_or(LoweringError::Unsupported(
        "attached Unit member has no checked terminal selection",
    ))?;
    if matches.next().is_some()
        || selection.signature != CheckedTerminalSignatureEligibility::Attached
        || selection.name.is_empty()
    {
        return unsupported("attached Unit member has an invalid checked terminal selection");
    }
    Ok(&selection.name)
}

fn checked_unit_boundary_identity(
    checked: &CheckedTrees,
    symbol: psi_symbols::SymbolHandle,
) -> Result<String, LoweringError> {
    let requirements = checked
        .typed
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .flat_map(|definition| {
            checked
                .typed
                .trait_machine_signatures(definition)
                .iter()
                .filter(move |signature| signature.symbol == symbol)
                .map(move |signature| (definition, signature))
        })
        .collect::<Vec<_>>();
    if let [(definition, signature)] = requirements.as_slice() {
        let identity = checked
            .typed
            .normalized_trait_requirement_overload_identity(definition, signature)
            .identity();
        if !identity.is_empty() {
            return Ok(identity);
        }
    }
    checked_terminal_machine_name(checked, symbol).map(str::to_owned)
}

fn validate_unit_operation_sequence(
    machine: &CheckedUnitEffectMachinePlan,
) -> Result<(), LoweringError> {
    let Some(CheckedUnitEffectOperationPlan::ReturnUnit {
        statement_index, ..
    }) = machine.operations.last()
    else {
        return unsupported("Unit machine does not end in exactly one checked Unit return");
    };
    let mut previous = None;
    for operation in &machine.operations[..machine.operations.len() - 1] {
        let coordinate = match operation {
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index,
                declaration_ordinal,
                ..
            } => psi_checked_trees::CheckedUnitCallCoordinate {
                statement_index: *statement_index,
                call_ordinal: *declaration_ordinal,
            },
            CheckedUnitEffectOperationPlan::CallUnit { coordinate, .. }
            | CheckedUnitEffectOperationPlan::BoundaryCall { coordinate, .. }
            | CheckedUnitEffectOperationPlan::PortWrite { coordinate, .. } => *coordinate,
            CheckedUnitEffectOperationPlan::ReturnUnit { .. } => {
                return unsupported("Unit machine contains a nonfinal Unit return");
            }
        };
        let key = (coordinate.statement_index, coordinate.call_ordinal);
        if previous.is_some_and(|previous| previous >= key)
            || coordinate.statement_index >= *statement_index
        {
            return unsupported("Unit machine operation order is not canonical source order");
        }
        previous = Some(key);
    }
    Ok(())
}

fn reject_recursive_unit_closure(
    plans: &psi_checked_trees::CheckedUnitEffectPlans,
    closure: &[psi_symbols::SymbolHandle],
) -> Result<(), LoweringError> {
    fn visit(
        plans: &psi_checked_trees::CheckedUnitEffectPlans,
        symbol: psi_symbols::SymbolHandle,
        active: &mut Vec<psi_symbols::SymbolHandle>,
        complete: &mut Vec<psi_symbols::SymbolHandle>,
    ) -> Result<(), LoweringError> {
        if active.contains(&symbol) {
            return unsupported("recursive Unit call closure is not yet terminal");
        }
        if complete.contains(&symbol) {
            return Ok(());
        }
        active.push(symbol);
        for target in unique_unit_machine(plans, symbol)?
            .operations
            .iter()
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit { target_machine, .. } => {
                    Some(*target_machine)
                }
                _ => None,
            })
        {
            visit(plans, target, active, complete)?;
        }
        active.pop();
        complete.push(symbol);
        Ok(())
    }

    let mut active = Vec::new();
    let mut complete = Vec::new();
    for symbol in closure {
        visit(plans, *symbol, &mut active, &mut complete)?;
    }
    Ok(())
}

fn lower_unit_structural_types(
    checked: &CheckedTrees,
    closure: &[psi_symbols::SymbolHandle],
    boundaries: &[(&CheckedBoundaryMachinePlan, String)],
) -> Result<
    (
        Vec<StructuralTypeDeclaration>,
        Vec<(String, StructuralTypeId)>,
    ),
    LoweringError,
> {
    fn collect(
        plans: &psi_checked_trees::CheckedUnitEffectPlans,
        identity: &str,
        active: &mut Vec<String>,
        selected: &mut Vec<String>,
    ) -> Result<(), LoweringError> {
        if active.iter().any(|candidate| candidate == identity) {
            return unsupported("recursive structural type is outside the Unit terminal slice");
        }
        if selected.iter().any(|candidate| candidate == identity) {
            return Ok(());
        }
        let mut matches = plans
            .structural_types
            .iter()
            .filter(|plan| plan.identity == identity);
        let plan = matches.next().ok_or(LoweringError::Unsupported(
            "Unit closure references a missing structural type",
        ))?;
        if matches.next().is_some() || identity.is_empty() {
            return unsupported(
                "Unit closure contains a duplicate or empty structural type identity",
            );
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
            } => {
                collect(plans, element_type_identity, active, selected)?;
            }
        }
        active.pop();
        selected.push(identity.to_owned());
        Ok(())
    }

    let plans = &checked.facts.flow.terminal_unit_effects;
    let mut selected = Vec::new();
    let mut active = Vec::new();
    for symbol in closure {
        let machine = unique_unit_machine(plans, *symbol)?;
        collect(
            plans,
            &machine.attachment_type_identity,
            &mut active,
            &mut selected,
        )?;
        for parameter in &machine.structural_parameters {
            collect(plans, &parameter.type_identity, &mut active, &mut selected)?;
        }
        for local in &machine.trivial_affine_locals {
            collect(plans, &local.type_identity, &mut active, &mut selected)?;
        }
    }
    for (boundary, _) in boundaries {
        if let Some(identity) = &boundary.attachment_type_identity {
            collect(plans, identity, &mut active, &mut selected)?;
        }
        for parameter in &boundary.structural_parameters {
            collect(plans, &parameter.type_identity, &mut active, &mut selected)?;
        }
    }
    selected.sort();
    selected.dedup();
    let type_ids = selected
        .iter()
        .enumerate()
        .map(|(index, identity)| Ok((identity.clone(), structural_type_id(dense_identity(index)?))))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let mut next_field = 1_u64;
    let mut declarations = Vec::with_capacity(selected.len());
    for identity in selected {
        let plan = plans
            .structural_types
            .iter()
            .find(|plan| plan.identity == identity)
            .expect("selected structural type was validated above");
        let shape = match &plan.shape {
            CheckedUnitStructuralTypeShape::Record { fields } => {
                let mut field_identities = BTreeSet::new();
                let fields = fields.iter().map(|field| {
                if field.identity.is_empty() || !field_identities.insert(field.identity.as_str()) {
                    return Err(LoweringError::Unsupported(
                        "Unit structural type contains an empty or duplicate field identity",
                    ));
                }
                let field_type = match &field.field_type {
                    CheckedUnitStructuralFieldType::Scalar(primitive) => {
                        StructuralFieldType::Scalar(terminal_scalar_type(*primitive)?)
                    }
                    CheckedUnitStructuralFieldType::Structural { type_identity } => {
                        StructuralFieldType::Structural(lookup_type_id(&type_ids, type_identity)?)
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
        declarations.push(StructuralTypeDeclaration {
            id: lookup_type_id(&type_ids, &identity)?,
            identity,
            shape,
        });
    }
    Ok((declarations, type_ids))
}

fn lower_unit_structural_domains(
    checked: &CheckedTrees,
    closure: &[psi_symbols::SymbolHandle],
    boundaries: &[(&CheckedBoundaryMachinePlan, String)],
    type_ids: &[(String, StructuralTypeId)],
) -> Result<
    (
        Vec<StructuralDomainDeclaration>,
        Vec<(psi_language_semantics::SemanticDomainId, StructuralDomainId)>,
    ),
    LoweringError,
> {
    let plans = &checked.facts.flow.terminal_unit_effects;
    let mut selected = Vec::new();
    for symbol in closure {
        let machine = unique_unit_machine(plans, *symbol)?;
        for domain in machine
            .structural_parameters
            .iter()
            .flat_map(|parameter| &parameter.qualifications)
            .chain(&machine.body_qualifications)
        {
            if !selected.contains(domain) {
                selected.push(*domain);
            }
        }
    }
    for (boundary, _) in boundaries {
        for domain in boundary
            .structural_parameters
            .iter()
            .flat_map(|parameter| &parameter.qualifications)
            .chain(
                boundary
                    .domain_requirements
                    .iter()
                    .map(|requirement| &requirement.domain),
            )
        {
            if !selected.contains(domain) {
                selected.push(*domain);
            }
        }
    }
    let mut selected_plans = selected
        .into_iter()
        .map(|domain| {
            let mut matches = plans
                .structural_domains
                .iter()
                .filter(|plan| plan.domain == domain);
            let plan = matches.next().ok_or(LoweringError::Unsupported(
                "Unit closure references a missing structural domain",
            ))?;
            if matches.next().is_some()
                || !domain.is_valid()
                || plan.identity.is_empty()
                || plan.carrier_type_identity.is_empty()
            {
                return Err(LoweringError::Unsupported(
                    "Unit structural domain is duplicate, null, or incomplete",
                ));
            }
            Ok(plan)
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    selected_plans.sort_by(|left, right| left.identity.cmp(&right.identity));
    if selected_plans
        .windows(2)
        .any(|pair| pair[0].identity == pair[1].identity)
    {
        return unsupported("Unit structural domains have duplicate canonical identities");
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

fn lower_unit_services(
    checked: &CheckedTrees,
    closure: &[psi_symbols::SymbolHandle],
    boundaries: &[(&CheckedBoundaryMachinePlan, String)],
    provider_candidates: &[CheckedUnitProviderCandidate],
) -> Result<(Vec<ServiceDeclaration>, Vec<(ServiceReachId, ServiceId)>), LoweringError> {
    let facts = &checked.facts.service_reaches;
    let plans = &checked.facts.flow.terminal_unit_effects;
    let mut selected = Vec::<ServiceReachId>::new();
    for symbol in closure {
        let machine = unique_unit_machine(plans, *symbol)?;
        if let Some(provider) = provider_candidates
            .iter()
            .find(|candidate| candidate.candidate == *symbol)
        {
            collect_provider_candidate_services(
                &facts.rows,
                plans,
                provider,
                machine,
                &mut selected,
            )?;
        } else {
            collect_contract_services(
                &facts.rows,
                machine.contract_service_reach,
                machine.service_reach,
                &mut selected,
            )?;
        }
        for operation in &machine.operations {
            match operation {
                CheckedUnitEffectOperationPlan::CallUnit { service_reach, .. }
                | CheckedUnitEffectOperationPlan::BoundaryCall { service_reach, .. }
                | CheckedUnitEffectOperationPlan::PortWrite { service_reach, .. } => {
                    collect_service_summary(&facts.rows, *service_reach, &mut selected)?;
                }
                CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal { .. }
                | CheckedUnitEffectOperationPlan::ReturnUnit { .. } => {}
            }
        }
    }
    for (boundary, _) in boundaries {
        collect_contract_services(
            &facts.rows,
            boundary.contract_service_reach,
            boundary.service_reach,
            &mut selected,
        )?;
    }
    let mut next = 0_usize;
    while let Some(service) = selected.get(next).copied() {
        next += 1;
        let definition = facts
            .services
            .definition(service)
            .ok_or(LoweringError::Unsupported(
                "Unit closure references an unknown checked service",
            ))?;
        for parent in &definition.parents {
            if !selected.contains(parent) {
                selected.push(*parent);
            }
        }
    }
    let mut selected_definitions = selected
        .iter()
        .map(|service| {
            facts
                .services
                .definition(*service)
                .map(|definition| (*service, definition))
                .ok_or(LoweringError::Unsupported(
                    "Unit closure references an unknown checked service",
                ))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    selected_definitions.sort_by(|left, right| left.1.name.cmp(&right.1.name));
    if selected_definitions
        .iter()
        .any(|(_, definition)| definition.name.is_empty())
        || selected_definitions
            .windows(2)
            .any(|pair| pair[0].1.name == pair[1].1.name)
    {
        return unsupported("Unit services have empty or duplicate canonical identities");
    }
    let service_ids = selected_definitions
        .iter()
        .enumerate()
        .map(|(index, (source, _))| Ok((*source, service_id(dense_identity(index)?))))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let declarations = selected_definitions
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

fn collect_contract_services(
    rows: &psi_language_semantics::ServiceReachRowTable,
    contract: ServiceReachPlan,
    summary: ServiceReachSummary,
    selected: &mut Vec<ServiceReachId>,
) -> Result<(), LoweringError> {
    collect_service_summary(rows, summary, selected)?;
    if contract.checked_inferred != summary.transitive {
        return unsupported(
            "Unit contract reach does not match the exact checked transitive reach",
        );
    }
    let published = match contract.interface {
        ServiceReachInterface::PublishedCeiling(row) => row,
        ServiceReachInterface::InternalInferred => {
            if rows.services(summary.transitive).is_empty() {
                return Ok(());
            }
            return unsupported("effectful Unit machine has no published service ceiling");
        }
    };
    require_valid_service_row(published)?;
    let ceiling = rows.services(published);
    if rows
        .services(summary.transitive)
        .iter()
        .any(|service| !ceiling.contains(service))
    {
        return unsupported("checked Unit service reach exceeds its published ceiling");
    }
    for service in ceiling {
        if !selected.contains(service) {
            selected.push(*service);
        }
    }
    Ok(())
}

fn collect_provider_candidate_services(
    rows: &psi_language_semantics::ServiceReachRowTable,
    plans: &psi_checked_trees::CheckedUnitEffectPlans,
    provider: &CheckedUnitProviderCandidate,
    candidate: &CheckedUnitEffectMachinePlan,
    selected: &mut Vec<ServiceReachId>,
) -> Result<(), LoweringError> {
    collect_service_summary(rows, candidate.service_reach, selected)?;
    if candidate.contract_service_reach.checked_inferred != candidate.service_reach.transitive {
        return unsupported(
            "checked provider adapter contract reach does not match its transitive reach",
        );
    }
    let boundary = unique_unit_boundary(plans, provider.boundary)?;
    let ceiling = match boundary.contract_service_reach.interface {
        ServiceReachInterface::PublishedCeiling(row) => row,
        ServiceReachInterface::InternalInferred => {
            return unsupported("checked provider boundary has no published service ceiling");
        }
    };
    if rows
        .services(candidate.service_reach.transitive)
        .iter()
        .any(|service| !rows.services(ceiling).contains(service))
    {
        return unsupported("checked provider adapter reach exceeds its boundary requirement");
    }
    Ok(())
}

fn lower_provider_candidate_service_ceiling(
    checked: &CheckedTrees,
    plans: &psi_checked_trees::CheckedUnitEffectPlans,
    provider: &CheckedUnitProviderCandidate,
    candidate: &CheckedUnitEffectMachinePlan,
    service_ids: &[(ServiceReachId, ServiceId)],
) -> Result<Vec<ServiceId>, LoweringError> {
    let rows = &checked.facts.service_reaches.rows;
    let mut selected = Vec::new();
    collect_provider_candidate_services(rows, plans, provider, candidate, &mut selected)?;
    let source = rows.services(candidate.service_reach.transitive);
    let mut lowered = source
        .iter()
        .map(|service| lookup_service_id(service_ids, *service))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    lowered.sort();
    lowered.dedup();
    if lowered.len() != source.len() {
        return unsupported("checked provider adapter reach contains duplicates");
    }
    Ok(lowered)
}

fn checked_unit_target_reach_matches(
    call: ServiceReachSummary,
    target_contract: ServiceReachPlan,
) -> bool {
    let expected = match target_contract.interface {
        ServiceReachInterface::PublishedCeiling(row) => row,
        ServiceReachInterface::InternalInferred => target_contract.checked_inferred,
    };
    call.transitive == expected
}

fn collect_service_summary(
    rows: &psi_language_semantics::ServiceReachRowTable,
    summary: ServiceReachSummary,
    selected: &mut Vec<ServiceReachId>,
) -> Result<(), LoweringError> {
    require_valid_service_row(summary.direct)?;
    require_valid_service_row(summary.transitive)?;
    let transitive = rows.services(summary.transitive);
    if rows
        .services(summary.direct)
        .iter()
        .any(|service| !transitive.contains(service))
    {
        return unsupported("Unit direct service reach is not contained in transitive reach");
    }
    for service in rows.services(summary.direct).iter().chain(transitive) {
        if !selected.contains(service) {
            selected.push(*service);
        }
    }
    Ok(())
}

fn require_valid_service_row(row: ServiceReachRowId) -> Result<(), LoweringError> {
    if row.is_valid() {
        Ok(())
    } else {
        unsupported("Unit closure contains a null checked service row")
    }
}

fn lower_unit_parameters(
    parameters: &[psi_checked_trees::CheckedUnitStructuralParameterPlan],
    type_ids: &[(String, StructuralTypeId)],
    domain_ids: &[(psi_language_semantics::SemanticDomainId, StructuralDomainId)],
    next_place: &mut u64,
) -> Result<Vec<StructuralParameterDeclaration>, LoweringError> {
    let mut positions = BTreeSet::new();
    parameters
        .iter()
        .enumerate()
        .map(|(dense_position, parameter)| {
            if !positions.insert(parameter.position) {
                return Err(LoweringError::Unsupported(
                    "Unit structural parameters contain duplicate source positions",
                ));
            }
            let mut qualifications = parameter
                .qualifications
                .iter()
                .map(|domain| lookup_domain_id(domain_ids, *domain))
                .collect::<Result<Vec<_>, LoweringError>>()?;
            qualifications.sort();
            qualifications.dedup();
            if qualifications.len() != parameter.qualifications.len() {
                return Err(LoweringError::Unsupported(
                    "Unit structural parameter repeats a qualification",
                ));
            }
            Ok(StructuralParameterDeclaration {
                place: place_id(allocate_dense(next_place)?),
                position: u32::try_from(dense_position).map_err(|_| {
                    LoweringError::Unsupported("Unit structural parameter count exceeds u32")
                })?,
                is_self: parameter.is_self,
                structural_type: lookup_type_id(type_ids, &parameter.type_identity)?,
                multiplicity: match parameter.multiplicity {
                    Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
                    Multiplicity::Affine => StructuralMultiplicity::Affine,
                    Multiplicity::Linear => StructuralMultiplicity::Linear,
                },
                qualifications,
            })
        })
        .collect()
}

fn lower_published_service_ceiling(
    rows: &psi_language_semantics::ServiceReachRowTable,
    contract: ServiceReachPlan,
    summary: ServiceReachSummary,
    service_ids: &[(ServiceReachId, ServiceId)],
) -> Result<Vec<ServiceId>, LoweringError> {
    if contract.checked_inferred != summary.transitive {
        return unsupported("Unit contract reach does not match checked transitive reach");
    }
    let source = match contract.interface {
        ServiceReachInterface::PublishedCeiling(row) => {
            require_valid_service_row(row)?;
            rows.services(row)
        }
        ServiceReachInterface::InternalInferred if rows.services(summary.transitive).is_empty() => {
            &[]
        }
        ServiceReachInterface::InternalInferred => {
            return unsupported("effectful Unit machine has no published service ceiling");
        }
    };
    let mut lowered = source
        .iter()
        .map(|service| lookup_service_id(service_ids, *service))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    lowered.sort();
    lowered.dedup();
    if lowered.len() != source.len() {
        return unsupported("Unit published service ceiling contains duplicates");
    }
    Ok(lowered)
}

fn validate_transfer_shape(
    arguments: &[psi_checked_trees::CheckedUnitStructuralArgumentPlan],
    transfers: &[psi_checked_trees::CheckedUnitClaimTransferPlan],
    caller_parameters: &[StructuralParameterDeclaration],
    target_parameters: &[psi_checked_trees::CheckedUnitStructuralParameterPlan],
    type_ids: &[(String, StructuralTypeId)],
    expected_claim_arguments: &[u32],
) -> Result<(), LoweringError> {
    if arguments.len() != target_parameters.len() {
        return unsupported(
            "Unit call structural argument arity does not match its checked target",
        );
    }
    for (argument, target) in arguments.iter().zip(target_parameters) {
        let source = caller_parameters
            .get(
                usize::try_from(argument.source_parameter_index).map_err(|_| {
                    LoweringError::Unsupported("Unit structural argument index exceeds usize")
                })?,
            )
            .ok_or(LoweringError::Unsupported(
                "Unit structural argument has an invalid caller parameter index",
            ))?;
        if argument.type_identity != target.type_identity
            || (argument.path.is_empty()
                && source.structural_type != lookup_type_id(type_ids, &argument.type_identity)?)
        {
            return unsupported("Unit structural argument type identity is inconsistent");
        }
    }
    let actual = transfers
        .iter()
        .map(|transfer| transfer.argument_index)
        .collect::<Vec<_>>();
    if actual != expected_claim_arguments
        || actual.iter().any(|index| {
            usize::try_from(*index)
                .ok()
                .map_or(true, |index| index >= arguments.len())
        })
    {
        return unsupported("Unit claim transfer does not exactly match target entry custody");
    }
    Ok(())
}

fn lower_structural_arguments(
    arguments: &[psi_checked_trees::CheckedUnitStructuralArgumentPlan],
    parameters: &[StructuralParameterDeclaration],
) -> Result<Vec<StructuralArgument>, LoweringError> {
    arguments
        .iter()
        .map(|argument| {
            let parameter = parameters
                .get(
                    usize::try_from(argument.source_parameter_index).map_err(|_| {
                        LoweringError::Unsupported("Unit structural argument index exceeds usize")
                    })?,
                )
                .ok_or(LoweringError::Unsupported(
                    "Unit structural argument has an invalid caller parameter index",
                ))?;
            Ok(StructuralArgument {
                place: parameter.place,
                path: lower_structural_path(&argument.path),
            })
        })
        .collect()
}

fn lower_structural_path(path: &[CheckedUnitStructuralPathSegment]) -> Vec<StructuralPathSegment> {
    path.iter()
        .map(|segment| match segment {
            CheckedUnitStructuralPathSegment::Field(identity) => {
                StructuralPathSegment::Field(identity.clone())
            }
            CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                StructuralPathSegment::FixedIndex(*index)
            }
        })
        .collect()
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

fn shared_boolean_runtime_parameters(
    expression: &LoweredBooleanReturnExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    match expression {
        LoweredBooleanReturnExpression::Constant { .. } => Some(BTreeSet::new()),
        LoweredBooleanReturnExpression::Parameter { position } => {
            Some(BTreeSet::from([SharedBooleanRuntimeInput::BooleanScalar(
                *position,
            )]))
        }
        LoweredBooleanReturnExpression::StructuralField { source, field } => {
            Some(BTreeSet::from([
                SharedBooleanRuntimeInput::StructuralField {
                    source: *source,
                    field: *field,
                },
            ]))
        }
        LoweredBooleanReturnExpression::Not { operand } => {
            shared_boolean_runtime_parameters(operand)
        }
        LoweredBooleanReturnExpression::And { left, right }
        | LoweredBooleanReturnExpression::Or { left, right } => {
            let mut parameters = shared_boolean_runtime_parameters(left)?;
            parameters.extend(shared_boolean_runtime_parameters(right)?);
            Some(parameters)
        }
        LoweredBooleanReturnExpression::IntegerComparison { left, right, .. } => {
            let mut parameters = shared_integer_runtime_parameters(left)?;
            parameters.extend(shared_integer_runtime_parameters(right)?);
            Some(parameters)
        }
        LoweredBooleanReturnExpression::Local { .. }
        | LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. }
        | LoweredBooleanReturnExpression::Equal { .. } => None,
    }
}

fn shared_integer_runtime_parameters(
    expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    shared_integer_runtime_parameters_with_shells(expression, 1)
}

fn shared_integer_runtime_parameters_with_shells(
    expression: &LoweredDirectExpression,
    remaining_shells: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    match expression {
        LoweredDirectExpression::IntegerLiteral { .. } => Some(BTreeSet::new()),
        LoweredDirectExpression::Parameter {
            position,
            scalar_type,
        } => matches!(scalar_type, ScalarType::Integer(_))
            .then(|| BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(*position)])),
        LoweredDirectExpression::IntegerBinary {
            kind:
                LoweredIntegerBinaryKind::BitwiseAnd
                | LoweredIntegerBinaryKind::BitwiseOr
                | LoweredIntegerBinaryKind::BitwiseXor
                | LoweredIntegerBinaryKind::WrappingShiftLeft
                | LoweredIntegerBinaryKind::WrappingShiftRight
                | LoweredIntegerBinaryKind::WrappingAdd
                | LoweredIntegerBinaryKind::SaturatingAdd
                | LoweredIntegerBinaryKind::WrappingSubtract
                | LoweredIntegerBinaryKind::SaturatingSubtract
                | LoweredIntegerBinaryKind::WrappingMultiply
                | LoweredIntegerBinaryKind::SaturatingMultiply,
            left,
            right,
            ..
        } if remaining_shells > 0 => {
            let mut parameters =
                shared_integer_runtime_parameters_with_shells(left, remaining_shells - 1)?;
            parameters.extend(shared_integer_runtime_parameters_with_shells(
                right,
                remaining_shells - 1,
            )?);
            Some(parameters)
        }
        LoweredDirectExpression::IntegerBitwiseNot { operand, .. } if remaining_shells > 0 => {
            shared_integer_runtime_parameters_with_shells(operand, remaining_shells - 1)
        }
        LoweredDirectExpression::IntegerWiden { operand, .. } if remaining_shells > 0 => {
            shared_integer_runtime_parameters_with_shells(operand, remaining_shells - 1)
        }
        LoweredDirectExpression::Local { .. }
        | LoweredDirectExpression::IntegerBinary { .. }
        | LoweredDirectExpression::IntegerBitwiseNot { .. }
        | LoweredDirectExpression::IntegerWiden { .. }
        | LoweredDirectExpression::IntegerExactCast { .. }
        | LoweredDirectExpression::Boolean { .. } => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SharedBooleanRuntimeInput {
    BooleanScalar(usize),
    IntegerScalar(usize),
    StructuralField {
        source: PlaceId,
        field: StructuralFieldId,
    },
}

fn valid_shared_boolean_runtime_inputs(inputs: &BTreeSet<SharedBooleanRuntimeInput>) -> bool {
    let has_structural_field = inputs
        .iter()
        .any(|input| matches!(input, SharedBooleanRuntimeInput::StructuralField { .. }));
    !inputs.is_empty()
        && (!has_structural_field
            || (inputs
                .iter()
                .any(|input| matches!(input, SharedBooleanRuntimeInput::BooleanScalar(_)))
                && !inputs
                    .iter()
                    .any(|input| matches!(input, SharedBooleanRuntimeInput::IntegerScalar(_)))))
}

fn resolve_shared_boolean_member_fields(
    expression: LoweredBooleanReturnExpression,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<LoweredBooleanReturnExpression, LoweringError> {
    Ok(match expression {
        LoweredBooleanReturnExpression::UnresolvedStructuralParameterField {
            parameter_position,
            path,
        } => {
            let [field_name] = path.as_slice() else {
                return unsupported(
                    "shared Boolean convergence admits only one direct structural field",
                );
            };
            let parameter = parameters
                .iter()
                .find(|parameter| parameter.position == parameter_position)
                .filter(|parameter| {
                    parameter.multiplicity == StructuralMultiplicity::Affine
                        && parameter.qualifications.is_empty()
                })
                .ok_or(LoweringError::Unsupported(
                    "shared Boolean member source is not one claim-free affine parameter",
                ))?;
            let declaration = structural_types
                .iter()
                .find(|declaration| declaration.id == parameter.structural_type)
                .ok_or(LoweringError::Unsupported(
                    "shared Boolean member source type is absent",
                ))?;
            let StructuralTypeShape::Record { fields } = &declaration.shape else {
                return unsupported("shared Boolean member source is not a record");
            };
            let field = fields
                .iter()
                .find(|field| field.identity == *field_name)
                .filter(|field| {
                    !field.relevance.is_erased()
                        && field.field_type == StructuralFieldType::Scalar(ScalarType::Boolean)
                })
                .ok_or(LoweringError::Unsupported(
                    "shared Boolean member is absent, erased, or non-Boolean",
                ))?;
            LoweredBooleanReturnExpression::StructuralField {
                source: parameter.place,
                field: field.id,
            }
        }
        LoweredBooleanReturnExpression::Not { operand } => LoweredBooleanReturnExpression::Not {
            operand: Box::new(resolve_shared_boolean_member_fields(
                *operand,
                parameters,
                structural_types,
            )?),
        },
        LoweredBooleanReturnExpression::Equal { left, right } => {
            LoweredBooleanReturnExpression::Equal {
                left: Box::new(resolve_shared_boolean_member_fields(
                    *left,
                    parameters,
                    structural_types,
                )?),
                right: Box::new(resolve_shared_boolean_member_fields(
                    *right,
                    parameters,
                    structural_types,
                )?),
            }
        }
        LoweredBooleanReturnExpression::And { left, right } => {
            LoweredBooleanReturnExpression::And {
                left: Box::new(resolve_shared_boolean_member_fields(
                    *left,
                    parameters,
                    structural_types,
                )?),
                right: Box::new(resolve_shared_boolean_member_fields(
                    *right,
                    parameters,
                    structural_types,
                )?),
            }
        }
        LoweredBooleanReturnExpression::Or { left, right } => LoweredBooleanReturnExpression::Or {
            left: Box::new(resolve_shared_boolean_member_fields(
                *left,
                parameters,
                structural_types,
            )?),
            right: Box::new(resolve_shared_boolean_member_fields(
                *right,
                parameters,
                structural_types,
            )?),
        },
        expression => expression,
    })
}

/// Normalize the comparison leaves accepted by the checked shared-convergence
/// plan into the existing identity/negation carrier. Boolean equality is
/// admitted only when at least one operand is constant. Checked integer
/// comparisons retain their exact operation and bounded total-computation
/// operands. The one already-resolved structural-field leaf is preserved
/// unchanged.
fn normalize_shared_boolean_comparison_leaves(
    expression: &LoweredBooleanReturnExpression,
) -> Option<LoweredBooleanReturnExpression> {
    Some(match expression {
        LoweredBooleanReturnExpression::Constant { .. }
        | LoweredBooleanReturnExpression::Parameter { .. }
        | LoweredBooleanReturnExpression::StructuralField { .. }
        | LoweredBooleanReturnExpression::IntegerComparison { .. } => expression.clone(),
        LoweredBooleanReturnExpression::Not { operand } => LoweredBooleanReturnExpression::Not {
            operand: Box::new(normalize_shared_boolean_comparison_leaves(operand)?),
        },
        LoweredBooleanReturnExpression::Equal { left, right } => {
            let left = normalize_shared_boolean_comparison_leaves(left)?;
            let right = normalize_shared_boolean_comparison_leaves(right)?;
            match (left, right) {
                (
                    LoweredBooleanReturnExpression::Constant { value: left },
                    LoweredBooleanReturnExpression::Constant { value: right },
                ) => LoweredBooleanReturnExpression::Constant {
                    value: left == right,
                },
                (LoweredBooleanReturnExpression::Constant { value: true }, expression)
                | (expression, LoweredBooleanReturnExpression::Constant { value: true }) => {
                    expression
                }
                (LoweredBooleanReturnExpression::Constant { value: false }, expression)
                | (expression, LoweredBooleanReturnExpression::Constant { value: false }) => {
                    LoweredBooleanReturnExpression::Not {
                        operand: Box::new(expression),
                    }
                }
                _ => return None,
            }
        }
        LoweredBooleanReturnExpression::And { left, right } => {
            LoweredBooleanReturnExpression::And {
                left: Box::new(normalize_shared_boolean_comparison_leaves(left)?),
                right: Box::new(normalize_shared_boolean_comparison_leaves(right)?),
            }
        }
        LoweredBooleanReturnExpression::Or { left, right } => LoweredBooleanReturnExpression::Or {
            left: Box::new(normalize_shared_boolean_comparison_leaves(left)?),
            right: Box::new(normalize_shared_boolean_comparison_leaves(right)?),
        },
        LoweredBooleanReturnExpression::Local { .. }
        | LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. } => return None,
    })
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
        let proof = proof_from_semantic_axioms(&site.obligation.proposition, &site.semantic_axioms);
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

fn proof_from_semantic_axioms(
    goal: &Proposition,
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
        .map(|conjunct| proof_from_semantic_axioms(conjunct, semantic_axioms))
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
