//! Content conservation, identity reshuffle, and partition-composition lowering.
//!
//! The upstream checking::value_custody::content_conservation normalizer checks
//! projection calls, entry revisions, separation, and equality. This owner
//! lowers those normalized projections and exact theorem/argument substitutions;
//! neither stage infers sealed introductions or custody exits.
//!
//! Keep claim identity distinct from structural path and input/output equality.
//! Compact report equality cannot justify changing the projection, algebra,
//! call, or subject. Unsupported staged derivations reject.
//!
//! Static field, case, and fixed-index paths do not establish general runtime-
//! indexed owned extraction: the frontier must identify the unique moved
//! element. Passing reshuffle or partition helpers establishes neither an
//! ambient mint route nor complete source-to-provider/native conservation.
use super::{
    BTreeMap, BTreeSet, BoundaryContentGuarantee, CheckedContentAlgebraIdentity,
    CheckedContentConservationTerm, CheckedContentPlaceRoot, CheckedContentPlaceSegment,
    CheckedContentPlaceVersion, CheckedContentProjectionExpression, CheckedContentScalarExpression,
    CheckedContentStructuralPlace, CheckedTrees, CheckedUnitEntryClaimPlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment, ClaimContentProjection,
    ClaimId, ContentAlgebra, ContentAlgebraKind, ContentArithmeticOperator, ContentConservation,
    ContentConservationGuarantee, ContentConservationOwnerKind, ContentConservationPlan,
    ContentDomainId, ContentEntryClaim, ContentIdentityReshuffle, ContentIdentityReshuffleFact,
    ContentPlaceSegment, ContentPlaceSubstitution, ContentPlaceVersion,
    ContentProjectionExpression, ContentProjectionIdentity, ContentProjectionScalar,
    ContentStructuralPlace, ContentTerm, LoweringError, PermissionClaimIdentity, PlaceId,
    Proposition, PropositionContext, SemanticDomainId, StructuralContentProjection,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralPlaceKind,
    StructuralTypeDeclaration, StructuralTypeShape, conservation_report_fingerprint,
    lookup_claim_id, unsupported,
};
use crate::emission::operation_emission::buffer::SourceCallCoordinate;

/// One checked content equation translated into terminal-Psi identities.
/// Arena-local domain, projection-machine, and field symbols are deliberately
/// absent; only normalized semantic identities and stable spellings survive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredContentConservation {
    /// Non-authoritative compact coordinate beside the exact proposition.
    pub source_report_fingerprint: u64,
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
    pub compositions: Vec<LoweredContentPartitionComposition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredContentPartitionComposition {
    pub(crate) producer_coordinate: SourceCallCoordinate,
    pub(crate) source_callable: symbols::SymbolHandle,
    /// Non-authoritative compact coordinate beside the exact retained source.
    pub(crate) source_report_fingerprint: u64,
    pub(crate) source_structural_places: Vec<StructuralPlaceDeclaration>,
    pub(crate) source: ContentConservation,
    pub(crate) input_claims: Vec<ClaimId>,
    pub(crate) substitutions: Vec<ContentPlaceSubstitution>,
    pub(crate) derived: ContentConservation,
}
fn content_field_identity(checked: &CheckedTrees, symbol: symbols::SymbolHandle) -> Option<String> {
    checked.data_definitions().iter().find_map(|definition| {
        checked.data_members(definition).iter().find_map(|member| {
            let checked_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            (field.symbol == symbol).then(|| field.path_identity())
        })
    })
}

fn lower_content_projection_scalar(
    checked: &CheckedTrees,
    value: &CheckedContentScalarExpression,
    depth: usize,
) -> Result<ContentProjectionScalar, LoweringError> {
    if depth > 256 {
        return unsupported("content projection expression is too deeply nested");
    }
    let path = |segments: &[language_semantics::content::ContentFieldSegment]| {
        segments
            .iter()
            .map(|segment| {
                let identity = content_field_identity(checked, segment.symbol).ok_or(
                    LoweringError::Unsupported(
                        "content projection references an unknown structural field",
                    ),
                )?;
                Ok(identity)
            })
            .collect::<Result<Vec<_>, LoweringError>>()
    };
    Ok(match value {
        CheckedContentScalarExpression::SubjectField(segments) => {
            ContentProjectionScalar::SubjectField(path(segments)?)
        }
        CheckedContentScalarExpression::RuntimeScalarEmbedding(segments) => {
            ContentProjectionScalar::RuntimeScalarEmbedding(path(segments)?)
        }
        CheckedContentScalarExpression::Natural(value) => {
            if value.is_empty() {
                return unsupported("content projection contains an empty natural");
            }
            ContentProjectionScalar::Natural(value.clone())
        }
        CheckedContentScalarExpression::Successor(inner) => ContentProjectionScalar::Successor(
            Box::new(lower_content_projection_scalar(checked, inner, depth + 1)?),
        ),
        CheckedContentScalarExpression::Arithmetic {
            operator,
            left,
            right,
        } => {
            let left = Box::new(lower_content_projection_scalar(checked, left, depth + 1)?);
            let right = Box::new(lower_content_projection_scalar(checked, right, depth + 1)?);
            match operator {
                ContentArithmeticOperator::Add => ContentProjectionScalar::Add(left, right),
                ContentArithmeticOperator::Subtract => {
                    ContentProjectionScalar::Subtract(left, right)
                }
                ContentArithmeticOperator::Multiply => {
                    ContentProjectionScalar::Multiply(left, right)
                }
            }
        }
    })
}

fn lower_content_projection_expression(
    checked: &CheckedTrees,
    expression: &CheckedContentProjectionExpression,
) -> Result<ContentProjectionExpression, LoweringError> {
    Ok(match expression {
        CheckedContentProjectionExpression::IntervalSet { members } => {
            ContentProjectionExpression::IntervalSet(
                members
                    .iter()
                    .map(|member| {
                        Ok((
                            lower_content_projection_scalar(checked, member.start(), 0)?,
                            lower_content_projection_scalar(checked, member.end(), 0)?,
                        ))
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?,
            )
        }
        CheckedContentProjectionExpression::CountedQuantity { magnitude } => {
            ContentProjectionExpression::CountedQuantity(lower_content_projection_scalar(
                checked, magnitude, 0,
            )?)
        }
    })
}

pub(crate) fn lower_structural_content_projection(
    checked: &CheckedTrees,
    domain: SemanticDomainId,
    expected_carrier_identity: &str,
) -> Result<Option<StructuralContentProjection>, LoweringError> {
    let Some(projection) = checked
        .facts
        .qualifications
        .content
        .for_semantic_domain(domain)
    else {
        return Ok(None);
    };
    if projection.carrier_identity != expected_carrier_identity
        || projection.report_fingerprint == 0
    {
        return unsupported(
            "structural domain content projection disagrees with its carrier or identity",
        );
    }
    let algebra = match &projection.algebra {
        CheckedContentAlgebraIdentity::IntervalSet { coordinate_space } => ContentAlgebra {
            kind: ContentAlgebraKind::IntervalSet,
            parameter: coordinate_space.clone(),
        },
        CheckedContentAlgebraIdentity::CountedQuantity { unit } => ContentAlgebra {
            kind: ContentAlgebraKind::CountedQuantity,
            parameter: unit.clone(),
        },
    };
    let expression = lower_content_projection_expression(checked, &projection.expression)?;
    let identity = ContentProjectionIdentity {
        domain: ContentDomainId::new(u64::from(domain.0))
            .ok_or(LoweringError::InvalidContentDomainIdentity)?,
        projection_report_fingerprint: projection.report_fingerprint,
    };
    if language_semantics::content::terminal_projection_report_fingerprint(&algebra, &expression)
        != identity.projection_report_fingerprint
    {
        return unsupported(
            "structural domain content projection report_fingerprint does not replay",
        );
    }
    Ok(Some(StructuralContentProjection {
        identity,
        algebra,
        expression,
    }))
}

/// Publish the exact content carried by claims below whole structural entries.
///
/// Structural claim identity remains authoritative. A qualification contributes
/// content only when the checker retained its owner-unique `Content<Owner, A>` plan,
/// and the projection must describe the carrier at that exact claim path.
/// Projected qualifications never qualify a sibling or the whole owner. This
/// binds entry content, not a partition theorem or boundary admission.
pub(crate) fn lower_whole_content_entry_claims(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    structural_types: &[StructuralTypeDeclaration],
    checked_parameters: &[CheckedUnitStructuralParameterPlan],
    parameters: &[StructuralParameterDeclaration],
    entry_claims: &[CheckedUnitEntryClaimPlan],
    claim_bindings: &[(PermissionClaimIdentity, ClaimId)],
) -> Result<Vec<ContentEntryClaim>, LoweringError> {
    if checked_parameters.len() != parameters.len() {
        return unsupported("content entry parameter catalogs disagree");
    }

    let mut output = Vec::new();
    for entry_claim in entry_claims {
        let parameter_index = usize::try_from(entry_claim.parameter_index).map_err(|_| {
            LoweringError::Unsupported("content entry claim parameter index exceeds usize")
        })?;
        let checked_parameter =
            checked_parameters
                .get(parameter_index)
                .ok_or(LoweringError::Unsupported(
                    "content entry claim has an invalid checked parameter",
                ))?;
        let parameter = parameters
            .get(parameter_index)
            .ok_or(LoweringError::Unsupported(
                "content entry claim has an invalid terminal parameter",
            ))?;

        if checked_parameter
            .qualifications
            .windows(2)
            .any(|pair| pair[0].0 >= pair[1].0)
            || checked_parameter
                .projected_qualifications
                .windows(2)
                .any(|pair| {
                    pair[0].path > pair[1].path
                        || (pair[0].path == pair[1].path && pair[0].domain.0 >= pair[1].domain.0)
                })
            || checked_parameter
                .projected_qualifications
                .iter()
                .any(|row| row.path.is_empty())
        {
            return unsupported("content entry qualifications are not canonical");
        }
        // Structural custody is already prepared against its declared root.
        // This owner translates only claims carrying Content projections;
        // an unqualified resource must not enter the content-place vocabulary.
        let content = checked_parameter
            .qualifications
            .iter()
            .copied()
            .filter(|_| entry_claim.path.is_empty())
            .chain(
                checked_parameter
                    .projected_qualifications
                    .iter()
                    .filter(|row| row.path == entry_claim.path)
                    .map(|row| row.domain),
            )
            .filter_map(|qualification| {
                checked
                    .facts
                    .qualifications
                    .content
                    .for_semantic_domain(qualification)
            })
            .collect::<Vec<_>>();
        if content.is_empty() {
            continue;
        }
        let mut carrier =
            terminal_psi::find_structural_type(structural_types, parameter.structural_type).ok_or(
                LoweringError::Unsupported("content entry carrier is absent"),
            )?;
        let mut segments = Vec::with_capacity(entry_claim.path.len());
        for segment in &entry_claim.path {
            let child = match (segment, &carrier.shape) {
                (
                    CheckedUnitStructuralPathSegment::Field(identity),
                    StructuralTypeShape::Record { fields, .. },
                ) => {
                    let field = terminal_psi::field_by_identity(
                        fields,
                        identities.intern(&identity.spelling()),
                    )
                    .ok_or(LoweringError::Unsupported(
                        "content claim field is absent from its carrier",
                    ))?;
                    if field.relevance != language_core::BindingRelevance::Relevant {
                        return unsupported("content claim traverses an erased field");
                    }
                    let Some(child) = field.field_type.as_structural() else {
                        return unsupported("content claim selects a nonstructural field");
                    };
                    segments.push(ContentPlaceSegment::Field(identity.spelling()));
                    child
                }
                (
                    CheckedUnitStructuralPathSegment::FixedIndex(index),
                    StructuralTypeShape::FixedArray { element, length },
                ) if index < length => {
                    segments.push(ContentPlaceSegment::FixedIndex(*index));
                    *element
                }
                _ => return unsupported("content claim has an unsupported or mistyped path"),
            };
            carrier = terminal_psi::find_structural_type(structural_types, child).ok_or(
                LoweringError::Unsupported("content claim leaf carrier is absent"),
            )?;
        }
        let mut projections = content
            .into_iter()
            .map(|projection| {
                if projection.carrier_identity
                    != identities.text(carrier.identity).unwrap_or_default()
                {
                    return unsupported(
                        "content projection carrier disagrees with its qualified parameter",
                    );
                }
                if projection.report_fingerprint == 0 {
                    return Err(LoweringError::ZeroContentProjectionFingerprint);
                }
                let domain = ContentDomainId::new(u64::from(projection.semantic_domain.0))
                    .ok_or(LoweringError::InvalidContentDomainIdentity)?;
                let algebra = match &projection.algebra {
                    CheckedContentAlgebraIdentity::IntervalSet { coordinate_space } => {
                        ContentAlgebra {
                            kind: ContentAlgebraKind::IntervalSet,
                            parameter: coordinate_space.clone(),
                        }
                    }
                    CheckedContentAlgebraIdentity::CountedQuantity { unit } => ContentAlgebra {
                        kind: ContentAlgebraKind::CountedQuantity,
                        parameter: unit.clone(),
                    },
                };
                Ok(ClaimContentProjection {
                    projection: ContentProjectionIdentity {
                        domain,
                        projection_report_fingerprint: projection.report_fingerprint,
                    },
                    algebra,
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        if projections.is_empty() {
            continue;
        }
        projections.sort();
        if projections.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(LoweringError::DuplicateContentIdentityProjection);
        }
        let claim = lookup_claim_id(claim_bindings, entry_claim.claim_identity)?;
        output.push(ContentEntryClaim {
            claim,
            input: ContentStructuralPlace {
                version: ContentPlaceVersion::Entry,
                root: parameter.place,
                segments,
            },
            projections,
        });
    }

    output.sort_by_key(|entry| entry.claim);
    for (index, entry) in output.iter().enumerate() {
        let expected = ClaimId::new(
            u64::try_from(index)
                .expect("an in-memory content claim count fits u64")
                .checked_add(1)
                .expect("an in-memory content claim count cannot exhaust u64"),
        )
        .expect("dense content claim identities begin at one");
        if entry.claim != expected {
            return unsupported(
                "bodyless content-bearing claims must form the leading claim frontier",
            );
        }
    }
    Ok(output)
}

/// Lower a validated checked-tree content equation into the current terminal-Psi
/// proposition vocabulary. This translation is independent of the narrow
/// executable source slice so broader terminal lowering can reuse it directly.
pub fn lower_content_conservation_plan(
    plan: &ContentConservationPlan,
) -> Result<LoweredContentConservation, LoweringError> {
    let expected_fingerprint = conservation_report_fingerprint(&plan.algebra, &plan.equation);
    if plan.report_fingerprint != expected_fingerprint {
        return Err(LoweringError::ContentConservationFingerprintMismatch {
            expected: expected_fingerprint,
            actual: plan.report_fingerprint,
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
        source_report_fingerprint: plan.report_fingerprint,
        structural_places: structural_places
            .into_iter()
            .map(|(id, kind)| StructuralPlaceDeclaration { id, kind })
            .collect(),
        proposition,
    })
}

pub fn lower_boundary_content_guarantees(
    plans: &[ContentConservationPlan],
    callable: symbols::SymbolHandle,
) -> Result<Vec<BoundaryContentGuarantee>, LoweringError> {
    let mut guarantees = plans
        .iter()
        .filter(|plan| {
            plan.owner_kind == ContentConservationOwnerKind::TraitRequirement
                && plan.callable == callable
        })
        .map(|plan| {
            let lowered = lower_content_conservation_plan(plan)?;
            let conservation = lowered_conservation(lowered.proposition)?;
            Ok(BoundaryContentGuarantee::Conservation(
                ContentConservationGuarantee {
                    report_fingerprint: lowered.source_report_fingerprint,
                    structural_places: lowered.structural_places,
                    conservation,
                },
            ))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    guarantees.sort();
    if guarantees.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(LoweringError::DuplicateContentPartitionComposition);
    }
    Ok(guarantees)
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
            .find(|group| group.source_claim == fact.claim_identity && group.output == output)
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
    for (position, group) in groups.iter().enumerate() {
        for previous in &groups[..position] {
            if previous.source_claim == group.source_claim {
                if previous.input != group.input
                    || previous.projections != group.projections
                    || !previous.output.is_case_alternative(&group.output)
                {
                    return Err(LoweringError::ContentIdentityClaimMapsMultiplePlaces);
                }
            } else if previous.input == group.input {
                return Err(LoweringError::DuplicateContentIdentityInput);
            } else if content_places_overlap(&previous.input, &group.input) {
                return Err(LoweringError::OverlappingContentIdentityInput);
            }
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
        .map(|group| {
            let claim = source_claims
                .iter()
                .find_map(|(source, claim)| (*source == group.source_claim).then_some(*claim))
                .unwrap_or_else(|| {
                    let claim = ClaimId::new(
                        u64::try_from(source_claims.len())
                            .expect("an in-memory fact count fits u64")
                            + 1,
                    )
                    .expect("dense claim identities begin at one");
                    source_claims.push((group.source_claim, claim));
                    claim
                });
            ContentIdentityReshuffle {
                claim,
                input: group.input,
                output: group.output,
                projections: group.projections,
            }
        })
        .collect::<Vec<_>>();
    let mut entry_claims = Vec::new();
    for reshuffle in &reshuffles {
        if !entry_claims
            .iter()
            .any(|entry: &ContentEntryClaim| entry.claim == reshuffle.claim)
        {
            entry_claims.push(ContentEntryClaim {
                claim: reshuffle.claim,
                input: reshuffle.input.clone(),
                projections: reshuffle.projections.clone(),
            });
        }
    }
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

fn lowered_conservation(proposition: Proposition) -> Result<ContentConservation, LoweringError> {
    match proposition {
        Proposition::ContentConservation(conservation) => Ok(conservation),
        _ => Err(LoweringError::ContentPartitionNotConservation),
    }
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
pub(crate) const RESULT_STRUCTURAL_PLACE_ID: u64 = 4_294_967_297;

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
            projection_report_fingerprint,
            subject,
            ..
        } => {
            let domain = ContentDomainId::new(u64::from(semantic_domain.0))
                .ok_or(LoweringError::InvalidContentDomainIdentity)?;
            if *projection_report_fingerprint == 0 {
                return Err(LoweringError::ZeroContentProjectionFingerprint);
            }
            Ok(ContentTerm::Projection {
                projection: ContentProjectionIdentity {
                    domain,
                    projection_report_fingerprint: *projection_report_fingerprint,
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
        CheckedContentPlaceRoot::Parameter { position, .. } => (
            PlaceId::new(u64::from(*position) + 1)
                .expect("a parameter position plus one is nonzero"),
            StructuralPlaceKind::Parameter {
                position: *position,
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
