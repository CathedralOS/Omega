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
    CheckedOperatorResolutionStatus, CheckedTrees, ContentIdentityReshuffleFact,
    ContentPartitionCompositionFact,
    expression::{BinaryOperator, ExpressionNode, UnaryOperator},
    signature::SignatureContractKind,
    statement::{StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode},
    types::PrimitiveType,
};
use psi_core::{
    BlockId, ClaimId, ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentDomainId,
    ContentPlaceSegment, ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace,
    ContentTerm, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
    ObligationId, OperationId, PlaceId, Proposition, PropositionContext, PropositionError,
    PropositionId, ScalarTerm, ScalarType, StructuralPlaceKind, ValueId,
};
use psi_language_semantics::PermissionClaimIdentity;
use psi_language_semantics::content::{
    ContentAlgebraIdentity as CheckedContentAlgebraIdentity, ContentConservationEquation,
    ContentConservationOwnerKind, ContentConservationPlan,
    ContentConservationTerm as CheckedContentConservationTerm,
    ContentPlaceRoot as CheckedContentPlaceRoot, ContentPlaceSegment as CheckedContentPlaceSegment,
    ContentPlaceVersion as CheckedContentPlaceVersion,
    ContentStructuralPlace as CheckedContentStructuralPlace, conservation_fingerprint,
};
use psi_numerics::arithmetic::ArithmeticDomain;
use psi_proof_kernel::{EvidenceRoute, PrimitiveJudgment};
use psi_terminal::{
    Block, ClaimContentProjection, ContentEntryClaim, ContentIdentityReshuffle,
    ContentPartitionComposition, ContentPlaceSubstitution, ContractClause,
    CrashCause as TerminalCrashCause, MachineContract, Operation, OperationKind,
    PropositionApplicationIdentity, PropositionBinderArgumentIdentity,
    PropositionBinderArgumentKind, PropositionBinderDeclaration, PropositionBinderKind,
    PropositionDeclaration, PropositionEvidence, SemanticVersion, StructuralPlaceDeclaration,
    SuccessorEdge, TerminalMachine, TerminalModule, Terminator, ValueDeclaration,
};
use psi_terminal_codec::{
    DebugFileId, DebugSite, DebugSourceFile, DebugSourceOrigin, DebugSourceSpan, DebugSubject,
    TerminalDebugMap, source_digest, terminal_psi_identity, validate_debug_map,
};
use psi_terminal_verifier::{ObligationEvidence, ProofBundle};
use psi_typed_trees::domain::ProofFact;

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
    Boolean {
        expression: Box<LoweredBooleanReturnExpression>,
    },
}

impl LoweredDirectExpression {
    const fn scalar_type(&self) -> ScalarType {
        match self {
            Self::Parameter { scalar_type, .. }
            | Self::IntegerLiteral { scalar_type, .. }
            | Self::IntegerBinary { scalar_type, .. } => *scalar_type,
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
    WrappingAdd,
    SaturatingAdd,
    WrappingSubtract,
    SaturatingSubtract,
    WrappingMultiply,
    SaturatingMultiply,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LoweredIntegerBranchTerminator {
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
    damage_minimum: String,
    containment_demand: String,
    frontier_lower_bound: Vec<ClaimId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoweredIntegerBranchState {
    parameter_types: Vec<ScalarType>,
    terminator: LoweredIntegerBranchTerminator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LoweredBooleanBranchTerminator {
    Jump {
        target: usize,
        arguments: Vec<LoweredBooleanReturnExpression>,
    },
    Conditional {
        condition: LoweredBooleanReturnExpression,
        when_true_target: usize,
        when_true_arguments: Vec<LoweredBooleanReturnExpression>,
        when_false_target: usize,
        when_false_arguments: Vec<LoweredBooleanReturnExpression>,
    },
    Return {
        expression: LoweredBooleanReturnExpression,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoweredBooleanBranchState {
    parameter_count: usize,
    terminator: LoweredBooleanBranchTerminator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingBooleanValueBlocks {
    first_id: BlockId,
    parameters: Vec<ValueDeclaration>,
    decision: LoweredBooleanDecision,
    exit: LoweredBooleanDecisionExit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingBooleanTupleBindingBlocks {
    first_id: BlockId,
    original_parameter_count: usize,
    arguments: Vec<LoweredBooleanReturnExpression>,
    stage_parameters: Vec<Vec<ValueDeclaration>>,
    target: BlockId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingBooleanDirectBindingBlock {
    id: BlockId,
    parameters: Vec<ValueDeclaration>,
    target: BlockId,
    arguments: Vec<LoweredBooleanReturnExpression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PendingBooleanBlockGroup {
    Guard(PendingShortCircuitGuardBlocks),
    Value(PendingBooleanValueBlocks),
    TupleBinding(PendingBooleanTupleBindingBlocks),
    DirectBinding(PendingBooleanDirectBindingBlock),
}

impl PendingBooleanBlockGroup {
    fn first_id(&self) -> BlockId {
        match self {
            Self::Guard(blocks) => blocks.first_id,
            Self::Value(blocks) => blocks.first_id,
            Self::TupleBinding(blocks) => blocks.first_id,
            Self::DirectBinding(block) => block.id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingConditionalBindingBlock {
    id: BlockId,
    parameters: Vec<ValueDeclaration>,
    target: usize,
    arguments: Vec<LoweredDirectExpression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingShortCircuitGuardBlocks {
    first_id: BlockId,
    parameters: Vec<ValueDeclaration>,
    decision: LoweredBooleanDecision,
    when_true: LoweredBooleanDecisionTarget,
    when_false: LoweredBooleanDecisionTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PendingNestedBlockGroup {
    ConditionalBinding(PendingConditionalBindingBlock),
    ShortCircuitGuard(PendingShortCircuitGuardBlocks),
}

impl PendingNestedBlockGroup {
    fn first_id(&self) -> BlockId {
        match self {
            Self::ConditionalBinding(block) => block.id,
            Self::ShortCircuitGuard(blocks) => blocks.first_id,
        }
    }
}

impl LoweredIntegerBinaryKind {
    fn operation(self, left: ValueId, right: ValueId) -> OperationKind {
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

/// Lower a validated checked-tree content equation into the terminal-Psi v9
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

/// Lower checker-proved direct partition composition into terminal-Psi v12.
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

/// Lower one named checked free machine through the first terminal-Psi slice.
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
///     state next(v0: integer, v1: integer, ...) -> integer {
///         transition { _ -> done(E0, E1, ...) }
///     }
///     state done(v0: integer, v1: integer, ...) -> integer { E }
/// }
/// ```
///
/// The first explicit-crash slice also accepts a one-state scalar machine whose
/// sole statement is `crash Cause;` and whose checked site cites exactly one
/// prechecked guard-and-damage-covering bucket. It emits a distinct terminal-
/// Psi crash terminator; it never reuses ordinary return lowering.
pub fn lower_machine(
    checked: &CheckedTrees,
    machine_name: &str,
) -> Result<LoweredTerminalPsi, LoweringError> {
    lower_machine_with_crash_context(
        checked,
        machine_name,
        psi_terminal::CrashContextMaximum::portable_root(),
    )
}

/// Lower one checked machine under an already selected portable crash-context
/// plan. This is the provider/Build composition seam for a narrower activation,
/// task, or supervisor context; ordinary artifact-root lowering uses
/// [`lower_machine`] and supplies `ExecutionDomain` for both closed causes.
pub fn lower_machine_with_crash_context(
    checked: &CheckedTrees,
    machine_name: &str,
    crash_context: Vec<psi_terminal::CrashContextMaximum>,
) -> Result<LoweredTerminalPsi, LoweringError> {
    let mut matches = checked
        .machines()
        .iter()
        .filter(|machine| machine.name.as_str() == machine_name);
    let machine = matches
        .next()
        .ok_or_else(|| LoweringError::MachineNotFound(machine_name.to_owned()))?;
    if matches.next().is_some() {
        return Err(LoweringError::AmbiguousMachineName(machine_name.to_owned()));
    }
    let mut lowered = lower_selected_machine(checked, machine)?;
    let (declarations, applications) = lower_proposition_vocabulary(checked);
    lowered.semantic_module.proposition_declarations = declarations;
    lowered.semantic_module.proposition_applications = applications;
    for machine in &mut lowered.semantic_module.machines {
        machine.contract.crash_context = crash_context.clone();
    }
    psi_terminal_verifier::validate_module(&lowered.semantic_module)
        .map_err(LoweringError::InvalidTerminalModule)?;
    lowered.debug_map = Some(build_debug_map(checked, machine, &lowered.semantic_module)?);
    Ok(lowered)
}

fn lower_proposition_vocabulary(
    checked: &CheckedTrees,
) -> (
    Vec<PropositionDeclaration>,
    Vec<PropositionApplicationIdentity>,
) {
    let placeholder = proposition_id(1);
    let mut declarations = checked
        .propositions()
        .iter()
        .filter_map(|declaration| {
            let evidence = match declaration.body {
                psi_typed_trees::proposition::PropositionBody::Primitive => {
                    PropositionEvidence::FactOnly
                }
                psi_typed_trees::proposition::PropositionBody::Witness { evidence } => {
                    PropositionEvidence::Witness {
                        evidence_type: checked.display_type_reference(evidence),
                    }
                }
                psi_typed_trees::proposition::PropositionBody::Transparent { .. } => return None,
            };
            let binders = checked
                .proposition_binders(declaration)
                .iter()
                .map(|binder| PropositionBinderDeclaration {
                    name: binder.name.as_str().to_owned(),
                    kind: match binder.kind {
                        psi_typed_trees::proposition::PropositionBinderKind::Type => {
                            PropositionBinderKind::Type
                        }
                        psi_typed_trees::proposition::PropositionBinderKind::Const {
                            type_reference,
                        } => PropositionBinderKind::Const {
                            type_identity: checked.display_type_reference(type_reference),
                        },
                        psi_typed_trees::proposition::PropositionBinderKind::Machine => {
                            PropositionBinderKind::Machine
                        }
                    },
                })
                .collect();
            let parameter_types = checked
                .proposition_parameters(declaration)
                .iter()
                .map(|parameter| checked.display_type_reference(parameter.type_reference))
                .collect();
            Some((
                declaration.symbol,
                PropositionDeclaration {
                    id: placeholder,
                    name: declaration.name.as_str().to_owned(),
                    binders,
                    parameter_types,
                    evidence,
                },
            ))
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

    let mut applications = checked
        .proof_facts
        .iter()
        .filter_map(|(_, fact)| {
            let ProofFact::Proposition(application) = fact else {
                return None;
            };
            let normalized = checked.normalize_nominal_proposition_application(application)?;
            let declaration = declaration_ids
                .iter()
                .find_map(|(symbol, id)| (*symbol == normalized.declaration).then_some(*id))?;
            Some(PropositionApplicationIdentity {
                id: placeholder,
                declaration,
                binder_arguments: normalized
                    .binder_arguments
                    .into_iter()
                    .map(|argument| PropositionBinderArgumentIdentity {
                        kind: match argument.kind {
                            psi_typed_trees::proposition::PropositionBinderArgumentKind::Type => {
                                PropositionBinderArgumentKind::Type
                            }
                            psi_typed_trees::proposition::PropositionBinderArgumentKind::Const => {
                                PropositionBinderArgumentKind::Const
                            }
                            psi_typed_trees::proposition::PropositionBinderArgumentKind::Machine => {
                                PropositionBinderArgumentKind::Machine
                            }
                        },
                        identity: argument.identity,
                    })
                    .collect(),
                arguments: normalized.arguments,
            })
        })
        .collect::<Vec<_>>();
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
    (
        declarations
            .into_iter()
            .map(|(_, declaration)| declaration)
            .collect(),
        applications,
    )
}

fn lower_selected_machine(
    checked: &CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
) -> Result<LoweredTerminalPsi, LoweringError> {
    if machine.attached_data.is_some() {
        return unsupported("attached machines are not in the first terminal-Psi source slice");
    }
    if !machine.type_parameters.is_empty()
        || !machine.owned_data.is_empty()
        || !machine.satisfies.is_empty()
        || !machine.decreases.is_empty()
        || !machine.decrease_view_arguments.is_empty()
        || machine.decrease_range.is_valid()
        || !machine.service_reaches.is_empty()
        || !machine.invokes.is_empty()
        || machine.suspends
        || machine.blocks
        || machine.boundary
    {
        return unsupported("machine signature is outside the first terminal-Psi source slice");
    }

    let states = checked.machine_states(machine);
    if let [entry_state] = states
        && is_explicit_crash_state(checked, entry_state)
    {
        return lower_explicit_crash_machine(checked, machine, entry_state);
    }
    if let [entry_state] = states {
        return match checked.primitive_type_reference(entry_state.return_type) {
            Some(PrimitiveType::Bool) => lower_boolean_machine(checked, machine, entry_state),
            _ => lower_direct_parameter_machine(checked, machine, entry_state),
        };
    }
    if states.len() == 3
        && entry_has_ordered_boolean_conditional(checked, &states[0])
        && !states[1..]
            .iter()
            .any(|state| is_explicit_crash_state(checked, state))
    {
        return match checked.primitive_type_reference(states[0].return_type) {
            Some(PrimitiveType::Bool) => {
                lower_boolean_conditional_machine(checked, machine, states)
            }
            _ => lower_integer_conditional_machine(checked, machine, states),
        };
    }
    if states.len() >= 2
        && states
            .iter()
            .any(|state| entry_has_ordered_boolean_conditional(checked, state))
    {
        return match checked.primitive_type_reference(states[0].return_type) {
            Some(PrimitiveType::Bool) => {
                lower_nested_boolean_branch_machine(checked, machine, states)
            }
            _ => lower_nested_integer_branch_machine(checked, machine, states),
        };
    }
    if states.len() >= 2
        && checked.primitive_type_reference(states[0].return_type) == Some(PrimitiveType::Bool)
    {
        return if states[1..]
            .iter()
            .all(|state| checked.state_parameters(state).len() == 1)
        {
            lower_boolean_state_chain(checked, machine, states)
        } else {
            lower_nested_boolean_branch_machine(checked, machine, states)
        };
    }
    if states.len() >= 2 {
        return lower_integer_state_chain(checked, machine, states);
    }
    unsupported("machine must contain at least one state")
}

fn is_explicit_crash_state(
    checked: &CheckedTrees,
    state: &psi_checked_trees::state::State,
) -> bool {
    matches!(
        checked.statement_table.statements(state.statement_nodes),
        [StatementNode::Transition(transition)]
            if matches!(transition.exit, TransitionExit::Crash(_))
                && transition.guard == TransitionGuardNode::Always
                && !transition.continuation.is_valid()
                && matches!(
                    checked.statement_table.transition_target(transition.target),
                    TransitionTargetNode::Terminal
                )
    )
}

fn lower_explicit_crash_machine(
    checked: &CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
    entry_state: &psi_checked_trees::state::State,
) -> Result<LoweredTerminalPsi, LoweringError> {
    if !checked.state_contracts(entry_state).is_empty() {
        return unsupported("crash-only state contracts are not supported");
    }
    let parameters = checked.state_parameters(entry_state);
    if parameters
        .iter()
        .any(|parameter| parameter.is_self || parameter.is_const || parameter.is_mutable)
    {
        return unsupported("crash-only parameters must be ordinary scalar values");
    }
    let parameter_types = parameters
        .iter()
        .map(|parameter| {
            terminal_scalar_type(
                checked
                    .primitive_type_reference(parameter.type_reference)
                    .ok_or(LoweringError::Unsupported(
                        "crash-only parameters must be primitive Boolean or integer values",
                    ))?,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let result_type = terminal_scalar_type(
        checked
            .primitive_type_reference(entry_state.return_type)
            .ok_or(LoweringError::Unsupported(
                "crash-only machine result must be a primitive Boolean or integer",
            ))?,
    )?;
    let [StatementNode::Transition(_transition)] = checked
        .statement_table
        .statements(entry_state.statement_nodes)
    else {
        unreachable!("crash-only source shape was selected above")
    };
    let crash = lower_checked_crash_exit(checked, machine, entry_state, 0, &[])?;

    let terminal_parameters = parameter_types
        .iter()
        .enumerate()
        .map(|(index, scalar_type)| ValueDeclaration {
            id: value_id(
                u64::try_from(index)
                    .expect("parameter index fits a semantic identity")
                    .checked_add(1)
                    .expect("parameter identity is nonzero"),
            ),
            scalar_type: *scalar_type,
        })
        .collect::<Vec<_>>();
    let result = ValueDeclaration {
        id: value_id(
            u64::try_from(parameter_types.len())
                .expect("parameter count fits a semantic identity")
                .checked_add(1)
                .expect("result identity follows parameter identities"),
        ),
        scalar_type: result_type,
    };
    Ok(LoweredTerminalPsi {
        semantic_module: TerminalModule {
            semantic_version: SemanticVersion::CURRENT,
            entry: machine_id(1),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            machines: vec![TerminalMachine {
                id: machine_id(1),
                parameters: terminal_parameters,
                result,
                structural_places: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(1),
                blocks: vec![Block {
                    id: block_id(1),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Crash {
                        edge: edge_id(1),
                        cause: crash.cause,
                        damage_minimum: crash.damage_minimum,
                        containment_demand: crash.containment_demand,
                        frontier_lower_bound: crash.frontier_lower_bound,
                    },
                }],
                contract: MachineContract {
                    id: contract_id(1),
                    crash_context: psi_terminal::CrashContextMaximum::portable_root(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                },
            }],
        },
        proof_bundle: ProofBundle {
            evidence: Vec::new(),
        },
        debug_map: None,
    })
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

fn lower_checked_crash_exit(
    checked: &CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
    state: &psi_checked_trees::state::State,
    statement_ordinal: u32,
    source_claims: &[(PermissionClaimIdentity, ClaimId)],
) -> Result<LoweredCrashExit, LoweringError> {
    let Some(crash_plan) = checked
        .facts
        .contract_plans
        .for_machine(machine.symbol)
        .map(|contract| &contract.crash)
    else {
        return unsupported("explicit crash has no checked machine-contract plan");
    };
    let Some(checked_site) = crash_plan.checked_site_at(state.symbol, statement_ordinal) else {
        return unsupported("explicit crash has no body-derived checked crash-site row");
    };
    let matching_contracts = crash_plan
        .covering_buckets_for_site(checked_site)
        .map(|(_, bucket)| bucket)
        .collect::<Vec<_>>();
    let [covering_bucket] = matching_contracts.as_slice() else {
        return unsupported(
            "an explicit crash in the terminal-Psi source slice requires exactly one prechecked covering bucket",
        );
    };
    Ok(LoweredCrashExit {
        cause: match checked_site.cause() {
            psi_checked_trees::CrashCause::Trap => TerminalCrashCause::Trap,
            psi_checked_trees::CrashCause::Abort => TerminalCrashCause::Abort,
        },
        damage_minimum: checked_site.damage_minimum().to_owned(),
        containment_demand: covering_bucket.containment_demand().to_owned(),
        frontier_lower_bound: lower_checked_crash_frontier(
            checked_site.frontier_lower_bound(),
            source_claims,
        )?,
    })
}

fn lower_boolean_conditional_machine(
    checked: &CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
    states: &[psi_checked_trees::state::State],
) -> Result<LoweredTerminalPsi, LoweringError> {
    let [entry, when_true_state, when_false_state] = states else {
        unreachable!("conditional source shape requires exactly three states")
    };
    if states
        .iter()
        .any(|state| !checked.state_contracts(state).is_empty())
    {
        return unsupported("conditional state contracts are not supported");
    }
    let entry_parameters = checked.state_parameters(entry);
    if entry_parameters.iter().any(|parameter| {
        parameter.is_self
            || parameter.is_const
            || parameter.is_mutable
            || checked.primitive_type_reference(parameter.type_reference)
                != Some(PrimitiveType::Bool)
    }) {
        return unsupported("Boolean conditional parameters must be ordinary Boolean values");
    }
    let statements = checked.statement_table.statements(entry.statement_nodes);
    let [
        StatementNode::Transition(when_true),
        StatementNode::Transition(when_false),
    ] = statements
    else {
        unreachable!("conditional source shape was selected above")
    };
    let TransitionGuardNode::When(condition) = when_true.guard else {
        unreachable!("conditional source shape requires a guarded first arm")
    };
    if when_true.continuation.is_valid() || when_false.continuation.is_valid() {
        return unsupported("conditional transitions cannot carry continuations");
    }
    let condition = lower_positive_boolean_guard(checked, condition, entry_parameters)?;
    validate_short_circuit_expression(&condition)?;

    let mut branch_parameter_counts = Vec::with_capacity(2);
    let mut branch_expressions = Vec::with_capacity(2);
    for state in [when_true_state, when_false_state] {
        if checked.primitive_type_reference(state.return_type) != Some(PrimitiveType::Bool) {
            return unsupported("Boolean conditional branch results must remain Boolean");
        }
        let parameters = checked.state_parameters(state);
        if parameters.iter().any(|parameter| {
            parameter.is_self
                || parameter.is_const
                || parameter.is_mutable
                || checked.primitive_type_reference(parameter.type_reference)
                    != Some(PrimitiveType::Bool)
        }) {
            return unsupported(
                "Boolean conditional branch parameters must be ordinary Boolean values",
            );
        }
        let [StatementNode::Expression(return_expression)] =
            checked.statement_table.statements(state.statement_nodes)
        else {
            return unsupported("Boolean conditional branch must contain one value expression");
        };
        let branch_expression = lower_boolean_expression(checked, *return_expression, parameters)?;
        validate_short_circuit_expression(&branch_expression)?;
        branch_parameter_counts.push(parameters.len());
        branch_expressions.push(branch_expression);
    }

    let branch_arguments = |transition: &psi_checked_trees::statement::TableTransition,
                            expected_state: &psi_checked_trees::state::State,
                            expected_count: usize|
     -> Result<Vec<usize>, LoweringError> {
        let TransitionTargetNode::Named { path, arguments } =
            checked.statement_table.transition_target(transition.target)
        else {
            return unsupported("conditional successors must target named states");
        };
        if path.symbol != expected_state.symbol {
            return unsupported("conditional successors must follow declared true/false order");
        }
        let arguments = checked.statement_table.expression_handles(*arguments);
        if arguments.len() != expected_count {
            return unsupported(
                "conditional successor bindings must match the target parameter count",
            );
        }
        arguments
            .iter()
            .map(|argument| {
                let ExpressionNode::Name(path) = checked.expression_table.expression(*argument)
                else {
                    return unsupported(
                        "Boolean conditional bindings require already-defined parameters",
                    );
                };
                direct_parameter_position(checked, path, entry_parameters)
            })
            .collect()
    };
    let when_true_arguments =
        branch_arguments(when_true, when_true_state, branch_parameter_counts[0])?;
    let when_false_arguments =
        branch_arguments(when_false, when_false_state, branch_parameter_counts[1])?;
    let [when_true_expression, when_false_expression]: [LoweredBooleanReturnExpression; 2] =
        branch_expressions
            .try_into()
            .expect("two Boolean branches each lower one expression");
    let contract_value = validate_boolean_contract(checked, machine, None)?;
    let (identity_reshuffles, partition_compositions) =
        lower_content_evidence(checked, machine, entry)?;
    Ok(build_boolean_conditional_module(
        entry_parameters.len(),
        condition,
        when_true_arguments,
        when_false_arguments,
        branch_parameter_counts[0],
        branch_parameter_counts[1],
        when_true_expression,
        when_false_expression,
        contract_value,
        identity_reshuffles,
        partition_compositions,
    ))
}

fn entry_has_ordered_boolean_conditional(
    checked: &CheckedTrees,
    entry: &psi_checked_trees::state::State,
) -> bool {
    matches!(
        checked.statement_table.statements(entry.statement_nodes),
        [
            StatementNode::Transition(first),
            StatementNode::Transition(second)
        ] if matches!(first.guard, TransitionGuardNode::When(_))
            && second.guard == TransitionGuardNode::Always
    )
}

fn lower_integer_conditional_machine(
    checked: &CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
    states: &[psi_checked_trees::state::State],
) -> Result<LoweredTerminalPsi, LoweringError> {
    let [entry, when_true_state, when_false_state] = states else {
        unreachable!("conditional source shape requires exactly three states")
    };
    if states
        .iter()
        .any(|state| !checked.state_contracts(state).is_empty())
    {
        return unsupported("conditional state contracts are not supported");
    }
    let entry_parameters = checked.state_parameters(entry);
    if entry_parameters
        .iter()
        .any(|parameter| parameter.is_self || parameter.is_const || parameter.is_mutable)
    {
        return unsupported("qualified conditional machine parameters are not supported");
    }
    let parameter_types = entry_parameters
        .iter()
        .map(|parameter| {
            terminal_scalar_type(
                checked
                    .primitive_type_reference(parameter.type_reference)
                    .ok_or(LoweringError::Unsupported(
                        "conditional parameters must be primitive Boolean or integer values",
                    ))?,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let result_type =
        integer_scalar_type(checked.primitive_type_reference(entry.return_type).ok_or(
            LoweringError::Unsupported("conditional result must be a primitive integer"),
        )?)?;

    let statements = checked.statement_table.statements(entry.statement_nodes);
    let [
        StatementNode::Transition(when_true),
        StatementNode::Transition(when_false),
    ] = statements
    else {
        unreachable!("conditional source shape was selected above")
    };
    let TransitionGuardNode::When(condition) = when_true.guard else {
        unreachable!("conditional source shape requires a guarded first arm")
    };
    if when_true.continuation.is_valid() || when_false.continuation.is_valid() {
        return unsupported("conditional transitions cannot carry continuations");
    }
    let condition = lower_positive_boolean_guard(checked, condition, entry_parameters)?;
    if let LoweredBooleanReturnExpression::Parameter { position } = condition
        && parameter_types[position] != ScalarType::Boolean
    {
        return unsupported("conditional guard parameter must be Boolean");
    }

    let mut branch_expressions = Vec::with_capacity(2);
    let mut branch_parameter_types = Vec::with_capacity(2);
    for state in [when_true_state, when_false_state] {
        if integer_scalar_type(checked.primitive_type_reference(state.return_type).ok_or(
            LoweringError::Unsupported("conditional branch result must be a primitive integer"),
        )?)? != result_type
        {
            return unsupported("conditional branch result types must match exactly");
        }
        let parameters = checked.state_parameters(state);
        if parameters
            .iter()
            .any(|parameter| parameter.is_self || parameter.is_const || parameter.is_mutable)
        {
            return unsupported("qualified conditional branch parameters are not supported");
        }
        let state_parameter_types = parameters
            .iter()
            .map(|parameter| {
                integer_scalar_type(
                    checked
                        .primitive_type_reference(parameter.type_reference)
                        .ok_or(LoweringError::Unsupported(
                            "conditional branch parameters must be primitive integers",
                        ))?,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let [StatementNode::Expression(return_expression)] =
            checked.statement_table.statements(state.statement_nodes)
        else {
            return unsupported("conditional branch state must contain one integer expression");
        };
        let (expression, _) = lower_direct_return_expression(
            checked,
            *return_expression,
            parameters,
            &state_parameter_types,
            result_type,
        )?;
        branch_parameter_types.push(state_parameter_types);
        branch_expressions.push(expression);
    }
    let branch_arguments = |transition: &psi_checked_trees::statement::TableTransition,
                            expected_state: &psi_checked_trees::state::State,
                            expected_types: &[ScalarType]|
     -> Result<Vec<usize>, LoweringError> {
        let TransitionTargetNode::Named { path, arguments } =
            checked.statement_table.transition_target(transition.target)
        else {
            return unsupported("conditional successors must target named states");
        };
        if path.symbol != expected_state.symbol {
            return unsupported("conditional successors must follow declared true/false order");
        }
        let arguments = checked.statement_table.expression_handles(*arguments);
        if arguments.len() != expected_types.len() {
            return unsupported(
                "conditional successor bindings must match the target parameter count",
            );
        }
        arguments
                .iter()
                .zip(expected_types)
                .map(|(argument, expected_type)| {
                    let ExpressionNode::Name(path) =
                        checked.expression_table.expression(*argument)
                    else {
                        return unsupported(
                            "conditional successor bindings currently require already-defined parameters",
                        );
                    };
                    let position = direct_parameter_position(checked, path, entry_parameters)?;
                    if parameter_types[position] != *expected_type {
                        return unsupported(
                            "conditional successor argument must match its target parameter type",
                        );
                    }
                    Ok(position)
                })
                .collect()
    };
    let when_true_arguments =
        branch_arguments(when_true, when_true_state, &branch_parameter_types[0])?;
    let when_false_arguments =
        branch_arguments(when_false, when_false_state, &branch_parameter_types[1])?;
    let [when_true_expression, when_false_expression]: [LoweredDirectExpression; 2] =
        branch_expressions
            .try_into()
            .expect("the two conditional branch states each lower one expression");

    let contract_value = validate_contract(checked, machine, result_type, None, false)?;
    let (identity_reshuffles, partition_compositions) =
        lower_content_evidence(checked, machine, entry)?;
    Ok(build_integer_conditional_module(
        &parameter_types,
        condition,
        when_true_arguments,
        when_false_arguments,
        branch_parameter_types[0].clone(),
        branch_parameter_types[1].clone(),
        when_true_expression,
        when_false_expression,
        result_type,
        contract_value,
        identity_reshuffles,
        partition_compositions,
    ))
}

fn lower_nested_boolean_branch_machine(
    checked: &CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
    states: &[psi_checked_trees::state::State],
) -> Result<LoweredTerminalPsi, LoweringError> {
    if states
        .iter()
        .any(|state| !checked.state_contracts(state).is_empty())
    {
        return unsupported("nested Boolean state contracts are not supported");
    }
    let mut lowered_states = Vec::with_capacity(states.len());
    let mut successors = vec![Vec::new(); states.len()];
    let mut indegree = vec![0usize; states.len()];

    for (state_index, state) in states.iter().enumerate() {
        if checked.primitive_type_reference(state.return_type) != Some(PrimitiveType::Bool) {
            return unsupported("nested Boolean state results must remain Boolean");
        }
        let parameters = checked.state_parameters(state);
        if parameters.iter().any(|parameter| {
            parameter.is_self
                || parameter.is_const
                || parameter.is_mutable
                || checked.primitive_type_reference(parameter.type_reference)
                    != Some(PrimitiveType::Bool)
        }) {
            return unsupported("nested Boolean parameters must be ordinary Boolean values");
        }
        let statements = checked.statement_table.statements(state.statement_nodes);
        let terminator = match statements {
            [StatementNode::Expression(return_expression)] => {
                let expression = lower_boolean_expression(checked, *return_expression, parameters)?;
                validate_short_circuit_expression(&expression)?;
                LoweredBooleanBranchTerminator::Return { expression }
            }
            [
                StatementNode::Transition(when_true),
                StatementNode::Transition(when_false),
            ] if matches!(when_true.guard, TransitionGuardNode::When(_))
                && when_false.guard == TransitionGuardNode::Always =>
            {
                if when_true.continuation.is_valid() || when_false.continuation.is_valid() {
                    return unsupported("nested Boolean transitions cannot carry continuations");
                }
                let TransitionGuardNode::When(condition) = when_true.guard else {
                    unreachable!("match guard establishes a conditional transition")
                };
                let condition = lower_positive_boolean_guard(checked, condition, parameters)?;
                validate_short_circuit_expression(&condition)?;
                let (when_true_target, when_true_arguments) =
                    lower_nested_boolean_conditional_successor(
                        checked, states, parameters, when_true,
                    )?;
                let (when_false_target, when_false_arguments) =
                    lower_nested_boolean_conditional_successor(
                        checked, states, parameters, when_false,
                    )?;
                successors[state_index] = vec![when_true_target, when_false_target];
                indegree[when_true_target] = indegree[when_true_target]
                    .checked_add(1)
                    .expect("Boolean source state count fits usize");
                indegree[when_false_target] = indegree[when_false_target]
                    .checked_add(1)
                    .expect("Boolean source state count fits usize");
                LoweredBooleanBranchTerminator::Conditional {
                    condition,
                    when_true_target,
                    when_true_arguments,
                    when_false_target,
                    when_false_arguments,
                }
            }
            [StatementNode::Transition(transition)]
                if transition.guard == TransitionGuardNode::Always =>
            {
                if transition.continuation.is_valid() {
                    return unsupported("nested Boolean transitions cannot carry continuations");
                }
                let (target, arguments) =
                    lower_nested_boolean_jump_successor(checked, states, parameters, transition)?;
                successors[state_index] = vec![target];
                indegree[target] = indegree[target]
                    .checked_add(1)
                    .expect("Boolean source state count fits usize");
                LoweredBooleanBranchTerminator::Jump { target, arguments }
            }
            _ => {
                return unsupported(
                    "nested Boolean states must return one expression, jump unconditionally, or contain one ordered transition",
                );
            }
        };
        lowered_states.push(LoweredBooleanBranchState {
            parameter_count: parameters.len(),
            terminator,
        });
    }

    if indegree[0] != 0 || indegree[1..].contains(&0) {
        return unsupported(
            "nested Boolean control must be rooted at the machine entry and reach every state",
        );
    }
    let mut visited = vec![false; states.len()];
    let mut active = vec![false; states.len()];
    validate_nested_branch_graph(0, &successors, &mut visited, &mut active)?;
    if visited.iter().any(|visited| !*visited) {
        return unsupported("nested Boolean control contains an unreachable state");
    }

    let expected_value = evaluate_known_boolean_graph(&lowered_states);
    let contract_value = validate_boolean_contract(checked, machine, expected_value)?;
    let (identity_reshuffles, partition_compositions) =
        lower_content_evidence(checked, machine, &states[0])?;
    Ok(build_nested_boolean_branch_module(
        &lowered_states,
        contract_value,
        identity_reshuffles,
        partition_compositions,
    ))
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

fn evaluate_known_boolean_graph(states: &[LoweredBooleanBranchState]) -> Option<bool> {
    let successors = states
        .iter()
        .map(|state| match &state.terminator {
            LoweredBooleanBranchTerminator::Jump { target, .. } => vec![*target],
            LoweredBooleanBranchTerminator::Conditional {
                when_true_target,
                when_false_target,
                ..
            } => vec![*when_true_target, *when_false_target],
            LoweredBooleanBranchTerminator::Return { .. } => Vec::new(),
        })
        .collect::<Vec<_>>();
    let topological_order = acyclic_topological_order(&successors);

    let mut known_parameters = vec![None; states.len()];
    known_parameters[0] = Some(vec![None; states[0].parameter_count]);
    let mut return_values = Vec::new();
    for state_index in topological_order {
        let Some(parameters) = known_parameters[state_index].clone() else {
            continue;
        };
        let evaluate_arguments = |arguments: &[LoweredBooleanReturnExpression]| {
            arguments
                .iter()
                .map(|argument| evaluate_boolean_expression(argument, &parameters))
                .collect::<Vec<_>>()
        };
        match &states[state_index].terminator {
            LoweredBooleanBranchTerminator::Jump { target, arguments } => {
                merge_known_parameters(
                    &mut known_parameters[*target],
                    evaluate_arguments(arguments),
                );
            }
            LoweredBooleanBranchTerminator::Conditional {
                condition,
                when_true_target,
                when_true_arguments,
                when_false_target,
                when_false_arguments,
            } => match evaluate_boolean_expression(condition, &parameters) {
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
            LoweredBooleanBranchTerminator::Return { expression } => {
                return_values.push(evaluate_boolean_expression(expression, &parameters));
            }
        }
    }

    let expected = return_values.first().copied().flatten()?;
    return_values
        .into_iter()
        .all(|value| value == Some(expected))
        .then_some(expected)
}

fn evaluate_known_integer_graph(states: &[LoweredIntegerBranchState]) -> Option<IntegerValue> {
    let successors = states
        .iter()
        .map(|state| match &state.terminator {
            LoweredIntegerBranchTerminator::Jump { target, .. } => vec![*target],
            LoweredIntegerBranchTerminator::Conditional {
                when_true_target,
                when_false_target,
                ..
            } => vec![*when_true_target, *when_false_target],
            LoweredIntegerBranchTerminator::Return { .. }
            | LoweredIntegerBranchTerminator::Crash(_) => Vec::new(),
        })
        .collect::<Vec<_>>();
    let topological_order = acyclic_topological_order(&successors);
    let mut known_parameters = vec![None; states.len()];
    known_parameters[0] = Some(vec![None; states[0].parameter_types.len()]);
    let mut return_values = Vec::new();
    let mut reachable_crash = false;
    for state_index in topological_order {
        let Some(parameters) = known_parameters[state_index].clone() else {
            continue;
        };
        let evaluate_arguments = |arguments: &[LoweredDirectExpression]| {
            arguments
                .iter()
                .map(|argument| evaluate_direct_expression(argument, &parameters))
                .collect::<Vec<_>>()
        };
        match &states[state_index].terminator {
            LoweredIntegerBranchTerminator::Jump { target, arguments } => {
                merge_known_parameters(
                    &mut known_parameters[*target],
                    evaluate_arguments(arguments),
                );
            }
            LoweredIntegerBranchTerminator::Conditional {
                condition,
                when_true_target,
                when_true_arguments,
                when_false_target,
                when_false_arguments,
            } => match evaluate_compile_known_boolean_expression(condition, &parameters) {
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
            LoweredIntegerBranchTerminator::Return { expression } => {
                return_values.push(evaluate_direct_expression(expression, &parameters));
            }
            LoweredIntegerBranchTerminator::Crash(_) => reachable_crash = true,
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

fn lower_nested_boolean_conditional_successor(
    checked: &CheckedTrees,
    states: &[psi_checked_trees::state::State],
    parameters: &[psi_checked_trees::signature::StateParameter],
    transition: &psi_checked_trees::statement::TableTransition,
) -> Result<(usize, Vec<LoweredBooleanReturnExpression>), LoweringError> {
    let TransitionTargetNode::Named { path, arguments } =
        checked.statement_table.transition_target(transition.target)
    else {
        return unsupported("nested Boolean successors must target named states");
    };
    let target = states
        .iter()
        .position(|candidate| candidate.symbol == path.symbol)
        .ok_or(LoweringError::Unsupported(
            "nested Boolean successor must belong to the selected machine",
        ))?;
    let target_parameters = checked.state_parameters(&states[target]);
    let arguments = checked.statement_table.expression_handles(*arguments);
    if arguments.len() != target_parameters.len() {
        return unsupported(
            "nested Boolean successor bindings must match the target parameter count",
        );
    }
    let arguments = arguments
        .iter()
        .zip(target_parameters)
        .map(|(argument, target_parameter)| {
            if checked.primitive_type_reference(target_parameter.type_reference)
                != Some(PrimitiveType::Bool)
            {
                return unsupported("nested Boolean targets require Boolean parameters");
            }
            let expression = lower_boolean_expression(checked, *argument, parameters)?;
            validate_short_circuit_expression(&expression)?;
            Ok(expression)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((target, arguments))
}

fn lower_nested_boolean_jump_successor(
    checked: &CheckedTrees,
    states: &[psi_checked_trees::state::State],
    parameters: &[psi_checked_trees::signature::StateParameter],
    transition: &psi_checked_trees::statement::TableTransition,
) -> Result<(usize, Vec<LoweredBooleanReturnExpression>), LoweringError> {
    let TransitionTargetNode::Named { path, arguments } =
        checked.statement_table.transition_target(transition.target)
    else {
        return unsupported("nested Boolean successors must target named states");
    };
    let target = states
        .iter()
        .position(|candidate| candidate.symbol == path.symbol)
        .ok_or(LoweringError::Unsupported(
            "nested Boolean successor must belong to the selected machine",
        ))?;
    let target_parameters = checked.state_parameters(&states[target]);
    let arguments = checked.statement_table.expression_handles(*arguments);
    if arguments.len() != target_parameters.len() {
        return unsupported(
            "nested Boolean successor bindings must match the target parameter count",
        );
    }
    let arguments = arguments
        .iter()
        .zip(target_parameters)
        .map(|(argument, target_parameter)| {
            if checked.primitive_type_reference(target_parameter.type_reference)
                != Some(PrimitiveType::Bool)
            {
                return unsupported("nested Boolean targets require Boolean parameters");
            }
            let expression = lower_boolean_expression(checked, *argument, parameters)?;
            validate_short_circuit_expression(&expression)?;
            Ok(expression)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((target, arguments))
}

fn lower_nested_integer_branch_machine(
    checked: &CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
    states: &[psi_checked_trees::state::State],
) -> Result<LoweredTerminalPsi, LoweringError> {
    if states
        .iter()
        .any(|state| !checked.state_contracts(state).is_empty())
    {
        return unsupported("nested branch state contracts are not supported");
    }
    let result_type = integer_scalar_type(
        checked
            .primitive_type_reference(states[0].return_type)
            .ok_or(LoweringError::Unsupported(
                "nested branch result must be a primitive integer",
            ))?,
    )?;
    let (identity_reshuffles, partition_compositions) =
        lower_content_evidence(checked, machine, &states[0])?;
    let mut lowered_states = Vec::with_capacity(states.len());
    let mut successors = vec![Vec::new(); states.len()];
    let mut indegree = vec![0usize; states.len()];

    for (state_index, state) in states.iter().enumerate() {
        if integer_scalar_type(checked.primitive_type_reference(state.return_type).ok_or(
            LoweringError::Unsupported("nested branch states must return a primitive integer"),
        )?)? != result_type
        {
            return unsupported("nested branch state result types must match exactly");
        }
        let parameters = checked.state_parameters(state);
        if parameters
            .iter()
            .any(|parameter| parameter.is_self || parameter.is_const || parameter.is_mutable)
        {
            return unsupported("qualified nested branch parameters are not supported");
        }
        let parameter_types = parameters
            .iter()
            .map(|parameter| {
                terminal_scalar_type(
                    checked
                        .primitive_type_reference(parameter.type_reference)
                        .ok_or(LoweringError::Unsupported(
                            "nested branch parameters must be primitive Boolean or integer values",
                        ))?,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let statements = checked.statement_table.statements(state.statement_nodes);
        let terminator = match statements {
            [StatementNode::Expression(return_expression)] => {
                let (expression, _) = lower_direct_return_expression(
                    checked,
                    *return_expression,
                    parameters,
                    &parameter_types,
                    result_type,
                )?;
                LoweredIntegerBranchTerminator::Return { expression }
            }
            [StatementNode::Transition(transition)]
                if matches!(transition.exit, TransitionExit::Crash(_))
                    && transition.guard == TransitionGuardNode::Always
                    && !transition.continuation.is_valid()
                    && matches!(
                        checked.statement_table.transition_target(transition.target),
                        TransitionTargetNode::Terminal
                    ) =>
            {
                LoweredIntegerBranchTerminator::Crash(lower_checked_crash_exit(
                    checked,
                    machine,
                    state,
                    0,
                    &identity_reshuffles.source_claims,
                )?)
            }
            [
                StatementNode::Transition(when_true),
                StatementNode::Transition(when_false),
            ] if matches!(when_true.guard, TransitionGuardNode::When(_))
                && when_false.guard == TransitionGuardNode::Always =>
            {
                if when_true.continuation.is_valid() || when_false.continuation.is_valid() {
                    return unsupported("nested branch transitions cannot carry continuations");
                }
                let TransitionGuardNode::When(condition) = when_true.guard else {
                    unreachable!("match guard establishes a conditional transition")
                };
                let condition = lower_positive_boolean_guard(checked, condition, parameters)?;
                validate_short_circuit_expression(&condition)?;
                validate_boolean_parameter_types(&condition, &parameter_types)?;

                let (when_true_target, when_true_arguments) =
                    lower_nested_branch_conditional_successor(
                        checked,
                        states,
                        parameters,
                        &parameter_types,
                        when_true,
                    )?;
                let (when_false_target, when_false_arguments) =
                    lower_nested_branch_conditional_successor(
                        checked,
                        states,
                        parameters,
                        &parameter_types,
                        when_false,
                    )?;
                successors[state_index] = vec![when_true_target, when_false_target];
                indegree[when_true_target] = indegree[when_true_target]
                    .checked_add(1)
                    .expect("source state count fits usize");
                indegree[when_false_target] = indegree[when_false_target]
                    .checked_add(1)
                    .expect("source state count fits usize");
                LoweredIntegerBranchTerminator::Conditional {
                    condition,
                    when_true_target,
                    when_true_arguments,
                    when_false_target,
                    when_false_arguments,
                }
            }
            [StatementNode::Transition(transition)]
                if transition.guard == TransitionGuardNode::Always =>
            {
                if transition.continuation.is_valid() {
                    return unsupported("nested branch transitions cannot carry continuations");
                }
                let (target, arguments) = lower_nested_branch_jump_successor(
                    checked,
                    states,
                    parameters,
                    &parameter_types,
                    transition,
                )?;
                successors[state_index] = vec![target];
                indegree[target] = indegree[target]
                    .checked_add(1)
                    .expect("source state count fits usize");
                LoweredIntegerBranchTerminator::Jump { target, arguments }
            }
            _ => {
                return unsupported(
                    "nested branch states must return one integer expression, jump unconditionally, or contain one ordered Boolean transition",
                );
            }
        };
        lowered_states.push(LoweredIntegerBranchState {
            parameter_types,
            terminator,
        });
    }

    if indegree[0] != 0 || indegree[1..].contains(&0) {
        return unsupported(
            "nested terminal branch control must be rooted at the machine entry and reach every state",
        );
    }
    let mut visited = vec![false; states.len()];
    let mut active = vec![false; states.len()];
    validate_nested_branch_graph(0, &successors, &mut visited, &mut active)?;
    if visited.iter().any(|visited| !*visited) {
        return unsupported("nested terminal branch control contains an unreachable state");
    }

    let has_crash = lowered_states
        .iter()
        .any(|state| matches!(&state.terminator, LoweredIntegerBranchTerminator::Crash(_)));
    let expected_value = evaluate_known_integer_graph(&lowered_states);
    let contract_value =
        validate_contract(checked, machine, result_type, expected_value, has_crash)?;
    Ok(build_nested_integer_branch_module(
        &lowered_states,
        result_type,
        contract_value,
        identity_reshuffles,
        partition_compositions,
    ))
}

fn lower_nested_branch_conditional_successor(
    checked: &CheckedTrees,
    states: &[psi_checked_trees::state::State],
    parameters: &[psi_checked_trees::signature::StateParameter],
    parameter_types: &[ScalarType],
    transition: &psi_checked_trees::statement::TableTransition,
) -> Result<(usize, Vec<LoweredDirectExpression>), LoweringError> {
    let TransitionTargetNode::Named { path, arguments } =
        checked.statement_table.transition_target(transition.target)
    else {
        return unsupported("nested branch successors must target named states");
    };
    let target = states
        .iter()
        .position(|candidate| candidate.symbol == path.symbol)
        .ok_or(LoweringError::Unsupported(
            "nested branch successor must belong to the selected machine",
        ))?;
    let target_parameters = checked.state_parameters(&states[target]);
    let arguments = checked.statement_table.expression_handles(*arguments);
    if arguments.len() != target_parameters.len() {
        return unsupported(
            "nested branch successor bindings must match the target parameter count",
        );
    }
    let arguments = arguments
        .iter()
        .zip(target_parameters)
        .map(|(argument, target_parameter)| {
            let target_type = terminal_scalar_type(
                checked
                    .primitive_type_reference(target_parameter.type_reference)
                    .ok_or(LoweringError::Unsupported(
                        "nested branch target parameters must be primitive Boolean or integer values",
                    ))?,
            )?;
            lower_direct_return_expression(
                checked,
                *argument,
                parameters,
                parameter_types,
                target_type,
            )
            .map(|(expression, _)| expression)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((target, arguments))
}

fn lower_nested_branch_jump_successor(
    checked: &CheckedTrees,
    states: &[psi_checked_trees::state::State],
    parameters: &[psi_checked_trees::signature::StateParameter],
    parameter_types: &[ScalarType],
    transition: &psi_checked_trees::statement::TableTransition,
) -> Result<(usize, Vec<LoweredDirectExpression>), LoweringError> {
    let TransitionTargetNode::Named { path, arguments } =
        checked.statement_table.transition_target(transition.target)
    else {
        return unsupported("nested branch successors must target named states");
    };
    let target = states
        .iter()
        .position(|candidate| candidate.symbol == path.symbol)
        .ok_or(LoweringError::Unsupported(
            "nested branch successor must belong to the selected machine",
        ))?;
    let target_parameters = checked.state_parameters(&states[target]);
    let arguments = checked.statement_table.expression_handles(*arguments);
    if arguments.len() != target_parameters.len() {
        return unsupported(
            "nested branch successor bindings must match the target parameter count",
        );
    }
    let arguments = arguments
        .iter()
        .zip(target_parameters)
        .map(|(argument, target_parameter)| {
            let target_type = terminal_scalar_type(
                checked
                    .primitive_type_reference(target_parameter.type_reference)
                    .ok_or(LoweringError::Unsupported(
                        "nested branch target parameters must be primitive Boolean or integer values",
                    ))?,
            )?;
            lower_direct_return_expression(
                checked,
                *argument,
                parameters,
                parameter_types,
                target_type,
            )
            .map(|(expression, _)| expression)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((target, arguments))
}

fn validate_boolean_parameter_types(
    expression: &LoweredBooleanReturnExpression,
    parameter_types: &[ScalarType],
) -> Result<(), LoweringError> {
    match expression {
        LoweredBooleanReturnExpression::Constant { .. } => Ok(()),
        LoweredBooleanReturnExpression::Parameter { position } => {
            if parameter_types.get(*position) == Some(&ScalarType::Boolean) {
                Ok(())
            } else {
                unsupported("nested branch guard parameters must be Boolean")
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
        } => {
            if parameter_types.get(*position) == Some(scalar_type) {
                Ok(())
            } else {
                unsupported("nested branch integer guard parameter type does not match")
            }
        }
        LoweredDirectExpression::IntegerLiteral { .. } => Ok(()),
        LoweredDirectExpression::IntegerBinary { left, right, .. } => {
            validate_direct_parameter_types(left, parameter_types)?;
            validate_direct_parameter_types(right, parameter_types)
        }
        LoweredDirectExpression::Boolean { expression } => {
            validate_boolean_parameter_types(expression, parameter_types)
        }
    }
}

fn validate_nested_branch_graph(
    state: usize,
    successors: &[Vec<usize>],
    visited: &mut [bool],
    active: &mut [bool],
) -> Result<(), LoweringError> {
    if active[state] {
        return unsupported("nested terminal branch control must be acyclic");
    }
    if visited[state] {
        return Ok(());
    }
    active[state] = true;
    for successor in &successors[state] {
        validate_nested_branch_graph(*successor, successors, visited, active)?;
    }
    active[state] = false;
    visited[state] = true;
    Ok(())
}

fn lower_integer_state_chain(
    checked: &CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
    states: &[psi_checked_trees::state::State],
) -> Result<LoweredTerminalPsi, LoweringError> {
    let entry_state = &states[0];
    let entry_parameters = checked.state_parameters(entry_state);
    if entry_parameters
        .iter()
        .any(|parameter| parameter.is_self || parameter.is_const || parameter.is_mutable)
    {
        return unsupported("qualified linear-state machine parameters are not supported");
    }
    let parameter_types = entry_parameters
        .iter()
        .map(|parameter| {
            integer_scalar_type(
                checked
                    .primitive_type_reference(parameter.type_reference)
                    .ok_or(LoweringError::Unsupported(
                        "linear-state machine parameters must be primitive integers",
                    ))?,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let result_type = integer_scalar_type(
        checked
            .primitive_type_reference(entry_state.return_type)
            .ok_or(LoweringError::Unsupported(
                "linear-state machine result must be a primitive integer",
            ))?,
    )?;
    let mut state_parameter_types = Vec::with_capacity(states.len());
    state_parameter_types.push(parameter_types.clone());
    for state in &states[1..] {
        let state_parameters = checked.state_parameters(state);
        if state_parameters
            .iter()
            .any(|parameter| parameter.is_self || parameter.is_const || parameter.is_mutable)
        {
            return unsupported("qualified linear-state parameters are not supported");
        }
        if integer_scalar_type(checked.primitive_type_reference(state.return_type).ok_or(
            LoweringError::Unsupported("linear-state result must be a primitive integer"),
        )?)? != result_type
        {
            return unsupported("machine and state result types must match exactly");
        }
        state_parameter_types.push(
            state_parameters
                .iter()
                .map(|parameter| {
                    integer_scalar_type(
                        checked
                            .primitive_type_reference(parameter.type_reference)
                            .ok_or(LoweringError::Unsupported(
                                "linear-state parameters must be primitive integers",
                            ))?,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    if states
        .iter()
        .any(|state| !checked.state_contracts(state).is_empty())
    {
        return unsupported("state contracts are not supported");
    }

    let mut jump_expressions = Vec::with_capacity(states.len() - 1);
    let mut known_parameters = vec![None; parameter_types.len()];
    for (index, state) in states[..states.len() - 1].iter().enumerate() {
        let statements = checked.statement_table.statements(state.statement_nodes);
        let [StatementNode::Transition(transition)] = statements else {
            return unsupported("each nonterminal linear state must contain one transition");
        };
        if transition.guard != TransitionGuardNode::Always || transition.continuation.is_valid() {
            return unsupported(
                "linear-state transitions must be unconditional and have no continuation",
            );
        }
        let TransitionTargetNode::Named { path, arguments } =
            checked.statement_table.transition_target(transition.target)
        else {
            return unsupported("a linear-state transition must target its next state by name");
        };
        if path.symbol != states[index + 1].symbol {
            return unsupported("a linear-state transition must target the next declared state");
        }
        let arguments = checked.statement_table.expression_handles(*arguments);
        let target_types = &state_parameter_types[index + 1];
        if arguments.len() != target_types.len() {
            return unsupported(
                "a linear-state transition must bind every next-state parameter exactly once",
            );
        }
        let mut expressions = Vec::with_capacity(arguments.len());
        let mut next_known_parameters = Vec::with_capacity(arguments.len());
        for (argument, target_type) in arguments.iter().zip(target_types) {
            let (expression, _) = lower_direct_return_expression(
                checked,
                *argument,
                checked.state_parameters(state),
                &state_parameter_types[index],
                *target_type,
            )?;
            next_known_parameters.push(evaluate_direct_expression(&expression, &known_parameters));
            expressions.push(expression);
        }
        jump_expressions.push(expressions);
        known_parameters = next_known_parameters;
    }

    let return_state = states.last().expect("linear chain is nonempty");
    let return_parameters = checked.state_parameters(return_state);
    let return_statements = checked
        .statement_table
        .statements(return_state.statement_nodes);
    let [StatementNode::Expression(return_expression)] = return_statements else {
        return unsupported("return state must contain exactly one value expression");
    };
    let (return_expression, _) = lower_direct_return_expression(
        checked,
        *return_expression,
        return_parameters,
        state_parameter_types
            .last()
            .expect("linear chain retains final parameter types"),
        result_type,
    )?;
    let expected_value = evaluate_direct_expression(&return_expression, &known_parameters);

    let contract_value = validate_contract(checked, machine, result_type, expected_value, false)?;
    let (identity_reshuffles, partition_compositions) =
        lower_content_evidence(checked, machine, entry_state)?;
    Ok(build_integer_state_chain_module(
        &state_parameter_types,
        jump_expressions,
        return_expression,
        result_type,
        contract_value,
        identity_reshuffles,
        partition_compositions,
    ))
}

fn lower_boolean_machine(
    checked: &CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
    entry_state: &psi_checked_trees::state::State,
) -> Result<LoweredTerminalPsi, LoweringError> {
    if !checked.state_contracts(entry_state).is_empty() {
        return unsupported("state contracts are not supported");
    }
    let parameters = checked.state_parameters(entry_state);
    if !parameters.is_empty()
        && parameters.iter().all(|parameter| {
            checked
                .primitive_type_reference(parameter.type_reference)
                .is_some_and(|primitive| {
                    !matches!(
                        primitive,
                        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64
                    )
                })
        })
    {
        return lower_integer_comparison_machine(checked, machine, entry_state);
    }
    if parameters
        .iter()
        .any(|parameter| parameter.is_self || parameter.is_const || parameter.is_mutable)
    {
        return unsupported("qualified Boolean machine parameters are not supported");
    }
    if parameters.iter().any(|parameter| {
        checked.primitive_type_reference(parameter.type_reference) != Some(PrimitiveType::Bool)
    }) {
        return unsupported("Boolean source machines require Boolean parameters");
    }
    let statements = checked
        .statement_table
        .statements(entry_state.statement_nodes);
    let [StatementNode::Expression(return_expression)] = statements else {
        return unsupported("Boolean source machine must contain exactly one value expression");
    };
    let return_expression = lower_boolean_expression(checked, *return_expression, parameters)?;
    validate_short_circuit_expression(&return_expression)?;
    let known_parameters = vec![None; parameters.len()];
    let expected_value = evaluate_boolean_expression(&return_expression, &known_parameters);
    let contract_value = validate_boolean_contract(checked, machine, expected_value)?;
    let (identity_reshuffles, partition_compositions) =
        lower_content_evidence(checked, machine, entry_state)?;
    Ok(build_boolean_module(
        parameters.len(),
        return_expression,
        contract_value,
        identity_reshuffles,
        partition_compositions,
    ))
}

fn lower_integer_comparison_machine(
    checked: &CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
    entry_state: &psi_checked_trees::state::State,
) -> Result<LoweredTerminalPsi, LoweringError> {
    let parameters = checked.state_parameters(entry_state);
    if parameters
        .iter()
        .any(|parameter| parameter.is_self || parameter.is_const || parameter.is_mutable)
    {
        return unsupported("qualified integer-comparison parameters are not supported");
    }
    let statements = checked
        .statement_table
        .statements(entry_state.statement_nodes);
    let [StatementNode::Expression(expression)] = statements else {
        return unsupported("integer-comparison source machines require one value expression");
    };
    let parameter_types = parameters
        .iter()
        .map(|parameter| {
            integer_scalar_type(
                checked
                    .primitive_type_reference(parameter.type_reference)
                    .ok_or(LoweringError::Unsupported(
                        "integer-comparison parameters must have primitive integer type",
                    ))?,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let return_expression = lower_boolean_expression(checked, *expression, parameters)?;
    if !is_integer_comparison_expression(&return_expression) {
        return unsupported("integer-comparison source machines require a builtin comparison");
    }

    let contract_value = validate_boolean_contract(checked, machine, None)?;
    let (identity_reshuffles, partition_compositions) =
        lower_content_evidence(checked, machine, entry_state)?;
    let terminal_parameters = parameter_types
        .iter()
        .enumerate()
        .map(|(index, scalar_type)| ValueDeclaration {
            id: value_id(
                u64::try_from(index)
                    .expect("parameter index fits a semantic identity")
                    .checked_add(1)
                    .expect("parameter identity is nonzero"),
            ),
            scalar_type: *scalar_type,
        })
        .collect::<Vec<_>>();
    Ok(build_integer_comparison_module(
        terminal_parameters,
        return_expression,
        contract_value,
        identity_reshuffles,
        partition_compositions,
    ))
}

fn lower_boolean_state_chain(
    checked: &CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
    states: &[psi_checked_trees::state::State],
) -> Result<LoweredTerminalPsi, LoweringError> {
    let entry_state = &states[0];
    let entry_parameters = checked.state_parameters(entry_state);
    if entry_parameters
        .iter()
        .any(|parameter| parameter.is_self || parameter.is_const || parameter.is_mutable)
    {
        return unsupported("qualified Boolean state-chain parameters are not supported");
    }
    if entry_parameters.iter().any(|parameter| {
        checked.primitive_type_reference(parameter.type_reference) != Some(PrimitiveType::Bool)
    }) {
        return unsupported("Boolean state-chain machine parameters must be Boolean");
    }
    for state in &states[1..] {
        let [parameter] = checked.state_parameters(state) else {
            return unsupported("every non-entry Boolean state must have exactly one parameter");
        };
        if parameter.is_self || parameter.is_const || parameter.is_mutable {
            return unsupported("qualified Boolean state-chain parameters are not supported");
        }
        if checked.primitive_type_reference(state.return_type) != Some(PrimitiveType::Bool)
            || checked.primitive_type_reference(parameter.type_reference)
                != Some(PrimitiveType::Bool)
        {
            return unsupported("Boolean state and carried parameter types must remain Boolean");
        }
    }
    if states
        .iter()
        .any(|state| !checked.state_contracts(state).is_empty())
    {
        return unsupported("state contracts are not supported");
    }

    let mut jump_expressions = Vec::with_capacity(states.len() - 1);
    let mut known_parameters = vec![None; entry_parameters.len()];
    for (index, state) in states[..states.len() - 1].iter().enumerate() {
        let statements = checked.statement_table.statements(state.statement_nodes);
        let [StatementNode::Transition(transition)] = statements else {
            return unsupported("each nonterminal Boolean state must contain one transition");
        };
        if transition.guard != TransitionGuardNode::Always || transition.continuation.is_valid() {
            return unsupported(
                "Boolean state-chain transitions must be unconditional and have no continuation",
            );
        }
        let TransitionTargetNode::Named { path, arguments } =
            checked.statement_table.transition_target(transition.target)
        else {
            return unsupported("a Boolean state-chain transition must target its next state");
        };
        if path.symbol != states[index + 1].symbol {
            return unsupported(
                "a Boolean state-chain transition must target the next declared state",
            );
        }
        let [argument] = checked.statement_table.expression_handles(*arguments) else {
            return unsupported("a Boolean state-chain transition must carry exactly one argument");
        };
        let expression =
            lower_boolean_expression(checked, *argument, checked.state_parameters(state))?;
        validate_short_circuit_expression(&expression)?;
        let known_value = evaluate_boolean_expression(&expression, &known_parameters);
        jump_expressions.push(expression);
        known_parameters = vec![known_value];
    }

    let return_state = states.last().expect("Boolean state chain is nonempty");
    let [return_parameter] = checked.state_parameters(return_state) else {
        unreachable!("non-entry Boolean state shape was validated above");
    };
    let return_statements = checked
        .statement_table
        .statements(return_state.statement_nodes);
    let [StatementNode::Expression(return_expression)] = return_statements else {
        return unsupported("return Boolean state must contain exactly one value expression");
    };
    let return_expression = lower_boolean_expression(
        checked,
        *return_expression,
        std::slice::from_ref(return_parameter),
    )?;
    validate_short_circuit_expression(&return_expression)?;
    let expected_value = evaluate_boolean_expression(&return_expression, &known_parameters);

    let contract_value = validate_boolean_contract(checked, machine, expected_value)?;
    let (identity_reshuffles, partition_compositions) =
        lower_content_evidence(checked, machine, entry_state)?;
    Ok(build_boolean_state_chain_module(
        entry_parameters.len(),
        jump_expressions,
        return_expression,
        contract_value,
        identity_reshuffles,
        partition_compositions,
    ))
}

fn lower_boolean_expression(
    checked: &CheckedTrees,
    expression: psi_checked_trees::expression::ExpressionHandle,
    parameters: &[psi_checked_trees::signature::StateParameter],
) -> Result<LoweredBooleanReturnExpression, LoweringError> {
    match checked.expression_table.expression(expression) {
        ExpressionNode::Boolean(value) => {
            Ok(LoweredBooleanReturnExpression::Constant { value: *value })
        }
        ExpressionNode::Name(path) => Ok(LoweredBooleanReturnExpression::Parameter {
            position: direct_parameter_position(checked, path, parameters)?,
        }),
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            if let Some(operator_use) = checked.facts.operators.expression_use(expression)
                && operator_use.status != CheckedOperatorResolutionStatus::BuiltinFallback
            {
                return unsupported("terminal Boolean negation must use the builtin operator");
            }
            Ok(LoweredBooleanReturnExpression::Not {
                operand: Box::new(lower_boolean_expression(
                    checked,
                    unary.operand,
                    parameters,
                )?),
            })
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
            ) =>
        {
            if let Some(operator_use) = checked.facts.operators.expression_use(expression)
                && operator_use.status != CheckedOperatorResolutionStatus::BuiltinFallback
            {
                return unsupported("terminal Boolean comparison must use the builtin operator");
            }
            let parameter_types = parameters
                .iter()
                .map(|parameter| {
                    terminal_scalar_type(
                        checked
                            .primitive_type_reference(parameter.type_reference)
                            .ok_or(LoweringError::Unsupported(
                                "terminal comparison parameters must be primitive scalar values",
                            ))?,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let integer_operands = (|| {
                let (left, _) =
                    lower_direct_expression(checked, binary.left, parameters, &parameter_types)?;
                let (right, _) =
                    lower_direct_expression(checked, binary.right, parameters, &parameter_types)?;
                if !matches!(left.scalar_type(), ScalarType::Integer(_))
                    || left.scalar_type() != right.scalar_type()
                {
                    return unsupported(
                        "terminal integer comparison operands must have one exact integer type",
                    );
                }
                Ok((left, right))
            })();
            if !matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) || integer_operands.is_ok()
            {
                let (mut left, mut right) = integer_operands?;
                let (kind, negated) = match binary.operator {
                    BinaryOperator::Equal => (LoweredIntegerComparisonKind::Equal, false),
                    BinaryOperator::NotEqual => (LoweredIntegerComparisonKind::Equal, true),
                    BinaryOperator::Less => (LoweredIntegerComparisonKind::LessThan, false),
                    BinaryOperator::LessOrEqual => {
                        (LoweredIntegerComparisonKind::LessOrEqual, false)
                    }
                    BinaryOperator::Greater => {
                        std::mem::swap(&mut left, &mut right);
                        (LoweredIntegerComparisonKind::LessThan, false)
                    }
                    BinaryOperator::GreaterOrEqual => {
                        std::mem::swap(&mut left, &mut right);
                        (LoweredIntegerComparisonKind::LessOrEqual, false)
                    }
                    _ => unreachable!("comparison expression filters operators"),
                };
                let comparison = LoweredBooleanReturnExpression::IntegerComparison {
                    kind,
                    left: Box::new(left),
                    right: Box::new(right),
                };
                return Ok(if negated {
                    LoweredBooleanReturnExpression::Not {
                        operand: Box::new(comparison),
                    }
                } else {
                    comparison
                });
            }
            let equality = LoweredBooleanReturnExpression::Equal {
                left: Box::new(lower_boolean_expression(checked, binary.left, parameters)?),
                right: Box::new(lower_boolean_expression(checked, binary.right, parameters)?),
            };
            Ok(if binary.operator == BinaryOperator::NotEqual {
                LoweredBooleanReturnExpression::Not {
                    operand: Box::new(equality),
                }
            } else {
                equality
            })
        }
        ExpressionNode::Binary(binary)
            if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) =>
        {
            if let Some(operator_use) = checked.facts.operators.expression_use(expression)
                && operator_use.status != CheckedOperatorResolutionStatus::BuiltinFallback
            {
                return unsupported("terminal short-circuit logic must use the builtin operator");
            }
            let left = Box::new(lower_boolean_expression(checked, binary.left, parameters)?);
            let right = Box::new(lower_boolean_expression(checked, binary.right, parameters)?);
            Ok(if binary.operator == BinaryOperator::And {
                LoweredBooleanReturnExpression::And { left, right }
            } else {
                LoweredBooleanReturnExpression::Or { left, right }
            })
        }
        _ => unsupported(
            "Boolean terminal expressions require a literal, declared parameter, logical not, builtin Boolean equality/inequality, exact-type integer comparison, or short-circuit logic",
        ),
    }
}

fn contains_short_circuit(expression: &LoweredBooleanReturnExpression) -> bool {
    match expression {
        LoweredBooleanReturnExpression::Constant { .. }
        | LoweredBooleanReturnExpression::Parameter { .. }
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

fn validate_short_circuit_expression(
    expression: &LoweredBooleanReturnExpression,
) -> Result<(), LoweringError> {
    match expression {
        LoweredBooleanReturnExpression::Constant { .. }
        | LoweredBooleanReturnExpression::Parameter { .. }
        | LoweredBooleanReturnExpression::IntegerComparison { .. } => Ok(()),
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

fn lower_positive_boolean_guard(
    checked: &CheckedTrees,
    expression: psi_checked_trees::expression::ExpressionHandle,
    parameters: &[psi_checked_trees::signature::StateParameter],
) -> Result<LoweredBooleanReturnExpression, LoweringError> {
    let ExpressionNode::Binary(binary) = checked.expression_table.expression(expression) else {
        return lower_boolean_expression(checked, expression, parameters);
    };
    if binary.operator == BinaryOperator::Equal {
        match (
            checked.expression_table.expression(binary.left),
            checked.expression_table.expression(binary.right),
        ) {
            (ExpressionNode::Boolean(true), _) => {
                return lower_boolean_expression(checked, binary.right, parameters);
            }
            (_, ExpressionNode::Boolean(true)) => {
                return lower_boolean_expression(checked, binary.left, parameters);
            }
            _ => {}
        }
    }
    let guard = lower_boolean_expression(checked, expression, parameters)?;
    if is_integer_comparison_expression(&guard) || contains_short_circuit(&guard) {
        Ok(guard)
    } else {
        unsupported("conditional guards require a positive Boolean pattern")
    }
}

fn is_integer_comparison_expression(expression: &LoweredBooleanReturnExpression) -> bool {
    match expression {
        LoweredBooleanReturnExpression::IntegerComparison { .. } => true,
        LoweredBooleanReturnExpression::Not { operand } => matches!(
            operand.as_ref(),
            LoweredBooleanReturnExpression::IntegerComparison { .. }
        ),
        _ => false,
    }
}

fn evaluate_boolean_expression(
    expression: &LoweredBooleanReturnExpression,
    parameters: &[Option<bool>],
) -> Option<bool> {
    match expression {
        LoweredBooleanReturnExpression::Constant { value } => Some(*value),
        LoweredBooleanReturnExpression::Parameter { position } => {
            parameters.get(*position).copied().flatten()
        }
        LoweredBooleanReturnExpression::Not { operand } => {
            Some(!evaluate_boolean_expression(operand, parameters)?)
        }
        LoweredBooleanReturnExpression::Equal { left, right } => Some(
            evaluate_boolean_expression(left, parameters)?
                == evaluate_boolean_expression(right, parameters)?,
        ),
        LoweredBooleanReturnExpression::IntegerComparison { .. } => None,
        LoweredBooleanReturnExpression::And { left, right } => {
            let left = evaluate_boolean_expression(left, parameters)?;
            if left {
                evaluate_boolean_expression(right, parameters)
            } else {
                Some(false)
            }
        }
        LoweredBooleanReturnExpression::Or { left, right } => {
            let left = evaluate_boolean_expression(left, parameters)?;
            if left {
                Some(true)
            } else {
                evaluate_boolean_expression(right, parameters)
            }
        }
    }
}

fn lower_direct_parameter_machine(
    checked: &CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
    entry_state: &psi_checked_trees::state::State,
) -> Result<LoweredTerminalPsi, LoweringError> {
    if !checked.state_contracts(entry_state).is_empty() {
        return unsupported("state contracts are not supported");
    }
    let parameters = checked.state_parameters(entry_state);
    if parameters
        .iter()
        .any(|parameter| parameter.is_self || parameter.is_const || parameter.is_mutable)
    {
        return unsupported("qualified direct-machine parameters are not supported");
    }
    let parameter_types = parameters
        .iter()
        .map(|parameter| {
            integer_scalar_type(
                checked
                    .primitive_type_reference(parameter.type_reference)
                    .ok_or(LoweringError::Unsupported(
                        "direct-machine parameters must be primitive integers",
                    ))?,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let result_type = integer_scalar_type(
        checked
            .primitive_type_reference(entry_state.return_type)
            .ok_or(LoweringError::Unsupported(
                "machine result must be a primitive integer",
            ))?,
    )?;
    let statements = checked
        .statement_table
        .statements(entry_state.statement_nodes);
    let [StatementNode::Expression(return_expression)] = statements else {
        return unsupported("direct-parameter machine must contain exactly one value expression");
    };
    let (return_expression, _) = lower_direct_return_expression(
        checked,
        *return_expression,
        parameters,
        &parameter_types,
        result_type,
    )?;
    let known_parameters = vec![None; parameter_types.len()];
    let expected_value = evaluate_direct_expression(&return_expression, &known_parameters);
    let contract_value = validate_contract(checked, machine, result_type, expected_value, false)?;
    let (identity_reshuffles, partition_compositions) =
        lower_content_evidence(checked, machine, entry_state)?;
    Ok(build_direct_parameter_module(
        &parameter_types,
        return_expression,
        result_type,
        contract_value,
        identity_reshuffles,
        partition_compositions,
    ))
}

fn lower_direct_return_expression(
    checked: &CheckedTrees,
    expression: psi_checked_trees::expression::ExpressionHandle,
    parameters: &[psi_checked_trees::signature::StateParameter],
    parameter_types: &[ScalarType],
    result_type: ScalarType,
) -> Result<(LoweredDirectExpression, ArithmeticDomain), LoweringError> {
    if result_type == ScalarType::Boolean {
        let expression = lower_boolean_expression(checked, expression, parameters)?;
        validate_short_circuit_expression(&expression)?;
        validate_boolean_parameter_types(&expression, parameter_types)?;
        if contains_short_circuit(&expression) {
            return unsupported(
                "mixed scalar graph bindings do not support short-circuit Boolean expressions yet",
            );
        }
        return Ok((
            LoweredDirectExpression::Boolean {
                expression: Box::new(expression),
            },
            ArithmeticDomain::Exact,
        ));
    }
    let (expression, domain) =
        lower_direct_expression(checked, expression, parameters, parameter_types)?;
    if expression.scalar_type() != result_type {
        return unsupported("direct expression and destination types must match exactly");
    }
    Ok((expression, domain))
}

fn lower_direct_expression(
    checked: &CheckedTrees,
    expression: psi_checked_trees::expression::ExpressionHandle,
    parameters: &[psi_checked_trees::signature::StateParameter],
    parameter_types: &[ScalarType],
) -> Result<(LoweredDirectExpression, ArithmeticDomain), LoweringError> {
    match checked.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let position = direct_parameter_position(checked, path, parameters)?;
            let scalar_type = parameter_types[position];
            Ok((
                LoweredDirectExpression::Parameter {
                    position,
                    scalar_type,
                },
                checked.arithmetic_domain_for_type_reference(parameters[position].type_reference),
            ))
        }
        ExpressionNode::Integer(literal) => {
            let scalar_type = integer_landing_scalar_type(literal)?;
            Ok((
                LoweredDirectExpression::IntegerLiteral {
                    value: integer_value(literal, scalar_type)?,
                    scalar_type,
                },
                literal
                    .landing()
                    .map(|landing| landing.domain)
                    .unwrap_or(ArithmeticDomain::Exact),
            ))
        }
        ExpressionNode::Mutable(_) => {
            unsupported("direct terminal expressions do not support mutable-place wrappers")
        }
        ExpressionNode::Binary(binary) => {
            if let Some(operator_use) = checked.facts.operators.expression_use(expression)
                && operator_use.status != CheckedOperatorResolutionStatus::BuiltinFallback
            {
                return unsupported(
                    "terminal integer binary expression must use the builtin operator",
                );
            }
            let (left, left_domain) =
                lower_direct_expression(checked, binary.left, parameters, parameter_types)?;
            let (right, right_domain) =
                lower_direct_expression(checked, binary.right, parameters, parameter_types)?;
            let shift = matches!(
                binary.operator,
                BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
            );
            let domain = if shift {
                left_domain
            } else {
                combine_terminal_arithmetic_domains(left_domain, right_domain)?
            };
            let kind = lowered_integer_binary_kind(binary.operator, domain)?;
            let scalar_type = left.scalar_type();
            if !matches!(scalar_type, ScalarType::Integer(_))
                || !matches!(right.scalar_type(), ScalarType::Integer(_))
                || (!shift && right.scalar_type() != scalar_type)
            {
                return unsupported("terminal integer operation has incompatible operand types");
            }
            Ok((
                LoweredDirectExpression::IntegerBinary {
                    kind,
                    scalar_type,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                domain,
            ))
        }
        _ => unsupported("direct-parameter machine must return a supported integer expression"),
    }
}

fn evaluate_direct_expression(
    expression: &LoweredDirectExpression,
    parameters: &[Option<IntegerValue>],
) -> Option<IntegerValue> {
    match expression {
        LoweredDirectExpression::Parameter { position, .. } => {
            parameters.get(*position).copied().flatten()
        }
        LoweredDirectExpression::IntegerLiteral { value, .. } => Some(*value),
        LoweredDirectExpression::IntegerBinary {
            kind,
            scalar_type,
            left,
            right,
        } => {
            let count_type = right.scalar_type();
            let left = evaluate_direct_expression(left, parameters)?;
            let right = evaluate_direct_expression(right, parameters)?;
            evaluate_lowered_integer_binary(*kind, *scalar_type, count_type, left, right)
        }
        LoweredDirectExpression::Boolean { .. } => None,
    }
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
    parameters: &[Option<IntegerValue>],
) -> Option<bool> {
    match expression {
        LoweredBooleanReturnExpression::Constant { value } => Some(*value),
        LoweredBooleanReturnExpression::Parameter { .. } => None,
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
            let left = evaluate_direct_expression(left, parameters)?;
            let right = evaluate_direct_expression(right, parameters)?;
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

fn combine_terminal_arithmetic_domains(
    left: ArithmeticDomain,
    right: ArithmeticDomain,
) -> Result<ArithmeticDomain, LoweringError> {
    match (left, right) {
        (ArithmeticDomain::Exact, domain) | (domain, ArithmeticDomain::Exact) => Ok(domain),
        (left, right) if left == right => Ok(left),
        _ => unsupported("terminal integer binary expression cannot mix arithmetic domains"),
    }
}

fn direct_parameter_position(
    checked: &CheckedTrees,
    path: &psi_checked_trees::expression::TableNamePath,
    parameters: &[psi_checked_trees::signature::StateParameter],
) -> Result<usize, LoweringError> {
    if checked
        .expression_table
        .name_path_members(path.members)
        .len()
        != 1
    {
        return unsupported("direct expression operand must name one declared parameter");
    }
    parameters
        .iter()
        .position(|parameter| {
            parameter.symbol == path.symbol || parameter.symbol == path.head_symbol
        })
        .ok_or(LoweringError::Unsupported(
            "direct expression operand must name one declared parameter",
        ))
}

fn lowered_integer_binary_kind(
    operator: BinaryOperator,
    domain: ArithmeticDomain,
) -> Result<LoweredIntegerBinaryKind, LoweringError> {
    match (operator, domain) {
        (BinaryOperator::BitwiseAnd, _) => Ok(LoweredIntegerBinaryKind::BitwiseAnd),
        (BinaryOperator::BitwiseOr, _) => Ok(LoweredIntegerBinaryKind::BitwiseOr),
        (BinaryOperator::BitwiseXor, _) => Ok(LoweredIntegerBinaryKind::BitwiseXor),
        (BinaryOperator::ShiftLeft, ArithmeticDomain::Wrapping) => {
            Ok(LoweredIntegerBinaryKind::WrappingShiftLeft)
        }
        (BinaryOperator::ShiftRight, ArithmeticDomain::Wrapping) => {
            Ok(LoweredIntegerBinaryKind::WrappingShiftRight)
        }
        (BinaryOperator::Add, ArithmeticDomain::Wrapping) => {
            Ok(LoweredIntegerBinaryKind::WrappingAdd)
        }
        (BinaryOperator::Add, ArithmeticDomain::Saturating) => {
            Ok(LoweredIntegerBinaryKind::SaturatingAdd)
        }
        (BinaryOperator::Subtract, ArithmeticDomain::Wrapping) => {
            Ok(LoweredIntegerBinaryKind::WrappingSubtract)
        }
        (BinaryOperator::Subtract, ArithmeticDomain::Saturating) => {
            Ok(LoweredIntegerBinaryKind::SaturatingSubtract)
        }
        (BinaryOperator::Multiply, ArithmeticDomain::Wrapping) => {
            Ok(LoweredIntegerBinaryKind::WrappingMultiply)
        }
        (BinaryOperator::Multiply, ArithmeticDomain::Saturating) => {
            Ok(LoweredIntegerBinaryKind::SaturatingMultiply)
        }
        (BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply, _) => {
            unsupported("terminal integer binary expression requires Wrapping or Saturating")
        }
        _ => unsupported("terminal source producer does not support this integer operation"),
    }
}

fn lower_content_evidence(
    checked: &CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
    state: &psi_checked_trees::state::State,
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
        .filter(|fact| fact.machine_symbol == machine.symbol && fact.state_symbol == state.symbol)
        .cloned()
        .collect::<Vec<_>>();
    let mut identity_reshuffles = lower_content_identity_reshuffles(&identity_facts)?;
    let partition_facts = checked
        .facts
        .qualifications
        .content
        .partition_compositions
        .iter()
        .filter(|fact| fact.machine_symbol == machine.symbol && fact.state_symbol == state.symbol)
        .cloned()
        .collect::<Vec<_>>();
    let partition_compositions =
        lower_content_partition_compositions(&partition_facts, &mut identity_reshuffles)?;
    Ok((identity_reshuffles, partition_compositions))
}

fn validate_contract(
    checked: &CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
    result_type: ScalarType,
    expected_value: Option<IntegerValue>,
    allow_crash_contracts: bool,
) -> Result<IntegerValue, LoweringError> {
    let contracts = checked.machine_contracts(machine);
    let value_contract_count = contracts
        .iter()
        .filter(|contract| {
            matches!(
                contract.kind,
                SignatureContractKind::Requires | SignatureContractKind::Ensures
            )
        })
        .count();
    if value_contract_count != 2
        || (!allow_crash_contracts && contracts.len() != value_contract_count)
        || contracts.iter().any(|contract| {
            !matches!(
                contract.kind,
                SignatureContractKind::Requires
                    | SignatureContractKind::Ensures
                    | SignatureContractKind::Crashes { .. }
            )
        })
    {
        return unsupported("machine must have exactly one requires and one ensures clause");
    };
    let mut shared_value = None;
    for kind in [
        SignatureContractKind::Requires,
        SignatureContractKind::Ensures,
    ] {
        let contract = contracts
            .iter()
            .find(|contract| contract.kind == kind)
            .ok_or(LoweringError::Unsupported(
                "machine must have exactly one requires and one ensures clause",
            ))?;
        let facts = checked.proof_facts.span_or_empty(contract.facts);
        let [ProofFact::Expression(fact)] = facts else {
            return unsupported("each contract clause must contain exactly one expression fact");
        };
        let ExpressionNode::Binary(binary) = checked.expression_table.expression(*fact) else {
            return unsupported("contract facts must be equalities");
        };
        if binary.operator != BinaryOperator::Equal {
            return unsupported("contract facts must be equalities");
        }
        let (left_literal, right_literal) = match (
            checked.expression_table.expression(binary.left),
            checked.expression_table.expression(binary.right),
        ) {
            (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => (left, right),
            _ => {
                return unsupported(
                    "contract facts must have the form `integer-literal == integer-literal`",
                );
            }
        };
        let left = integer_value(left_literal, result_type)?;
        let right = integer_value(right_literal, result_type)?;
        if left != right {
            return unsupported("contract equality must be reflexive");
        }
        if expected_value.is_some_and(|expected| left != expected) {
            return unsupported("contract literals must equal the executed literal");
        }
        if shared_value.is_some_and(|previous| previous != left) {
            return unsupported("requires and ensures must carry the same closed equality");
        }
        shared_value = Some(left);
    }
    shared_value.ok_or(LoweringError::Unsupported(
        "machine must have exactly one requires and one ensures clause",
    ))
}

fn validate_boolean_contract(
    checked: &CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
    expected_value: Option<bool>,
) -> Result<bool, LoweringError> {
    let contracts = checked.machine_contracts(machine);
    if contracts.len() != 2 {
        return unsupported("machine must have exactly one requires and one ensures clause");
    }
    let mut shared_value = None;
    for kind in [
        SignatureContractKind::Requires,
        SignatureContractKind::Ensures,
    ] {
        let contract = contracts
            .iter()
            .find(|contract| contract.kind == kind)
            .ok_or(LoweringError::Unsupported(
                "machine must have exactly one requires and one ensures clause",
            ))?;
        let facts = checked.proof_facts.span_or_empty(contract.facts);
        let [ProofFact::Expression(fact)] = facts else {
            return unsupported("each contract clause must contain exactly one expression fact");
        };
        let ExpressionNode::Binary(binary) = checked.expression_table.expression(*fact) else {
            return unsupported("contract facts must be equalities");
        };
        if binary.operator != BinaryOperator::Equal {
            return unsupported("contract facts must be equalities");
        }
        let (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) = (
            checked.expression_table.expression(binary.left),
            checked.expression_table.expression(binary.right),
        ) else {
            return unsupported("Boolean contract facts must compare Boolean literals");
        };
        if left != right {
            return unsupported("contract equality must be reflexive");
        }
        if expected_value.is_some_and(|expected| *left != expected) {
            return unsupported("Boolean contract literal must match the compile-known result");
        }
        if shared_value.is_some_and(|previous| previous != *left) {
            return unsupported("requires and ensures must carry the same closed equality");
        }
        shared_value = Some(*left);
    }
    shared_value.ok_or(LoweringError::Unsupported(
        "machine must have exactly one requires and one ensures clause",
    ))
}

fn integer_scalar_type(primitive: PrimitiveType) -> Result<ScalarType, LoweringError> {
    let (sign, bits) = match primitive {
        PrimitiveType::I8 => (IntegerSign::Signed, 8),
        PrimitiveType::I16 => (IntegerSign::Signed, 16),
        PrimitiveType::I32 => (IntegerSign::Signed, 32),
        PrimitiveType::I64 => (IntegerSign::Signed, 64),
        PrimitiveType::U8 => (IntegerSign::Unsigned, 8),
        PrimitiveType::U16 => (IntegerSign::Unsigned, 16),
        PrimitiveType::U32 => (IntegerSign::Unsigned, 32),
        PrimitiveType::U64 | PrimitiveType::Addr => (IntegerSign::Unsigned, 64),
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
fn build_integer_conditional_module(
    parameter_types: &[ScalarType],
    condition: LoweredBooleanReturnExpression,
    when_true_arguments: Vec<usize>,
    when_false_arguments: Vec<usize>,
    when_true_parameter_types: Vec<ScalarType>,
    when_false_parameter_types: Vec<ScalarType>,
    when_true_expression: LoweredDirectExpression,
    when_false_expression: LoweredDirectExpression,
    result_type: ScalarType,
    contract_value: IntegerValue,
    identity_reshuffles: LoweredContentIdentityReshuffles,
    partition_compositions: LoweredContentPartitionCompositions,
) -> LoweredTerminalPsi {
    let parameters = parameter_types
        .iter()
        .enumerate()
        .map(|(index, scalar_type)| ValueDeclaration {
            id: value_id(
                u64::try_from(index)
                    .expect("parameter index fits a semantic identity")
                    .checked_add(1)
                    .expect("parameter identity is nonzero"),
            ),
            scalar_type: *scalar_type,
        })
        .collect::<Vec<_>>();
    let mut next_value_identity = u64::try_from(parameters.len())
        .expect("parameter count fits a semantic identity")
        .checked_add(1)
        .expect("generated identities follow parameter identities");
    let mut all_operations = Vec::new();
    let condition = emit_boolean_expression(
        &condition,
        &parameters,
        &mut next_value_identity,
        &mut all_operations,
    );
    let entry_operation_end = all_operations.len();
    let mut allocate_parameters = |types: &[ScalarType]| {
        types
            .iter()
            .map(|scalar_type| {
                let parameter = ValueDeclaration {
                    id: value_id(next_value_identity),
                    scalar_type: *scalar_type,
                };
                next_value_identity = next_value_identity
                    .checked_add(1)
                    .expect("branch parameter identities advance");
                parameter
            })
            .collect::<Vec<_>>()
    };
    let true_parameters = allocate_parameters(&when_true_parameter_types);
    let false_parameters = allocate_parameters(&when_false_parameter_types);
    let true_operation_start = all_operations.len();
    let true_value = emit_direct_expression(
        &when_true_expression,
        &true_parameters,
        &mut next_value_identity,
        &mut all_operations,
    );
    let true_operation_end = all_operations.len();
    let false_value = emit_direct_expression(
        &when_false_expression,
        &false_parameters,
        &mut next_value_identity,
        &mut all_operations,
    );
    let result = ValueDeclaration {
        id: value_id(next_value_identity),
        scalar_type: result_type,
    };

    let ScalarType::Integer(integer_type) = result_type else {
        unreachable!("conditional source slice has an integer result")
    };
    let literal = ScalarTerm::integer(integer_type, contract_value)
        .expect("validated source contract fits the result type");
    let goal = Proposition::Equal(literal.clone(), literal);
    let obligation = obligation_id(1);
    let mut structural_places = identity_reshuffles
        .structural_places
        .into_iter()
        .map(|place| (place.id, place.kind))
        .collect::<BTreeMap<_, _>>();
    for place in partition_compositions.structural_places {
        merge_content_place_declaration(&mut structural_places, place)
            .expect("checked lowering rejects conflicting structural places");
    }

    LoweredTerminalPsi {
        semantic_module: TerminalModule {
            semantic_version: SemanticVersion::CURRENT,
            entry: machine_id(1),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            machines: vec![TerminalMachine {
                id: machine_id(1),
                parameters: parameters.clone(),
                result,
                structural_places: structural_places
                    .into_iter()
                    .map(|(id, kind)| StructuralPlaceDeclaration { id, kind })
                    .collect(),
                content_entry_claims: identity_reshuffles.entry_claims,
                content_identity_reshuffles: identity_reshuffles.reshuffles,
                content_partition_compositions: partition_compositions.compositions,
                entry: block_id(1),
                blocks: vec![
                    Block {
                        id: block_id(1),
                        parameters: Vec::new(),
                        operations: all_operations[..entry_operation_end].to_vec(),
                        terminator: Terminator::Conditional {
                            condition,
                            when_true: SuccessorEdge {
                                edge: edge_id(1),
                                target: block_id(2),
                                arguments: when_true_arguments
                                    .iter()
                                    .map(|position| parameters[*position].id)
                                    .collect(),
                            },
                            when_false: SuccessorEdge {
                                edge: edge_id(2),
                                target: block_id(3),
                                arguments: when_false_arguments
                                    .iter()
                                    .map(|position| parameters[*position].id)
                                    .collect(),
                            },
                        },
                    },
                    Block {
                        id: block_id(2),
                        parameters: true_parameters,
                        operations: all_operations[true_operation_start..true_operation_end]
                            .to_vec(),
                        terminator: Terminator::Return {
                            edge: edge_id(3),
                            value: true_value,
                        },
                    },
                    Block {
                        id: block_id(3),
                        parameters: false_parameters,
                        operations: all_operations[true_operation_end..].to_vec(),
                        terminator: Terminator::Return {
                            edge: edge_id(4),
                            value: false_value,
                        },
                    },
                ],
                contract: MachineContract {
                    id: contract_id(1),
                    crash_context: psi_terminal::CrashContextMaximum::portable_root(),
                    requires: vec![goal.clone()],
                    ensures: vec![ContractClause {
                        obligation,
                        proposition: goal,
                    }],
                },
            }],
        },
        proof_bundle: ProofBundle {
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::ClosedIntegerRelation),
            }],
        },
        debug_map: None,
    }
}

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
fn emit_boolean_guard_decision_blocks(
    decision: &LoweredBooleanDecision,
    parameters: &[ValueDeclaration],
    when_true_target: &LoweredBooleanDecisionTarget,
    when_false_target: &LoweredBooleanDecisionTarget,
    next_value_identity: &mut u64,
    next_edge_identity: &mut u64,
    all_operations: &mut Vec<Operation>,
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
                u64::try_from(block_index)
                    .expect("block index fits a semantic identity")
                    .checked_add(1)
                    .expect("guard decision block identity is nonzero"),
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
                    .expect("guard false edge identity advances"),
            );
            *next_edge_identity = next_edge_identity
                .checked_add(2)
                .expect("guard decision edge identities advance");
            let when_true = emit_boolean_guard_decision_blocks(
                when_true,
                parameters,
                when_true_target,
                when_false_target,
                next_value_identity,
                next_edge_identity,
                all_operations,
                blocks,
            );
            let when_false = emit_boolean_guard_decision_blocks(
                when_false,
                parameters,
                when_true_target,
                when_false_target,
                next_value_identity,
                next_edge_identity,
                all_operations,
                blocks,
            );
            blocks[block_index] = Some(Block {
                id: block,
                parameters: Vec::new(),
                operations: all_operations[operation_start..operation_end].to_vec(),
                terminator: Terminator::Conditional {
                    condition,
                    when_true: SuccessorEdge {
                        edge: true_edge,
                        target: when_true.block,
                        arguments: when_true.arguments,
                    },
                    when_false: SuccessorEdge {
                        edge: false_edge,
                        target: when_false.block,
                        arguments: when_false.arguments,
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
fn emit_reserved_boolean_guard_decision_blocks(
    decision: &LoweredBooleanDecision,
    parameters: &[ValueDeclaration],
    block_parameters: Vec<ValueDeclaration>,
    when_true_target: &LoweredBooleanDecisionTarget,
    when_false_target: &LoweredBooleanDecisionTarget,
    first_block_identity: u64,
    next_value_identity: &mut u64,
    next_edge_identity: &mut u64,
    all_operations: &mut Vec<Operation>,
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
                    },
                    when_false: SuccessorEdge {
                        edge: false_edge,
                        target: when_false.block,
                        arguments: when_false.arguments,
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
    all_operations: &mut Vec<Operation>,
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
                LoweredBooleanDecisionExit::Return => Terminator::Return { edge, value },
                LoweredBooleanDecisionExit::Jump { target } => Terminator::Jump {
                    edge,
                    target,
                    arguments: vec![value],
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
                    },
                    when_false: SuccessorEdge {
                        edge: false_edge,
                        target: when_false,
                        arguments: Vec::new(),
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
fn emit_reserved_boolean_tuple_stage_blocks(
    decision: &LoweredBooleanDecision,
    parameters: &[ValueDeclaration],
    block_parameters: Vec<ValueDeclaration>,
    next_stage: BlockId,
    carried_arguments: &[ValueId],
    first_block_identity: u64,
    next_value_identity: &mut u64,
    next_edge_identity: &mut u64,
    all_operations: &mut Vec<Operation>,
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
                    },
                    when_false: SuccessorEdge {
                        edge: false_edge,
                        target: when_false,
                        arguments: Vec::new(),
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
fn emit_boolean_decision_blocks(
    decision: &LoweredBooleanDecision,
    expression_parameters: &[ValueDeclaration],
    block_parameters: Vec<ValueDeclaration>,
    exit: LoweredBooleanDecisionExit,
    next_value_identity: &mut u64,
    next_edge_identity: &mut u64,
    all_operations: &mut Vec<Operation>,
    blocks: &mut Vec<Option<Block>>,
) -> BlockId {
    let block_index = blocks.len();
    let block = block_id(
        u64::try_from(block_index)
            .expect("block index fits a semantic identity")
            .checked_add(1)
            .expect("block identity is nonzero"),
    );
    blocks.push(None);
    let operation_start = all_operations.len();
    let (terminator, operation_end) = match decision {
        LoweredBooleanDecision::Value(expression) => {
            let returned = emit_boolean_expression(
                expression,
                expression_parameters,
                next_value_identity,
                all_operations,
            );
            let edge = edge_id(*next_edge_identity);
            *next_edge_identity = next_edge_identity
                .checked_add(1)
                .expect("short-circuit exit edge identities advance");
            let terminator = match exit {
                LoweredBooleanDecisionExit::Return => Terminator::Return {
                    edge,
                    value: returned,
                },
                LoweredBooleanDecisionExit::Jump { target } => Terminator::Jump {
                    edge,
                    target,
                    arguments: vec![returned],
                },
            };
            (terminator, all_operations.len())
        }
        LoweredBooleanDecision::Test {
            condition,
            when_true,
            when_false,
        } => {
            let condition = emit_boolean_expression(
                condition,
                expression_parameters,
                next_value_identity,
                all_operations,
            );
            let operation_end = all_operations.len();
            let true_edge = edge_id(*next_edge_identity);
            let false_edge = edge_id(
                next_edge_identity
                    .checked_add(1)
                    .expect("short-circuit false edge identity advances"),
            );
            *next_edge_identity = next_edge_identity
                .checked_add(2)
                .expect("short-circuit conditional edge identities advance");
            let true_target = emit_boolean_decision_blocks(
                when_true,
                expression_parameters,
                Vec::new(),
                exit,
                next_value_identity,
                next_edge_identity,
                all_operations,
                blocks,
            );
            let false_target = emit_boolean_decision_blocks(
                when_false,
                expression_parameters,
                Vec::new(),
                exit,
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
                        target: true_target,
                        arguments: Vec::new(),
                    },
                    when_false: SuccessorEdge {
                        edge: false_edge,
                        target: false_target,
                        arguments: Vec::new(),
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
fn emit_boolean_return_blocks(
    expression: &LoweredBooleanReturnExpression,
    parameters: &[ValueDeclaration],
    next_value_identity: &mut u64,
    next_edge_identity: &mut u64,
    all_operations: &mut Vec<Operation>,
    blocks: &mut Vec<Option<Block>>,
) -> BlockId {
    if contains_short_circuit(expression) {
        let decision = lower_boolean_value_decision(expression);
        return emit_boolean_decision_blocks(
            &decision,
            parameters,
            parameters.to_vec(),
            LoweredBooleanDecisionExit::Return,
            next_value_identity,
            next_edge_identity,
            all_operations,
            blocks,
        );
    }

    let block = block_id(
        u64::try_from(blocks.len())
            .expect("block count fits a semantic identity")
            .checked_add(1)
            .expect("Boolean return block identity is nonzero"),
    );
    let operation_start = all_operations.len();
    let value =
        emit_boolean_expression(expression, parameters, next_value_identity, all_operations);
    let edge = edge_id(*next_edge_identity);
    *next_edge_identity = next_edge_identity
        .checked_add(1)
        .expect("Boolean return edge identities advance");
    blocks.push(Some(Block {
        id: block,
        parameters: parameters.to_vec(),
        operations: all_operations[operation_start..].to_vec(),
        terminator: Terminator::Return { edge, value },
    }));
    block
}

fn build_boolean_short_circuit_module(
    parameter_count: usize,
    return_expression: LoweredBooleanReturnExpression,
    contract_value: bool,
    identity_reshuffles: LoweredContentIdentityReshuffles,
    partition_compositions: LoweredContentPartitionCompositions,
) -> LoweredTerminalPsi {
    let parameters = (0..parameter_count)
        .map(|index| ValueDeclaration {
            id: value_id(
                u64::try_from(index)
                    .expect("parameter index fits a semantic identity")
                    .checked_add(1)
                    .expect("parameter identity is nonzero"),
            ),
            scalar_type: ScalarType::Boolean,
        })
        .collect::<Vec<_>>();
    let mut next_value_identity = u64::try_from(parameter_count)
        .expect("parameter count fits a semantic identity")
        .checked_add(1)
        .expect("generated identities follow parameter identities");
    let decision = lower_boolean_value_decision(&return_expression);
    let mut all_operations = Vec::new();
    let mut blocks = Vec::new();
    let mut next_edge_identity = 1_u64;
    let entry = emit_boolean_decision_blocks(
        &decision,
        &parameters,
        Vec::new(),
        LoweredBooleanDecisionExit::Return,
        &mut next_value_identity,
        &mut next_edge_identity,
        &mut all_operations,
        &mut blocks,
    );
    let result = ValueDeclaration {
        id: value_id(next_value_identity),
        scalar_type: ScalarType::Boolean,
    };
    let literal = ScalarTerm::boolean(contract_value);
    let goal = Proposition::Equal(literal.clone(), literal);
    let obligation = obligation_id(1);
    let mut structural_places = identity_reshuffles
        .structural_places
        .into_iter()
        .map(|place| (place.id, place.kind))
        .collect::<BTreeMap<_, _>>();
    for place in partition_compositions.structural_places {
        merge_content_place_declaration(&mut structural_places, place)
            .expect("checked lowering rejects conflicting structural places");
    }
    LoweredTerminalPsi {
        semantic_module: TerminalModule {
            semantic_version: SemanticVersion::CURRENT,
            entry: machine_id(1),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            machines: vec![TerminalMachine {
                id: machine_id(1),
                parameters,
                result,
                structural_places: structural_places
                    .into_iter()
                    .map(|(id, kind)| StructuralPlaceDeclaration { id, kind })
                    .collect(),
                content_entry_claims: identity_reshuffles.entry_claims,
                content_identity_reshuffles: identity_reshuffles.reshuffles,
                content_partition_compositions: partition_compositions.compositions,
                entry,
                blocks: blocks
                    .into_iter()
                    .map(|block| block.expect("every decision block is finalized"))
                    .collect(),
                contract: MachineContract {
                    id: contract_id(1),
                    crash_context: psi_terminal::CrashContextMaximum::portable_root(),
                    requires: vec![goal.clone()],
                    ensures: vec![ContractClause {
                        obligation,
                        proposition: goal,
                    }],
                },
            }],
        },
        proof_bundle: ProofBundle {
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::ReflexiveEquality),
            }],
        },
        debug_map: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_nested_conditional_target(
    target: usize,
    arguments: &[LoweredDirectExpression],
    current_parameters: &[ValueDeclaration],
    current_parameter_types: &[ScalarType],
    next_block_identity: &mut u64,
    next_value_identity: &mut u64,
    pending_blocks: &mut Vec<PendingNestedBlockGroup>,
) -> LoweredBooleanDecisionTarget {
    let direct_arguments = arguments
        .iter()
        .map(|argument| match argument {
            LoweredDirectExpression::Parameter { position, .. } => {
                Some(current_parameters[*position].id)
            }
            LoweredDirectExpression::Boolean { expression } => match expression.as_ref() {
                LoweredBooleanReturnExpression::Parameter { position } => {
                    Some(current_parameters[*position].id)
                }
                _ => None,
            },
            LoweredDirectExpression::IntegerLiteral { .. }
            | LoweredDirectExpression::IntegerBinary { .. } => None,
        })
        .collect::<Option<Vec<_>>>();
    if let Some(arguments) = direct_arguments {
        return LoweredBooleanDecisionTarget {
            block: block_id(
                u64::try_from(target)
                    .expect("state index fits a semantic identity")
                    .checked_add(1)
                    .expect("block identity is nonzero"),
            ),
            arguments,
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
            target,
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

#[allow(clippy::too_many_arguments)]
fn build_nested_boolean_conditional_target(
    target: usize,
    arguments: &[LoweredBooleanReturnExpression],
    current_parameters: &[ValueDeclaration],
    next_block_identity: &mut u64,
    next_value_identity: &mut u64,
    pending_blocks: &mut Vec<PendingBooleanBlockGroup>,
) -> LoweredBooleanDecisionTarget {
    let target = block_id(
        u64::try_from(target)
            .expect("state index fits a semantic identity")
            .checked_add(1)
            .expect("block identity is nonzero"),
    );
    let direct_arguments = arguments
        .iter()
        .map(|argument| match argument {
            LoweredBooleanReturnExpression::Parameter { position } => {
                Some(current_parameters[*position].id)
            }
            LoweredBooleanReturnExpression::Constant { .. }
            | LoweredBooleanReturnExpression::Not { .. }
            | LoweredBooleanReturnExpression::Equal { .. }
            | LoweredBooleanReturnExpression::IntegerComparison { .. }
            | LoweredBooleanReturnExpression::And { .. }
            | LoweredBooleanReturnExpression::Or { .. } => None,
        })
        .collect::<Option<Vec<_>>>();
    if let Some(arguments) = direct_arguments {
        return LoweredBooleanDecisionTarget {
            block: target,
            arguments,
        };
    }

    let first_id = block_id(*next_block_identity);
    if let [argument] = arguments
        && contains_short_circuit(argument)
    {
        let decision = lower_boolean_value_decision(argument);
        *next_block_identity = next_block_identity
            .checked_add(
                u64::try_from(boolean_decision_block_count(&decision))
                    .expect("Boolean edge binding block count fits a semantic identity"),
            )
            .expect("Boolean edge binding block identities advance");
        let parameters = current_parameters
            .iter()
            .map(|_| {
                let parameter = ValueDeclaration {
                    id: value_id(*next_value_identity),
                    scalar_type: ScalarType::Boolean,
                };
                *next_value_identity = next_value_identity
                    .checked_add(1)
                    .expect("Boolean edge binding parameter identities advance");
                parameter
            })
            .collect::<Vec<_>>();
        pending_blocks.push(PendingBooleanBlockGroup::Value(PendingBooleanValueBlocks {
            first_id,
            parameters,
            decision,
            exit: LoweredBooleanDecisionExit::Jump { target },
        }));
    } else if arguments.iter().any(contains_short_circuit) {
        let reserved_block_count = arguments
            .iter()
            .map(|argument| {
                if contains_short_circuit(argument) {
                    boolean_decision_block_count(&lower_boolean_value_decision(argument))
                } else {
                    1
                }
            })
            .sum::<usize>()
            .checked_add(1)
            .expect("Boolean edge tuple convergence block count advances");
        *next_block_identity = next_block_identity
            .checked_add(
                u64::try_from(reserved_block_count)
                    .expect("Boolean edge tuple block count fits a semantic identity"),
            )
            .expect("Boolean edge tuple block identities advance");
        let stage_parameters = (0..=arguments.len())
            .map(|completed_argument_count| {
                (0..current_parameters.len() + completed_argument_count)
                    .map(|_| {
                        let parameter = ValueDeclaration {
                            id: value_id(*next_value_identity),
                            scalar_type: ScalarType::Boolean,
                        };
                        *next_value_identity = next_value_identity
                            .checked_add(1)
                            .expect("Boolean edge tuple parameter identities advance");
                        parameter
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        pending_blocks.push(PendingBooleanBlockGroup::TupleBinding(
            PendingBooleanTupleBindingBlocks {
                first_id,
                original_parameter_count: current_parameters.len(),
                arguments: arguments.to_vec(),
                stage_parameters,
                target,
            },
        ));
    } else {
        *next_block_identity = next_block_identity
            .checked_add(1)
            .expect("Boolean direct edge binding block identities advance");
        let parameters = current_parameters
            .iter()
            .map(|_| {
                let parameter = ValueDeclaration {
                    id: value_id(*next_value_identity),
                    scalar_type: ScalarType::Boolean,
                };
                *next_value_identity = next_value_identity
                    .checked_add(1)
                    .expect("Boolean direct edge binding parameter identities advance");
                parameter
            })
            .collect::<Vec<_>>();
        pending_blocks.push(PendingBooleanBlockGroup::DirectBinding(
            PendingBooleanDirectBindingBlock {
                id: first_id,
                parameters,
                target,
                arguments: arguments.to_vec(),
            },
        ));
    }

    LoweredBooleanDecisionTarget {
        block: first_id,
        arguments: current_parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect(),
    }
}

fn build_nested_boolean_branch_module(
    states: &[LoweredBooleanBranchState],
    contract_value: bool,
    identity_reshuffles: LoweredContentIdentityReshuffles,
    partition_compositions: LoweredContentPartitionCompositions,
) -> LoweredTerminalPsi {
    let parameters = (0..states[0].parameter_count)
        .map(|index| ValueDeclaration {
            id: value_id(
                u64::try_from(index)
                    .expect("parameter index fits a semantic identity")
                    .checked_add(1)
                    .expect("parameter identity is nonzero"),
            ),
            scalar_type: ScalarType::Boolean,
        })
        .collect::<Vec<_>>();
    let mut next_value_identity = u64::try_from(parameters.len())
        .expect("parameter count fits a semantic identity")
        .checked_add(1)
        .expect("nested Boolean values follow machine parameters");
    let mut state_parameters = Vec::with_capacity(states.len());
    state_parameters.push(parameters.clone());
    for state in &states[1..] {
        state_parameters.push(
            (0..state.parameter_count)
                .map(|_| {
                    let parameter = ValueDeclaration {
                        id: value_id(next_value_identity),
                        scalar_type: ScalarType::Boolean,
                    };
                    next_value_identity = next_value_identity
                        .checked_add(1)
                        .expect("nested Boolean block parameter identities advance");
                    parameter
                })
                .collect(),
        );
    }

    let mut all_operations = Vec::new();
    let mut next_edge_identity = 1_u64;
    let mut next_block_identity = u64::try_from(states.len())
        .expect("Boolean state count fits a semantic identity")
        .checked_add(1)
        .expect("Boolean decision blocks follow source blocks");
    let mut pending_blocks = Vec::new();
    let mut blocks = Vec::with_capacity(states.len());
    for (index, state) in states.iter().enumerate() {
        let operation_start = all_operations.len();
        let current_parameters = &state_parameters[index];
        let terminator = match &state.terminator {
            LoweredBooleanBranchTerminator::Jump { target, arguments } => {
                let target = block_id(
                    u64::try_from(*target)
                        .expect("state index fits a semantic identity")
                        .checked_add(1)
                        .expect("block identity is nonzero"),
                );
                if arguments.len() > 1 && arguments.iter().any(contains_short_circuit) {
                    let first_id = block_id(next_block_identity);
                    let reserved_block_count = arguments
                        .iter()
                        .map(|argument| {
                            if contains_short_circuit(argument) {
                                boolean_decision_block_count(&lower_boolean_value_decision(
                                    argument,
                                ))
                            } else {
                                1
                            }
                        })
                        .sum::<usize>()
                        .checked_add(1)
                        .expect("Boolean tuple convergence block count advances");
                    next_block_identity = next_block_identity
                        .checked_add(
                            u64::try_from(reserved_block_count)
                                .expect("Boolean tuple block count fits a semantic identity"),
                        )
                        .expect("Boolean tuple block identities advance");
                    let stage_parameters = (0..=arguments.len())
                        .map(|completed_argument_count| {
                            (0..state.parameter_count + completed_argument_count)
                                .map(|_| {
                                    let parameter = ValueDeclaration {
                                        id: value_id(next_value_identity),
                                        scalar_type: ScalarType::Boolean,
                                    };
                                    next_value_identity = next_value_identity
                                        .checked_add(1)
                                        .expect("Boolean tuple parameter identities advance");
                                    parameter
                                })
                                .collect::<Vec<_>>()
                        })
                        .collect::<Vec<_>>();
                    pending_blocks.push(PendingBooleanBlockGroup::TupleBinding(
                        PendingBooleanTupleBindingBlocks {
                            first_id,
                            original_parameter_count: state.parameter_count,
                            arguments: arguments.clone(),
                            stage_parameters,
                            target,
                        },
                    ));
                    let edge = edge_id(next_edge_identity);
                    next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("Boolean tuple entry edge identity advances");
                    Terminator::Jump {
                        edge,
                        target: first_id,
                        arguments: current_parameters
                            .iter()
                            .map(|parameter| parameter.id)
                            .collect(),
                    }
                } else if let [argument] = arguments.as_slice()
                    && contains_short_circuit(argument)
                {
                    let decision = lower_boolean_value_decision(argument);
                    let first_id = block_id(next_block_identity);
                    next_block_identity = next_block_identity
                        .checked_add(
                            u64::try_from(boolean_decision_block_count(&decision))
                                .expect("Boolean binding block count fits a semantic identity"),
                        )
                        .expect("Boolean binding block identities advance");
                    let decision_parameters = (0..state.parameter_count)
                        .map(|_| {
                            let parameter = ValueDeclaration {
                                id: value_id(next_value_identity),
                                scalar_type: ScalarType::Boolean,
                            };
                            next_value_identity = next_value_identity
                                .checked_add(1)
                                .expect("Boolean binding parameter identities advance");
                            parameter
                        })
                        .collect::<Vec<_>>();
                    pending_blocks.push(PendingBooleanBlockGroup::Value(
                        PendingBooleanValueBlocks {
                            first_id,
                            parameters: decision_parameters,
                            decision,
                            exit: LoweredBooleanDecisionExit::Jump { target },
                        },
                    ));
                    let edge = edge_id(next_edge_identity);
                    next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("Boolean binding entry edge identity advances");
                    Terminator::Jump {
                        edge,
                        target: first_id,
                        arguments: current_parameters
                            .iter()
                            .map(|parameter| parameter.id)
                            .collect(),
                    }
                } else {
                    let arguments = arguments
                        .iter()
                        .map(|argument| {
                            emit_boolean_expression(
                                argument,
                                current_parameters,
                                &mut next_value_identity,
                                &mut all_operations,
                            )
                        })
                        .collect();
                    let edge = edge_id(next_edge_identity);
                    next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("nested Boolean jump edge identities advance");
                    Terminator::Jump {
                        edge,
                        target,
                        arguments,
                    }
                }
            }
            LoweredBooleanBranchTerminator::Conditional {
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
                    let first_id = block_id(next_block_identity);
                    next_block_identity = next_block_identity
                        .checked_add(
                            u64::try_from(decision_block_count)
                                .expect("Boolean guard block count fits a semantic identity"),
                        )
                        .expect("Boolean guard block identities advance");
                    let decision_parameters = (0..state.parameter_count)
                        .map(|_| {
                            let parameter = ValueDeclaration {
                                id: value_id(next_value_identity),
                                scalar_type: ScalarType::Boolean,
                            };
                            next_value_identity = next_value_identity
                                .checked_add(1)
                                .expect("Boolean guard parameter identities advance");
                            parameter
                        })
                        .collect::<Vec<_>>();
                    let when_true = build_nested_boolean_conditional_target(
                        *when_true_target,
                        when_true_arguments,
                        &decision_parameters,
                        &mut next_block_identity,
                        &mut next_value_identity,
                        &mut pending_blocks,
                    );
                    let when_false = build_nested_boolean_conditional_target(
                        *when_false_target,
                        when_false_arguments,
                        &decision_parameters,
                        &mut next_block_identity,
                        &mut next_value_identity,
                        &mut pending_blocks,
                    );
                    pending_blocks.push(PendingBooleanBlockGroup::Guard(
                        PendingShortCircuitGuardBlocks {
                            first_id,
                            parameters: decision_parameters,
                            decision,
                            when_true,
                            when_false,
                        },
                    ));
                    let edge = edge_id(next_edge_identity);
                    next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("Boolean guard entry edge identity advances");
                    Terminator::Jump {
                        edge,
                        target: first_id,
                        arguments: current_parameters
                            .iter()
                            .map(|parameter| parameter.id)
                            .collect(),
                    }
                } else {
                    let condition = emit_boolean_expression(
                        condition,
                        current_parameters,
                        &mut next_value_identity,
                        &mut all_operations,
                    );
                    let true_edge = edge_id(next_edge_identity);
                    let false_edge = edge_id(
                        next_edge_identity
                            .checked_add(1)
                            .expect("Boolean false edge identity advances"),
                    );
                    next_edge_identity = next_edge_identity
                        .checked_add(2)
                        .expect("Boolean conditional edge identities advance");
                    let when_true = build_nested_boolean_conditional_target(
                        *when_true_target,
                        when_true_arguments,
                        current_parameters,
                        &mut next_block_identity,
                        &mut next_value_identity,
                        &mut pending_blocks,
                    );
                    let when_false = build_nested_boolean_conditional_target(
                        *when_false_target,
                        when_false_arguments,
                        current_parameters,
                        &mut next_block_identity,
                        &mut next_value_identity,
                        &mut pending_blocks,
                    );
                    Terminator::Conditional {
                        condition,
                        when_true: SuccessorEdge {
                            edge: true_edge,
                            target: when_true.block,
                            arguments: when_true.arguments,
                        },
                        when_false: SuccessorEdge {
                            edge: false_edge,
                            target: when_false.block,
                            arguments: when_false.arguments,
                        },
                    }
                }
            }
            LoweredBooleanBranchTerminator::Return { expression } => {
                if contains_short_circuit(expression) {
                    let decision = lower_boolean_value_decision(expression);
                    let first_id = block_id(next_block_identity);
                    next_block_identity = next_block_identity
                        .checked_add(
                            u64::try_from(boolean_decision_block_count(&decision))
                                .expect("Boolean return block count fits a semantic identity"),
                        )
                        .expect("Boolean return block identities advance");
                    let decision_parameters = (0..state.parameter_count)
                        .map(|_| {
                            let parameter = ValueDeclaration {
                                id: value_id(next_value_identity),
                                scalar_type: ScalarType::Boolean,
                            };
                            next_value_identity = next_value_identity
                                .checked_add(1)
                                .expect("Boolean return parameter identities advance");
                            parameter
                        })
                        .collect::<Vec<_>>();
                    pending_blocks.push(PendingBooleanBlockGroup::Value(
                        PendingBooleanValueBlocks {
                            first_id,
                            parameters: decision_parameters,
                            decision,
                            exit: LoweredBooleanDecisionExit::Return,
                        },
                    ));
                    let edge = edge_id(next_edge_identity);
                    next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("Boolean return entry edge identity advances");
                    Terminator::Jump {
                        edge,
                        target: first_id,
                        arguments: current_parameters
                            .iter()
                            .map(|parameter| parameter.id)
                            .collect(),
                    }
                } else {
                    let value = emit_boolean_expression(
                        expression,
                        current_parameters,
                        &mut next_value_identity,
                        &mut all_operations,
                    );
                    let edge = edge_id(next_edge_identity);
                    next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("nested Boolean return edge identity advances");
                    Terminator::Return { edge, value }
                }
            }
        };
        blocks.push(Block {
            id: block_id(
                u64::try_from(index)
                    .expect("state index fits a semantic identity")
                    .checked_add(1)
                    .expect("block identity is nonzero"),
            ),
            parameters: if index == 0 {
                Vec::new()
            } else {
                current_parameters.clone()
            },
            operations: all_operations[operation_start..].to_vec(),
            terminator,
        });
    }
    pending_blocks.sort_by_key(PendingBooleanBlockGroup::first_id);
    for pending in pending_blocks {
        let mut decision_blocks = Vec::new();
        match pending {
            PendingBooleanBlockGroup::Guard(pending) => {
                let entry = emit_reserved_boolean_guard_decision_blocks(
                    &pending.decision,
                    &pending.parameters,
                    pending.parameters.clone(),
                    &pending.when_true,
                    &pending.when_false,
                    pending.first_id.get(),
                    &mut next_value_identity,
                    &mut next_edge_identity,
                    &mut all_operations,
                    &mut decision_blocks,
                );
                assert_eq!(entry.block, pending.first_id);
            }
            PendingBooleanBlockGroup::Value(pending) => {
                let entry = emit_reserved_boolean_value_blocks(
                    &pending.decision,
                    &pending.parameters,
                    pending.parameters.clone(),
                    pending.exit,
                    pending.first_id.get(),
                    &mut next_value_identity,
                    &mut next_edge_identity,
                    &mut all_operations,
                    &mut decision_blocks,
                );
                assert_eq!(entry, pending.first_id);
            }
            PendingBooleanBlockGroup::TupleBinding(pending) => {
                let mut next_stage_identity = pending.first_id.get();
                for (index, argument) in pending.arguments.iter().enumerate() {
                    let parameters = &pending.stage_parameters[index];
                    let carried_arguments = parameters
                        .iter()
                        .map(|parameter| parameter.id)
                        .collect::<Vec<_>>();
                    if contains_short_circuit(argument) {
                        let decision = lower_boolean_value_decision(argument);
                        let stage_block_count = boolean_decision_block_count(&decision);
                        let next_stage =
                            block_id(
                                next_stage_identity
                                    .checked_add(u64::try_from(stage_block_count).expect(
                                        "Boolean tuple stage count fits a semantic identity",
                                    ))
                                    .expect("Boolean tuple stage block identities advance"),
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
                        decision_blocks.extend(stage_blocks);
                        next_stage_identity = next_stage.get();
                    } else {
                        let operation_start = all_operations.len();
                        let value = emit_boolean_expression(
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
                                .expect("Boolean tuple stage block identity advances"),
                        );
                        let edge = edge_id(next_edge_identity);
                        next_edge_identity = next_edge_identity
                            .checked_add(1)
                            .expect("Boolean tuple stage edge identity advances");
                        decision_blocks.push(Some(Block {
                            id: block_id(next_stage_identity),
                            parameters: parameters.clone(),
                            operations: all_operations[operation_start..].to_vec(),
                            terminator: Terminator::Jump {
                                edge,
                                target: next_stage,
                                arguments,
                            },
                        }));
                        next_stage_identity = next_stage.get();
                    }
                }
                let parameters = pending
                    .stage_parameters
                    .last()
                    .expect("Boolean tuple has a convergence parameter set");
                let edge = edge_id(next_edge_identity);
                next_edge_identity = next_edge_identity
                    .checked_add(1)
                    .expect("Boolean tuple convergence edge identity advances");
                decision_blocks.push(Some(Block {
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
                    },
                }));
            }
            PendingBooleanBlockGroup::DirectBinding(pending) => {
                let operation_start = all_operations.len();
                let arguments = pending
                    .arguments
                    .iter()
                    .map(|argument| {
                        emit_boolean_expression(
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
                    .expect("Boolean direct binding edge identity advances");
                decision_blocks.push(Some(Block {
                    id: pending.id,
                    parameters: pending.parameters,
                    operations: all_operations[operation_start..].to_vec(),
                    terminator: Terminator::Jump {
                        edge,
                        target: pending.target,
                        arguments,
                    },
                }));
            }
        }
        blocks.extend(
            decision_blocks
                .into_iter()
                .map(|block| block.expect("every reserved nested Boolean block is finalized")),
        );
    }

    let result = ValueDeclaration {
        id: value_id(next_value_identity),
        scalar_type: ScalarType::Boolean,
    };
    let literal = ScalarTerm::boolean(contract_value);
    let goal = Proposition::Equal(literal.clone(), literal);
    let obligation = obligation_id(1);
    let mut structural_places = identity_reshuffles
        .structural_places
        .into_iter()
        .map(|place| (place.id, place.kind))
        .collect::<BTreeMap<_, _>>();
    for place in partition_compositions.structural_places {
        merge_content_place_declaration(&mut structural_places, place)
            .expect("checked lowering rejects conflicting structural places");
    }
    LoweredTerminalPsi {
        semantic_module: TerminalModule {
            semantic_version: SemanticVersion::CURRENT,
            entry: machine_id(1),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            machines: vec![TerminalMachine {
                id: machine_id(1),
                parameters,
                result,
                structural_places: structural_places
                    .into_iter()
                    .map(|(id, kind)| StructuralPlaceDeclaration { id, kind })
                    .collect(),
                content_entry_claims: identity_reshuffles.entry_claims,
                content_identity_reshuffles: identity_reshuffles.reshuffles,
                content_partition_compositions: partition_compositions.compositions,
                entry: block_id(1),
                blocks,
                contract: MachineContract {
                    id: contract_id(1),
                    crash_context: psi_terminal::CrashContextMaximum::portable_root(),
                    requires: vec![goal.clone()],
                    ensures: vec![ContractClause {
                        obligation,
                        proposition: goal,
                    }],
                },
            }],
        },
        proof_bundle: ProofBundle {
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::ReflexiveEquality),
            }],
        },
        debug_map: None,
    }
}

fn build_nested_integer_branch_module(
    states: &[LoweredIntegerBranchState],
    result_type: ScalarType,
    contract_value: IntegerValue,
    identity_reshuffles: LoweredContentIdentityReshuffles,
    partition_compositions: LoweredContentPartitionCompositions,
) -> LoweredTerminalPsi {
    let parameters = states[0]
        .parameter_types
        .iter()
        .enumerate()
        .map(|(index, scalar_type)| ValueDeclaration {
            id: value_id(
                u64::try_from(index)
                    .expect("parameter index fits a semantic identity")
                    .checked_add(1)
                    .expect("parameter identity is nonzero"),
            ),
            scalar_type: *scalar_type,
        })
        .collect::<Vec<_>>();
    let mut next_value_identity = u64::try_from(parameters.len())
        .expect("parameter count fits a semantic identity")
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
                        .expect("nested block parameter identities advance");
                    parameter
                })
                .collect(),
        );
    }

    let mut all_operations = Vec::new();
    let mut next_edge_identity = 1_u64;
    let mut next_block_identity = u64::try_from(states.len())
        .expect("state count fits a semantic identity")
        .checked_add(1)
        .expect("conditional binding blocks follow source blocks");
    let mut pending_blocks = Vec::new();
    let mut blocks = Vec::with_capacity(states.len());
    for (index, state) in states.iter().enumerate() {
        let operation_start = all_operations.len();
        let current_parameters = &state_parameters[index];
        let terminator = match &state.terminator {
            LoweredIntegerBranchTerminator::Jump { target, arguments } => {
                let arguments = arguments
                    .iter()
                    .map(|argument| {
                        emit_direct_expression(
                            argument,
                            current_parameters,
                            &mut next_value_identity,
                            &mut all_operations,
                        )
                    })
                    .collect();
                let edge = edge_id(next_edge_identity);
                next_edge_identity = next_edge_identity
                    .checked_add(1)
                    .expect("nested jump edge identities advance");
                Terminator::Jump {
                    edge,
                    target: block_id(
                        u64::try_from(*target)
                            .expect("state index fits a semantic identity")
                            .checked_add(1)
                            .expect("block identity is nonzero"),
                    ),
                    arguments,
                }
            }
            LoweredIntegerBranchTerminator::Conditional {
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
                    let first_id = block_id(next_block_identity);
                    next_block_identity = next_block_identity
                        .checked_add(
                            u64::try_from(decision_block_count)
                                .expect("nested guard block count fits a semantic identity"),
                        )
                        .expect("nested guard block identities advance");
                    let decision_parameters = state
                        .parameter_types
                        .iter()
                        .map(|scalar_type| {
                            let parameter = ValueDeclaration {
                                id: value_id(next_value_identity),
                                scalar_type: *scalar_type,
                            };
                            next_value_identity = next_value_identity
                                .checked_add(1)
                                .expect("nested guard parameter identities advance");
                            parameter
                        })
                        .collect::<Vec<_>>();
                    let when_true = build_nested_conditional_target(
                        *when_true_target,
                        when_true_arguments,
                        &decision_parameters,
                        &state.parameter_types,
                        &mut next_block_identity,
                        &mut next_value_identity,
                        &mut pending_blocks,
                    );
                    let when_false = build_nested_conditional_target(
                        *when_false_target,
                        when_false_arguments,
                        &decision_parameters,
                        &state.parameter_types,
                        &mut next_block_identity,
                        &mut next_value_identity,
                        &mut pending_blocks,
                    );
                    pending_blocks.push(PendingNestedBlockGroup::ShortCircuitGuard(
                        PendingShortCircuitGuardBlocks {
                            first_id,
                            parameters: decision_parameters,
                            decision,
                            when_true,
                            when_false,
                        },
                    ));
                    let edge = edge_id(next_edge_identity);
                    next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("nested guard entry edge identity advances");
                    Terminator::Jump {
                        edge,
                        target: first_id,
                        arguments: current_parameters
                            .iter()
                            .map(|parameter| parameter.id)
                            .collect(),
                    }
                } else {
                    let condition = emit_boolean_expression(
                        condition,
                        current_parameters,
                        &mut next_value_identity,
                        &mut all_operations,
                    );
                    let when_true_edge = edge_id(next_edge_identity);
                    next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("nested branch edge identities advance");
                    let when_false_edge = edge_id(next_edge_identity);
                    next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("nested branch edge identities advance");
                    let when_true = build_nested_conditional_target(
                        *when_true_target,
                        when_true_arguments,
                        current_parameters,
                        &state.parameter_types,
                        &mut next_block_identity,
                        &mut next_value_identity,
                        &mut pending_blocks,
                    );
                    let when_false = build_nested_conditional_target(
                        *when_false_target,
                        when_false_arguments,
                        current_parameters,
                        &state.parameter_types,
                        &mut next_block_identity,
                        &mut next_value_identity,
                        &mut pending_blocks,
                    );
                    Terminator::Conditional {
                        condition,
                        when_true: SuccessorEdge {
                            edge: when_true_edge,
                            target: when_true.block,
                            arguments: when_true.arguments,
                        },
                        when_false: SuccessorEdge {
                            edge: when_false_edge,
                            target: when_false.block,
                            arguments: when_false.arguments,
                        },
                    }
                }
            }
            LoweredIntegerBranchTerminator::Return { expression } => {
                let value = emit_direct_expression(
                    expression,
                    current_parameters,
                    &mut next_value_identity,
                    &mut all_operations,
                );
                let edge = edge_id(next_edge_identity);
                next_edge_identity = next_edge_identity
                    .checked_add(1)
                    .expect("nested return edge identities advance");
                Terminator::Return { edge, value }
            }
            LoweredIntegerBranchTerminator::Crash(crash) => {
                let edge = edge_id(next_edge_identity);
                next_edge_identity = next_edge_identity
                    .checked_add(1)
                    .expect("nested crash edge identities advance");
                Terminator::Crash {
                    edge,
                    cause: crash.cause,
                    damage_minimum: crash.damage_minimum.clone(),
                    containment_demand: crash.containment_demand.clone(),
                    frontier_lower_bound: crash.frontier_lower_bound.clone(),
                }
            }
        };
        blocks.push(Block {
            id: block_id(
                u64::try_from(index)
                    .expect("state index fits a semantic identity")
                    .checked_add(1)
                    .expect("block identity is nonzero"),
            ),
            parameters: if index == 0 {
                Vec::new()
            } else {
                current_parameters.clone()
            },
            operations: all_operations[operation_start..].to_vec(),
            terminator,
        });
    }
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
                        target: block_id(
                            u64::try_from(pending.target)
                                .expect("state index fits a semantic identity")
                                .checked_add(1)
                                .expect("block identity is nonzero"),
                        ),
                        arguments,
                    },
                });
            }
            PendingNestedBlockGroup::ShortCircuitGuard(pending) => {
                let mut decision_blocks = Vec::new();
                let entry = emit_reserved_boolean_guard_decision_blocks(
                    &pending.decision,
                    &pending.parameters,
                    pending.parameters.clone(),
                    &pending.when_true,
                    &pending.when_false,
                    pending.first_id.get(),
                    &mut next_value_identity,
                    &mut next_edge_identity,
                    &mut all_operations,
                    &mut decision_blocks,
                );
                assert_eq!(entry.block, pending.first_id);
                assert!(entry.arguments.is_empty());
                blocks.extend(
                    decision_blocks.into_iter().map(|block| {
                        block.expect("every reserved nested guard block is finalized")
                    }),
                );
            }
        }
    }
    let result = ValueDeclaration {
        id: value_id(next_value_identity),
        scalar_type: result_type,
    };
    let ScalarType::Integer(integer_type) = result_type else {
        unreachable!("nested branch source slice has an integer result")
    };
    let literal = ScalarTerm::integer(integer_type, contract_value)
        .expect("validated source contract fits the result type");
    let goal = Proposition::Equal(literal.clone(), literal);
    let obligation = obligation_id(1);
    let mut structural_places = identity_reshuffles
        .structural_places
        .into_iter()
        .map(|place| (place.id, place.kind))
        .collect::<BTreeMap<_, _>>();
    for place in partition_compositions.structural_places {
        merge_content_place_declaration(&mut structural_places, place)
            .expect("checked lowering rejects conflicting structural places");
    }
    LoweredTerminalPsi {
        semantic_module: TerminalModule {
            semantic_version: SemanticVersion::CURRENT,
            entry: machine_id(1),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            machines: vec![TerminalMachine {
                id: machine_id(1),
                parameters,
                result,
                structural_places: structural_places
                    .into_iter()
                    .map(|(id, kind)| StructuralPlaceDeclaration { id, kind })
                    .collect(),
                content_entry_claims: identity_reshuffles.entry_claims,
                content_identity_reshuffles: identity_reshuffles.reshuffles,
                content_partition_compositions: partition_compositions.compositions,
                entry: block_id(1),
                blocks,
                contract: MachineContract {
                    id: contract_id(1),
                    crash_context: psi_terminal::CrashContextMaximum::portable_root(),
                    requires: vec![goal.clone()],
                    ensures: vec![ContractClause {
                        obligation,
                        proposition: goal,
                    }],
                },
            }],
        },
        proof_bundle: ProofBundle {
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::ClosedIntegerRelation),
            }],
        },
        debug_map: None,
    }
}

fn build_boolean_module(
    parameter_count: usize,
    return_expression: LoweredBooleanReturnExpression,
    contract_value: bool,
    identity_reshuffles: LoweredContentIdentityReshuffles,
    partition_compositions: LoweredContentPartitionCompositions,
) -> LoweredTerminalPsi {
    if contains_short_circuit(&return_expression) {
        return build_boolean_short_circuit_module(
            parameter_count,
            return_expression,
            contract_value,
            identity_reshuffles,
            partition_compositions,
        );
    }
    let parameters = (0..parameter_count)
        .map(|index| ValueDeclaration {
            id: value_id(
                u64::try_from(index)
                    .expect("parameter index fits a semantic identity")
                    .checked_add(1)
                    .expect("parameter identity is nonzero"),
            ),
            scalar_type: ScalarType::Boolean,
        })
        .collect::<Vec<_>>();
    let mut next_value_identity = u64::try_from(parameter_count)
        .expect("parameter count fits a semantic identity")
        .checked_add(1)
        .expect("generated identities follow the parameter identities");
    let mut operations = Vec::new();
    let returned = emit_boolean_expression(
        &return_expression,
        &parameters,
        &mut next_value_identity,
        &mut operations,
    );
    let result_id = value_id(next_value_identity);
    let literal = ScalarTerm::boolean(contract_value);
    let goal = Proposition::Equal(literal.clone(), literal);
    let obligation = obligation_id(1);
    let mut structural_places = identity_reshuffles
        .structural_places
        .into_iter()
        .map(|place| (place.id, place.kind))
        .collect::<BTreeMap<_, _>>();
    for place in partition_compositions.structural_places {
        merge_content_place_declaration(&mut structural_places, place)
            .expect("checked lowering rejects conflicting structural places");
    }
    LoweredTerminalPsi {
        semantic_module: TerminalModule {
            semantic_version: SemanticVersion::CURRENT,
            entry: machine_id(1),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            machines: vec![TerminalMachine {
                id: machine_id(1),
                parameters,
                result: ValueDeclaration {
                    id: result_id,
                    scalar_type: ScalarType::Boolean,
                },
                structural_places: structural_places
                    .into_iter()
                    .map(|(id, kind)| StructuralPlaceDeclaration { id, kind })
                    .collect(),
                content_entry_claims: identity_reshuffles.entry_claims,
                content_identity_reshuffles: identity_reshuffles.reshuffles,
                content_partition_compositions: partition_compositions.compositions,
                entry: block_id(1),
                blocks: vec![Block {
                    id: block_id(1),
                    parameters: Vec::new(),
                    operations,
                    terminator: Terminator::Return {
                        edge: edge_id(1),
                        value: returned,
                    },
                }],
                contract: MachineContract {
                    id: contract_id(1),
                    crash_context: psi_terminal::CrashContextMaximum::portable_root(),
                    requires: vec![goal.clone()],
                    ensures: vec![ContractClause {
                        obligation,
                        proposition: goal,
                    }],
                },
            }],
        },
        proof_bundle: ProofBundle {
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::ReflexiveEquality),
            }],
        },
        debug_map: None,
    }
}

fn build_integer_comparison_module(
    parameters: Vec<ValueDeclaration>,
    return_expression: LoweredBooleanReturnExpression,
    contract_value: bool,
    identity_reshuffles: LoweredContentIdentityReshuffles,
    partition_compositions: LoweredContentPartitionCompositions,
) -> LoweredTerminalPsi {
    let mut next_value_identity = u64::try_from(parameters.len())
        .expect("parameter count fits a semantic identity")
        .checked_add(1)
        .expect("comparison value identity is nonzero");
    let mut operations = Vec::new();
    let returned = emit_boolean_expression(
        &return_expression,
        &parameters,
        &mut next_value_identity,
        &mut operations,
    );
    let result_id = value_id(next_value_identity);
    let literal = ScalarTerm::boolean(contract_value);
    let goal = Proposition::Equal(literal.clone(), literal);
    let obligation = obligation_id(1);
    let mut structural_places = identity_reshuffles
        .structural_places
        .into_iter()
        .map(|place| (place.id, place.kind))
        .collect::<BTreeMap<_, _>>();
    for place in partition_compositions.structural_places {
        merge_content_place_declaration(&mut structural_places, place)
            .expect("checked lowering rejects conflicting structural places");
    }
    LoweredTerminalPsi {
        semantic_module: TerminalModule {
            semantic_version: SemanticVersion::CURRENT,
            entry: machine_id(1),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            machines: vec![TerminalMachine {
                id: machine_id(1),
                parameters,
                result: ValueDeclaration {
                    id: result_id,
                    scalar_type: ScalarType::Boolean,
                },
                structural_places: structural_places
                    .into_iter()
                    .map(|(id, kind)| StructuralPlaceDeclaration { id, kind })
                    .collect(),
                content_entry_claims: identity_reshuffles.entry_claims,
                content_identity_reshuffles: identity_reshuffles.reshuffles,
                content_partition_compositions: partition_compositions.compositions,
                entry: block_id(1),
                blocks: vec![Block {
                    id: block_id(1),
                    parameters: Vec::new(),
                    operations,
                    terminator: Terminator::Return {
                        edge: edge_id(1),
                        value: returned,
                    },
                }],
                contract: MachineContract {
                    id: contract_id(1),
                    crash_context: psi_terminal::CrashContextMaximum::portable_root(),
                    requires: vec![goal.clone()],
                    ensures: vec![ContractClause {
                        obligation,
                        proposition: goal,
                    }],
                },
            }],
        },
        proof_bundle: ProofBundle {
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::ReflexiveEquality),
            }],
        },
        debug_map: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_boolean_conditional_module(
    parameter_count: usize,
    condition: LoweredBooleanReturnExpression,
    when_true_arguments: Vec<usize>,
    when_false_arguments: Vec<usize>,
    when_true_parameter_count: usize,
    when_false_parameter_count: usize,
    when_true_expression: LoweredBooleanReturnExpression,
    when_false_expression: LoweredBooleanReturnExpression,
    contract_value: bool,
    identity_reshuffles: LoweredContentIdentityReshuffles,
    partition_compositions: LoweredContentPartitionCompositions,
) -> LoweredTerminalPsi {
    fn allocate_boolean_parameters(count: usize, next: &mut u64) -> Vec<ValueDeclaration> {
        (0..count)
            .map(|_| {
                let parameter = ValueDeclaration {
                    id: value_id(*next),
                    scalar_type: ScalarType::Boolean,
                };
                *next = next
                    .checked_add(1)
                    .expect("Boolean parameter identities advance");
                parameter
            })
            .collect()
    }
    let mut next_value_identity = 1_u64;
    let parameters = allocate_boolean_parameters(parameter_count, &mut next_value_identity);
    let condition_decision = lower_boolean_control_decision(
        &condition,
        LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant { value: true }),
        LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant { value: false }),
    );
    let guard_block_count = boolean_decision_test_count(&condition_decision);
    let true_block_count = if contains_short_circuit(&when_true_expression) {
        boolean_decision_block_count(&lower_boolean_value_decision(&when_true_expression))
    } else {
        1
    };
    let true_target_block = block_id(
        u64::try_from(guard_block_count)
            .expect("guard block count fits a semantic identity")
            .checked_add(1)
            .expect("true branch follows the guard decision"),
    );
    let false_target_block = block_id(
        true_target_block
            .get()
            .checked_add(
                u64::try_from(true_block_count)
                    .expect("true branch block count fits a semantic identity"),
            )
            .expect("false branch follows the true branch"),
    );
    let mut all_operations = Vec::new();
    let mut blocks = Vec::new();
    let mut next_edge_identity = 1_u64;
    let entry = emit_boolean_guard_decision_blocks(
        &condition_decision,
        &parameters,
        &LoweredBooleanDecisionTarget {
            block: true_target_block,
            arguments: when_true_arguments
                .iter()
                .map(|position| parameters[*position].id)
                .collect(),
        },
        &LoweredBooleanDecisionTarget {
            block: false_target_block,
            arguments: when_false_arguments
                .iter()
                .map(|position| parameters[*position].id)
                .collect(),
        },
        &mut next_value_identity,
        &mut next_edge_identity,
        &mut all_operations,
        &mut blocks,
    );
    assert_eq!(entry.block, block_id(1));
    assert!(entry.arguments.is_empty());
    assert_eq!(blocks.len(), guard_block_count);
    let true_parameters =
        allocate_boolean_parameters(when_true_parameter_count, &mut next_value_identity);
    let false_parameters =
        allocate_boolean_parameters(when_false_parameter_count, &mut next_value_identity);
    let true_target = emit_boolean_return_blocks(
        &when_true_expression,
        &true_parameters,
        &mut next_value_identity,
        &mut next_edge_identity,
        &mut all_operations,
        &mut blocks,
    );
    assert_eq!(true_target, true_target_block);
    let false_target = emit_boolean_return_blocks(
        &when_false_expression,
        &false_parameters,
        &mut next_value_identity,
        &mut next_edge_identity,
        &mut all_operations,
        &mut blocks,
    );
    assert_eq!(false_target, false_target_block);
    let result = ValueDeclaration {
        id: value_id(next_value_identity),
        scalar_type: ScalarType::Boolean,
    };
    let literal = ScalarTerm::boolean(contract_value);
    let goal = Proposition::Equal(literal.clone(), literal);
    let obligation = obligation_id(1);
    let mut structural_places = identity_reshuffles
        .structural_places
        .into_iter()
        .map(|place| (place.id, place.kind))
        .collect::<BTreeMap<_, _>>();
    for place in partition_compositions.structural_places {
        merge_content_place_declaration(&mut structural_places, place)
            .expect("checked lowering rejects conflicting structural places");
    }
    LoweredTerminalPsi {
        semantic_module: TerminalModule {
            semantic_version: SemanticVersion::CURRENT,
            entry: machine_id(1),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            machines: vec![TerminalMachine {
                id: machine_id(1),
                parameters: parameters.clone(),
                result,
                structural_places: structural_places
                    .into_iter()
                    .map(|(id, kind)| StructuralPlaceDeclaration { id, kind })
                    .collect(),
                content_entry_claims: identity_reshuffles.entry_claims,
                content_identity_reshuffles: identity_reshuffles.reshuffles,
                content_partition_compositions: partition_compositions.compositions,
                entry: block_id(1),
                blocks: blocks
                    .into_iter()
                    .map(|block| block.expect("every Boolean conditional block is finalized"))
                    .collect(),
                contract: MachineContract {
                    id: contract_id(1),
                    crash_context: psi_terminal::CrashContextMaximum::portable_root(),
                    requires: vec![goal.clone()],
                    ensures: vec![ContractClause {
                        obligation,
                        proposition: goal,
                    }],
                },
            }],
        },
        proof_bundle: ProofBundle {
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::ReflexiveEquality),
            }],
        },
        debug_map: None,
    }
}

fn emit_boolean_expression(
    expression: &LoweredBooleanReturnExpression,
    parameters: &[ValueDeclaration],
    next_value_identity: &mut u64,
    operations: &mut Vec<Operation>,
) -> ValueId {
    match expression {
        LoweredBooleanReturnExpression::Constant { value } => {
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after a Boolean literal");
            operations.push(Operation {
                id: operation_id(
                    u64::try_from(operations.len())
                        .expect("operation count fits a semantic identity")
                        .checked_add(1)
                        .expect("operation identity is nonzero"),
                ),
                result: ValueDeclaration {
                    id,
                    scalar_type: ScalarType::Boolean,
                },
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
            operations.push(Operation {
                id: operation_id(
                    u64::try_from(operations.len())
                        .expect("operation count fits a semantic identity")
                        .checked_add(1)
                        .expect("operation identity is nonzero"),
                ),
                result: ValueDeclaration {
                    id,
                    scalar_type: ScalarType::Boolean,
                },
                kind: kind.operation(left, right),
            });
            id
        }
        LoweredBooleanReturnExpression::Parameter { position } => parameters[*position].id,
        LoweredBooleanReturnExpression::Not { operand } => {
            let operand =
                emit_boolean_expression(operand, parameters, next_value_identity, operations);
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after Boolean negation");
            operations.push(Operation {
                id: operation_id(
                    u64::try_from(operations.len())
                        .expect("operation count fits a semantic identity")
                        .checked_add(1)
                        .expect("operation identity is nonzero"),
                ),
                result: ValueDeclaration {
                    id,
                    scalar_type: ScalarType::Boolean,
                },
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
            operations.push(Operation {
                id: operation_id(
                    u64::try_from(operations.len())
                        .expect("operation count fits a semantic identity")
                        .checked_add(1)
                        .expect("operation identity is nonzero"),
                ),
                result: ValueDeclaration {
                    id,
                    scalar_type: ScalarType::Boolean,
                },
                kind: OperationKind::BooleanEqual { left, right },
            });
            id
        }
        LoweredBooleanReturnExpression::And { .. } | LoweredBooleanReturnExpression::Or { .. } => {
            unreachable!("short-circuit Boolean expressions lower through terminal control")
        }
    }
}

fn build_boolean_state_chain_module(
    parameter_count: usize,
    jump_expressions: Vec<LoweredBooleanReturnExpression>,
    return_expression: LoweredBooleanReturnExpression,
    contract_value: bool,
    identity_reshuffles: LoweredContentIdentityReshuffles,
    partition_compositions: LoweredContentPartitionCompositions,
) -> LoweredTerminalPsi {
    let terminal_parameters = (0..parameter_count)
        .map(|index| ValueDeclaration {
            id: value_id(
                u64::try_from(index)
                    .expect("parameter index fits a semantic identity")
                    .checked_add(1)
                    .expect("parameter identity is nonzero"),
            ),
            scalar_type: ScalarType::Boolean,
        })
        .collect::<Vec<_>>();
    let mut next_value_identity = u64::try_from(parameter_count)
        .expect("parameter count fits a semantic identity")
        .checked_add(1)
        .expect("generated identities follow the parameter identities");
    let mut all_operations = Vec::new();
    let mut blocks = Vec::with_capacity(jump_expressions.len() + 1);
    let mut current_parameters = terminal_parameters.clone();
    let mut next_edge_identity = 1_u64;
    for (index, jump_expression) in jump_expressions.iter().enumerate() {
        let block_parameters = if index == 0 {
            Vec::new()
        } else {
            current_parameters.clone()
        };
        if contains_short_circuit(jump_expression) {
            let decision = lower_boolean_value_decision(jump_expression);
            let decision_block_count = boolean_decision_block_count(&decision);
            let target = block_id(
                u64::try_from(blocks.len())
                    .expect("generated block count fits a semantic identity")
                    .checked_add(
                        u64::try_from(decision_block_count)
                            .expect("decision block count fits a semantic identity"),
                    )
                    .and_then(|identity| identity.checked_add(1))
                    .expect("the next source-state block follows its decision tree"),
            );
            let expected_entry = block_id(
                u64::try_from(blocks.len())
                    .expect("generated block count fits a semantic identity")
                    .checked_add(1)
                    .expect("decision entry block identity is nonzero"),
            );
            let entry = emit_boolean_decision_blocks(
                &decision,
                &current_parameters,
                block_parameters,
                LoweredBooleanDecisionExit::Jump { target },
                &mut next_value_identity,
                &mut next_edge_identity,
                &mut all_operations,
                &mut blocks,
            );
            assert_eq!(entry, expected_entry);
            assert_eq!(
                blocks.len(),
                usize::try_from(target.get()).expect("block identity fits usize") - 1,
                "decision block count predicts the next source-state identity"
            );
        } else {
            let operation_start = all_operations.len();
            let jump_value = emit_boolean_expression(
                jump_expression,
                &current_parameters,
                &mut next_value_identity,
                &mut all_operations,
            );
            let block = block_id(
                u64::try_from(blocks.len())
                    .expect("block count fits a semantic identity")
                    .checked_add(1)
                    .expect("block identity is nonzero"),
            );
            let target = block_id(
                block
                    .get()
                    .checked_add(1)
                    .expect("target block identity follows its source"),
            );
            let edge = edge_id(next_edge_identity);
            next_edge_identity = next_edge_identity
                .checked_add(1)
                .expect("state-chain jump edge identities advance");
            blocks.push(Some(Block {
                id: block,
                parameters: block_parameters,
                operations: all_operations[operation_start..].to_vec(),
                terminator: Terminator::Jump {
                    edge,
                    target,
                    arguments: vec![jump_value],
                },
            }));
        }
        let next_parameter = ValueDeclaration {
            id: value_id(next_value_identity),
            scalar_type: ScalarType::Boolean,
        };
        next_value_identity = next_value_identity
            .checked_add(1)
            .expect("generated identities advance after a Boolean block parameter");
        current_parameters = vec![next_parameter];
    }
    if contains_short_circuit(&return_expression) {
        let decision = lower_boolean_value_decision(&return_expression);
        let expected_entry = block_id(
            u64::try_from(blocks.len())
                .expect("generated block count fits a semantic identity")
                .checked_add(1)
                .expect("decision entry block identity is nonzero"),
        );
        let entry = emit_boolean_decision_blocks(
            &decision,
            &current_parameters,
            current_parameters.clone(),
            LoweredBooleanDecisionExit::Return,
            &mut next_value_identity,
            &mut next_edge_identity,
            &mut all_operations,
            &mut blocks,
        );
        assert_eq!(entry, expected_entry);
    } else {
        let return_operation_start = all_operations.len();
        let return_value = emit_boolean_expression(
            &return_expression,
            &current_parameters,
            &mut next_value_identity,
            &mut all_operations,
        );
        let block = block_id(
            u64::try_from(blocks.len())
                .expect("block count fits a semantic identity")
                .checked_add(1)
                .expect("block identity is nonzero"),
        );
        let edge = edge_id(next_edge_identity);
        blocks.push(Some(Block {
            id: block,
            parameters: current_parameters,
            operations: all_operations[return_operation_start..].to_vec(),
            terminator: Terminator::Return {
                edge,
                value: return_value,
            },
        }));
    }
    let result_id = value_id(next_value_identity);
    let literal = ScalarTerm::boolean(contract_value);
    let goal = Proposition::Equal(literal.clone(), literal);
    let obligation = obligation_id(1);
    let mut structural_places = identity_reshuffles
        .structural_places
        .into_iter()
        .map(|place| (place.id, place.kind))
        .collect::<BTreeMap<_, _>>();
    for place in partition_compositions.structural_places {
        merge_content_place_declaration(&mut structural_places, place)
            .expect("checked lowering rejects conflicting structural places");
    }

    LoweredTerminalPsi {
        semantic_module: TerminalModule {
            semantic_version: SemanticVersion::CURRENT,
            entry: machine_id(1),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            machines: vec![TerminalMachine {
                id: machine_id(1),
                parameters: terminal_parameters,
                result: ValueDeclaration {
                    id: result_id,
                    scalar_type: ScalarType::Boolean,
                },
                structural_places: structural_places
                    .into_iter()
                    .map(|(id, kind)| StructuralPlaceDeclaration { id, kind })
                    .collect(),
                content_entry_claims: identity_reshuffles.entry_claims,
                content_identity_reshuffles: identity_reshuffles.reshuffles,
                content_partition_compositions: partition_compositions.compositions,
                entry: block_id(1),
                blocks: blocks
                    .into_iter()
                    .map(|block| block.expect("every Boolean state-chain block is finalized"))
                    .collect(),
                contract: MachineContract {
                    id: contract_id(1),
                    crash_context: psi_terminal::CrashContextMaximum::portable_root(),
                    requires: vec![goal.clone()],
                    ensures: vec![ContractClause {
                        obligation,
                        proposition: goal,
                    }],
                },
            }],
        },
        proof_bundle: ProofBundle {
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::ReflexiveEquality),
            }],
        },
        debug_map: None,
    }
}

fn build_direct_parameter_module(
    parameter_types: &[ScalarType],
    return_expression: LoweredDirectExpression,
    result_type: ScalarType,
    contract_value: IntegerValue,
    identity_reshuffles: LoweredContentIdentityReshuffles,
    partition_compositions: LoweredContentPartitionCompositions,
) -> LoweredTerminalPsi {
    let terminal_parameters = parameter_types
        .iter()
        .enumerate()
        .map(|(index, scalar_type)| ValueDeclaration {
            id: value_id(
                u64::try_from(index)
                    .expect("parameter index fits a semantic identity")
                    .checked_add(1)
                    .expect("parameter identity is nonzero"),
            ),
            scalar_type: *scalar_type,
        })
        .collect::<Vec<_>>();
    let mut next_value_identity = u64::try_from(parameter_types.len())
        .expect("parameter count fits a semantic identity")
        .checked_add(1)
        .expect("generated identities follow the parameter identities");
    let mut operations = Vec::new();
    let returned = emit_direct_expression(
        &return_expression,
        &terminal_parameters,
        &mut next_value_identity,
        &mut operations,
    );
    let result_id = value_id(next_value_identity);
    let ScalarType::Integer(integer_type) = result_type else {
        unreachable!("source slice accepts only integer results");
    };
    let literal = ScalarTerm::integer(integer_type, contract_value)
        .expect("validated source contract value fits its terminal integer type");
    let goal = Proposition::Equal(literal.clone(), literal);
    let obligation = obligation_id(1);
    let mut structural_places = identity_reshuffles
        .structural_places
        .into_iter()
        .map(|place| (place.id, place.kind))
        .collect::<BTreeMap<_, _>>();
    for place in partition_compositions.structural_places {
        merge_content_place_declaration(&mut structural_places, place)
            .expect("checked lowering rejects conflicting structural places");
    }
    LoweredTerminalPsi {
        semantic_module: TerminalModule {
            semantic_version: SemanticVersion::CURRENT,
            entry: machine_id(1),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            machines: vec![TerminalMachine {
                id: machine_id(1),
                parameters: terminal_parameters,
                result: ValueDeclaration {
                    id: result_id,
                    scalar_type: result_type,
                },
                structural_places: structural_places
                    .into_iter()
                    .map(|(id, kind)| StructuralPlaceDeclaration { id, kind })
                    .collect(),
                content_entry_claims: identity_reshuffles.entry_claims,
                content_identity_reshuffles: identity_reshuffles.reshuffles,
                content_partition_compositions: partition_compositions.compositions,
                entry: block_id(1),
                blocks: vec![Block {
                    id: block_id(1),
                    parameters: Vec::new(),
                    operations,
                    terminator: Terminator::Return {
                        edge: edge_id(1),
                        value: returned,
                    },
                }],
                contract: MachineContract {
                    id: contract_id(1),
                    crash_context: psi_terminal::CrashContextMaximum::portable_root(),
                    requires: vec![goal.clone()],
                    ensures: vec![ContractClause {
                        obligation,
                        proposition: goal,
                    }],
                },
            }],
        },
        proof_bundle: ProofBundle {
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::ClosedIntegerRelation),
            }],
        },
        debug_map: None,
    }
}

fn emit_direct_expression(
    expression: &LoweredDirectExpression,
    parameters: &[ValueDeclaration],
    next_value_identity: &mut u64,
    operations: &mut Vec<Operation>,
) -> ValueId {
    match expression {
        LoweredDirectExpression::Parameter { position, .. } => parameters[*position].id,
        LoweredDirectExpression::IntegerLiteral { value, scalar_type } => {
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after a literal");
            operations.push(Operation {
                id: operation_id(
                    u64::try_from(operations.len())
                        .expect("operation count fits a semantic identity")
                        .checked_add(1)
                        .expect("operation identity is nonzero"),
                ),
                result: ValueDeclaration {
                    id,
                    scalar_type: *scalar_type,
                },
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
            operations.push(Operation {
                id: operation_id(
                    u64::try_from(operations.len())
                        .expect("operation count fits a semantic identity")
                        .checked_add(1)
                        .expect("operation identity is nonzero"),
                ),
                result: ValueDeclaration {
                    id,
                    scalar_type: *scalar_type,
                },
                kind: kind.operation(left, right),
            });
            id
        }
        LoweredDirectExpression::Boolean { expression } => {
            emit_boolean_expression(expression, parameters, next_value_identity, operations)
        }
    }
}

fn build_integer_state_chain_module(
    state_parameter_types: &[Vec<ScalarType>],
    jump_expressions: Vec<Vec<LoweredDirectExpression>>,
    return_expression: LoweredDirectExpression,
    result_type: ScalarType,
    contract_value: IntegerValue,
    identity_reshuffles: LoweredContentIdentityReshuffles,
    partition_compositions: LoweredContentPartitionCompositions,
) -> LoweredTerminalPsi {
    let parameter_types = state_parameter_types
        .first()
        .expect("linear state chain has an entry state");
    let terminal_parameters = parameter_types
        .iter()
        .enumerate()
        .map(|(index, scalar_type)| ValueDeclaration {
            id: value_id(
                u64::try_from(index)
                    .expect("parameter index fits a semantic identity")
                    .checked_add(1)
                    .expect("parameter identity is nonzero"),
            ),
            scalar_type: *scalar_type,
        })
        .collect::<Vec<_>>();
    let mut next_value_identity = u64::try_from(parameter_types.len())
        .expect("parameter count fits a semantic identity")
        .checked_add(1)
        .expect("generated identities follow the parameter identities");
    let mut all_operations = Vec::new();
    let mut blocks = Vec::with_capacity(jump_expressions.len() + 1);
    let mut current_parameters = terminal_parameters.clone();
    for (index, jump_expressions) in jump_expressions.iter().enumerate() {
        let operation_start = all_operations.len();
        let target_types = &state_parameter_types[index + 1];
        let jump_values = jump_expressions
            .iter()
            .zip(target_types)
            .map(|(jump_expression, _scalar_type)| {
                emit_direct_expression(
                    jump_expression,
                    &current_parameters,
                    &mut next_value_identity,
                    &mut all_operations,
                )
            })
            .collect::<Vec<_>>();
        let next_parameters = target_types
            .iter()
            .map(|scalar_type| {
                let parameter = ValueDeclaration {
                    id: value_id(next_value_identity),
                    scalar_type: *scalar_type,
                };
                next_value_identity = next_value_identity
                    .checked_add(1)
                    .expect("generated identities advance after a block parameter");
                parameter
            })
            .collect::<Vec<_>>();
        blocks.push(Block {
            id: block_id(
                u64::try_from(index)
                    .expect("block index fits a semantic identity")
                    .checked_add(1)
                    .expect("block identity is nonzero"),
            ),
            parameters: if index == 0 {
                Vec::new()
            } else {
                current_parameters.clone()
            },
            operations: all_operations[operation_start..].to_vec(),
            terminator: Terminator::Jump {
                edge: edge_id(
                    u64::try_from(index)
                        .expect("edge index fits a semantic identity")
                        .checked_add(1)
                        .expect("edge identity is nonzero"),
                ),
                target: block_id(
                    u64::try_from(index)
                        .expect("block index fits a semantic identity")
                        .checked_add(2)
                        .expect("target block identity follows its source"),
                ),
                arguments: jump_values,
            },
        });
        current_parameters = next_parameters;
    }
    let return_operation_start = all_operations.len();
    let return_value = emit_direct_expression(
        &return_expression,
        &current_parameters,
        &mut next_value_identity,
        &mut all_operations,
    );
    let final_index = jump_expressions.len();
    blocks.push(Block {
        id: block_id(
            u64::try_from(final_index)
                .expect("block index fits a semantic identity")
                .checked_add(1)
                .expect("block identity is nonzero"),
        ),
        parameters: current_parameters,
        operations: all_operations[return_operation_start..].to_vec(),
        terminator: Terminator::Return {
            edge: edge_id(
                u64::try_from(final_index)
                    .expect("edge index fits a semantic identity")
                    .checked_add(1)
                    .expect("edge identity is nonzero"),
            ),
            value: return_value,
        },
    });
    let result_id = value_id(next_value_identity);

    let ScalarType::Integer(integer_type) = result_type else {
        unreachable!("source slice accepts only integer results");
    };
    let literal = ScalarTerm::integer(integer_type, contract_value)
        .expect("validated source contract value fits its terminal integer type");
    let goal = Proposition::Equal(literal.clone(), literal);
    let obligation = obligation_id(1);
    let mut structural_places = identity_reshuffles
        .structural_places
        .into_iter()
        .map(|place| (place.id, place.kind))
        .collect::<BTreeMap<_, _>>();
    for place in partition_compositions.structural_places {
        merge_content_place_declaration(&mut structural_places, place)
            .expect("checked lowering rejects conflicting structural places");
    }

    LoweredTerminalPsi {
        semantic_module: TerminalModule {
            semantic_version: SemanticVersion::CURRENT,
            entry: machine_id(1),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            machines: vec![TerminalMachine {
                id: machine_id(1),
                parameters: terminal_parameters,
                result: ValueDeclaration {
                    id: result_id,
                    scalar_type: result_type,
                },
                structural_places: structural_places
                    .into_iter()
                    .map(|(id, kind)| StructuralPlaceDeclaration { id, kind })
                    .collect(),
                content_entry_claims: identity_reshuffles.entry_claims,
                content_identity_reshuffles: identity_reshuffles.reshuffles,
                content_partition_compositions: partition_compositions.compositions,
                entry: block_id(1),
                blocks,
                contract: MachineContract {
                    id: contract_id(1),
                    crash_context: psi_terminal::CrashContextMaximum::portable_root(),
                    requires: vec![goal.clone()],
                    ensures: vec![ContractClause {
                        obligation,
                        proposition: goal,
                    }],
                },
            }],
        },
        proof_bundle: ProofBundle {
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::ClosedIntegerRelation),
            }],
        },
        debug_map: None,
    }
}

fn build_debug_map(
    checked: &CheckedTrees,
    source_machine: &psi_checked_trees::machine::Machine,
    module: &TerminalModule,
) -> Result<TerminalDebugMap, LoweringError> {
    let terminal_machine = module
        .machines
        .first()
        .expect("the exact source slice always emits one terminal machine");
    let source_states = checked.machine_states(source_machine);
    let mut subjects = Vec::<(DebugSubject, psi_source::SourceSpan)>::new();
    let mut push = |subject, span| {
        if let Some(span) = span {
            subjects.push((subject, span));
        }
    };

    push(
        DebugSubject::Machine(terminal_machine.id),
        checked.symbols.symbol_source_span(source_machine.symbol),
    );
    let contract_span = source_ensures_span_for_machine(checked, source_machine)
        .filter(|span| *span != psi_source::SourceSpan::default())
        .filter(|span| checked.symbols.source_file(*span).is_some())
        .or_else(|| checked.symbols.symbol_source_span(source_machine.symbol));
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
        push(
            DebugSubject::Block(block.id),
            checked.symbols.symbol_source_span(source_state.symbol),
        );
        let transition_spans = source_transition_spans_for_state(checked, source_state);
        for (edge_index, edge) in block.terminator.edges().enumerate() {
            let transition_span = transition_spans
                .get(edge_index)
                .or_else(|| (transition_spans.len() == 1).then(|| &transition_spans[0]))
                .copied()
                .filter(|span| *span != psi_source::SourceSpan::default())
                .filter(|span| checked.symbols.source_file(*span).is_some());
            push(
                DebugSubject::Edge(edge),
                transition_span.or_else(|| checked.symbols.symbol_source_span(source_state.symbol)),
            );
        }
        let operation_spans = source_operation_spans_for_state(checked, source_state);
        for (operation_index, operation) in block.operations.iter().enumerate() {
            let source_span = operation_spans
                .get(operation_index)
                .copied()
                .filter(|span| *span != psi_source::SourceSpan::default())
                .filter(|span| checked.symbols.source_file(*span).is_some());
            if let Some(source_span) = source_span {
                push(DebugSubject::Operation(operation.id), Some(source_span));
                push(DebugSubject::Value(operation.result.id), Some(source_span));
            } else {
                push(
                    DebugSubject::Operation(operation.id),
                    checked.symbols.symbol_source_span(source_state.symbol),
                );
                push(
                    DebugSubject::Value(operation.result.id),
                    checked.symbols.symbol_source_span(source_state.symbol),
                );
            }
        }
        for (parameter_index, parameter) in block.parameters.iter().enumerate() {
            if let Some(source_parameter) = checked
                .state_parameters(source_state)
                .iter()
                .filter(|parameter| !parameter.is_self)
                .nth(parameter_index)
            {
                push(
                    DebugSubject::Value(parameter.id),
                    checked.symbols.symbol_source_span(source_parameter.symbol),
                );
            }
        }
    }

    if let Some(entry_state) = source_states.first() {
        for (parameter_index, parameter) in terminal_machine.parameters.iter().enumerate() {
            if let Some(source_parameter) = checked
                .state_parameters(entry_state)
                .iter()
                .filter(|parameter| !parameter.is_self)
                .nth(parameter_index)
            {
                push(
                    DebugSubject::Value(parameter.id),
                    checked.symbols.symbol_source_span(source_parameter.symbol),
                );
            }
        }
    }
    push(
        DebugSubject::Value(terminal_machine.result.id),
        checked.symbols.symbol_source_span(source_machine.symbol),
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
        let source_span = psi_source::SourceSpan::new(
            psi_source::SourceId(source_id),
            psi_source::Span::default(),
        );
        let source_file = checked
            .symbols
            .source_file(source_span)
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

fn source_ensures_span_for_machine(
    checked: &CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
) -> Option<psi_source::SourceSpan> {
    let contract = checked
        .machine_contracts(machine)
        .iter()
        .find(|contract| contract.kind == SignatureContractKind::Ensures)?;
    let [ProofFact::Expression(expression)] = checked.proof_facts.span_or_empty(contract.facts)
    else {
        return None;
    };
    Some(checked.expression_table.source_span(*expression))
}

fn source_transition_spans_for_state(
    checked: &CheckedTrees,
    state: &psi_checked_trees::state::State,
) -> Vec<psi_source::SourceSpan> {
    checked
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| match statement {
            StatementNode::Transition(transition) => Some(transition.source_span),
            StatementNode::Expression(expression) => {
                Some(checked.expression_table.source_span(*expression))
            }
            _ => None,
        })
        .collect()
}

fn source_operation_spans_for_state(
    checked: &CheckedTrees,
    state: &psi_checked_trees::state::State,
) -> Vec<psi_source::SourceSpan> {
    let mut spans = Vec::new();
    for statement in checked.statement_table.statements(state.statement_nodes) {
        match statement {
            StatementNode::Expression(expression) => {
                collect_source_operation_spans(checked, *expression, &mut spans);
            }
            StatementNode::Transition(transition) => {
                if let TransitionGuardNode::When(guard) = transition.guard {
                    collect_source_operation_spans(checked, guard, &mut spans);
                }
                if let TransitionTargetNode::Named { arguments, .. } =
                    checked.statement_table.transition_target(transition.target)
                {
                    for expression in checked.statement_table.expression_handles(*arguments) {
                        collect_source_operation_spans(checked, *expression, &mut spans);
                    }
                }
            }
            _ => {}
        }
    }
    spans
}

fn collect_source_operation_spans(
    checked: &CheckedTrees,
    expression: psi_checked_trees::expression::ExpressionHandle,
    spans: &mut Vec<psi_source::SourceSpan>,
) {
    match checked.expression_table.expression(expression) {
        ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => {
            spans.push(checked.expression_table.source_span(expression));
        }
        ExpressionNode::Binary(binary) => {
            collect_source_operation_spans(checked, binary.left, spans);
            collect_source_operation_spans(checked, binary.right, spans);
            spans.push(checked.expression_table.source_span(expression));
        }
        ExpressionNode::Unary(unary) => {
            collect_source_operation_spans(checked, unary.operand, spans);
            spans.push(checked.expression_table.source_span(expression));
        }
        _ => {}
    }
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
id_constructor!(machine_id, MachineId);
id_constructor!(block_id, BlockId);
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
    use psi_symbols::SymbolHandle;

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
}
