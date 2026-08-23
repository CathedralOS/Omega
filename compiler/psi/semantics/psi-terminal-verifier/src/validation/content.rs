use super::*;

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
            if let Some(previous) = registry
                .content_projection_algebras
                .insert(content.projection, content.algebra.clone())
                && previous != content.algebra
            {
                return Err(ModuleError::ContentProjectionAlgebraMismatch(
                    content.projection,
                ));
            }
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
            if let Some(previous) = registry
                .content_projection_algebras
                .insert(content.projection, content.algebra.clone())
                && previous != content.algebra
            {
                return Err(ModuleError::ContentProjectionAlgebraMismatch(
                    content.projection,
                ));
            }
            context
                .validate(&proposition)
                .map_err(ModuleError::MalformedProposition)?;
        }
    }
    Ok(())
}

pub(super) fn validate_content_partition_compositions(
    machine: &TerminalMachine,
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
            content_conservation_fingerprint(&composition.source, &source_kinds);
        if reconstructed_fingerprint != Some(composition.source_fingerprint) {
            return Err(ModuleError::ContentPartitionSourceFingerprintMismatch {
                recorded: composition.source_fingerprint,
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
    }
    Ok(())
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
        if let Some(previous) = registry
            .content_projection_algebras
            .insert(projection, conservation.algebra().clone())
            && previous != *conservation.algebra()
        {
            return Err(ModuleError::ContentProjectionAlgebraMismatch(projection));
        }
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
