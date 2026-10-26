//! Rejoin input membership catalogs at the actual component installation.
//!
//! A membership belongs to the actual caller occurrence; the receiving formal
//! cannot supply it. Dense domain, semantic-domain and route coordinates belong
//! to separate artifacts. Preserve a relation only when nominal ownership,
//! carrier and the retained theory agree. Names alone do not pin a theory's
//! revision. Storage-observing predicates need a structural observation join;
//! this transfer admits only observation-free predicates rather than inferring
//! that opacity hides every retained representation field.

use semantic_vocabulary::StructuralDomainId;
use terminal_psi::{
    BoundaryMachineDeclaration, NominalDeclarationIdentity, StructuralDomainDeclaration,
    StructuralEstablishmentRoute, TerminalModule,
};

pub(super) type InputMemberships = Vec<Vec<(StructuralDomainId, StructuralDomainId)>>;

pub(super) fn admit_input_memberships(
    receiver: &TerminalModule,
    required: &BoundaryMachineDeclaration,
    provider: &TerminalModule,
    supplied: &BoundaryMachineDeclaration,
) -> Result<InputMemberships, String> {
    if required.structural_parameters.len() != supplied.structural_parameters.len() {
        return Err("component input membership arities differ".into());
    }
    required
        .structural_parameters
        .iter()
        .zip(&supplied.structural_parameters)
        .map(|(left, right)| {
            let mut bindings = Vec::new();
            for source in receiver
                .structural_domains
                .iter()
                .filter(|row| row.carrier == left.structural_type)
            {
                let mut matches = provider.structural_domains.iter().filter(|row| {
                    row.carrier == right.structural_type
                        && same_domain(receiver, source, provider, row)
                });
                if let Some(destination) = matches.next() {
                    if matches.next().is_some() {
                        return Err("component input has ambiguous membership theories".into());
                    }
                    bindings.push((source.id, destination.id));
                }
            }
            let mut mapped = left
                .qualifications
                .iter()
                .map(|source| {
                    bindings
                        .iter()
                        .find(|(domain, _)| domain == source)
                        .map(|(_, domain)| *domain)
                        .ok_or_else(|| "component input membership theories differ".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?;
            mapped.sort();
            if mapped != right.qualifications {
                return Err("component input membership requirements differ".into());
            }
            Ok(bindings)
        })
        .collect()
}

fn same_domain(
    left: &TerminalModule,
    source: &StructuralDomainDeclaration,
    right: &TerminalModule,
    target: &StructuralDomainDeclaration,
) -> bool {
    let same_name = |a, b| {
        NominalDeclarationIdentity::new(a).same_identity_as(
            &left.identities,
            &NominalDeclarationIdentity::new(b),
            &right.identities,
        )
    };
    let carrier = |module: &TerminalModule, id| {
        module
            .structural_types
            .iter()
            .find(|row| row.id == id)
            .map(|row| row.identity)
    };
    if !same_name(source.identity, target.identity)
        || !carrier(left, source.carrier)
            .zip(carrier(right, target.carrier))
            .is_some_and(|(a, b)| same_name(a, b))
        || source.byte_predicate != target.byte_predicate
        || source
            .content_projection
            .as_ref()
            .map(|row| (&row.algebra, &row.expression))
            != target
                .content_projection
                .as_ref()
                .map(|row| (&row.algebra, &row.expression))
    {
        return false;
    }
    match (&source.predicate, &target.predicate) {
        (Some(a), Some(b))
            if a.proposition == b.proposition
                && terminal_semantics::domain_predicate_observes_no_storage(
                    left.structural_types.iter(),
                    source.carrier,
                    a,
                )
                && terminal_semantics::domain_predicate_observes_no_storage(
                    right.structural_types.iter(),
                    target.carrier,
                    b,
                ) => {}
        _ => return false,
    }
    let routes = |module: &TerminalModule, domain: &StructuralDomainDeclaration| {
        let mut rows = domain
            .establishment_routes
            .iter()
            .map(|route| {
                let (kind, identity) = match route {
                    StructuralEstablishmentRoute::Requirement { requirement } => (0, *requirement),
                    StructuralEstablishmentRoute::ExactMachine { machine } => (1, *machine),
                    StructuralEstablishmentRoute::BoundaryRequirement { requirement } => {
                        (2, *requirement)
                    }
                };
                // Invocation-local issuer coordinates cannot establish a portable
                // cross-artifact relation by spelling. The regular declaration join
                // retains that restriction for domain and carrier identities too.
                if module.identities.is_invocation_local(identity) {
                    return None;
                }
                module
                    .identities
                    .text(identity)
                    .filter(|name| !name.is_empty())
                    .map(|name| (kind, name.to_owned()))
            })
            .collect::<Option<Vec<_>>>()?;
        rows.sort();
        Some(rows)
    };
    routes(left, source)
        .zip(routes(right, target))
        .is_some_and(|(a, b)| a == b)
}
