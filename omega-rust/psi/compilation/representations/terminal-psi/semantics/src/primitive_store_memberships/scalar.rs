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

pub(super) fn same_routes(
    structural: &StructuralDomainDeclaration,
    scalar: &terminal_psi::ScalarDomainDeclaration,
) -> bool {
    // Both presentations retain the same complete owner-bound route protocol.
    // Require canonical unique rosters before equality: duplicated matches do
    // not form a bijection even if both malformed lists repeat the same row.
    structural
        .establishment_routes
        .windows(2)
        .all(|pair| pair[0] < pair[1])
        && scalar
            .establishment_routes
            .windows(2)
            .all(|pair| pair[0] < pair[1])
        && structural.establishment_routes == scalar.establishment_routes
}
