//! Freeze complete scalar value clauses after collecting prerequisite rows.
//! Issuer requirements remain separate; predicate truth never supplies them.

use semantic_vocabulary::Proposition;
use terminal_psi::{ScalarDomainDeclaration, ScalarDomainPredicate};

pub(super) fn complete_value_components(domains: &mut [ScalarDomainDeclaration]) {
    let represented = domains
        .iter()
        .map(|domain| domain.required_memberships_are_well_formed(domains))
        .collect::<Vec<_>>();
    for (domain, represented) in domains.iter_mut().zip(represented) {
        if !represented {
            domain.required_memberships = None;
            domain.predicate = None;
        }
    }
    let mut values = vec![None; domains.len()];
    for index in 0..domains.len() {
        complete(domains, index, &mut Vec::new(), &mut values);
    }
    for (domain, value) in domains.iter_mut().zip(values) {
        // Preserve the established predicate-free representation of an empty
        // tag. Only the conjunction operation treats such a known tag as True.
        if domain.predicate.is_none()
            && domain.has_no_required_memberships()
            && domain.establishment_routes.is_empty()
        {
            continue;
        }
        domain.predicate = value.flatten().map(|proposition| ScalarDomainPredicate {
            subject: crate::terminal_identities::value_id(1),
            proposition,
        });
    }
}

fn complete(
    domains: &[ScalarDomainDeclaration],
    index: usize,
    active: &mut Vec<usize>,
    values: &mut [Option<Option<Proposition>>],
) -> Option<Proposition> {
    if let Some(value) = &values[index] {
        return value.clone();
    }
    if active.contains(&index) {
        return None;
    }
    let domain = &domains[index];
    let mut value = match &domain.predicate {
        Some(predicate) => predicate.proposition.clone(),
        None if domain.has_no_required_memberships() && domain.establishment_routes.is_empty() => {
            Proposition::Truth
        }
        None => return None,
    };
    active.push(index);
    let result = (|| {
        if let Some(required) = &domain.required_memberships {
            for parent in required {
                let mut matches = domains.iter().enumerate().filter(|(_, candidate)| {
                    candidate.semantic_domain == *parent && candidate.carrier == domain.carrier
                });
                let (parent, _) = matches.next()?;
                if matches.next().is_some() {
                    return None;
                }
                let predicate = complete(domains, parent, active, values)?;
                value = match (value, predicate) {
                    (Proposition::Truth, right) => right,
                    (left, Proposition::Truth) => left,
                    (Proposition::Falsehood, _) | (_, Proposition::Falsehood) => {
                        Proposition::Falsehood
                    }
                    (left, right) if left == right => left,
                    (left, right) => Proposition::Conjunction(vec![left, right]),
                };
            }
        }
        Some(value)
    })();
    active.pop();
    values[index] = Some(result.clone());
    result
}
