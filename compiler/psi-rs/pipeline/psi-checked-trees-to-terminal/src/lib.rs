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
    expression::{BinaryOperator, ExpressionNode},
    signature::SignatureContractKind,
    statement::{StatementNode, TransitionGuardNode, TransitionTargetNode},
    types::PrimitiveType,
};
use psi_core::{
    BlockId, ClaimId, ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentDomainId,
    ContentPlaceSegment, ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace,
    ContentTerm, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
    ObligationId, OperationId, PlaceId, Proposition, PropositionContext, PropositionError,
    ScalarTerm, ScalarType, StructuralPlaceKind, ValueId,
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
    Block, ClaimContentProjection, ContentIdentityReshuffle, ContentPartitionComposition,
    ContentPlaceSubstitution, ContractClause, MachineContract, Operation, OperationKind,
    SemanticVersion, StructuralPlaceDeclaration, TerminalMachine, TerminalModule, Terminator,
    ValueDeclaration,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoweredReturnExpression {
    Literal,
    IntegerBinary {
        kind: LoweredIntegerBinaryKind,
        right: IntegerValue,
        result: IntegerValue,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LoweredDirectExpression {
    Parameter {
        position: usize,
    },
    IntegerLiteral {
        value: IntegerValue,
    },
    IntegerBinary {
        kind: LoweredIntegerBinaryKind,
        left: Box<LoweredDirectExpression>,
        right: Box<LoweredDirectExpression>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoweredBooleanReturnExpression {
    Constant { value: bool },
    Parameter { position: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoweredIntegerBinaryKind {
    WrappingAdd,
    SaturatingAdd,
    WrappingSubtract,
    SaturatingSubtract,
    WrappingMultiply,
    SaturatingMultiply,
}

impl LoweredIntegerBinaryKind {
    fn operation(self, left: ValueId, right: ValueId) -> OperationKind {
        match self {
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
        .collect();
    Ok(LoweredContentIdentityReshuffles {
        structural_places: structural_places
            .into_iter()
            .map(|(id, kind)| StructuralPlaceDeclaration { id, kind })
            .collect(),
        reshuffles,
        source_claims,
    })
}

/// Lower checker-proved direct partition composition into terminal-Psi v12.
/// The terminal row retains both equations and the exact place substitution so
/// the verifier can replay it and reject any manufactured `separate(...)` node.
pub fn lower_content_partition_compositions(
    facts: &[ContentPartitionCompositionFact],
    identity_reshuffles: &LoweredContentIdentityReshuffles,
) -> Result<LoweredContentPartitionCompositions, LoweringError> {
    let Some(first) = facts.first() else {
        return Ok(LoweredContentPartitionCompositions {
            structural_places: Vec::new(),
            compositions: Vec::new(),
        });
    };
    let callable = (first.machine_symbol, first.state_symbol);
    let mut target_places = BTreeMap::new();
    let mut compositions = Vec::new();

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
                identity_reshuffles
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
    Ok(LoweredContentPartitionCompositions {
        structural_places: target_places
            .into_iter()
            .map(|(id, kind)| StructuralPlaceDeclaration { id, kind })
            .collect(),
        compositions,
    })
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
/// machine name(p0: integer, ...) -> integer
/// requires C == C
/// ensures C == C
/// {
///     E
/// }
/// E := pN | L | E (+|-|*) E
///
/// machine name() -> integer
/// requires L == L
/// ensures L == L
/// {
///     transition { _ -> done(L) }
///     state done(value: integer) -> integer { L | value (+|-|*) R }
/// }
/// ```
pub fn lower_machine(
    checked: &CheckedTrees,
    machine_name: &str,
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
    lowered.debug_map = Some(build_debug_map(checked, machine, &lowered.semantic_module)?);
    Ok(lowered)
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
    if let [entry_state] = states {
        return match checked.primitive_type_reference(entry_state.return_type) {
            Some(PrimitiveType::Bool) => lower_boolean_machine(checked, machine, entry_state),
            _ => lower_direct_parameter_machine(checked, machine, entry_state),
        };
    }
    if states.len() >= 2 && !checked.state_parameters(&states[0]).is_empty() {
        return lower_parameterized_state_chain(checked, machine, states);
    }
    let [entry_state, return_state] = states else {
        return unsupported(
            "machine must contain one direct-parameter state or an entry state and one return state",
        );
    };
    let [return_parameter] = checked.state_parameters(return_state) else {
        return unsupported("return state must have exactly one parameter");
    };
    if return_parameter.is_self || return_parameter.is_const || return_parameter.is_mutable {
        return unsupported("qualified return-state parameters are not supported");
    }
    if !checked.state_contracts(entry_state).is_empty()
        || !checked.state_contracts(return_state).is_empty()
    {
        return unsupported("state contracts are not supported");
    }

    let return_type = integer_scalar_type(
        checked
            .primitive_type_reference(entry_state.return_type)
            .ok_or(LoweringError::Unsupported(
                "machine result must be a primitive integer",
            ))?,
    )?;
    if integer_scalar_type(
        checked
            .primitive_type_reference(return_state.return_type)
            .ok_or(LoweringError::Unsupported(
                "return-state result must be a primitive integer",
            ))?,
    )? != return_type
        || integer_scalar_type(
            checked
                .primitive_type_reference(return_parameter.type_reference)
                .ok_or(LoweringError::Unsupported(
                    "return-state parameter must be a primitive integer",
                ))?,
        )? != return_type
    {
        return unsupported("machine, return-state, and parameter types must match exactly");
    }

    let entry_statements = checked
        .statement_table
        .statements(entry_state.statement_nodes);
    let [StatementNode::Transition(transition)] = entry_statements else {
        return unsupported("entry state must contain exactly one transition");
    };
    if transition.guard != TransitionGuardNode::Always || transition.continuation.is_valid() {
        return unsupported("entry transition must be unconditional and have no continuation");
    }
    let TransitionTargetNode::Named { path, arguments } =
        checked.statement_table.transition_target(transition.target)
    else {
        return unsupported("entry transition must target the return state by name");
    };
    if path.symbol != return_state.symbol {
        return unsupported("entry transition must target the sole return state");
    }
    let [argument] = checked.statement_table.expression_handles(*arguments) else {
        return unsupported("entry transition must carry exactly one argument");
    };
    let ExpressionNode::Integer(argument_literal) = checked.expression_table.expression(*argument)
    else {
        return unsupported("entry transition argument must be an integer literal");
    };
    let value = integer_value(argument_literal, return_type)?;

    let return_statements = checked
        .statement_table
        .statements(return_state.statement_nodes);
    let [StatementNode::Expression(return_expression)] = return_statements else {
        return unsupported("return state must contain exactly one value expression");
    };
    let lowered_return = lower_return_expression(
        checked,
        *return_expression,
        return_parameter,
        return_type,
        value,
    )?;
    let executed_value = match lowered_return {
        LoweredReturnExpression::Literal => value,
        LoweredReturnExpression::IntegerBinary { result, .. } => result,
    };

    let contract_value = validate_contract(checked, machine, return_type, Some(executed_value))?;
    let (identity_reshuffles, partition_compositions) =
        lower_content_evidence(checked, machine, entry_state)?;
    Ok(build_module(
        return_type,
        value,
        lowered_return,
        contract_value,
        identity_reshuffles,
        partition_compositions,
    ))
}

fn lower_parameterized_state_chain(
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
        let [parameter] = checked.state_parameters(state) else {
            return unsupported("every non-entry linear state must have exactly one parameter");
        };
        if parameter.is_self || parameter.is_const || parameter.is_mutable {
            return unsupported("qualified linear-state parameters are not supported");
        }
        if integer_scalar_type(checked.primitive_type_reference(state.return_type).ok_or(
            LoweringError::Unsupported("linear-state result must be a primitive integer"),
        )?)? != result_type
            || integer_scalar_type(
                checked
                    .primitive_type_reference(parameter.type_reference)
                    .ok_or(LoweringError::Unsupported(
                        "linear-state parameter must be a primitive integer",
                    ))?,
            )? != result_type
        {
            return unsupported("machine, state, and carried parameter types must match exactly");
        }
        state_parameter_types.push(vec![result_type]);
    }
    if states
        .iter()
        .any(|state| !checked.state_contracts(state).is_empty())
    {
        return unsupported("state contracts are not supported");
    }

    let mut jump_expressions = Vec::with_capacity(states.len() - 1);
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
        let [argument] = checked.statement_table.expression_handles(*arguments) else {
            return unsupported("a linear-state transition must carry exactly one argument");
        };
        let (expression, _) = lower_direct_return_expression(
            checked,
            *argument,
            checked.state_parameters(state),
            &state_parameter_types[index],
            result_type,
        )?;
        jump_expressions.push(expression);
    }

    let return_state = states.last().expect("linear chain is nonempty");
    let return_parameter = &checked.state_parameters(return_state)[0];
    let return_statements = checked
        .statement_table
        .statements(return_state.statement_nodes);
    let [StatementNode::Expression(return_expression)] = return_statements else {
        return unsupported("return state must contain exactly one value expression");
    };
    let (return_expression, _) = lower_direct_return_expression(
        checked,
        *return_expression,
        std::slice::from_ref(return_parameter),
        &[result_type],
        result_type,
    )?;

    let contract_value = validate_contract(checked, machine, result_type, None)?;
    let (identity_reshuffles, partition_compositions) =
        lower_content_evidence(checked, machine, entry_state)?;
    Ok(build_parameterized_state_chain_module(
        &parameter_types,
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
    let return_expression = match checked.expression_table.expression(*return_expression) {
        ExpressionNode::Boolean(value) => {
            LoweredBooleanReturnExpression::Constant { value: *value }
        }
        ExpressionNode::Name(path) => LoweredBooleanReturnExpression::Parameter {
            position: direct_parameter_position(checked, path, parameters)?,
        },
        _ => {
            return unsupported(
                "Boolean source machine must return a literal or declared parameter",
            );
        }
    };
    let contract_value = validate_boolean_contract(checked, machine)?;
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

fn lower_direct_parameter_machine(
    checked: &CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
    entry_state: &psi_checked_trees::state::State,
) -> Result<LoweredTerminalPsi, LoweringError> {
    if !checked.state_contracts(entry_state).is_empty() {
        return unsupported("state contracts are not supported");
    }
    let parameters = checked.state_parameters(entry_state);
    if parameters.is_empty() {
        return unsupported("direct-parameter machine must declare at least one parameter");
    }
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
    let contract_value = validate_contract(checked, machine, result_type, None)?;
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
    match checked.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let position = direct_parameter_position(checked, path, parameters)?;
            if parameter_types[position] != result_type {
                return unsupported(
                    "returned parameter and machine result types must match exactly",
                );
            }
            Ok((
                LoweredDirectExpression::Parameter { position },
                checked.arithmetic_domain_for_type_reference(parameters[position].type_reference),
            ))
        }
        ExpressionNode::Integer(literal) => Ok((
            LoweredDirectExpression::IntegerLiteral {
                value: integer_value(literal, result_type)?,
            },
            ArithmeticDomain::Exact,
        )),
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
            let (left, left_domain) = lower_direct_return_expression(
                checked,
                binary.left,
                parameters,
                parameter_types,
                result_type,
            )?;
            let (right, right_domain) = lower_direct_return_expression(
                checked,
                binary.right,
                parameters,
                parameter_types,
                result_type,
            )?;
            let domain = combine_terminal_arithmetic_domains(left_domain, right_domain)?;
            let kind = lowered_integer_binary_kind(binary.operator, domain)?;
            Ok((
                LoweredDirectExpression::IntegerBinary {
                    kind,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                domain,
            ))
        }
        _ => unsupported("direct-parameter machine must return a supported integer expression"),
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
        _ => unsupported(
            "terminal source producer supports only integer add, subtract, and multiply",
        ),
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
    let identity_reshuffles = lower_content_identity_reshuffles(&identity_facts)?;
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
        lower_content_partition_compositions(&partition_facts, &identity_reshuffles)?;
    Ok((identity_reshuffles, partition_compositions))
}

fn lower_return_expression(
    checked: &CheckedTrees,
    expression: psi_checked_trees::expression::ExpressionHandle,
    parameter: &psi_checked_trees::signature::StateParameter,
    result_type: ScalarType,
    parameter_value: IntegerValue,
) -> Result<LoweredReturnExpression, LoweringError> {
    match checked.expression_table.expression(expression) {
        ExpressionNode::Integer(literal) => {
            if integer_value(literal, result_type)? != parameter_value {
                return unsupported("jump and return literals must be equal");
            }
            Ok(LoweredReturnExpression::Literal)
        }
        ExpressionNode::Binary(binary) => {
            let ExpressionNode::Name(left) = checked.expression_table.expression(binary.left)
            else {
                return unsupported(
                    "terminal integer binary left operand must be the state parameter",
                );
            };
            if checked
                .expression_table
                .name_path_members(left.members)
                .len()
                != 1
                || (left.symbol != parameter.symbol && left.head_symbol != parameter.symbol)
            {
                return unsupported(
                    "terminal integer binary left operand must be the state parameter",
                );
            }
            let ExpressionNode::Integer(right_literal) =
                checked.expression_table.expression(binary.right)
            else {
                return unsupported(
                    "terminal integer binary right operand must be an integer literal",
                );
            };
            if let Some(operator_use) = checked.facts.operators.expression_use(expression)
                && operator_use.status != CheckedOperatorResolutionStatus::BuiltinFallback
            {
                return unsupported(
                    "terminal integer binary expression must use the builtin operator",
                );
            }
            let domain = checked.arithmetic_domain_for_type_reference(parameter.type_reference);
            let kind = lowered_integer_binary_kind(binary.operator, domain)?;
            let right = integer_value(right_literal, result_type)?;
            let ScalarType::Integer(integer_type) = result_type else {
                return Err(LoweringError::InvalidPsiIntegerType);
            };
            let result = match kind {
                LoweredIntegerBinaryKind::WrappingAdd => {
                    integer_type.wrapping_add(parameter_value, right)
                }
                LoweredIntegerBinaryKind::SaturatingAdd => {
                    integer_type.saturating_add(parameter_value, right)
                }
                LoweredIntegerBinaryKind::WrappingSubtract => {
                    integer_type.wrapping_sub(parameter_value, right)
                }
                LoweredIntegerBinaryKind::SaturatingSubtract => {
                    integer_type.saturating_sub(parameter_value, right)
                }
                LoweredIntegerBinaryKind::WrappingMultiply => {
                    integer_type.wrapping_mul(parameter_value, right)
                }
                LoweredIntegerBinaryKind::SaturatingMultiply => {
                    integer_type.saturating_mul(parameter_value, right)
                }
            }
            .ok_or(LoweringError::IntegerLiteralOutsidePsiType)?;
            Ok(LoweredReturnExpression::IntegerBinary {
                kind,
                right,
                result,
            })
        }
        _ => unsupported(
            "return state must return an integer literal or supported integer binary expression",
        ),
    }
}

fn validate_contract(
    checked: &CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
    result_type: ScalarType,
    expected_value: Option<IntegerValue>,
) -> Result<IntegerValue, LoweringError> {
    let contracts = checked.machine_contracts(machine);
    if contracts.len() != 2 {
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

fn build_boolean_module(
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
        .expect("generated identities follow the parameter identities");
    let mut operations = Vec::new();
    let returned = match return_expression {
        LoweredBooleanReturnExpression::Constant { value } => {
            let id = value_id(next_value_identity);
            next_value_identity = next_value_identity
                .checked_add(1)
                .expect("machine result identity follows the Boolean constant");
            operations.push(Operation {
                id: operation_id(1),
                result: ValueDeclaration {
                    id,
                    scalar_type: ScalarType::Boolean,
                },
                kind: OperationKind::BooleanConstant { value },
            });
            id
        }
        LoweredBooleanReturnExpression::Parameter { position } => parameters[position].id,
    };
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
        result_type,
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
    scalar_type: ScalarType,
    next_value_identity: &mut u64,
    operations: &mut Vec<Operation>,
) -> ValueId {
    match expression {
        LoweredDirectExpression::Parameter { position } => parameters[*position].id,
        LoweredDirectExpression::IntegerLiteral { value } => {
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
                result: ValueDeclaration { id, scalar_type },
                kind: OperationKind::IntegerConstant { value: *value },
            });
            id
        }
        LoweredDirectExpression::IntegerBinary { kind, left, right } => {
            let left = emit_direct_expression(
                left,
                parameters,
                scalar_type,
                next_value_identity,
                operations,
            );
            let right = emit_direct_expression(
                right,
                parameters,
                scalar_type,
                next_value_identity,
                operations,
            );
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
                result: ValueDeclaration { id, scalar_type },
                kind: kind.operation(left, right),
            });
            id
        }
    }
}

fn build_parameterized_state_chain_module(
    parameter_types: &[ScalarType],
    jump_expressions: Vec<LoweredDirectExpression>,
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
    let mut all_operations = Vec::new();
    let mut blocks = Vec::with_capacity(jump_expressions.len() + 1);
    let mut current_parameters = terminal_parameters.clone();
    for (index, jump_expression) in jump_expressions.iter().enumerate() {
        let operation_start = all_operations.len();
        let jump_value = emit_direct_expression(
            jump_expression,
            &current_parameters,
            result_type,
            &mut next_value_identity,
            &mut all_operations,
        );
        let next_parameter = ValueDeclaration {
            id: value_id(next_value_identity),
            scalar_type: result_type,
        };
        next_value_identity = next_value_identity
            .checked_add(1)
            .expect("generated identities advance after a block parameter");
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
                arguments: vec![jump_value],
            },
        });
        current_parameters = vec![next_parameter];
    }
    let return_operation_start = all_operations.len();
    let return_value = emit_direct_expression(
        &return_expression,
        &current_parameters,
        result_type,
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
                content_identity_reshuffles: identity_reshuffles.reshuffles,
                content_partition_compositions: partition_compositions.compositions,
                entry: block_id(1),
                blocks,
                contract: MachineContract {
                    id: contract_id(1),
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

fn build_module(
    result_type: ScalarType,
    value: IntegerValue,
    return_expression: LoweredReturnExpression,
    contract_value: IntegerValue,
    identity_reshuffles: LoweredContentIdentityReshuffles,
    partition_compositions: LoweredContentPartitionCompositions,
) -> LoweredTerminalPsi {
    let jump_constant_id = value_id(1);
    let parameter_id = value_id(2);
    let ScalarType::Integer(integer_type) = result_type else {
        unreachable!("source slice accepts only integer results");
    };
    let (result_id, return_operations, return_value) = match return_expression {
        LoweredReturnExpression::Literal => {
            let return_constant_id = value_id(3);
            (
                value_id(4),
                vec![Operation {
                    id: operation_id(2),
                    result: ValueDeclaration {
                        id: return_constant_id,
                        scalar_type: result_type,
                    },
                    kind: OperationKind::IntegerConstant { value },
                }],
                return_constant_id,
            )
        }
        LoweredReturnExpression::IntegerBinary {
            kind,
            right,
            result: _,
        } => {
            let right_id = value_id(3);
            let binary_result_id = value_id(4);
            (
                value_id(5),
                vec![
                    Operation {
                        id: operation_id(2),
                        result: ValueDeclaration {
                            id: right_id,
                            scalar_type: result_type,
                        },
                        kind: OperationKind::IntegerConstant { value: right },
                    },
                    Operation {
                        id: operation_id(3),
                        result: ValueDeclaration {
                            id: binary_result_id,
                            scalar_type: result_type,
                        },
                        kind: kind.operation(parameter_id, right_id),
                    },
                ],
                binary_result_id,
            )
        }
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
            machines: vec![TerminalMachine {
                id: machine_id(1),
                parameters: Vec::new(),
                result: ValueDeclaration {
                    id: result_id,
                    scalar_type: result_type,
                },
                structural_places: structural_places
                    .into_iter()
                    .map(|(id, kind)| StructuralPlaceDeclaration { id, kind })
                    .collect(),
                content_identity_reshuffles: identity_reshuffles.reshuffles,
                content_partition_compositions: partition_compositions.compositions,
                entry: block_id(1),
                blocks: vec![
                    Block {
                        id: block_id(1),
                        parameters: Vec::new(),
                        operations: vec![Operation {
                            id: operation_id(1),
                            result: ValueDeclaration {
                                id: jump_constant_id,
                                scalar_type: result_type,
                            },
                            kind: OperationKind::IntegerConstant { value },
                        }],
                        terminator: Terminator::Jump {
                            edge: edge_id(1),
                            target: block_id(2),
                            arguments: vec![jump_constant_id],
                        },
                    },
                    Block {
                        id: block_id(2),
                        parameters: vec![ValueDeclaration {
                            id: parameter_id,
                            scalar_type: result_type,
                        }],
                        operations: return_operations,
                        terminator: Terminator::Return {
                            edge: edge_id(2),
                            value: return_value,
                        },
                    },
                ],
                contract: MachineContract {
                    id: contract_id(1),
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
            .expect("terminal blocks follow accepted source-state order");
        push(
            DebugSubject::Block(block.id),
            checked.symbols.symbol_source_span(source_state.symbol),
        );
        let transition_span = source_transition_span_for_state(checked, source_state)
            .filter(|span| *span != psi_source::SourceSpan::default())
            .filter(|span| checked.symbols.source_file(*span).is_some());
        push(
            DebugSubject::Edge(block.terminator.edge()),
            transition_span.or_else(|| checked.symbols.symbol_source_span(source_state.symbol)),
        );
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

fn source_transition_span_for_state(
    checked: &CheckedTrees,
    state: &psi_checked_trees::state::State,
) -> Option<psi_source::SourceSpan> {
    checked
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            StatementNode::Transition(transition) => Some(transition.source_span),
            _ => None,
        })
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweringError {
    MachineNotFound(String),
    AmbiguousMachineName(String),
    DebugSourceFileCountOverflow,
    DebugSourceLengthOverflow,
    MissingDebugSourceFile(usize),
    DebugSemanticCodec(psi_terminal_codec::CodecError),
    InvalidDebugMap(psi_terminal_codec::DebugMapError),
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
        ContentPartitionCompositionFact {
            machine_symbol: plan.owner,
            state_symbol: plan.callable,
            source_callable: source_plan.callable,
            source_fingerprint: source_plan.fingerprint,
            source_derivation_depth: 0,
            source_plan,
            statement_index: 4,
            call_ordinal: 2,
            input_claim_identities: vec![
                identity_fact(SemanticDomainId(9), "left", 1).claim_identity,
            ],
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
    fn checked_partition_composition_lowers_with_exact_source_and_dense_claims() {
        let identity = identity_fact(SemanticDomainId(9), "left", 1);
        let identities =
            lower_content_identity_reshuffles(&[identity]).expect("identity fact lowers");
        let fact = partition_composition_fact();
        let lowered =
            lower_content_partition_compositions(std::slice::from_ref(&fact), &identities)
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
        assert_eq!(
            lower_content_partition_compositions(&[staged], &identities),
            Err(LoweringError::ContentPartitionResultRewriteUnsupported)
        );

        let mut derived_source = fact.clone();
        derived_source.source_derivation_depth = 1;
        assert_eq!(
            lower_content_partition_compositions(&[derived_source], &identities),
            Err(LoweringError::ContentPartitionDerivedSourceUnsupported)
        );

        let mut drifted = fact;
        let projection = drifted.plan.equation.left().clone();
        drifted.plan.equation = ContentConservationEquation::new(
            projection.clone(),
            CheckedContentConservationTerm::separate([projection.clone(), projection]),
        );
        assert_eq!(
            lower_content_partition_compositions(&[drifted], &identities),
            Err(LoweringError::ContentPartitionReplayMismatch)
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
