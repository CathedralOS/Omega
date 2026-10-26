//! Receive a conditional checked-provider body relation without issuing D.
//!
//! The source/proof boundary proves numeric and address equations. This owner
//! authenticates their complete typed correspondence, exact public law and
//! checked candidate mapping. Selected installation and invocation authority
//! are separate; neither a matching count nor this record introduces a root.

use semantic_vocabulary::{
    ContentPlaceSegment, ContentPlaceVersion, ContentTerm, PlaceId, StructuralTypeId,
};
use terminal_psi::{
    ContentClaimAddress, ContentTransformationCustody, ContentTransformationDerivation,
    StructuralFieldDeclaration, StructuralFieldType, StructuralOperationResult,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalIdentityTable, TerminalMachine,
    TerminalModule,
};

pub(super) fn validate_relation(
    derived: &ContentTransformationDerivation,
) -> Result<(), &'static str> {
    let ContentTransformationCustody::Boundary {
        requirements,
        addresses,
    } = &derived.custody
    else {
        return Ok(());
    };
    if requirements.is_empty()
        || addresses.is_empty()
        || requirements.windows(2).any(|pair| pair[0] >= pair[1])
        || addresses.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(
            "boundary body custody requires canonical complete public laws and backing paths",
        );
    }
    for requirement in requirements {
        if requirement.substitutions.is_empty()
            || requirement
                .substitutions
                .windows(2)
                .any(|pair| pair[0].source >= pair[1].source)
        {
            return Err("boundary body public-law substitutions are not exact and canonical");
        }
        let mut used = vec![false; requirement.substitutions.len()];
        fn substitute(
            term: &ContentTerm,
            substitutions: &[terminal_psi::ContentBodyProjectionSubstitution],
            used: &mut [bool],
        ) -> Result<ContentTerm, &'static str> {
            match term {
                ContentTerm::Projection { projection, .. } => {
                    let position = substitutions
                        .iter()
                        .position(|substitution| &substitution.source == term)
                        .ok_or("boundary body public projection has no exact substitution")?;
                    fn same_owner(
                        term: &ContentTerm,
                        expected: semantic_vocabulary::ContentProjectionIdentity,
                    ) -> bool {
                        match term {
                            ContentTerm::Projection {
                                projection,
                                subject,
                            } => {
                                *projection == expected
                                    && subject.version == ContentPlaceVersion::Current
                            }
                            ContentTerm::Separate(children) => {
                                !children.is_empty()
                                    && children.iter().all(|child| same_owner(child, expected))
                            }
                        }
                    }
                    if !same_owner(&substitutions[position].target, *projection) {
                        return Err(
                            "boundary body substitution changes a projection owner or observation version",
                        );
                    }
                    used[position] = true;
                    Ok(substitutions[position].target.clone())
                }
                ContentTerm::Separate(children) => ContentTerm::separate(
                    children
                        .iter()
                        .map(|child| substitute(child, substitutions, used))
                        .collect::<Result<Vec<_>, _>>()?,
                )
                .map_err(|_| "boundary body substitution repeats a public projection"),
            }
        }
        let equation = semantic_vocabulary::ContentConservation::new(
            requirement.guarantee.conservation.algebra().clone(),
            substitute(
                requirement.guarantee.conservation.left(),
                &requirement.substitutions,
                &mut used,
            )?,
            substitute(
                requirement.guarantee.conservation.right(),
                &requirement.substitutions,
                &mut used,
            )?,
        );
        if used.contains(&false) || equation != derived.conservation {
            return Err("boundary body custody differs from the complete substituted public law");
        }
    }
    Ok(())
}

pub(super) fn validate_authority(
    module: &TerminalModule,
    machine: &TerminalMachine,
    derived: &ContentTransformationDerivation,
) -> Result<(), &'static str> {
    let ContentTransformationCustody::Boundary { requirements, .. } = &derived.custody else {
        return Ok(());
    };
    for requirement in requirements {
        let boundary = module
            .boundary_machines
            .iter()
            .find(|boundary| boundary.id == requirement.boundary)
            .ok_or("boundary body custody requirement is absent")?;
        if !boundary.content_guarantees.iter().any(|guarantee| matches!(guarantee,
            terminal_psi::BoundaryContentGuarantee::Conservation(guarantee) if guarantee == &requirement.guarantee)) || !module
            .provider_candidates
            .iter()
            .any(|candidate| candidate.boundary == boundary.id && candidate.candidate == machine.id)
        {
            return Err(
                "boundary body custody lacks the exact declared law and checked candidate correspondence",
            );
        }
        for substitution in &requirement.substitutions {
            let ContentTerm::Projection {
                projection,
                subject,
            } = &substitution.source
            else {
                return Err("boundary body substitution source is not one public projection");
            };
            let root = requirement
                .guarantee
                .structural_places
                .iter()
                .find(|place| place.id == subject.root)
                .ok_or("boundary body public subject has no declaration")?;
            let roster = match root.kind {
                semantic_vocabulary::StructuralPlaceKind::Parameter { position }
                    if subject.version == ContentPlaceVersion::Entry =>
                {
                    let boundary_index = boundary
                        .structural_parameters
                        .iter()
                        .position(|parameter| parameter.position == position)
                        .ok_or("boundary body public subject is not a structural formal")?;
                    let anchored = module
                        .provider_candidates
                        .iter()
                        .filter(|candidate| {
                            candidate.boundary == boundary.id && candidate.candidate == machine.id
                        })
                        .any(|candidate| {
                            candidate
                                .refinement
                                .positional_parameters
                                .iter()
                                .filter(|row| row.boundary_index as usize == boundary_index)
                                .any(|row| {
                                    machine
                                        .structural_parameters
                                        .get(row.candidate_index as usize)
                                        .is_some_and(|parameter| {
                                            machine.content_entry_claims.iter().any(|entry| {
                                                derived.lineage_roots.contains(&entry.claim)
                                                    && entry.input.root == parameter.place
                                                    && entry.input.segments == subject.segments
                                                    && entry.projections.iter().any(|entry| {
                                                        entry.projection == *projection
                                                    })
                                            })
                                        })
                                })
                        });
                    if !anchored {
                        return Err(
                            "boundary body input law is not anchored to its exact public formal and entry claim",
                        );
                    }
                    &derived.inputs
                }
                semantic_vocabulary::StructuralPlaceKind::Result
                    if subject.version == ContentPlaceVersion::Current =>
                {
                    &derived.outputs
                }
                _ => {
                    return Err("boundary body law does not bind an entry input or current result");
                }
            };
            fn in_roster(
                term: &ContentTerm,
                roster: &[terminal_psi::ContentClaimProjection],
            ) -> bool {
                match term {
                    ContentTerm::Projection {
                        projection,
                        subject,
                    } => roster.iter().any(|binding| {
                        binding.projection == *projection && binding.subject == *subject
                    }),
                    ContentTerm::Separate(children) => {
                        children.iter().all(|child| in_roster(child, roster))
                    }
                }
            }
            if !in_roster(&substitution.target, roster) {
                return Err(
                    "boundary body public subject is substituted into another custody occurrence",
                );
            }
        }
        for output in &derived.outputs {
            let domain = module
                .structural_domains
                .iter()
                .find(|domain| {
                    domain
                        .content_projection
                        .as_ref()
                        .is_some_and(|projection| projection.identity == output.projection)
                })
                .ok_or("boundary body output has no exact domain owner")?;
            if !domain.authorizes_boundary_requirement(
                boundary.identity,
                boundary
                    .declaration
                    .as_ref()
                    .map(|checked| checked.source.semantic_declaration),
            ) {
                return Err(
                    "boundary body public requirement is not authorized for this output domain",
                );
            }
        }
    }
    Ok(())
}

pub(super) fn validate_backing(
    types: &[StructuralTypeDeclaration],
    identities: &TerminalIdentityTable,
    carrier_of: &impl Fn(PlaceId) -> Option<StructuralTypeId>,
    result: &StructuralOperationResult,
    derived: &ContentTransformationDerivation,
) -> Result<(), &'static str> {
    let ContentTransformationCustody::Boundary { addresses, .. } = &derived.custody else {
        return Ok(());
    };
    let mut expected = [Vec::new(), Vec::new()];
    for (position, bindings) in [&derived.inputs, &derived.outputs].into_iter().enumerate() {
        for binding in bindings {
            let root = if position == 0 {
                carrier_of(binding.subject.root)
                    .ok_or("boundary backing input carrier is absent")?
            } else if binding.subject.root == result.place {
                result.structural_type
            } else {
                return Err("boundary backing output names another occurrence");
            };
            let path =
                super::structural_path(types.iter(), identities, root, &binding.subject.segments)
                    .ok_or("boundary backing claim has no exact typed owned path")?;
            let carrier = crate::runtime_structural_path_tip(types.iter(), root, &path)
                .ok_or("boundary backing claim carrier is absent")?;
            for relative in owned_addresses(types, identities, carrier.id, addresses.len())? {
                let mut subject = binding.subject.clone();
                subject.segments.extend(relative);
                expected[position].push(ContentClaimAddress {
                    claim: binding.claim,
                    subject,
                });
            }
        }
        expected[position].sort();
        expected[position].dedup();
    }
    let mut actual = [Vec::new(), Vec::new()];
    for address in addresses {
        actual[0].push(address.input.clone());
        actual[1].push(address.output.clone());
    }
    for position in 0..2 {
        actual[position].sort();
        if actual[position].is_empty()
            || actual[position].windows(2).any(|pair| pair[0] == pair[1])
            || actual[position] != expected[position]
        {
            return Err(
                "boundary custody backing relation is not a bijection of the complete owned address rosters",
            );
        }
    }
    Ok(())
}

fn owned_addresses(
    types: &[StructuralTypeDeclaration],
    identities: &TerminalIdentityTable,
    root: StructuralTypeId,
    declared_addresses: usize,
) -> Result<Vec<Vec<ContentPlaceSegment>>, &'static str> {
    fn fields(
        types: &[StructuralTypeDeclaration],
        identities: &TerminalIdentityTable,
        declarations: &[StructuralFieldDeclaration],
        path: &mut Vec<ContentPlaceSegment>,
        ancestors: &mut Vec<StructuralTypeId>,
        output: &mut Vec<Vec<ContentPlaceSegment>>,
        declared_addresses: usize,
    ) -> Result<(), &'static str> {
        for field in declarations
            .iter()
            .filter(|field| !field.relevance.is_erased())
        {
            path.push(ContentPlaceSegment::Field(
                identities
                    .text(field.identity)
                    .ok_or("backing field has no declared identity")?
                    .to_owned(),
            ));
            match &field.field_type {
                StructuralFieldType::Structural(child) => visit(
                    types,
                    identities,
                    *child,
                    path,
                    ancestors,
                    output,
                    declared_addresses,
                )?,
                StructuralFieldType::ByteSequence(_)
                | StructuralFieldType::PredicateByteSequence { .. } => {
                    return Err("boundary backing contains a borrowed byte descriptor");
                }
                StructuralFieldType::Atomic(_) => {
                    return Err("boundary backing contains an atomic cell");
                }
                field => {
                    if matches!(field.scalar_type(), Some(semantic_vocabulary::ScalarType::Integer(integer)) if integer.is_address())
                    {
                        output.push(path.clone());
                    }
                }
            }
            path.pop();
            if output.len() > declared_addresses {
                return Err("boundary backing omits declared address occurrences");
            }
        }
        Ok(())
    }
    fn visit(
        types: &[StructuralTypeDeclaration],
        identities: &TerminalIdentityTable,
        root: StructuralTypeId,
        path: &mut Vec<ContentPlaceSegment>,
        ancestors: &mut Vec<StructuralTypeId>,
        output: &mut Vec<Vec<ContentPlaceSegment>>,
        declared_addresses: usize,
    ) -> Result<(), &'static str> {
        if ancestors.contains(&root) {
            return Err("boundary backing has recursive owned storage");
        }
        let carrier = types
            .iter()
            .find(|carrier| carrier.id == root)
            .ok_or("boundary backing storage declaration is absent")?;
        ancestors.push(root);
        match &carrier.shape {
            StructuralTypeShape::PrimitiveScalar(scalar) => {
                if matches!(scalar, semantic_vocabulary::ScalarType::Integer(integer) if integer.is_address())
                {
                    output.push(path.clone());
                }
            }
            StructuralTypeShape::FixedArray { element, length } => {
                // Inspect one element before expanding the finite occurrence
                // roster. Address-free arrays cost no per-element work, and
                // a short encoded witness cannot force a huge expansion.
                let mut element_paths = Vec::new();
                visit(
                    types,
                    identities,
                    *element,
                    &mut Vec::new(),
                    ancestors,
                    &mut element_paths,
                    declared_addresses,
                )?;
                if !element_paths.is_empty() {
                    let count = usize::try_from(*length)
                        .ok()
                        .and_then(|length| length.checked_mul(element_paths.len()))
                        .and_then(|count| output.len().checked_add(count))
                        .ok_or("boundary backing address count overflows")?;
                    if count > declared_addresses {
                        return Err("boundary backing omits array address occurrences");
                    }
                    for index in 0..*length {
                        for relative in &element_paths {
                            let mut indexed = path.clone();
                            indexed.push(ContentPlaceSegment::FixedIndex(index));
                            indexed.extend_from_slice(relative);
                            output.push(indexed);
                        }
                    }
                }
            }
            StructuralTypeShape::Record {
                fields: declarations,
                ..
            } => fields(
                types,
                identities,
                declarations,
                path,
                ancestors,
                output,
                declared_addresses,
            )?,
            StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
                if let StructuralTypeShape::Mixed {
                    fields: declarations,
                    ..
                } = &carrier.shape
                {
                    fields(
                        types,
                        identities,
                        declarations,
                        path,
                        ancestors,
                        output,
                        declared_addresses,
                    )?;
                }
                for case in cases {
                    path.push(ContentPlaceSegment::Case(
                        identities
                            .text(case.identity)
                            .ok_or("backing case has no declared identity")?
                            .to_owned(),
                    ));
                    fields(
                        types,
                        identities,
                        &case.fields,
                        path,
                        ancestors,
                        output,
                        declared_addresses,
                    )?;
                    path.pop();
                }
            }
            StructuralTypeShape::Reference { .. }
            | StructuralTypeShape::ByteSequence(_)
            | StructuralTypeShape::ElementView { .. }
            | StructuralTypeShape::BindingCarrier => {
                return Err("boundary backing is not a closed owned storage shape");
            }
        }
        ancestors.pop();
        if output.len() > declared_addresses {
            return Err("boundary backing omits declared address occurrences");
        }
        Ok(())
    }
    let mut output = Vec::new();
    visit(
        types,
        identities,
        root,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut output,
        declared_addresses,
    )?;
    Ok(output)
}
