/*
Shared operation semantics for content transformations, domain-membership preservation across
mutations and transfers (`domain_preservation`), and quotient representation opacity (`quotients`).
*/

use crate::exactly_one::exactly_one;
use crate::runtime_structural_path_tip;
use semantic_vocabulary::{
    ContentPlaceSegment, ContentPlaceVersion, ContentProjectionIdentity, ContentTerm, PlaceId,
    StructuralDomainId, StructuralTypeId,
};
use terminal_psi::ContentTransformationCustody;
use terminal_psi::{
    ContentClaimProjection, ContentTransformationDerivation, StructuralArgument,
    StructuralDomainDeclaration, StructuralOperationResult, StructuralPathQualification,
    StructuralPathSegment, StructuralTypeDeclaration, TerminalIdentityTable,
};

pub(crate) mod ancestry;
pub(crate) mod boundary_correspondence;
pub mod entry_claims;

pub fn structural_path<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    identities: &TerminalIdentityTable,
    root: StructuralTypeId,
    segments: &[ContentPlaceSegment],
) -> Option<Vec<StructuralPathSegment>> {
    let mut path = Vec::new();
    for segment in segments {
        path.push(match segment {
            ContentPlaceSegment::Field(identity) => {
                StructuralPathSegment::Field(identities.index_of(identity)?)
            }
            ContentPlaceSegment::FixedIndex(position) => {
                StructuralPathSegment::FixedIndex(*position)
            }
            ContentPlaceSegment::Case(identity) => {
                let carrier = runtime_structural_path_tip(types.clone(), root, &path)?;
                let identity = identities.index_of(identity)?;
                StructuralPathSegment::Case(
                    carrier
                        .shape
                        .sum_cases()?
                        .iter()
                        .find(|case| case.identity == identity)?
                        .id,
                )
            }
        });
    }
    runtime_structural_path_tip(types, root, &path)?;
    Some(path)
}

/// Name the static fields, cases and indices of a structural claim. Case IDs
/// are local to the sum reached at their position in the path.
pub fn content_path<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    identities: &TerminalIdentityTable,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<Vec<ContentPlaceSegment>> {
    runtime_structural_path_tip(types.clone(), root, path)?;
    let name = |identity| {
        identities
            .text(identity)
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
    };
    path.iter()
        .enumerate()
        .map(|(index, step)| {
            Some(match step {
                StructuralPathSegment::Field(field) => ContentPlaceSegment::Field(name(*field)?),
                StructuralPathSegment::FixedIndex(index) => ContentPlaceSegment::FixedIndex(*index),
                StructuralPathSegment::Case(id) => {
                    let parent =
                        crate::runtime_structural_path_tip(types.clone(), root, &path[..index])?;
                    let case =
                        exactly_one(parent.shape.sum_cases()?.iter(), |case| case.id == *id)?;
                    ContentPlaceSegment::Case(name(case.identity)?)
                }
                _ => return None,
            })
        })
        .collect()
}

pub fn claim_term(bindings: &[ContentClaimProjection]) -> Result<ContentTerm, &'static str> {
    if bindings.is_empty()
        || bindings
            .iter()
            .any(|binding| binding.subject.version != ContentPlaceVersion::Current)
    {
        return Err("content transformation requires current claim projections");
    }
    let mut terms = bindings
        .iter()
        .map(|binding| ContentTerm::Projection {
            projection: binding.projection,
            subject: binding.subject.clone(),
        })
        .collect::<Vec<_>>();
    if terms.len() == 1 {
        Ok(terms.remove(0))
    } else {
        ContentTerm::separate(terms).map_err(|_| "content transformation repeats a projection")
    }
}

pub fn place_type(
    machine: &terminal_psi::TerminalMachine,
    place: semantic_vocabulary::PlaceId,
) -> Option<StructuralTypeId> {
    machine
        .structural_parameter(place)
        .map(|parameter| parameter.structural_type)
        .or_else(|| {
            machine
                .producing_operation(place)
                .map(|(_, carrier)| carrier)
        })
}

pub fn validate_shape(
    module: &terminal_psi::TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<(), &'static str> {
    use terminal_psi::OperationKind;
    let OperationKind::TransformContent {
        source,
        parents,
        derivations,
    } = &operation.kind
    else {
        return Err("content transformation dispatch differs");
    };
    let result = operation
        .result
        .as_structural()
        .ok_or("content transformation requires a structural result")?;
    validate_contract(
        &module.types.structural_types,
        &module.types.structural_domains,
        &module.identities,
        &|place| place_type(machine, place),
        source,
        parents,
        derivations,
        result,
    )?;
    ancestry::validate_producers(module, machine, operation, derivations)?;
    for derived in derivations {
        boundary_correspondence::validate_authority(module, machine, derived)?;
    }
    let position = machine
        .blocks
        .iter()
        .find_map(|block| {
            block
                .operations
                .iter()
                .position(|candidate| candidate.id == operation.id)
                .and_then(|ordinal| {
                    u32::try_from(ordinal)
                        .ok()
                        .map(|ordinal| (block.id, ordinal))
                })
        })
        .ok_or("content transformation position is absent")?;
    let graph = crate::descriptors::descriptor_results::terminal_graph(module);
    let contract = crate::descriptors::descriptor_results::current_selected_payload_contract(
        &module.types.structural_types,
        &module.types.structural_domains,
        &graph,
        machine.id,
        position,
        source.place,
        &source.path,
        &|place| crate::descriptors::descriptor_results::terminal_payload_contract(machine, place),
    )?;
    validate_memberships(
        &module.types.structural_types,
        &module.types.structural_domains,
        &module.identities,
        result,
        derivations,
        &contract.qualifications,
        &contract.projected_qualifications,
    )?;
    Ok(())
}

fn projection_owner<'domains>(
    domains: &'domains [StructuralDomainDeclaration],
    carrier: StructuralTypeId,
    projection: &ContentProjectionIdentity,
    derived: &ContentTransformationDerivation,
) -> Option<&'domains StructuralDomainDeclaration> {
    let algebra = derived.conservation.algebra();
    domains.iter().find(|domain| {
        let Some(declared) = &domain.content_projection else {
            return false;
        };
        domain.carrier == carrier
            && declared.identity == *projection
            && declared.algebra == *algebra
    })
}

pub fn validate_parent_memberships(
    types: &[StructuralTypeDeclaration],
    domains: &[StructuralDomainDeclaration],
    identities: &TerminalIdentityTable,
    root_type: StructuralTypeId,
    parent: &StructuralArgument,
    derivations: &[ContentTransformationDerivation],
    whole: &[StructuralDomainId],
    projected: &[StructuralPathQualification],
    inactive: &impl Fn(&[StructuralPathSegment]) -> bool,
) -> Result<(), &'static str> {
    let mut actual = Vec::new();
    for (path, identity) in whole.iter().map(|domain| (&[][..], domain)).chain(
        projected
            .iter()
            .map(|row| (row.path.as_slice(), &row.domain)),
    ) {
        let domain = domains
            .iter()
            .find(|domain| domain.id == *identity)
            .ok_or("content parent membership owner is absent")?;
        if domain.content_projection.is_some() && !inactive(path) {
            actual.push(StructuralPathQualification {
                path: path.to_vec(),
                domain: *identity,
            });
        }
    }
    let mut expected = Vec::new();
    for derived in derivations {
        for input in &derived.inputs {
            if input.subject.root != parent.place {
                continue;
            }
            let path =
                structural_path(types.iter(), identities, root_type, &input.subject.segments)
                    .ok_or("content parent membership has no typed path")?;
            let Some(relative) = path.strip_prefix(parent.path.as_slice()) else {
                continue;
            };
            let carrier = runtime_structural_path_tip(types.iter(), root_type, &path)
                .ok_or("content parent membership carrier is absent")?;
            let domain = projection_owner(domains, carrier.id, &input.projection, derived)
                .ok_or("content parent membership has no exact qualification owner")?;
            expected.push(StructuralPathQualification {
                path: relative.to_vec(),
                domain: domain.id,
            });
        }
    }
    actual.sort();
    actual.dedup();
    expected.sort();
    expected.dedup();
    if actual != expected {
        return Err("content inputs differ from the parent's complete current membership roster");
    }
    Ok(())
}

pub fn validate_contract(
    types: &[StructuralTypeDeclaration],
    domains: &[StructuralDomainDeclaration],
    identities: &TerminalIdentityTable,
    carrier_of: &impl Fn(PlaceId) -> Option<StructuralTypeId>,
    source: &StructuralArgument,
    parents: &[StructuralArgument],
    derivations: &[ContentTransformationDerivation],
    result: &StructuralOperationResult,
) -> Result<(), &'static str> {
    use terminal_psi::StructuralAccess;
    if parents.is_empty()
        || derivations.is_empty()
        || !result.qualification_establishments.is_empty()
    {
        return Err("content transformation requires parents and equations, not an issuance route");
    }
    if derivations.windows(2).any(|pair| pair[0] >= pair[1])
        || derivations.iter().any(|derived| {
            derived.inputs.windows(2).any(|pair| pair[0] >= pair[1])
                || derived.outputs.windows(2).any(|pair| pair[0] >= pair[1])
        })
    {
        return Err("content transformation rosters are not strictly canonical");
    }
    let arguments = std::iter::once(source).chain(parents).collect::<Vec<_>>();
    for (position, argument) in arguments.iter().enumerate() {
        if argument.access != StructuralAccess::Owned
            || argument.place == result.place
            || arguments[..position].iter().any(|previous| {
                previous.place == argument.place
                    && crate::structural_paths_may_overlap(&previous.path, &argument.path)
            })
        {
            return Err("content transformation requires disjoint owned operands");
        }
    }
    let source_type = carrier_of(source.place).ok_or("content payload has no carrier")?;
    if runtime_structural_path_tip(types.iter(), source_type, &source.path)
        .is_none_or(|carrier| carrier.id != result.structural_type)
    {
        return Err("content transformation changes payload carrier");
    }
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    let mut covered_parents = vec![false; parents.len()];
    for derived in derivations {
        let mut qualification_owner = None;
        let ordinary = matches!(derived.custody, ContentTransformationCustody::Derived);
        boundary_correspondence::validate_relation(derived)?;
        boundary_correspondence::validate_backing(types, identities, carrier_of, result, derived)?;
        let equation = semantic_vocabulary::ContentConservation::new(
            derived.conservation.algebra().clone(),
            claim_term(&derived.inputs)?,
            claim_term(&derived.outputs)?,
        );
        ancestry::validate_frontier(derived)?;
        if equation != derived.conservation {
            return Err(
                "content equation or authenticated lineage differs from its claim projections",
            );
        }
        for (is_input, bindings) in [(true, &derived.inputs), (false, &derived.outputs)] {
            for binding in bindings {
                let roster = if is_input { &mut inputs } else { &mut outputs };
                if roster.contains(&(binding.claim, binding.projection)) {
                    return Err("content transformation repeats a claim projection");
                }
                roster.push((binding.claim, binding.projection));
                let root_type = if is_input {
                    carrier_of(binding.subject.root).ok_or("content parent has no carrier")?
                } else if binding.subject.root == result.place {
                    result.structural_type
                } else {
                    return Err("content output names another occurrence");
                };
                let path = structural_path(
                    types.iter(),
                    identities,
                    root_type,
                    &binding.subject.segments,
                )
                .ok_or("content claim projection has no exact typed path")?;
                let carrier = runtime_structural_path_tip(types.iter(), root_type, &path)
                    .ok_or("content claim projection carrier is absent")?;
                let domain = projection_owner(domains, carrier.id, &binding.projection, derived)
                    .ok_or("content projection differs from its qualification owner")?;
                let owner = (domain.id, domain.semantic_domain, binding.projection);
                if ordinary && qualification_owner.is_some_and(|previous| previous != owner) {
                    return Err("content equation combines different qualification owners");
                }
                qualification_owner = Some(owner);
                if is_input {
                    let position = parents
                        .iter()
                        .position(|parent| {
                            parent.place == binding.subject.root && path.starts_with(&parent.path)
                        })
                        .ok_or("content input is outside every consumed parent")?;
                    covered_parents[position] = true;
                } else {
                    if !result
                        .claims
                        .iter()
                        .any(|claim| claim.claim == binding.claim && claim.path == path)
                    {
                        return Err("content output differs from the existing result claim");
                    }
                    if ordinary
                        && if path.is_empty() {
                            !result.qualifications.contains(&domain.id)
                        } else {
                            !result
                                .projected_qualifications
                                .iter()
                                .any(|row| row.path == path && row.domain == domain.id)
                        }
                    {
                        return Err("content output membership is absent from the result");
                    }
                }
            }
        }
    }
    if covered_parents.contains(&false)
        || inputs
            .iter()
            .any(|(claim, _)| outputs.iter().any(|(output, _)| claim == output))
    {
        return Err("content transformation omits a parent or both consumes and returns one claim");
    }
    Ok(())
}

pub fn validate_memberships(
    types: &[StructuralTypeDeclaration],
    domains: &[StructuralDomainDeclaration],
    identities: &TerminalIdentityTable,
    result: &StructuralOperationResult,
    derivations: &[ContentTransformationDerivation],
    inherited: &[StructuralDomainId],
    inherited_projected: &[StructuralPathQualification],
) -> Result<(), &'static str> {
    let mut whole = inherited.to_vec();
    let mut projected = inherited_projected.to_vec();
    for derived in derivations {
        if !matches!(derived.custody, ContentTransformationCustody::Derived) {
            continue;
        }
        for output in &derived.outputs {
            if output.subject.root != result.place {
                return Err("content output names another occurrence");
            }
            let path = structural_path(
                types.iter(),
                identities,
                result.structural_type,
                &output.subject.segments,
            )
            .ok_or("content output membership has no exact typed path")?;
            let carrier = runtime_structural_path_tip(types.iter(), result.structural_type, &path)
                .ok_or("content output membership carrier is absent")?;
            let domain = projection_owner(domains, carrier.id, &output.projection, derived)
                .ok_or("content output membership has no qualification owner")?;
            if path.is_empty() {
                if whole.contains(&domain.id) {
                    return Err("content output already carries its derived membership");
                }
                whole.push(domain.id);
            } else {
                let membership = StructuralPathQualification {
                    path,
                    domain: domain.id,
                };
                if projected.contains(&membership) {
                    return Err("content output already carries its derived membership");
                }
                projected.push(membership);
            }
        }
    }
    whole.sort();
    projected.sort();
    if whole != result.qualifications || projected != result.projected_qualifications {
        return Err("content result memberships differ from inherited and derived occurrences");
    }
    Ok(())
}

pub mod domain_preservation;

pub mod quotients;
