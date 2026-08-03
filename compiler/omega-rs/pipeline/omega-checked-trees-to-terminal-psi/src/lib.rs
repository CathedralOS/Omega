#![forbid(unsafe_code)]

//! Transitional checked-Omega-trees to terminal-Psi compatibility lowering.
//!
//! Psi will ultimately own parsing through terminal-module production. While
//! that frontend ownership migrates, this adapter proves a real source program
//! can cross the terminal boundary without retaining source trees. Its accepted
//! surface is deliberately tiny and exact; unsupported source constructs fail
//! closed instead of being dropped.

use std::collections::{BTreeMap, BTreeSet};

use omega_checked_trees::{
    CheckedTrees, ContentIdentityReshuffleFact, ContentPartitionCompositionFact,
    expression::{BinaryOperator, ExpressionNode},
    signature::SignatureContractKind,
    statement::{StatementNode, TransitionGuardNode, TransitionTargetNode},
    types::PrimitiveType,
};
use omega_core::content::{
    ContentAlgebraIdentity as OmegaContentAlgebraIdentity, ContentConservationEquation,
    ContentConservationOwnerKind, ContentConservationPlan,
    ContentConservationTerm as OmegaContentConservationTerm,
    ContentPlaceRoot as OmegaContentPlaceRoot, ContentPlaceSegment as OmegaContentPlaceSegment,
    ContentPlaceVersion as OmegaContentPlaceVersion,
    ContentStructuralPlace as OmegaContentStructuralPlace, conservation_fingerprint,
};
use omega_core::semantics::PermissionClaimIdentity;
use omega_typed_trees::domain::ProofFact;
use psi_core::{
    BlockId, ClaimId, ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentDomainId,
    ContentPlaceSegment, ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace,
    ContentTerm, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
    ObligationId, OperationId, PlaceId, Proposition, PropositionContext, PropositionError,
    ScalarTerm, ScalarType, StructuralPlaceKind, ValueId,
};
use psi_proof_kernel::{EvidenceRoute, PrimitiveJudgment};
use psi_terminal::{
    Block, ClaimContentProjection, ContentIdentityReshuffle, ContentPartitionComposition,
    ContentPlaceSubstitution, ContractClause, MachineContract, Operation, OperationKind,
    SemanticVersion, StructuralPlaceDeclaration, TerminalMachine, TerminalModule, Terminator,
    ValueDeclaration,
};
use psi_terminal_verifier::{ObligationEvidence, ProofBundle};

/// Semantic module and separate replaceable proof artifact produced by the
/// transitional frontend adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredTerminalPsi {
    pub semantic_module: TerminalModule,
    pub proof_bundle: ProofBundle,
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
    /// Transitional checked identities paired with their dense terminal IDs.
    /// This map never enters terminal Psi; later derived rows consume it while
    /// the adapter still owns both representations.
    pub source_claims: Vec<(PermissionClaimIdentity, ClaimId)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredContentPartitionCompositions {
    pub structural_places: Vec<StructuralPlaceDeclaration>,
    pub compositions: Vec<ContentPartitionComposition>,
}

/// Lower a validated checked-tree content equation into the terminal-Psi v9
/// proposition vocabulary. This translation is independent of the temporary
/// executable source slice so later frontend migration can reuse it directly.
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
        OmegaContentAlgebraIdentity::IntervalSet { coordinate_space } => ContentAlgebra {
            kind: ContentAlgebraKind::IntervalSet,
            parameter: coordinate_space.clone(),
        },
        OmegaContentAlgebraIdentity::CountedQuantity { unit } => ContentAlgebra {
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
            OmegaContentConservationTerm::Separate(_)
        ) && !matches!(
            fact.source_plan.equation.right(),
            OmegaContentConservationTerm::Separate(_)
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
    term: &OmegaContentConservationTerm,
    expected: &OmegaContentStructuralPlace,
) -> bool {
    match term {
        OmegaContentConservationTerm::Projection { subject, .. } => subject == expected,
        OmegaContentConservationTerm::Separate(terms) => terms
            .iter()
            .any(|term| checked_partition_term_contains_subject(term, expected)),
    }
}

fn replay_checked_partition_term(
    term: &OmegaContentConservationTerm,
    substitutions: &[omega_checked_trees::ContentPartitionPlaceSubstitution],
) -> Result<OmegaContentConservationTerm, LoweringError> {
    match term {
        OmegaContentConservationTerm::Projection {
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
            Ok(OmegaContentConservationTerm::Projection {
                domain: *domain,
                semantic_domain: *semantic_domain,
                projection_machine: *projection_machine,
                projection_fingerprint: *projection_fingerprint,
                subject: target,
            })
        }
        OmegaContentConservationTerm::Separate(terms) => {
            Ok(OmegaContentConservationTerm::separate(
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
            OmegaContentConservationTerm::Projection {
                subject: OmegaContentStructuralPlace {
                    version: OmegaContentPlaceVersion::Entry,
                    root: OmegaContentPlaceRoot::Parameter { symbol, .. },
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
    term: &OmegaContentConservationTerm,
    structural_places: &mut BTreeMap<PlaceId, StructuralPlaceKind>,
    depth: usize,
) -> Result<ContentTerm, LoweringError> {
    if depth > MAX_CONTENT_TERM_DEPTH {
        return Err(LoweringError::ContentTermNestingTooDeep);
    }
    match term {
        OmegaContentConservationTerm::Projection {
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
        OmegaContentConservationTerm::Separate(terms) => ContentTerm::separate(
            terms
                .iter()
                .map(|term| lower_content_term(term, structural_places, depth + 1))
                .collect::<Result<Vec<_>, _>>()?,
        )
        .map_err(LoweringError::InvalidContentProposition),
    }
}

fn lower_content_place(
    place: &OmegaContentStructuralPlace,
    structural_places: &mut BTreeMap<PlaceId, StructuralPlaceKind>,
) -> Result<ContentStructuralPlace, LoweringError> {
    let version = match place.version {
        OmegaContentPlaceVersion::Entry => ContentPlaceVersion::Entry,
        OmegaContentPlaceVersion::Current => ContentPlaceVersion::Current,
    };
    let (root, kind) = match &place.root {
        OmegaContentPlaceRoot::Parameter {
            position, is_self, ..
        } => (
            PlaceId::new(u64::from(*position) + 1)
                .expect("a parameter position plus one is nonzero"),
            StructuralPlaceKind::Parameter {
                position: *position,
                is_self: *is_self,
            },
        ),
        OmegaContentPlaceRoot::Result => (
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
            OmegaContentPlaceSegment::Case(case) => ContentPlaceSegment::Case(case.name.clone()),
            OmegaContentPlaceSegment::Field(field) => {
                ContentPlaceSegment::Field(field.name.clone())
            }
            OmegaContentPlaceSegment::FixedIndex(index) => ContentPlaceSegment::FixedIndex(*index),
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
/// machine name() -> integer
/// requires L == L
/// ensures L == L
/// {
///     transition { _ -> done(L) }
///     state done(value: integer) -> integer { L }
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
    let [entry_state, return_state] = states else {
        return unsupported("machine must contain exactly an entry state and one return state");
    };
    if !checked.state_parameters(entry_state).is_empty() {
        return unsupported("entry-state parameters are not supported");
    }
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
    let ExpressionNode::Integer(return_literal) =
        checked.expression_table.expression(*return_expression)
    else {
        return unsupported("return state must return an integer literal");
    };
    if integer_value(return_literal, return_type)? != value {
        return unsupported("jump and return literals must be equal");
    }

    validate_contract(checked, machine, return_type, value)?;
    let identity_facts = checked
        .facts
        .qualifications
        .content
        .identity_reshuffles
        .iter()
        .filter(|fact| {
            fact.machine_symbol == machine.symbol && fact.state_symbol == entry_state.symbol
        })
        .cloned()
        .collect::<Vec<_>>();
    let identity_reshuffles = lower_content_identity_reshuffles(&identity_facts)?;
    let partition_facts = checked
        .facts
        .qualifications
        .content
        .partition_compositions
        .iter()
        .filter(|fact| {
            fact.machine_symbol == machine.symbol && fact.state_symbol == entry_state.symbol
        })
        .cloned()
        .collect::<Vec<_>>();
    let partition_compositions =
        lower_content_partition_compositions(&partition_facts, &identity_reshuffles)?;
    Ok(build_module(
        return_type,
        value,
        identity_reshuffles,
        partition_compositions,
    ))
}

fn validate_contract(
    checked: &CheckedTrees,
    machine: &omega_checked_trees::machine::Machine,
    result_type: ScalarType,
    expected_value: IntegerValue,
) -> Result<(), LoweringError> {
    let contracts = checked.machine_contracts(machine);
    if contracts.len() != 2 {
        return unsupported("machine must have exactly one requires and one ensures clause");
    };
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
        if integer_value(left_literal, result_type)? != expected_value
            || integer_value(right_literal, result_type)? != expected_value
        {
            return unsupported("contract literals must equal the executed literal");
        }
    }
    Ok(())
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
    literal: &omega_core::literals::IntegerLiteral,
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

fn build_module(
    result_type: ScalarType,
    value: IntegerValue,
    identity_reshuffles: LoweredContentIdentityReshuffles,
    partition_compositions: LoweredContentPartitionCompositions,
) -> LoweredTerminalPsi {
    let jump_constant_id = value_id(1);
    let parameter_id = value_id(2);
    let return_constant_id = value_id(3);
    let result_id = value_id(4);
    let ScalarType::Integer(integer_type) = result_type else {
        unreachable!("source slice accepts only integer results");
    };
    let literal = ScalarTerm::integer(integer_type, value)
        .expect("validated source literal fits its terminal integer type");
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
                        operations: vec![Operation {
                            id: operation_id(2),
                            result: ValueDeclaration {
                                id: return_constant_id,
                                scalar_type: result_type,
                            },
                            kind: OperationKind::IntegerConstant { value },
                        }],
                        terminator: Terminator::Return {
                            edge: edge_id(2),
                            value: return_constant_id,
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
    use omega_core::{
        content::{
            ContentCaseSegment, ContentConservationEquation, ContentConservationOwnerKind,
            ContentFieldSegment,
        },
        semantics::{PermissionEventSource, SemanticDomainId},
        symbols::SymbolHandle,
    };

    fn source_projection(
        version: OmegaContentPlaceVersion,
        root: OmegaContentPlaceRoot,
        fields: &[(&str, u32)],
        semantic_domain: SemanticDomainId,
    ) -> OmegaContentConservationTerm {
        OmegaContentConservationTerm::Projection {
            domain: SymbolHandle::from_arena_index(70),
            semantic_domain,
            projection_machine: SymbolHandle::from_arena_index(71),
            projection_fingerprint: 0xfeed,
            subject: OmegaContentStructuralPlace {
                version,
                root,
                segments: fields
                    .iter()
                    .map(|(name, symbol)| {
                        OmegaContentPlaceSegment::Field(ContentFieldSegment {
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
            OmegaContentPlaceVersion::Entry,
            OmegaContentPlaceRoot::Parameter {
                position: 0,
                symbol: SymbolHandle::from_arena_index(10),
                name: "extent".to_owned(),
                is_self: false,
            },
            &[],
            semantic_domain,
        );
        let left = source_projection(
            OmegaContentPlaceVersion::Current,
            OmegaContentPlaceRoot::Result,
            &[("left", 11)],
            semantic_domain,
        );
        let right = source_projection(
            OmegaContentPlaceVersion::Current,
            OmegaContentPlaceRoot::Result,
            &[("right", 12)],
            semantic_domain,
        );
        let algebra = OmegaContentAlgebraIdentity::IntervalSet {
            coordinate_space: "Address".to_owned(),
        };
        let equation = ContentConservationEquation::new(
            entry,
            OmegaContentConservationTerm::separate([right, left]),
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
            OmegaContentPlaceVersion::Entry,
            OmegaContentPlaceRoot::Parameter {
                position: 0,
                symbol: SymbolHandle::from_arena_index(10),
                name: "extent".to_owned(),
                is_self: false,
            },
            &[],
            semantic_domain,
        );
        let output = source_projection(
            OmegaContentPlaceVersion::Current,
            OmegaContentPlaceRoot::Result,
            &[(output_field, 11)],
            semantic_domain,
        );
        let algebra = OmegaContentAlgebraIdentity::IntervalSet {
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
                OmegaContentPlaceSegment::Case(ContentCaseSegment {
                    symbol: SymbolHandle::from_arena_index(30),
                    name: "Present".to_owned(),
                }),
                OmegaContentPlaceSegment::Field(ContentFieldSegment {
                    symbol: SymbolHandle::from_arena_index(31),
                    name: "region".to_owned(),
                }),
            ]
        };
        let projection = |version, root| OmegaContentConservationTerm::Projection {
            domain: SymbolHandle::from_arena_index(70),
            semantic_domain,
            projection_machine: SymbolHandle::from_arena_index(71),
            projection_fingerprint: 0xfeed,
            subject: OmegaContentStructuralPlace {
                version,
                root,
                segments: segments(),
            },
        };
        let equation = ContentConservationEquation::new(
            projection(
                OmegaContentPlaceVersion::Entry,
                OmegaContentPlaceRoot::Parameter {
                    position: 0,
                    symbol: SymbolHandle::from_arena_index(10),
                    name: "envelope".to_owned(),
                    is_self: false,
                },
            ),
            projection(
                OmegaContentPlaceVersion::Current,
                OmegaContentPlaceRoot::Result,
            ),
        );
        let algebra = OmegaContentAlgebraIdentity::IntervalSet {
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
            term: &OmegaContentConservationTerm,
            output: &mut Vec<OmegaContentStructuralPlace>,
        ) {
            match term {
                OmegaContentConservationTerm::Projection { subject, .. } => {
                    if !output.contains(subject) {
                        output.push(subject.clone());
                    }
                }
                OmegaContentConservationTerm::Separate(terms) => {
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
            source_plan,
            statement_index: 4,
            call_ordinal: 2,
            input_claim_identities: vec![
                identity_fact(SemanticDomainId(9), "left", 1).claim_identity,
            ],
            substitutions: places
                .into_iter()
                .map(
                    |place| omega_checked_trees::ContentPartitionPlaceSubstitution {
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

        let mut drifted = fact;
        let projection = drifted.plan.equation.left().clone();
        drifted.plan.equation = ContentConservationEquation::new(
            projection.clone(),
            OmegaContentConservationTerm::separate([projection.clone(), projection]),
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
