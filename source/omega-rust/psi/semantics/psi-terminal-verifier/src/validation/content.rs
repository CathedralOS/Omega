use super::*;

pub(super) fn validate_boundary_content_guarantees(
    module: &TerminalModule,
    registry: &mut IdRegistry,
) -> Result<(), ModuleError> {
    for boundary in &module.boundary_machines {
        if boundary
            .content_guarantees
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(ModuleError::NonCanonicalBoundaryContentGuarantees(
                boundary.id,
            ));
        }
        for guarantee in &boundary.content_guarantees {
            let BoundaryContentGuarantee::Conservation(guarantee) = guarantee else {
                let custody = match guarantee {
                    BoundaryContentGuarantee::RetainedBorrow(custody) => custody,
                    BoundaryContentGuarantee::Conservation(_) => unreachable!(),
                };
                if !validate_retained_borrow_custody(boundary, custody) {
                    return Err(ModuleError::InvalidBoundaryContentGuarantee(boundary.id));
                }
                continue;
            };
            let kinds =
                validate_guarantee_places(&guarantee.structural_places, |kind| match kind {
                    StructuralPlaceKind::Parameter { position, is_self } => {
                        boundary.structural_parameters.iter().any(|parameter| {
                            parameter.position == position && parameter.is_self == is_self
                        })
                    }
                    StructuralPlaceKind::Result
                    | StructuralPlaceKind::OperationResult { .. }
                    | StructuralPlaceKind::ByteSequenceLiteral { .. }
                    | StructuralPlaceKind::ProviderAttachment { .. }
                    | StructuralPlaceKind::TrivialAffineLocal { .. } => false,
                })
                .ok_or(ModuleError::InvalidBoundaryContentGuarantee(boundary.id))?;
            let context = PropositionContext::from_value_types_and_places(
                [],
                guarantee
                    .structural_places
                    .iter()
                    .map(|place| (place.id, place.kind)),
            )
            .map_err(ModuleError::MalformedProposition)?;
            context
                .validate(&Proposition::ContentConservation(
                    guarantee.conservation.clone(),
                ))
                .map_err(ModuleError::MalformedProposition)?;
            if content_conservation_report_fingerprint(&guarantee.conservation, &kinds)
                != Some(guarantee.report_fingerprint)
            {
                return Err(ModuleError::InvalidBoundaryContentGuarantee(boundary.id));
            }
            register_partition_projections(registry, &guarantee.conservation)?;
        }
    }
    Ok(())
}

fn validate_retained_projection(projection: &RetainedBorrowContentProjection) -> bool {
    !projection.carrier_identity.is_empty()
        && projection.semantic_domain.get() != 0
        && projection.projection.identity.domain.get() == projection.semantic_domain.get()
        && projection.projection.identity.projection_report_fingerprint != 0
        && psi_language_semantics::content::terminal_projection_report_fingerprint(
            &projection.projection.algebra,
            &projection.projection.expression,
        ) == projection.projection.identity.projection_report_fingerprint
}

fn validate_retained_borrow_custody(
    boundary: &BoundaryMachineDeclaration,
    custody: &psi_terminal::RetainedBorrowCustody,
) -> bool {
    let exact_source = matches!(
        &custody.source,
        psi_terminal::RetainedBorrowPlace {
            version: psi_core::ContentPlaceVersion::Entry,
            root: RetainedBorrowPlaceRoot::Parameter {
                identity,
                is_self: false,
                ..
            },
            segments,
        } if !identity.is_empty() && segments.is_empty()
    );
    let exact_result = matches!(
        &custody.result,
        psi_terminal::RetainedBorrowPlace {
            version: psi_core::ContentPlaceVersion::Current,
            root: RetainedBorrowPlaceRoot::Result,
            segments,
        } if segments.is_empty()
    );
    boundary.identity == custody.callable_identity
        && boundary.attachment.is_none()
        && boundary.scalar_parameters.is_empty()
        && boundary.structural_parameters.is_empty()
        && boundary.result.is_unit()
        && boundary.requires.is_empty()
        && boundary.program_local_root_introductions.is_empty()
        && boundary.published_service_ceiling.is_empty()
        && exact_source
        && exact_result
        && custody.access == StructuralAccess::SharedBorrow
        && custody.callable_lifetime_parameter_count > 0
        && custody.callable_lifetime_parameter_ordinal < custody.callable_lifetime_parameter_count
        && !custody.result_nominal_identity.is_empty()
        && custody.result_multiplicity == StructuralMultiplicity::Linear
        && custody.result_lifetime_argument_count == 1
        && custody.result_lifetime_argument_ordinal == 0
        && custody.result_lifetime_slot_is_erased
        && custody.retained_semantic_domain == custody.result_projection.semantic_domain
        && custody.result_nominal_identity == custody.result_projection.carrier_identity
        && custody.source_projection.semantic_domain != custody.result_projection.semantic_domain
        && custody.source_projection.projection.algebra
            == custody.result_projection.projection.algebra
        && validate_retained_projection(&custody.source_projection)
        && validate_retained_projection(&custody.result_projection)
}

fn validate_guarantee_places(
    places: &[StructuralPlaceDeclaration],
    accepts: impl Fn(StructuralPlaceKind) -> bool,
) -> Option<BTreeMap<PlaceId, StructuralPlaceKind>> {
    let mut ids = BTreeMap::new();
    let mut kinds = BTreeSet::new();
    for place in places {
        if !accepts(place.kind)
            || ids.insert(place.id, place.kind).is_some()
            || !kinds.insert(place.kind)
        {
            return None;
        }
    }
    (!ids.is_empty()).then_some(ids)
}

fn register_content_projection(
    registry: &mut IdRegistry,
    projection: ContentProjectionIdentity,
    algebra: &ContentAlgebra,
) -> Result<(), ModuleError> {
    let Some((owner_identity, owner_algebra)) =
        registry.owner_content_projections.get(&projection.domain)
    else {
        return Err(ModuleError::ContentProjectionOwnerMismatch(projection));
    };
    if *owner_identity != projection || owner_algebra != algebra {
        return Err(ModuleError::ContentProjectionOwnerMismatch(projection));
    }
    if let Some(previous) = registry
        .content_projection_algebras
        .insert(projection, algebra.clone())
        && previous != *algebra
    {
        return Err(ModuleError::ContentProjectionAlgebraMismatch(projection));
    }
    Ok(())
}

pub(super) fn validate_content_entry_claims(
    machine: &TerminalMachine,
    registry: &mut IdRegistry,
    structural_place_kinds: &BTreeMap<PlaceId, StructuralPlaceKind>,
    context: &PropositionContext,
) -> Result<(), ModuleError> {
    let mut inputs = BTreeSet::<ContentStructuralPlace>::new();
    for (index, binding) in machine.content_entry_claims.iter().enumerate() {
        let expected = ClaimId::new(
            u64::try_from(index)
                .expect("an in-memory claim count fits u64")
                .checked_add(1)
                .expect("an in-memory claim count cannot exhaust u64"),
        )
        .expect("dense claim identities begin at one");
        if binding.claim != expected {
            return Err(ModuleError::NonDenseContentEntryClaim {
                expected,
                actual: binding.claim,
            });
        }
        if binding.projections.is_empty() {
            return Err(ModuleError::ContentEntryClaimHasNoProjections(
                binding.claim,
            ));
        }
        if binding
            .projections
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(ModuleError::NonCanonicalContentEntryProjectionOrder(
                binding.claim,
            ));
        }
        if binding.input.version != psi_core::ContentPlaceVersion::Entry
            || !matches!(
                structural_place_kinds.get(&binding.input.root),
                Some(StructuralPlaceKind::Parameter { .. })
            )
        {
            return Err(ModuleError::ContentEntryClaimRequiresEntryParameter(
                binding.claim,
            ));
        }
        if let Some(structural_claim) = machine
            .entry_claims
            .iter()
            .find(|claim| claim.claim == binding.claim)
            && (structural_claim.input != binding.input.root
                || binding.input.segments
                    != structural_claim
                        .path
                        .iter()
                        .map(|segment| match segment {
                            StructuralPathSegment::Field(identity) => {
                                psi_core::ContentPlaceSegment::Field(identity.clone())
                            }
                            StructuralPathSegment::FixedIndex(index) => {
                                psi_core::ContentPlaceSegment::FixedIndex(*index)
                            }
                        })
                        .collect::<Vec<_>>())
        {
            return Err(ModuleError::ContentEntryClaimStructuralBindingMismatch(
                binding.claim,
            ));
        }
        if inputs.contains(&binding.input) {
            return Err(ModuleError::DuplicateContentEntryClaimInput(
                binding.input.clone(),
            ));
        }
        if let Some(previous) = inputs
            .iter()
            .find(|previous| content_places_overlap(previous, &binding.input))
        {
            return Err(ModuleError::OverlappingContentEntryClaimInput {
                first: previous.clone(),
                second: binding.input.clone(),
            });
        }
        inputs.insert(binding.input.clone());
        for content in &binding.projections {
            register_content_projection(registry, content.projection, &content.algebra)?;
            let term = ContentTerm::Projection {
                projection: content.projection,
                subject: binding.input.clone(),
            };
            context
                .validate(&Proposition::ContentConservation(ContentConservation::new(
                    content.algebra.clone(),
                    term.clone(),
                    term,
                )))
                .map_err(ModuleError::MalformedProposition)?;
        }
    }
    Ok(())
}

pub(super) fn validate_content_identity_reshuffles(
    machine: &TerminalMachine,
    registry: &mut IdRegistry,
    structural_place_kinds: &BTreeMap<PlaceId, StructuralPlaceKind>,
    context: &PropositionContext,
) -> Result<(), ModuleError> {
    let Some(result) = machine.result.structural() else {
        if machine.content_identity_reshuffles.is_empty() {
            return Ok(());
        }
        return Err(ModuleError::ContentIdentityReshuffleRequiresStructuralResult(machine.id));
    };
    if machine
        .content_identity_reshuffles
        .windows(2)
        .any(|pair| pair[0].claim >= pair[1].claim)
    {
        return Err(ModuleError::NonCanonicalContentIdentityReshuffles(
            machine.id,
        ));
    }
    let mut claims = BTreeSet::<ClaimId>::new();
    let mut inputs = BTreeSet::<ContentStructuralPlace>::new();
    let mut outputs = BTreeSet::<ContentStructuralPlace>::new();
    for reshuffle in &machine.content_identity_reshuffles {
        insert_unique(&mut claims, reshuffle.claim, ModuleError::DuplicateClaim)?;
        if reshuffle.projections.is_empty() {
            return Err(ModuleError::ContentIdentityReshuffleHasNoProjections(
                reshuffle.claim,
            ));
        }
        let Some(binding) = machine
            .content_entry_claims
            .iter()
            .find(|binding| binding.claim == reshuffle.claim)
        else {
            return Err(ModuleError::ContentIdentityClaimHasNoEntryBinding(
                reshuffle.claim,
            ));
        };
        if binding.input != reshuffle.input || binding.projections != reshuffle.projections {
            return Err(ModuleError::ContentIdentityEntryBindingMismatch(
                reshuffle.claim,
            ));
        }
        if reshuffle
            .projections
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(ModuleError::NonCanonicalContentIdentityProjectionOrder(
                reshuffle.claim,
            ));
        }
        if reshuffle.input.version != psi_core::ContentPlaceVersion::Entry
            || !matches!(
                structural_place_kinds.get(&reshuffle.input.root),
                Some(StructuralPlaceKind::Parameter { .. })
            )
        {
            return Err(ModuleError::ContentIdentityReshuffleRequiresEntryParameter(
                reshuffle.claim,
            ));
        }
        if reshuffle.output.version != psi_core::ContentPlaceVersion::Current
            || reshuffle.output.root != result.place
            || !matches!(
                structural_place_kinds.get(&reshuffle.output.root),
                Some(StructuralPlaceKind::Result)
            )
        {
            return Err(ModuleError::ContentIdentityReshuffleRequiresCurrentResult(
                reshuffle.claim,
            ));
        }
        if inputs.contains(&reshuffle.input) {
            return Err(ModuleError::DuplicateContentIdentityInput(
                reshuffle.input.clone(),
            ));
        }
        if let Some(previous) = inputs
            .iter()
            .find(|previous| content_places_overlap(previous, &reshuffle.input))
        {
            return Err(ModuleError::OverlappingContentIdentityInput {
                first: previous.clone(),
                second: reshuffle.input.clone(),
            });
        }
        inputs.insert(reshuffle.input.clone());
        if outputs.contains(&reshuffle.output) {
            return Err(ModuleError::DuplicateContentIdentityOutput(
                reshuffle.output.clone(),
            ));
        }
        if let Some(previous) = outputs
            .iter()
            .find(|previous| content_places_overlap(previous, &reshuffle.output))
        {
            return Err(ModuleError::OverlappingContentIdentityOutput {
                first: previous.clone(),
                second: reshuffle.output.clone(),
            });
        }
        outputs.insert(reshuffle.output.clone());
        for (content, proposition) in reshuffle
            .projections
            .iter()
            .zip(reshuffle.inferred_propositions())
        {
            register_content_projection(registry, content.projection, &content.algebra)?;
            context
                .validate(&proposition)
                .map_err(ModuleError::MalformedProposition)?;
        }
    }
    Ok(())
}

pub(super) fn validate_content_partition_compositions(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    registry: &mut IdRegistry,
    structural_place_kinds: &BTreeMap<PlaceId, StructuralPlaceKind>,
    context: &PropositionContext,
) -> Result<(), ModuleError> {
    let mut rows = BTreeSet::<&ContentPartitionComposition>::new();
    for composition in &machine.content_partition_compositions {
        if !rows.insert(composition) {
            return Err(ModuleError::DuplicateContentPartitionComposition);
        }
        if composition.input_claims.is_empty() {
            return Err(ModuleError::ContentPartitionCompositionHasNoInputClaims);
        }
        if composition
            .input_claims
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(ModuleError::NonCanonicalContentPartitionInputClaims);
        }
        if composition
            .substitutions
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(ModuleError::NonCanonicalContentPartitionSubstitutions);
        }
        if composition.source.algebra() != composition.derived.algebra() {
            return Err(ModuleError::ContentPartitionAlgebraMismatch);
        }
        if !content_term_contains_partition(composition.source.left())
            && !content_term_contains_partition(composition.source.right())
        {
            return Err(ModuleError::ContentPartitionSourceHasNoSeparation);
        }

        let source_kinds = validate_partition_source_places(composition)?;
        let source_context = PropositionContext::from_value_types_and_places(
            [],
            composition
                .source_structural_places
                .iter()
                .map(|place| (place.id, place.kind)),
        )
        .map_err(ModuleError::MalformedProposition)?;
        source_context
            .validate(&Proposition::ContentConservation(
                composition.source.clone(),
            ))
            .map_err(ModuleError::MalformedProposition)?;
        let reconstructed_fingerprint =
            content_conservation_report_fingerprint(&composition.source, &source_kinds);
        if reconstructed_fingerprint != Some(composition.source_report_fingerprint) {
            return Err(ModuleError::ContentPartitionSourceFingerprintMismatch {
                recorded: composition.source_report_fingerprint,
                reconstructed: reconstructed_fingerprint,
            });
        }
        context
            .validate(&composition.inferred_proposition())
            .map_err(ModuleError::MalformedProposition)?;
        register_partition_projections(registry, &composition.source)?;
        register_partition_projections(registry, &composition.derived)?;

        let substitutions = composition
            .substitutions
            .iter()
            .map(|substitution| (substitution.source.clone(), substitution.target.clone()))
            .collect::<BTreeMap<_, _>>();
        if substitutions.len() != composition.substitutions.len() {
            return Err(ModuleError::NonCanonicalContentPartitionSubstitutions);
        }
        let target_count = composition
            .substitutions
            .iter()
            .map(|substitution| &substitution.target)
            .collect::<BTreeSet<_>>()
            .len();
        if target_count != composition.substitutions.len() {
            return Err(ModuleError::DuplicateContentPartitionSubstitutionTarget);
        }
        let source_subjects = content_conservation_subjects(&composition.source);
        if source_subjects
            != substitutions
                .keys()
                .cloned()
                .collect::<BTreeSet<ContentStructuralPlace>>()
        {
            return Err(ModuleError::ContentPartitionSubstitutionCoverageMismatch);
        }
        for substitution in &composition.substitutions {
            validate_partition_substitution_shape(
                substitution,
                &source_kinds,
                structural_place_kinds,
            )?;
        }
        let replayed = replay_partition_conservation(&composition.source, &substitutions)?;
        if replayed != composition.derived {
            return Err(ModuleError::ContentPartitionReplayMismatch);
        }

        let listed_claims = composition
            .input_claims
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut used_claims = BTreeSet::new();
        for (projection, subject) in content_conservation_projections(&composition.derived) {
            if subject.version != psi_core::ContentPlaceVersion::Entry {
                continue;
            }
            let matching = machine
                .content_entry_claims
                .iter()
                .filter(|binding| {
                    binding.input == subject
                        && binding.projections.iter().any(|content| {
                            content.projection == projection
                                && content.algebra == *composition.derived.algebra()
                        })
                })
                .map(|binding| binding.claim)
                .collect::<Vec<_>>();
            let [claim] = matching.as_slice() else {
                return Err(ModuleError::ContentPartitionInputProjectionNotClaimBound(
                    subject,
                ));
            };
            if !listed_claims.contains(claim) {
                return Err(ModuleError::ContentPartitionInputClaimNotListed(*claim));
            }
            used_claims.insert(*claim);
        }
        if used_claims != listed_claims {
            return Err(ModuleError::ContentPartitionInputClaimUnused);
        }
        validate_partition_producer(module, machine, machines, composition)?;
    }
    Ok(())
}

fn validate_partition_producer(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    composition: &ContentPartitionComposition,
) -> Result<(), ModuleError> {
    let operation = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == composition.producer_operation)
        .ok_or(ModuleError::ContentPartitionProducerOperationMissing(
            composition.producer_operation,
        ))?;
    match &operation.kind {
        OperationKind::Call { callee, .. } => {
            let callee = machines
                .get(callee)
                .copied()
                .expect("validated call target exists");
            validate_partition_internal_guarantee(operation.id, composition, callee, &[], None)
        }
        OperationKind::CallUnit {
            callee,
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralScalar {
            callee,
            structural_arguments,
            ..
        } => {
            let callee = machines
                .get(callee)
                .copied()
                .expect("validated structural call target exists");
            validate_partition_internal_guarantee(
                operation.id,
                composition,
                callee,
                structural_arguments,
                None,
            )
        }
        OperationKind::CallStructural {
            callee,
            structural_arguments,
            ..
        } => {
            let callee = machines
                .get(callee)
                .copied()
                .expect("validated structural call target exists");
            validate_partition_internal_guarantee(
                operation.id,
                composition,
                callee,
                structural_arguments,
                operation.result.structural(),
            )
        }
        OperationKind::BoundaryCall {
            boundary,
            structural_arguments,
            ..
        } => {
            let boundary = module
                .boundary_machines
                .iter()
                .find(|candidate| candidate.id == *boundary)
                .expect("validated boundary call target exists");
            let guarantees = boundary
                .content_guarantees
                .iter()
                .filter_map(|guarantee| {
                    let BoundaryContentGuarantee::Conservation(guarantee) = guarantee else {
                        return None;
                    };
                    (
                        guarantee.structural_places.as_slice(),
                        &guarantee.conservation,
                    )
                        .into()
                })
                .collect::<Vec<_>>();
            validate_partition_guarantee_set(
                operation.id,
                composition,
                &boundary.structural_parameters,
                structural_arguments,
                None,
                &guarantees,
            )
        }
        _ => Err(ModuleError::ContentPartitionProducerNotCall(operation.id)),
    }
}

fn validate_partition_internal_guarantee(
    operation: OperationId,
    composition: &ContentPartitionComposition,
    callee: &TerminalMachine,
    structural_arguments: &[StructuralArgument],
    operation_result: Option<&psi_terminal::StructuralOperationResult>,
) -> Result<(), ModuleError> {
    let guarantees = callee
        .contract
        .ensures
        .iter()
        .filter_map(|clause| match &clause.proposition {
            Proposition::ContentConservation(conservation) => {
                Some((callee.structural_places.as_slice(), conservation))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    validate_partition_guarantee_set(
        operation,
        composition,
        &callee.structural_parameters,
        structural_arguments,
        operation_result,
        &guarantees,
    )
}

fn validate_partition_guarantee_set(
    operation: OperationId,
    composition: &ContentPartitionComposition,
    parameters: &[StructuralParameterDeclaration],
    structural_arguments: &[StructuralArgument],
    operation_result: Option<&psi_terminal::StructuralOperationResult>,
    guarantees: &[(&[StructuralPlaceDeclaration], &ContentConservation)],
) -> Result<(), ModuleError> {
    if !guarantees.iter().any(|(places, conservation)| {
        content_guarantees_alpha_equal(
            &composition.source_structural_places,
            &composition.source,
            places,
            conservation,
        )
    }) {
        return Err(ModuleError::ContentPartitionProducerGuaranteeMissing(
            operation,
        ));
    }
    if !partition_substitutions_match_arguments(
        composition,
        parameters,
        structural_arguments,
        operation_result,
    ) {
        return Err(ModuleError::ContentPartitionProducerArgumentMismatch(
            operation,
        ));
    }
    Ok(())
}

fn content_guarantees_alpha_equal(
    source_places: &[StructuralPlaceDeclaration],
    source: &ContentConservation,
    target_places: &[StructuralPlaceDeclaration],
    target: &ContentConservation,
) -> bool {
    if source.algebra() != target.algebra() {
        return false;
    }

    let source_roots = content_conservation_subjects(source)
        .into_iter()
        .map(|subject| subject.root)
        .collect::<BTreeSet<_>>();
    let target_roots = content_conservation_subjects(target)
        .into_iter()
        .map(|subject| subject.root)
        .collect::<BTreeSet<_>>();
    if source_roots.len() != target_roots.len() {
        return false;
    }

    let mut roots = BTreeMap::new();
    for target_root in target_roots {
        let Some(target_place) = target_places
            .iter()
            .find(|target_place| target_place.id == target_root)
        else {
            return false;
        };
        let Some(source_place) = source_places.iter().find(|source_place| {
            source_roots.contains(&source_place.id) && source_place.kind == target_place.kind
        }) else {
            return false;
        };
        if roots.insert(target_place.id, source_place.id).is_some() {
            return false;
        }
    }
    if roots.values().copied().collect::<BTreeSet<_>>() != source_roots {
        return false;
    }
    remap_content_conservation_roots(target, &roots) == *source
}

fn remap_content_conservation_roots(
    conservation: &ContentConservation,
    roots: &BTreeMap<PlaceId, PlaceId>,
) -> ContentConservation {
    ContentConservation::new(
        conservation.algebra().clone(),
        remap_content_term_roots(conservation.left(), roots),
        remap_content_term_roots(conservation.right(), roots),
    )
}

fn remap_content_term_roots(term: &ContentTerm, roots: &BTreeMap<PlaceId, PlaceId>) -> ContentTerm {
    match term {
        ContentTerm::Projection {
            projection,
            subject,
        } => ContentTerm::Projection {
            projection: *projection,
            subject: ContentStructuralPlace {
                version: subject.version,
                root: roots.get(&subject.root).copied().unwrap_or(subject.root),
                segments: subject.segments.clone(),
            },
        },
        ContentTerm::Separate(terms) => ContentTerm::Separate(
            terms
                .iter()
                .map(|term| remap_content_term_roots(term, roots))
                .collect(),
        ),
    }
}

fn partition_substitutions_match_arguments(
    composition: &ContentPartitionComposition,
    parameters: &[StructuralParameterDeclaration],
    structural_arguments: &[StructuralArgument],
    operation_result: Option<&psi_terminal::StructuralOperationResult>,
) -> bool {
    composition.substitutions.iter().all(|substitution| {
        let Some(source_place) = composition
            .source_structural_places
            .iter()
            .find(|place| place.id == substitution.source.root)
        else {
            return false;
        };
        if source_place.kind == StructuralPlaceKind::Result {
            let Some(result) = operation_result else {
                return false;
            };
            return substitution.target.version == substitution.source.version
                && substitution.target.root == result.place
                && substitution.target.segments == substitution.source.segments;
        }
        let StructuralPlaceKind::Parameter { position, is_self } = source_place.kind else {
            return false;
        };
        let Some(argument_index) = parameters
            .iter()
            .position(|parameter| parameter.position == position && parameter.is_self == is_self)
        else {
            return false;
        };
        let Some(argument) = structural_arguments.get(argument_index) else {
            return false;
        };
        let mut expected_segments = argument
            .path
            .iter()
            .map(|segment| match segment {
                StructuralPathSegment::Field(identity) => {
                    psi_core::ContentPlaceSegment::Field(identity.clone())
                }
                StructuralPathSegment::FixedIndex(index) => {
                    psi_core::ContentPlaceSegment::FixedIndex(*index)
                }
            })
            .collect::<Vec<_>>();
        expected_segments.extend(substitution.source.segments.iter().cloned());
        substitution.target.version == substitution.source.version
            && substitution.target.root == argument.place
            && substitution.target.segments == expected_segments
    })
}

fn validate_partition_source_places(
    composition: &ContentPartitionComposition,
) -> Result<BTreeMap<PlaceId, StructuralPlaceKind>, ModuleError> {
    let mut ids = BTreeMap::new();
    let mut roots = BTreeSet::new();
    for place in &composition.source_structural_places {
        if ids.insert(place.id, place.kind).is_some() {
            return Err(ModuleError::DuplicateContentPartitionSourcePlace(place.id));
        }
        let root = match place.kind {
            StructuralPlaceKind::Parameter { position, .. } => {
                StructuralRootKey::Parameter(position)
            }
            StructuralPlaceKind::Result => StructuralRootKey::Result,
            StructuralPlaceKind::OperationResult { .. } => {
                return Err(ModuleError::ContentPartitionSourceLocalUnsupported(
                    place.id,
                ));
            }
            StructuralPlaceKind::ByteSequenceLiteral { .. } => {
                return Err(ModuleError::ContentPartitionSourceLocalUnsupported(
                    place.id,
                ));
            }
            StructuralPlaceKind::ProviderAttachment { .. } => {
                return Err(ModuleError::ContentPartitionSourceLocalUnsupported(
                    place.id,
                ));
            }
            StructuralPlaceKind::TrivialAffineLocal { .. } => {
                return Err(ModuleError::ContentPartitionSourceLocalUnsupported(
                    place.id,
                ));
            }
        };
        if !roots.insert(root) {
            return Err(ModuleError::DuplicateContentPartitionSourceRoot(place.kind));
        }
    }
    Ok(ids)
}

fn validate_partition_substitution_shape(
    substitution: &psi_terminal::ContentPlaceSubstitution,
    source_kinds: &BTreeMap<PlaceId, StructuralPlaceKind>,
    target_kinds: &BTreeMap<PlaceId, StructuralPlaceKind>,
) -> Result<(), ModuleError> {
    match (
        substitution.source.version,
        source_kinds.get(&substitution.source.root),
        substitution.target.version,
        target_kinds.get(&substitution.target.root),
    ) {
        (
            psi_core::ContentPlaceVersion::Entry,
            Some(StructuralPlaceKind::Parameter { .. }),
            psi_core::ContentPlaceVersion::Entry,
            Some(StructuralPlaceKind::Parameter { .. }),
        )
        | (
            psi_core::ContentPlaceVersion::Current,
            Some(StructuralPlaceKind::Result),
            psi_core::ContentPlaceVersion::Current,
            Some(StructuralPlaceKind::Result),
        )
        | (
            psi_core::ContentPlaceVersion::Current,
            Some(StructuralPlaceKind::Result),
            psi_core::ContentPlaceVersion::Current,
            Some(StructuralPlaceKind::OperationResult { .. }),
        )
        | (
            psi_core::ContentPlaceVersion::Current,
            Some(StructuralPlaceKind::Parameter { .. }),
            psi_core::ContentPlaceVersion::Current,
            Some(StructuralPlaceKind::Parameter { .. }),
        ) => Ok(()),
        _ => Err(ModuleError::InvalidContentPartitionSubstitutionShape),
    }
}

fn replay_partition_conservation(
    source: &ContentConservation,
    substitutions: &BTreeMap<ContentStructuralPlace, ContentStructuralPlace>,
) -> Result<ContentConservation, ModuleError> {
    Ok(ContentConservation::new(
        source.algebra().clone(),
        replay_partition_term(source.left(), substitutions)?,
        replay_partition_term(source.right(), substitutions)?,
    ))
}

fn replay_partition_term(
    term: &ContentTerm,
    substitutions: &BTreeMap<ContentStructuralPlace, ContentStructuralPlace>,
) -> Result<ContentTerm, ModuleError> {
    match term {
        ContentTerm::Projection {
            projection,
            subject,
        } => Ok(ContentTerm::Projection {
            projection: *projection,
            subject: substitutions
                .get(subject)
                .cloned()
                .ok_or(ModuleError::ContentPartitionSubstitutionCoverageMismatch)?,
        }),
        ContentTerm::Separate(terms) => ContentTerm::separate(
            terms
                .iter()
                .map(|term| replay_partition_term(term, substitutions))
                .collect::<Result<Vec<_>, _>>()?,
        )
        .map_err(ModuleError::MalformedProposition),
    }
}

fn content_term_contains_partition(term: &ContentTerm) -> bool {
    match term {
        ContentTerm::Projection { .. } => false,
        ContentTerm::Separate(_) => true,
    }
}

fn content_conservation_subjects(
    conservation: &ContentConservation,
) -> BTreeSet<ContentStructuralPlace> {
    content_conservation_projections(conservation)
        .into_iter()
        .map(|(_, subject)| subject)
        .collect()
}

fn content_conservation_projections(
    conservation: &ContentConservation,
) -> Vec<(ContentProjectionIdentity, ContentStructuralPlace)> {
    fn collect(
        term: &ContentTerm,
        projections: &mut Vec<(ContentProjectionIdentity, ContentStructuralPlace)>,
    ) {
        match term {
            ContentTerm::Projection {
                projection,
                subject,
            } => projections.push((*projection, subject.clone())),
            ContentTerm::Separate(terms) => {
                for term in terms {
                    collect(term, projections);
                }
            }
        }
    }
    let mut projections = Vec::new();
    collect(conservation.left(), &mut projections);
    collect(conservation.right(), &mut projections);
    projections
}

fn register_partition_projections(
    registry: &mut IdRegistry,
    conservation: &ContentConservation,
) -> Result<(), ModuleError> {
    for (projection, _) in content_conservation_projections(conservation) {
        register_content_projection(registry, projection, conservation.algebra())?;
    }
    Ok(())
}

fn content_places_overlap(left: &ContentStructuralPlace, right: &ContentStructuralPlace) -> bool {
    if left.version != right.version || left.root != right.root {
        return false;
    }
    let shared = left.segments.len().min(right.segments.len());
    left.segments[..shared] == right.segments[..shared]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conservation(root: PlaceId) -> ContentConservation {
        let term = ContentTerm::Projection {
            projection: ContentProjectionIdentity {
                domain: psi_core::ContentDomainId::new(7).expect("content domain identity"),
                projection_report_fingerprint: 0xfeed,
            },
            subject: ContentStructuralPlace {
                version: psi_core::ContentPlaceVersion::Current,
                root,
                segments: Vec::new(),
            },
        };
        ContentConservation::new(
            ContentAlgebra {
                kind: psi_core::ContentAlgebraKind::IntervalSet,
                parameter: "Address".to_owned(),
            },
            term.clone(),
            term,
        )
    }

    #[test]
    fn guarantee_alpha_equality_ignores_unreferenced_callee_places() {
        let source_root = PlaceId::new(1).expect("source place");
        let target_root = PlaceId::new(2).expect("target place");
        let parameter = StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        };
        let source_places = [StructuralPlaceDeclaration {
            id: source_root,
            kind: parameter,
        }];
        let target_places = [
            StructuralPlaceDeclaration {
                id: target_root,
                kind: parameter,
            },
            StructuralPlaceDeclaration {
                id: PlaceId::new(3).expect("unrelated place"),
                kind: StructuralPlaceKind::Parameter {
                    position: 1,
                    is_self: false,
                },
            },
        ];

        assert!(content_guarantees_alpha_equal(
            &source_places,
            &conservation(source_root),
            &target_places,
            &conservation(target_root),
        ));
    }
}
