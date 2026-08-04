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
    statement::{StatementNode, TransitionGuardNode, TransitionTargetNode},
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
    ContentPartitionComposition, ContentPlaceSubstitution, ContractClause, MachineContract,
    Operation, OperationKind, PropositionApplicationIdentity, PropositionBinderArgumentIdentity,
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
    And {
        left: Box<LoweredBooleanReturnExpression>,
        right: Box<LoweredBooleanReturnExpression>,
    },
    Or {
        left: Box<LoweredBooleanReturnExpression>,
        right: Box<LoweredBooleanReturnExpression>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LoweredBooleanDecision {
    Return(bool),
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
    let (declarations, applications) = lower_proposition_vocabulary(checked);
    lowered.semantic_module.proposition_declarations = declarations;
    lowered.semantic_module.proposition_applications = applications;
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
    if let [entry_state] = states {
        return match checked.primitive_type_reference(entry_state.return_type) {
            Some(PrimitiveType::Bool) => lower_boolean_machine(checked, machine, entry_state),
            _ => lower_direct_parameter_machine(checked, machine, entry_state),
        };
    }
    if states.len() == 3 && entry_has_ordered_boolean_conditional(checked, &states[0]) {
        return match checked.primitive_type_reference(states[0].return_type) {
            Some(PrimitiveType::Bool) => {
                lower_boolean_conditional_machine(checked, machine, states)
            }
            _ => lower_integer_conditional_machine(checked, machine, states),
        };
    }
    if states.len() >= 2
        && checked.primitive_type_reference(states[0].return_type) == Some(PrimitiveType::Bool)
    {
        return lower_boolean_state_chain(checked, machine, states);
    }
    if states.len() >= 2 {
        return lower_integer_state_chain(checked, machine, states);
    }
    unsupported("machine must contain at least one state")
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
        if contains_short_circuit(&branch_expression) {
            return unsupported(
                "short-circuit logic in explicit conditional branches is not yet supported",
            );
        }
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

    let contract_value = validate_contract(checked, machine, result_type, None)?;
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
    let ScalarType::Integer(result_integer_type) = result_type else {
        unreachable!("integer source result lowered to a non-integer scalar type");
    };
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
            let ScalarType::Integer(target_integer_type) = target_type else {
                unreachable!("linear state parameters were restricted to integers");
            };
            next_known_parameters.push(evaluate_direct_expression(
                &expression,
                &known_parameters,
                *target_integer_type,
            ));
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
    let expected_value =
        evaluate_direct_expression(&return_expression, &known_parameters, result_integer_type);

    let contract_value = validate_contract(checked, machine, result_type, expected_value)?;
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
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) =>
        {
            if let Some(operator_use) = checked.facts.operators.expression_use(expression)
                && operator_use.status != CheckedOperatorResolutionStatus::BuiltinFallback
            {
                return unsupported("terminal Boolean comparison must use the builtin operator");
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
            "Boolean terminal expressions require a literal, declared parameter, logical not, builtin equality/inequality, or short-circuit logic",
        ),
    }
}

fn contains_short_circuit(expression: &LoweredBooleanReturnExpression) -> bool {
    match expression {
        LoweredBooleanReturnExpression::Constant { .. }
        | LoweredBooleanReturnExpression::Parameter { .. } => false,
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
        | LoweredBooleanReturnExpression::Parameter { .. } => Ok(()),
        LoweredBooleanReturnExpression::Not { operand } => {
            validate_short_circuit_expression(operand)
        }
        LoweredBooleanReturnExpression::Equal { left, right } => {
            if contains_short_circuit(left) || contains_short_circuit(right) {
                return unsupported(
                    "short-circuit expressions nested inside equality are not yet supported",
                );
            }
            Ok(())
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
    if binary.operator != BinaryOperator::Equal {
        return unsupported("conditional guards require a positive Boolean pattern");
    }
    match (
        checked.expression_table.expression(binary.left),
        checked.expression_table.expression(binary.right),
    ) {
        (ExpressionNode::Boolean(true), _) => {
            lower_boolean_expression(checked, binary.right, parameters)
        }
        (_, ExpressionNode::Boolean(true)) => {
            lower_boolean_expression(checked, binary.left, parameters)
        }
        _ => unsupported("conditional guards require a positive Boolean pattern"),
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
    let ScalarType::Integer(result_integer_type) = result_type else {
        unreachable!("integer source result lowered to a non-integer scalar type");
    };
    let known_parameters = vec![None; parameter_types.len()];
    let expected_value =
        evaluate_direct_expression(&return_expression, &known_parameters, result_integer_type);
    let contract_value = validate_contract(checked, machine, result_type, expected_value)?;
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

fn evaluate_direct_expression(
    expression: &LoweredDirectExpression,
    parameters: &[Option<IntegerValue>],
    integer_type: IntegerType,
) -> Option<IntegerValue> {
    match expression {
        LoweredDirectExpression::Parameter { position } => {
            parameters.get(*position).copied().flatten()
        }
        LoweredDirectExpression::IntegerLiteral { value } => Some(*value),
        LoweredDirectExpression::IntegerBinary { kind, left, right } => {
            let left = evaluate_direct_expression(left, parameters, integer_type)?;
            let right = evaluate_direct_expression(right, parameters, integer_type)?;
            match kind {
                LoweredIntegerBinaryKind::WrappingAdd => integer_type.wrapping_add(left, right),
                LoweredIntegerBinaryKind::SaturatingAdd => integer_type.saturating_add(left, right),
                LoweredIntegerBinaryKind::WrappingSubtract => {
                    integer_type.wrapping_sub(left, right)
                }
                LoweredIntegerBinaryKind::SaturatingSubtract => {
                    integer_type.saturating_sub(left, right)
                }
                LoweredIntegerBinaryKind::WrappingMultiply => {
                    integer_type.wrapping_mul(left, right)
                }
                LoweredIntegerBinaryKind::SaturatingMultiply => {
                    integer_type.saturating_mul(left, right)
                }
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
        result_type,
        &mut next_value_identity,
        &mut all_operations,
    );
    let true_operation_end = all_operations.len();
    let false_value = emit_direct_expression(
        &when_false_expression,
        &false_parameters,
        result_type,
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

fn lower_boolean_decision(
    expression: &LoweredBooleanReturnExpression,
    when_true: LoweredBooleanDecision,
    when_false: LoweredBooleanDecision,
) -> LoweredBooleanDecision {
    match expression {
        LoweredBooleanReturnExpression::And { left, right } => {
            let right = lower_boolean_decision(right, when_true, when_false.clone());
            lower_boolean_decision(left, right, when_false)
        }
        LoweredBooleanReturnExpression::Or { left, right } => {
            let right = lower_boolean_decision(right, when_true.clone(), when_false);
            lower_boolean_decision(left, when_true, right)
        }
        LoweredBooleanReturnExpression::Not { operand } if contains_short_circuit(operand) => {
            lower_boolean_decision(operand, when_false, when_true)
        }
        expression => LoweredBooleanDecision::Test {
            condition: expression.clone(),
            when_true: Box::new(when_true),
            when_false: Box::new(when_false),
        },
    }
}

fn boolean_decision_block_count(decision: &LoweredBooleanDecision) -> usize {
    match decision {
        LoweredBooleanDecision::Return(_) => 1,
        LoweredBooleanDecision::Test {
            when_true,
            when_false,
            ..
        } => 1 + boolean_decision_block_count(when_true) + boolean_decision_block_count(when_false),
    }
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
        LoweredBooleanDecision::Return(value) => {
            let returned = emit_boolean_expression(
                &LoweredBooleanReturnExpression::Constant { value: *value },
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
    let decision = lower_boolean_decision(
        &return_expression,
        LoweredBooleanDecision::Return(true),
        LoweredBooleanDecision::Return(false),
    );
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
    let mut all_operations = Vec::new();
    let condition = emit_boolean_expression(
        &condition,
        &parameters,
        &mut next_value_identity,
        &mut all_operations,
    );
    let entry_operation_end = all_operations.len();
    let true_parameters =
        allocate_boolean_parameters(when_true_parameter_count, &mut next_value_identity);
    let false_parameters =
        allocate_boolean_parameters(when_false_parameter_count, &mut next_value_identity);
    let true_operation_start = all_operations.len();
    let true_value = emit_boolean_expression(
        &when_true_expression,
        &true_parameters,
        &mut next_value_identity,
        &mut all_operations,
    );
    let true_operation_end = all_operations.len();
    let false_value = emit_boolean_expression(
        &when_false_expression,
        &false_parameters,
        &mut next_value_identity,
        &mut all_operations,
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
            let decision = lower_boolean_decision(
                jump_expression,
                LoweredBooleanDecision::Return(true),
                LoweredBooleanDecision::Return(false),
            );
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
        let decision = lower_boolean_decision(
            &return_expression,
            LoweredBooleanDecision::Return(true),
            LoweredBooleanDecision::Return(false),
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
            .map(|(jump_expression, scalar_type)| {
                emit_direct_expression(
                    jump_expression,
                    &current_parameters,
                    *scalar_type,
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
