//! Predicate freshness and conserved Content have independent write footprints.
//! Source checking invalidates or re-establishes predicate observations at their
//! actual use. Mutation may expire an incoming fact, but cannot silently change
//! a conserved projection. Borrowed ancestor contracts retain their own checks.
//! Unknown observations stay opaque; different descriptor fields do not prove
//! disjointness of the indirect backing they observe.
use crate::exactly_one::exactly_one;
use crate::projected_field_declaration;
use crate::structural_paths_may_overlap;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Segment, ContentProjectionExpression,
    ContentProjectionScalar, PlaceId, Proposition, ScalarReadProjection, ScalarTerm, ScalarType,
    StructuralTypeId,
};
use terminal_psi::{
    StructuralDomainDeclaration, StructuralDomainPredicate, StructuralPathQualification,
    StructuralPathSegment, StructuralTypeDeclaration, TerminalIdentityTable,
};

#[derive(Default)]
struct PredicateDependencies {
    paths: Vec<Vec<Segment>>,
    indirect_backing: bool,
}

fn term_dependencies<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    carrier: StructuralTypeId,
    subject: PlaceId,
    term: &ScalarTerm,
    output: &mut PredicateDependencies,
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
        ScalarTerm::ByteSequencePredicate { root, path, .. } => {
            if *root != subject {
                return None;
            }
            let selected = crate::canonical_structural_path_tip(types.clone(), carrier, path)?;
            if !crate::byte_predicate_carrier(types, selected.id) {
                return None;
            }
            // Fixed bytes are inline storage. A view observes indirect backing:
            // its descriptor field cannot establish disjointness from a write.
            let inline = matches!(
                selected.shape,
                terminal_psi::StructuralTypeShape::FixedArray { .. }
            );
            output.indirect_backing |= !inline;
            output
                .paths
                .push(if inline { path.clone() } else { Vec::new() });
            return Some(());
        }
        ScalarTerm::StructuralRead {
            root,
            path,
            scalar_type,
        } => {
            let path = path
                .iter()
                .map(|step| match step {
                    ScalarReadProjection::Static(step) => Some(*step),
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
        || crate::primitive_read_projection_type(
            types.clone(),
            carrier,
            &storage_path(types, carrier, &observation.1)?,
        )? != observation.2
    {
        return None;
    }
    output.paths.push(observation.1);
    Some(())
}

fn predicate_dependencies<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    carrier: StructuralTypeId,
    predicate: &StructuralDomainPredicate,
) -> Option<PredicateDependencies> {
    fn visit<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        carrier: StructuralTypeId,
        subject: PlaceId,
        clause: &Proposition,
        output: &mut PredicateDependencies,
        depth: usize,
    ) -> Option<()> {
        if depth > 256 {
            return None;
        }
        match clause {
            Proposition::Truth | Proposition::Falsehood => Some(()),
            semantic_vocabulary::integer_math_comparison_operands!(left, right) => {
                let mut pending = vec![(left, depth + 1), (right, depth + 1)];
                while let Some((term, depth)) = pending.pop() {
                    if depth > 256 {
                        return None;
                    }
                    match term {
                        semantic_vocabulary::IntegerMathTerm::Embed(scalar) => term_dependencies(
                            types.clone(),
                            carrier,
                            subject,
                            scalar,
                            output,
                            depth + 1,
                        )?,
                        semantic_vocabulary::IntegerMathTerm::MathValue { .. } => return None,
                        term => pending.extend(
                            term.operands()
                                .into_iter()
                                .map(|operand| (operand, depth + 1)),
                        ),
                    }
                }
                Some(())
            }
            Proposition::Equal(left, right)
            | Proposition::LessThan(left, right)
            | Proposition::LessOrEqual(left, right)
            | Proposition::ScalarIeeeFloatComparison { left, right, .. } => {
                term_dependencies(types.clone(), carrier, subject, left, output, 0)?;
                term_dependencies(types, carrier, subject, right, output, 0)
            }
            Proposition::IeeeFloatComparison {
                format,
                left,
                right,
                ..
            } => {
                // This older field-only spelling has the same dependencies;
                // neither traversal grants truth or changes IEEE comparison.
                for field in [left, right] {
                    term_dependencies(
                        types.clone(),
                        carrier,
                        subject,
                        &ScalarTerm::ieee_float_field_path(
                            field.root(),
                            field.path().to_vec(),
                            *format,
                        ),
                        output,
                        depth + 1,
                    )?;
                }
                Some(())
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
            // `self in T::Case` observes the tag of the sum at its path. The
            // case must be declared there; the whole sum value is the
            // dependency, so a write of the tag or of any payload overlaps it.
            Proposition::StructuralCaseMembership {
                subject: observed,
                case,
            } => {
                if observed.root() != subject {
                    return None;
                }
                crate::canonical_structural_path_tip(types, carrier, observed.path())?
                    .shape
                    .sum_cases()?
                    .iter()
                    .any(|declared| declared.id == *case)
                    .then_some(())?;
                output.paths.push(observed.path().to_vec());
                Some(())
            }
            _ => None,
        }
    }
    let mut output = PredicateDependencies::default();
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

/// A closed theory with no carrier observations contains no artifact-local
/// field or value coordinates. Its proposition can therefore be compared across
/// catalogs directly. This does not prove the proposition or issue membership.
pub fn domain_predicate_observes_no_storage<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    carrier: StructuralTypeId,
    predicate: &StructuralDomainPredicate,
) -> bool {
    predicate_dependencies(types, carrier, predicate)
        .is_some_and(|dependencies| dependencies.paths.is_empty())
}

fn storage_path<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    carrier: StructuralTypeId,
    path: &[Segment],
) -> Option<Vec<StructuralPathSegment>> {
    if path.contains(&Segment::Referent) {
        // A descriptor and its current backing are different storage subjects.
        // This inline domain framing query has no captured backing/alias cut.
        return None;
    }
    if let Some((Segment::Field(field), prefix)) = path.split_last() {
        // A payload field `self.lo` under `self in T::Range` is observed as
        // `[Case(Range), Field(lo)]`. The storage walk cannot end inside a
        // case namespace, so resolve the sum's own path and keep the Case
        // step; the field lookup below checks the case is declared there.
        let (sum, case) = match prefix.split_last() {
            Some((Segment::Case(case), sum)) => (sum, Some(*case)),
            _ => (prefix, None),
        };
        let mut storage = crate::canonical_structural_storage_path(types.clone(), carrier, sum)?;
        storage.extend(case.map(StructuralPathSegment::Case));
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
                let declaration =
                    exactly_one(types.clone(), |declaration| declaration.id == current)?;
                let field = exactly_one(declaration.shape.direct_fields()?.iter(), |field| {
                    identities.text(field.identity) == Some(identity.as_str())
                        && !field.relevance.is_erased()
                })?;
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
    let Some(mut dependencies) = predicate_dependencies(types.clone(), domain.carrier, predicate)
    else {
        return false;
    };
    content_dependencies(types.clone(), identities, domain, &mut dependencies.paths).is_some()
        && dependencies.paths.iter().all(|path| {
            storage_path(types.clone(), domain.carrier, path)
                .is_some_and(|dependency| !structural_paths_may_overlap(&dependency, write))
        })
}

/// Inline separation can frame a whole domain only when its complete rule
/// observes inline storage. A borrowed-byte observation retains an indirect
/// dependency even when its descriptor sits inside an ordinary record. Unused
/// borrowed fields do not add dependencies to a rule that does not observe them.
/// This query supplies no membership, physical separation, or backing proof.
pub(crate) fn domain_has_inline_storage_dependencies<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    identities: &TerminalIdentityTable,
    domain: &StructuralDomainDeclaration,
) -> bool {
    if !domain.complete_predicate_theory {
        return false;
    }
    let Some(predicate) = domain.predicate.as_ref() else {
        return false;
    };
    let Some(mut dependencies) = predicate_dependencies(types.clone(), domain.carrier, predicate)
    else {
        return false;
    };
    if dependencies.indirect_backing {
        return false;
    }
    if domain.byte_predicate.is_some()
        && !crate::descriptors::descriptor_results::plain_owned_shape(types.clone(), domain.carrier)
    {
        return false;
    }
    content_dependencies(types.clone(), identities, domain, &mut dependencies.paths).is_some()
        && dependencies
            .paths
            .iter()
            .all(|path| storage_path(types.clone(), domain.carrier, path).is_some())
}

/// Predicate observations may expire on a write; conserved Content does not.
/// Its exact projection remains an independent mutation constraint.
fn content_dependencies<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    identities: &TerminalIdentityTable,
    domain: &StructuralDomainDeclaration,
    paths: &mut Vec<Vec<Segment>>,
) -> Option<()> {
    if let Some(projection) = &domain.content_projection {
        let valid = match &projection.expression {
            ContentProjectionExpression::CountedQuantity(scalar) => {
                content_paths(types.clone(), identities, domain.carrier, scalar, paths, 0)
            }
            ContentProjectionExpression::IntervalSet(intervals) => {
                intervals.iter().try_for_each(|(start, end)| {
                    content_paths(types.clone(), identities, domain.carrier, start, paths, 0)?;
                    content_paths(types.clone(), identities, domain.carrier, end, paths, 0)
                })
            }
        };
        valid?;
    }
    Some(())
}

fn domain_content_write_is_disjoint<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    identities: &TerminalIdentityTable,
    domain: &StructuralDomainDeclaration,
    write: &[StructuralPathSegment],
) -> bool {
    if domain.content_projection.is_none() {
        return true;
    }
    if write.iter().any(|step| {
        matches!(
            step,
            StructuralPathSegment::Referent | StructuralPathSegment::RepresentationView { .. }
        )
    }) {
        return false;
    }
    let mut dependencies = Vec::new();
    content_dependencies(types.clone(), identities, domain, &mut dependencies).is_some()
        && dependencies.iter().all(|dependency| {
            storage_path(types.clone(), domain.carrier, dependency)
                .is_some_and(|dependency| !structural_paths_may_overlap(&dependency, write))
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
            if !structural_paths_may_overlap(&row.path, write) {
                return true;
            }
            write
                .strip_prefix(row.path.as_slice())
                .is_some_and(|relative| preserved(row.domain, relative))
        })
}

/// Ordinary predicate memberships are incoming facts, not perpetual storage
/// contracts. Checking owns their invalidation and consumption judgments.
/// Mutation still preserves every carried conserved projection independently.
pub fn store_preserves_domain_content<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'a StructuralDomainDeclaration> + Clone,
    identities: &TerminalIdentityTable,
    whole: &[semantic_vocabulary::StructuralDomainId],
    projected: &[StructuralPathQualification],
    write: &[StructuralPathSegment],
) -> bool {
    let preserved = |identity, path: &[StructuralPathSegment]| {
        let mut matching = domains.clone().filter(|domain| domain.id == identity);
        matching.next().is_some_and(|domain| {
            matching.next().is_none()
                && domain_content_write_is_disjoint(types.clone(), identities, domain, path)
        })
    };
    whole.iter().all(|domain| preserved(*domain, write))
        && projected.iter().all(|row| {
            !structural_paths_may_overlap(&row.path, write)
                || write
                    .strip_prefix(row.path.as_slice())
                    .is_some_and(|relative| preserved(row.domain, relative))
        })
}

/// Derive the complete field write footprint from the original declaration.
/// Predicate freshness belongs to the source checker, not an immutable roster
/// on a former producer. Runtime indices retain conservative Content overlap.
pub fn scalar_field_store_preserves_domain_content<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'a StructuralDomainDeclaration> + Clone,
    identities: &TerminalIdentityTable,
    root: StructuralTypeId,
    whole: &[semantic_vocabulary::StructuralDomainId],
    projected: &[StructuralPathQualification],
    path: &[StructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
) -> bool {
    let Some(declaration) = projected_field_declaration(types.clone(), root, path, field) else {
        return false;
    };
    let mut write = path.to_vec();
    write.push(StructuralPathSegment::Field(declaration.identity));
    store_preserves_domain_content(types, domains, identities, whole, projected, &write)
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
        .filter(|row| row.path.len() < path.len() && structural_paths_may_overlap(&row.path, path))
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
    let Some(declaration) = projected_field_declaration(types.clone(), root, path, field) else {
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
            let Some(input) = crate::content_transformations::structural_path(
                types.clone(),
                identities,
                root,
                &claim.input.segments,
            ) else {
                return false;
            };
            if !structural_paths_may_overlap(&input, write) {
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
                        && domain_content_write_is_disjoint(
                            types.clone(),
                            identities,
                            owner,
                            relative,
                        )
                })
            })
        })
}

#[cfg(test)]
mod tests;
