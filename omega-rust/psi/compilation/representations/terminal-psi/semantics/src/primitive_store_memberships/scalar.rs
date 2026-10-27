//! Whole primitive replacements bind a domain-local subject to one scalar.
//! Predicates become receiving goals; issuer identity remains a separate
//! requirement on the actual completed SSA operand, never a numeric theorem.
use semantic_vocabulary::{
    IntegerMathTerm, Proposition, ScalarRefinement, ScalarTerm, ScalarType, StructuralTypeId,
};
use terminal_psi::{
    StructuralDomainDeclaration, StructuralDomainPredicate, StructuralFieldType,
    StructuralTypeDeclaration, StructuralTypeShape,
};

/// Bind only the whole, exactly typed primitive subject. The closed refinement
/// admission rejects any unconverted structural observation or ambient value.
pub(super) fn refinement<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    domain: &StructuralDomainDeclaration,
) -> Option<ScalarRefinement> {
    if domain.content_projection.is_some() {
        return None;
    }
    let carrier = primitive_carrier(types.clone(), domain.carrier)?;
    let predicate = domain.predicate.as_ref()?;
    if !crate::domain_predicate_is_closed(types, domain.carrier, predicate) {
        return None;
    }
    bind_subject(carrier, predicate)
}

pub(super) fn primitive_carrier<'types>(
    mut types: impl Iterator<Item = &'types StructuralTypeDeclaration>,
    carrier: StructuralTypeId,
) -> Option<ScalarType> {
    let mut matching = types.by_ref().filter(|row| row.id == carrier);
    let declaration = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    match declaration.shape {
        StructuralTypeShape::PrimitiveScalar(carrier) => Some(carrier),
        _ => None,
    }
}

fn bind_subject(
    carrier: ScalarType,
    predicate: &StructuralDomainPredicate,
) -> Option<ScalarRefinement> {
    enum Pending<'a> {
        Proposition(&'a mut Proposition),
        Scalar(&'a mut ScalarTerm),
        Math(&'a mut IntegerMathTerm),
    }
    let mut proposition = predicate.proposition.clone();
    let mut pending = vec![Pending::Proposition(&mut proposition)];
    let mut remaining = 4096usize;
    while let Some(node) = pending.pop() {
        remaining = remaining.checked_sub(1)?;
        match node {
            Pending::Proposition(proposition) => match proposition {
                Proposition::Truth | Proposition::Falsehood => {}
                semantic_vocabulary::scalar_comparison_operands!(left, right) => {
                    pending.push(Pending::Scalar(right));
                    pending.push(Pending::Scalar(left));
                }
                semantic_vocabulary::integer_math_comparison_operands!(left, right) => {
                    pending.push(Pending::Math(right));
                    pending.push(Pending::Math(left));
                }
                Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
                    pending.extend(children.iter_mut().map(Pending::Proposition));
                }
                Proposition::Implication {
                    premise,
                    conclusion,
                } => {
                    pending.push(Pending::Proposition(conclusion));
                    pending.push(Pending::Proposition(premise));
                }
                _ => return None,
            },
            Pending::Math(term) => match term {
                IntegerMathTerm::Embed(term) => pending.push(Pending::Scalar(term)),
                IntegerMathTerm::MathValue { .. } => return None,
                term => pending.extend(term.operands_mut().into_iter().map(Pending::Math)),
            },
            Pending::Scalar(term) => {
                let observed = match term {
                    ScalarTerm::BooleanField { root, path } => {
                        Some((*root, path.is_empty(), ScalarType::Boolean))
                    }
                    ScalarTerm::IntegerField {
                        root,
                        path,
                        scalar_type,
                    } => Some((*root, path.is_empty(), ScalarType::Integer(*scalar_type))),
                    ScalarTerm::IeeeFloatField { root, path, format } => {
                        Some((*root, path.is_empty(), ScalarType::IeeeFloat(*format)))
                    }
                    ScalarTerm::StructuralRead {
                        root,
                        path,
                        scalar_type,
                    } => Some((*root, path.is_empty(), *scalar_type)),
                    _ => None,
                };
                if let Some((root, whole, actual)) = observed {
                    if root != predicate.subject || !whole || actual != carrier {
                        return None;
                    }
                    *term = ScalarTerm::value(ScalarRefinement::SUBJECT, carrier);
                } else {
                    pending.extend(term.operands_mut().into_iter().map(Pending::Scalar));
                }
            }
        }
    }
    ScalarRefinement::new(carrier, proposition).ok()
}

pub(super) fn receive(
    receiving: Option<StructuralFieldType>,
    refinement: ScalarRefinement,
) -> Option<Option<StructuralFieldType>> {
    let required = StructuralFieldType::RefinedScalar(refinement.clone());
    if receiving
        .as_ref()
        .is_some_and(|current| crate::scalar_field_entails(current, &required))
    {
        return Some(receiving);
    }
    if refinement.predicate() == &Proposition::Truth {
        return Some(receiving);
    }
    let mut clauses = vec![refinement.predicate().clone()];
    if let Some(current) = receiving {
        if current.scalar_type() != Some(refinement.carrier()) {
            return None;
        }
        clauses.push(crate::scalar_field_requirement(
            &current,
            ScalarRefinement::SUBJECT,
        )?);
    }
    clauses.sort();
    clauses.dedup();
    let predicate = if clauses.len() == 1 {
        clauses.pop()?
    } else {
        Proposition::Conjunction(clauses)
    };
    Some(Some(StructuralFieldType::RefinedScalar(
        ScalarRefinement::new(refinement.carrier(), predicate).ok()?,
    )))
}

/// Retain each prerequisite as a distinct same-carrier obligation. An unknown
/// or cyclic relation is not an empty receiving restriction.
pub(super) fn required_domains<'domain>(
    domains: impl Iterator<Item = &'domain StructuralDomainDeclaration> + Clone,
    root: &'domain StructuralDomainDeclaration,
) -> Option<Vec<&'domain StructuralDomainDeclaration>> {
    let mut pending = vec![(root.semantic_domain, false)];
    let mut active = Vec::new();
    let mut visited = Vec::new();
    let mut result = Vec::new();
    while let Some((semantic_domain, completed)) = pending.pop() {
        if completed {
            if active.pop() != Some(semantic_domain) {
                return None;
            }
            visited.push(semantic_domain);
            continue;
        }
        if active.contains(&semantic_domain) {
            return None;
        }
        if visited.contains(&semantic_domain) {
            continue;
        }
        let mut matching = domains.clone().filter(|domain| {
            domain.carrier == root.carrier && domain.semantic_domain == semantic_domain
        });
        let domain = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
        let required = domain.required_memberships.as_ref()?;
        if !required.windows(2).all(|pair| pair[0] < pair[1]) {
            return None;
        }
        result.push(domain);
        active.push(semantic_domain);
        pending.push((semantic_domain, true));
        pending.extend(required.iter().rev().map(|domain| (*domain, false)));
    }
    Some(result)
}

/// Scalar and addressable presentations share semantic coordinates only inside
/// this module. Match the complete parent graph and each independent issuer
/// group; root route equality alone can erase a routed parent.
pub(super) fn same_theory<'types, 'domain>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'domain StructuralDomainDeclaration> + Clone,
    structural: &'domain StructuralDomainDeclaration,
    scalar: &terminal_psi::ScalarDomainDeclaration,
    catalog: &terminal_psi::ScalarQualificationCatalog,
) -> bool {
    let Some(structural_nodes) = required_domains(domains, structural) else {
        return false;
    };
    let Some(scalar_nodes) = scalar.required_domain_closure(&catalog.domains) else {
        return false;
    };
    structural_nodes.len() == scalar_nodes.len()
        && structural_nodes.iter().all(|structural| {
            let mut matching = scalar_nodes.iter().copied().filter(|domain| {
                domain.semantic_domain == structural.semantic_domain
                    && domain.carrier == scalar.carrier
            });
            let Some(scalar) = matching.next() else {
                return false;
            };
            if matching.next().is_some()
                || structural.required_memberships != scalar.required_memberships
                || !structural
                    .establishment_routes
                    .windows(2)
                    .all(|pair| pair[0] < pair[1])
                || !scalar
                    .establishment_routes
                    .windows(2)
                    .all(|pair| pair[0] < pair[1])
                || structural.establishment_routes != scalar.establishment_routes
            {
                return false;
            }
            let Some(required) = refinement(types.clone(), structural) else {
                return false;
            };
            if required.carrier() != scalar.carrier {
                return false;
            }
            let proposition = match &scalar.predicate {
                Some(predicate) => {
                    let mut proposition = predicate.proposition.clone();
                    proposition.remap_value_ids(|value| {
                        if value == predicate.subject {
                            ScalarRefinement::SUBJECT
                        } else {
                            value
                        }
                    });
                    proposition
                }
                None if scalar.establishment_routes.is_empty()
                    && scalar.has_no_required_memberships() =>
                {
                    Proposition::Truth
                }
                None => return false,
            };
            let Ok(actual) = ScalarRefinement::new(scalar.carrier, proposition) else {
                return false;
            };
            let required = StructuralFieldType::RefinedScalar(required);
            let actual = StructuralFieldType::RefinedScalar(actual);
            crate::scalar_field_entails(&required, &actual)
                && crate::scalar_field_entails(&actual, &required)
        })
}
