//! Mutation preserves a qualification only outside its complete definition.
//! Predicate observations bind the declared domain-local subject and exact
//! carrier geometry. Unknown observations stay opaque; paths through indirect
//! backing never become disjoint because two descriptors have different fields.
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Segment, ContentProjectionExpression,
    ContentProjectionScalar, PlaceId, Proposition, ScalarReadProjection, ScalarTerm, ScalarType,
    StructuralTypeId,
};
use terminal_psi::{
    StructuralDomainDeclaration, StructuralDomainPredicate, StructuralPathQualification,
    StructuralPathSegment, StructuralTypeDeclaration, TerminalIdentityTable,
};

fn term_dependencies<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    carrier: StructuralTypeId,
    subject: PlaceId,
    term: &ScalarTerm,
    output: &mut Vec<Vec<Segment>>,
    depth: usize,
) -> Option<()> {
    if depth > 256 {
        return None;
    }
    let observation = match term {
        ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } | ScalarTerm::IeeeFloatLiteral(_) => {
            return Some(());
        }
        ScalarTerm::BooleanField { root, path } => (*root, path.clone(), ScalarType::Boolean),
        ScalarTerm::IntegerField {
            root,
            path,
            scalar_type,
        } => (*root, path.clone(), ScalarType::Integer(*scalar_type)),
        ScalarTerm::IeeeFloatField { root, path, format } => {
            (*root, path.clone(), ScalarType::IeeeFloat(*format))
        }
        ScalarTerm::StructuralRead {
            root,
            path,
            scalar_type,
        } => {
            let path = path
                .iter()
                .map(|step| match step {
                    ScalarReadProjection::Static(step) => Some(step.clone()),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?;
            (*root, path, *scalar_type)
        }
        semantic_vocabulary::scalar_term_unary_operand!(operand) => {
            return term_dependencies(types, carrier, subject, operand, output, depth + 1);
        }
        semantic_vocabulary::scalar_term_binary_operands!(left, right) => {
            term_dependencies(types.clone(), carrier, subject, left, output, depth + 1)?;
            return term_dependencies(types, carrier, subject, right, output, depth + 1);
        }
        _ => return None,
    };
    if observation.0 != subject
        || crate::primitive_place_type(types, carrier, &observation.1)? != observation.2
    {
        return None;
    }
    output.push(observation.1);
    Some(())
}

fn predicate_dependencies<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    carrier: StructuralTypeId,
    predicate: &StructuralDomainPredicate,
) -> Option<Vec<Vec<Segment>>> {
    fn visit<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        carrier: StructuralTypeId,
        subject: PlaceId,
        clause: &Proposition,
        output: &mut Vec<Vec<Segment>>,
        depth: usize,
    ) -> Option<()> {
        if depth > 256 {
            return None;
        }
        match clause {
            Proposition::Truth | Proposition::Falsehood => Some(()),
            Proposition::Equal(left, right)
            | Proposition::LessThan(left, right)
            | Proposition::LessOrEqual(left, right) => {
                term_dependencies(types.clone(), carrier, subject, left, output, 0)?;
                term_dependencies(types, carrier, subject, right, output, 0)
            }
            Proposition::Conjunction(clauses) | Proposition::Disjunction(clauses) => {
                for clause in clauses {
                    visit(types.clone(), carrier, subject, clause, output, depth + 1)?;
                }
                Some(())
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                visit(types.clone(), carrier, subject, premise, output, depth + 1)?;
                visit(types, carrier, subject, conclusion, output, depth + 1)
            }
            _ => None,
        }
    }
    let mut output = Vec::new();
    predicate.proposition.validate().ok()?;
    visit(
        types,
        carrier,
        predicate.subject,
        &predicate.proposition,
        &mut output,
        0,
    )?;
    Some(output)
}

pub fn domain_predicate_is_closed<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    carrier: StructuralTypeId,
    predicate: &StructuralDomainPredicate,
) -> bool {
    predicate_dependencies(types, carrier, predicate).is_some()
}

fn storage_path<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    carrier: StructuralTypeId,
    path: &[Segment],
) -> Option<Vec<StructuralPathSegment>> {
    if let Some((Segment::Field(field), prefix)) = path.split_last() {
        let mut storage = crate::canonical_structural_storage_path(types.clone(), carrier, prefix)?;
        let field = crate::observed_field_declaration(types, carrier, prefix, *field)?;
        storage.push(StructuralPathSegment::Field(field.identity));
        Some(storage)
    } else {
        crate::canonical_structural_storage_path(types, carrier, path)
    }
}

fn content_paths<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    identities: &TerminalIdentityTable,
    carrier: StructuralTypeId,
    scalar: &ContentProjectionScalar,
    output: &mut Vec<Vec<Segment>>,
    depth: usize,
) -> Option<()> {
    if depth > 256 {
        return None;
    }
    match scalar {
        ContentProjectionScalar::Natural(value) => (!value.is_empty()
            && value.bytes().all(|byte| byte.is_ascii_digit())
            && (value == "0" || !value.starts_with('0')))
        .then_some(()),
        ContentProjectionScalar::Successor(value) => {
            content_paths(types, identities, carrier, value, output, depth + 1)
        }
        ContentProjectionScalar::Add(left, right)
        | ContentProjectionScalar::Subtract(left, right)
        | ContentProjectionScalar::Multiply(left, right) => {
            content_paths(types.clone(), identities, carrier, left, output, depth + 1)?;
            content_paths(types, identities, carrier, right, output, depth + 1)
        }
        ContentProjectionScalar::SubjectField(path)
        | ContentProjectionScalar::RuntimeScalarEmbedding(path) => {
            let mut current = carrier;
            let mut canonical = Vec::new();
            for (index, identity) in path.iter().enumerate() {
                let mut declarations = types
                    .clone()
                    .filter(|declaration| declaration.id == current);
                let declaration = declarations.next()?;
                if declarations.next().is_some() {
                    return None;
                }
                let mut matching = declaration.shape.direct_fields()?.iter().filter(|field| {
                    identities.text(field.identity) == Some(identity.as_str())
                        && !field.relevance.is_erased()
                });
                let field = matching.next()?;
                if matching.next().is_some() {
                    return None;
                }
                canonical.push(Segment::Field(field.id));
                if index + 1 < path.len() {
                    current = match field.field_type {
                        terminal_psi::StructuralFieldType::Structural(child) => child,
                        _ => return None,
                    };
                } else if field.field_type.scalar_type().is_none() {
                    return None;
                }
            }
            if canonical.is_empty() {
                return None;
            }
            output.push(canonical);
            Some(())
        }
    }
}

/// `write` is relative to this exact domain's carrier. A runtime-selected
/// element is conservatively represented by its whole array prefix by callers.
/// This proves preservation only; it establishes neither membership nor access.
pub fn domain_write_is_disjoint<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    identities: &TerminalIdentityTable,
    domain: &StructuralDomainDeclaration,
    write: &[StructuralPathSegment],
) -> bool {
    if write.iter().any(|step| {
        matches!(
            step,
            StructuralPathSegment::Referent | StructuralPathSegment::RepresentationView { .. }
        )
    }) {
        return false;
    }
    let Some(predicate) = &domain.predicate else {
        return false;
    };
    let Some(mut paths) = predicate_dependencies(types.clone(), domain.carrier, predicate) else {
        return false;
    };
    if let Some(projection) = &domain.content_projection {
        let valid = match &projection.expression {
            ContentProjectionExpression::CountedQuantity(scalar) => content_paths(
                types.clone(),
                identities,
                domain.carrier,
                scalar,
                &mut paths,
                0,
            ),
            ContentProjectionExpression::IntervalSet(intervals) => {
                intervals.iter().try_for_each(|(start, end)| {
                    content_paths(
                        types.clone(),
                        identities,
                        domain.carrier,
                        start,
                        &mut paths,
                        0,
                    )?;
                    content_paths(
                        types.clone(),
                        identities,
                        domain.carrier,
                        end,
                        &mut paths,
                        0,
                    )
                })
            }
        };
        if valid.is_none() {
            return false;
        }
    }
    paths.iter().all(|path| {
        storage_path(types.clone(), domain.carrier, path)
            .is_some_and(|dependency| !crate::structural_paths_may_overlap(&dependency, write))
    })
}

/// Preserve every carried whole/projected domain against this exact write.
/// Projections select their own domain carrier; strict ancestors cannot be
/// silently forgotten, and different variant payload names may still overlap.
pub fn store_preserves_qualifications<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'a StructuralDomainDeclaration> + Clone,
    identities: &TerminalIdentityTable,
    whole: &[semantic_vocabulary::StructuralDomainId],
    projected: &[StructuralPathQualification],
    write: &[StructuralPathSegment],
) -> bool {
    // Compare complete paths before entering a qualification's carrier.
    // Two sibling fields behind the same retained reference remain disjoint;
    // the reference prefix is not itself a write to either field. Crossings
    // left inside a domain-relative suffix still fail in domain_write_is_disjoint,
    // and representation views remain conservative in the overlap judgment.
    let preserved = |id, path: &[StructuralPathSegment]| {
        let mut matching = domains.clone().filter(|domain| domain.id == id);
        matching.next().is_some_and(|domain| {
            matching.next().is_none()
                && domain_write_is_disjoint(types.clone(), identities, domain, path)
        })
    };
    whole.iter().all(|domain| preserved(*domain, write))
        && projected.iter().all(|row| {
            if !crate::structural_paths_may_overlap(&row.path, write) {
                return true;
            }
            write
                .strip_prefix(row.path.as_slice())
                .is_some_and(|relative| preserved(row.domain, relative))
        })
}

/// Derive the complete field write footprint from the original declaration.
/// Runtime indices retain their original conservative overlap semantics.
pub fn scalar_field_store_preserves_qualifications<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'a StructuralDomainDeclaration> + Clone,
    identities: &TerminalIdentityTable,
    root: StructuralTypeId,
    whole: &[semantic_vocabulary::StructuralDomainId],
    projected: &[StructuralPathQualification],
    path: &[StructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
) -> bool {
    let Some(declaration) = crate::projected_field_declaration(types.clone(), root, path, field)
    else {
        return false;
    };
    let mut write = path.to_vec();
    write.push(StructuralPathSegment::Field(declaration.identity));
    store_preserves_qualifications(types, domains, identities, whole, projected, &write)
}

/// An exclusive subtree loan may retain its exact and descendant obligations,
/// but cannot hide an ancestor predicate that its writes could invalidate.
/// Shared observation leaves the original occurrence unchanged.
pub fn reference_preserves_ancestor_qualifications<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'a StructuralDomainDeclaration> + Clone,
    identities: &TerminalIdentityTable,
    whole: &[semantic_vocabulary::StructuralDomainId],
    projected: &[StructuralPathQualification],
    path: &[StructuralPathSegment],
    access: terminal_psi::StructuralAccess,
) -> bool {
    if access == terminal_psi::StructuralAccess::SharedBorrow || path.is_empty() {
        return true;
    }
    let ancestors = projected
        .iter()
        .filter(|row| {
            row.path.len() < path.len() && crate::structural_paths_may_overlap(&row.path, path)
        })
        .cloned()
        .collect::<Vec<_>>();
    store_preserves_qualifications(types, domains, identities, whole, &ancestors, path)
}

/// Claim projections remain obligations even if a malformed artifact omits the
/// corresponding membership annotation. Rejoin each exact owner definition.
pub fn scalar_field_store_preserves_content<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'a StructuralDomainDeclaration> + Clone,
    identities: &TerminalIdentityTable,
    root: StructuralTypeId,
    place: PlaceId,
    claims: impl Iterator<Item = &'a terminal_psi::ContentEntryClaim>,
    path: &[StructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
) -> bool {
    let Some(declaration) = crate::projected_field_declaration(types.clone(), root, path, field)
    else {
        return false;
    };
    let mut write = path.to_vec();
    write.push(StructuralPathSegment::Field(declaration.identity));
    store_preserves_content(types, domains, identities, root, place, claims, &write)
}

/// Preserve independently declared Content projections at the original owner.
pub fn store_preserves_content<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'a StructuralDomainDeclaration> + Clone,
    identities: &TerminalIdentityTable,
    root: StructuralTypeId,
    place: PlaceId,
    claims: impl Iterator<Item = &'a terminal_psi::ContentEntryClaim>,
    write: &[StructuralPathSegment],
) -> bool {
    claims
        .filter(|claim| claim.input.root == place)
        .all(|claim| {
            if claim.projections.is_empty() {
                return false;
            }
            if write.iter().any(|step| {
                matches!(
                    step,
                    StructuralPathSegment::Referent
                        | StructuralPathSegment::RepresentationView { .. }
                )
            }) {
                return false;
            }
            let mut input = Vec::new();
            for segment in &claim.input.segments {
                let step = match segment {
                    semantic_vocabulary::ContentPlaceSegment::Field(identity) => identities
                        .index_of(identity)
                        .map(StructuralPathSegment::Field),
                    semantic_vocabulary::ContentPlaceSegment::FixedIndex(index) => {
                        Some(StructuralPathSegment::FixedIndex(*index))
                    }
                    semantic_vocabulary::ContentPlaceSegment::Case(identity) => {
                        let Some(carrier) =
                            crate::runtime_structural_path_tip(types.clone(), root, &input)
                        else {
                            return false;
                        };
                        identities.index_of(identity).and_then(|identity| {
                            carrier
                                .shape
                                .sum_cases()?
                                .iter()
                                .find(|case| case.identity == identity)
                                .map(|case| StructuralPathSegment::Case(case.id))
                        })
                    }
                };
                let Some(step) = step else {
                    return false;
                };
                input.push(step);
            }
            if !crate::structural_paths_may_overlap(&input, &write) {
                return true;
            }
            let Some(relative) = write.strip_prefix(input.as_slice()) else {
                return false;
            };
            let Some(carrier) = crate::runtime_structural_path_tip(types.clone(), root, &input)
            else {
                return false;
            };
            claim.projections.iter().all(|projection| {
                let mut owners = domains.clone().filter(|domain| {
                    domain.content_projection.as_ref().is_some_and(|owner| {
                        owner.identity == projection.projection
                            && owner.algebra == projection.algebra
                    })
                });
                owners.next().is_some_and(|owner| {
                    owners.next().is_none()
                        && owner.carrier == carrier.id
                        && domain_write_is_disjoint(types.clone(), identities, owner, relative)
                })
            })
        })
}
