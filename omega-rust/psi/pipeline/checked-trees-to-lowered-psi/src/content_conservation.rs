//! Content conservation, identity reshuffle, and partition-composition lowering.

use super::*;

fn content_field_identity(checked: &CheckedTrees, symbol: symbols::SymbolHandle) -> Option<String> {
    checked.data_definitions().iter().find_map(|definition| {
        checked.data_members(definition).iter().find_map(|member| {
            let checked_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            (field.symbol == symbol).then(|| {
                field
                    .identity
                    .map(|identity| format!("#{identity}"))
                    .unwrap_or_else(|| field.name.as_str().to_owned())
            })
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
                if identity != segment.name {
                    return unsupported(
                        "content projection field identity drifted from its definition",
                    );
                }
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

pub(super) fn lower_structural_content_projection(
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

/// Publish the exact content carried by whole structural entry claims.
///
/// Structural claim identity remains authoritative. A qualification contributes
/// content only when the checker retained its owner-unique `Content<A>` plan,
/// and the projection must describe the same carrier as the parameter. This
/// first bodyless-boundary slice deliberately rejects projected claim paths;
/// partial custody needs the authored partition/replay lane.
pub(super) fn lower_whole_content_entry_claims(
    checked: &CheckedTrees,
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

        let mut projections = checked_parameter
            .qualifications
            .iter()
            .filter_map(|qualification| {
                checked
                    .facts
                    .qualifications
                    .content
                    .for_semantic_domain(*qualification)
            })
            .map(|projection| {
                if projection.carrier_identity != checked_parameter.type_identity {
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
        if !entry_claim.path.is_empty() {
            return unsupported(
                "bodyless content custody currently requires a whole structural parameter",
            );
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
                segments: Vec::new(),
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
            || fact.source_plan.report_fingerprint != fact.source_report_fingerprint
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
        compositions.push(LoweredContentPartitionComposition {
            producer_coordinate: SourceCallCoordinate {
                state: fact.state_symbol,
                statement_index: fact.statement_index,
                call_ordinal: fact.call_ordinal,
            },
            source_callable: fact.source_callable,
            source_report_fingerprint: fact.source_report_fingerprint,
            source_structural_places,
            source: source_conservation,
            input_claims,
            substitutions,
            derived: derived_conservation,
        });
    }

    if compositions.iter().enumerate().any(|(index, composition)| {
        compositions[index + 1..]
            .iter()
            .any(|later| later == composition)
    }) {
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
    substitutions: &[checked_trees::ContentPartitionPlaceSubstitution],
) -> Result<CheckedContentConservationTerm, LoweringError> {
    match term {
        CheckedContentConservationTerm::Projection {
            domain,
            semantic_domain,
            projection_machine,
            projection_report_fingerprint,
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
                projection_report_fingerprint: *projection_report_fingerprint,
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

pub(super) fn merge_content_place_declaration(
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
pub(super) const RESULT_STRUCTURAL_PLACE_ID: u64 = 4_294_967_297;

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
