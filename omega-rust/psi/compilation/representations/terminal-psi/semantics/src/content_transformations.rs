//! Exact subjects and claim equations of a completed content transformation.
//! Payload assembly is independent of this operation. These helpers preserve
//! its operand namespace and complete projection roster; they never infer
//! authority from equal geometry or decide a numeric theorem by fingerprint.
//!
//! ## Completed content transformations
//!
//! `TransformContent` transports an already assembled owned payload and consumes
//! its disjoint owned parent operands atomically. Its equations bind exact current
//! claims, typed projection paths, qualification owners and output occurrences.
//! The numeric conservation theorem is admitted at the source/proof boundary;
//! equal geometry or a report fingerprint alone does not confer ownership.
//!
//! The boundary reconstructs the complete transaction before committing it:
//! every live parent content qualification is accounted for exactly once; active
//! nested ownership and cleanup obligations cannot disappear; loans cannot still
//! observe consumed storage; and each output names an existing payload claim.
//! Derived membership must not already exist on that payload. The result retains
//! exactly the inherited memberships plus the new derivations, without merging
//! independently owned accounts. Ordinary derivation cannot interchange
//! qualification owners or algebras. A conditional checked boundary-body
//! correspondence instead transports debt into the raw payload without adding
//! output membership. It retains the exact public law/candidate mapping and a
//! complete typed address correspondence proved at the same source completion;
//! later selected invocation still owes its independent establishment route.
//! Guarded declaration rows count only for an established active
//! case; unknown membership never proves absence.
//!
//! The retained `owns_linear_claim` declaration property preserves a carrier's
//! own nominal obligation independently of nested payload obligations. References
//! do not own their referents, and empty arrays have no element debt. A whole
//! accounted parent does not authorize discarding its active obligated children.
//!
//! Each derivation starts at the complete roster of owned roots and replaces
//! consumed claims with the exact descendants of completed producers. Reunion
//! consumes that entire live frontier. At external ingress, producer operations
//! and callee returns authenticate every edge and exact actual input roster;
//! numeric equality cannot substitute for this correspondence. Library contracts
//! supply any additional backing compatibility requirements.
//!
//! The interpreter stages checks and result construction before retiring parents.
//! Native lowering preserves parent consumption as an ownership effect while
//! copying only the raw payload into result storage. The operation is neither
//! erasable nor duplicable. Failure before completion publishes no result or
//! partial consumption. Logical fuel schedule identity 1 charges one unit for
//! the executed operation, as for other semantic operations; it adds no implicit
//! native metering.

use semantic_vocabulary::{
    ContentPlaceSegment, ContentPlaceVersion, ContentTerm, PlaceId, StructuralDomainId,
    StructuralTypeId,
};
use terminal_psi::{
    ContentClaimProjection, ContentTransformationDerivation, StructuralArgument,
    StructuralDomainDeclaration, StructuralOperationResult, StructuralPathQualification,
    StructuralPathSegment, StructuralTypeDeclaration, TerminalIdentityTable,
};

mod ancestry;
mod boundary_correspondence;

/// Resolve a content term's static path in the same carrier namespace used by
/// runtime ownership. Case spellings are local to their exact parent carrier.
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
                let carrier = crate::runtime_structural_path_tip(types.clone(), root, &path)?;
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
    crate::runtime_structural_path_tip(types, root, &path)?;
    Some(path)
}

/// Preserve an owned alias's exact typed coordinate in a content subject.
/// A case identifier belongs to its enclosing carrier, not a global spelling
/// search. Runtime indexes and reference crossings are not finite owned paths.
pub fn content_path<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    identities: &TerminalIdentityTable,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<Vec<ContentPlaceSegment>> {
    let mut segments = Vec::new();
    for (position, segment) in path.iter().enumerate() {
        segments.push(match segment {
            StructuralPathSegment::Field(identity) => {
                ContentPlaceSegment::Field(identities.text(*identity)?.to_owned())
            }
            StructuralPathSegment::FixedIndex(position) => {
                ContentPlaceSegment::FixedIndex(*position)
            }
            StructuralPathSegment::Case(identity) => {
                let carrier =
                    crate::runtime_structural_path_tip(types.clone(), root, &path[..position])?;
                let case = carrier
                    .shape
                    .sum_cases()?
                    .iter()
                    .find(|case| case.id == *identity)?;
                ContentPlaceSegment::Case(identities.text(case.identity)?.to_owned())
            }
            _ => return None,
        });
    }
    crate::runtime_structural_path_tip(types, root, path)?;
    Some(segments)
}

/// Reconstruct the complete closed term named by a claim roster. Inputs and
/// outputs bind current occurrences; the source checker has already proved
/// that each retained entry observation still equals its current input.
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

/// Typed shape of the retained transaction at an untrusted Terminal boundary.
/// This authenticates its correspondence, not the numeric proof: theorem
/// admission remains with the source/proof boundary that produced the module.
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
        &module.structural_types,
        &module.structural_domains,
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
    let graph = crate::descriptor_results::terminal_graph(module);
    let contract = crate::descriptor_results::current_selected_payload_contract(
        &module.structural_types,
        &module.structural_domains,
        &graph,
        machine.id,
        position,
        source.place,
        &source.path,
        &|place| crate::descriptor_results::terminal_payload_contract(machine, place),
    )?;
    validate_memberships(
        &module.structural_types,
        &module.structural_domains,
        &module.identities,
        result,
        derivations,
        &contract.qualifications,
        &contract.projected_qualifications,
    )?;
    // Foundation validation authenticates typed operands and equation owners.
    // A parameter's declared roster includes guarded, mutually exclusive case
    // payloads, not the current live accounts. Complete parent membership and
    // disposition are checked together at native/runtime custody boundaries,
    // where the selected cases and outstanding loans are available.
    Ok(())
}

/// Every content-bearing membership of a consumed parent participates in the
/// transaction, and each named input is an actual current membership. Ordinary
/// predicates are not content accounts; geometry alone cannot supply either.
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
            let carrier = crate::runtime_structural_path_tip(types.iter(), root_type, &path)
                .ok_or("content parent membership carrier is absent")?;
            let domain = domains
                .iter()
                .find(|domain| {
                    domain.carrier == carrier.id
                        && domain
                            .content_projection
                            .as_ref()
                            .is_some_and(|projection| {
                                projection.identity == input.projection
                                    && projection.algebra == *derived.conservation.algebra()
                            })
                })
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

/// Shared typed transaction contract for Terminal and native representations.
/// Carrier lookup stays with the representation; qualification identity and
/// conservation correspondence do not change at lowering.
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
    if crate::runtime_structural_path_tip(types.iter(), source_type, &source.path)
        .is_none_or(|carrier| carrier.id != result.structural_type)
    {
        return Err("content transformation changes payload carrier");
    }
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    let mut covered_parents = vec![false; parents.len()];
    for derived in derivations {
        let mut qualification_owner = None;
        let ordinary = matches!(
            derived.custody,
            terminal_psi::ContentTransformationCustody::Derived
        );
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
                let carrier = crate::runtime_structural_path_tip(types.iter(), root_type, &path)
                    .ok_or("content claim projection carrier is absent")?;
                let domain = domains
                    .iter()
                    .find(|domain| {
                        domain.carrier == carrier.id
                            && domain
                                .content_projection
                                .as_ref()
                                .is_some_and(|projection| {
                                    projection.identity == binding.projection
                                        && projection.algebra == *derived.conservation.algebra()
                                })
                    })
                    .ok_or("content projection differs from its qualification owner")?;
                // Equal algebras describe compatible arithmetic, not permission
                // to exchange one owner's qualification for another owner's.
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

/// A transaction inherits the raw payload's actual memberships and adds only
/// the exact output occurrences justified by its derivations. It is not the
/// predicate-establishment route used by an ordinary refinement operation.
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
        if !matches!(
            derived.custody,
            terminal_psi::ContentTransformationCustody::Derived
        ) {
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
            let carrier =
                crate::runtime_structural_path_tip(types.iter(), result.structural_type, &path)
                    .ok_or("content output membership carrier is absent")?;
            let domain = domains
                .iter()
                .find(|domain| {
                    domain.carrier == carrier.id
                        && domain
                            .content_projection
                            .as_ref()
                            .is_some_and(|projection| {
                                projection.identity == output.projection
                                    && projection.algebra == *derived.conservation.algebra()
                            })
                })
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
