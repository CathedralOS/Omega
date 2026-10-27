//! Rejoin input and result membership catalogs at the actual installation.
//!
//! A membership belongs to the actual transferred occurrence; receiving
//! annotations cannot supply it. Dense domain, semantic-domain and route coordinates belong
//! to separate artifacts. Preserve a relation only when nominal ownership,
//! carrier and the retained theory agree. Names alone do not pin a theory's
//! revision. Storage-observing predicates need a structural observation join;
//! this transfer admits only observation-free predicates rather than inferring
//! that opacity hides every retained representation field.

use semantic_vocabulary::{StructuralDomainId, StructuralTypeId};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, NominalDeclarationIdentity,
    StructuralDomainDeclaration, StructuralEstablishmentRoute, TerminalModule,
};

pub(super) type Memberships = Vec<(StructuralDomainId, StructuralDomainId)>;
pub(super) type InputMemberships = Vec<Memberships>;

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
            admit_memberships(
                receiver,
                left.structural_type,
                &left.qualifications,
                provider,
                right.structural_type,
                &right.qualifications,
            )
            .map_err(|reason| format!("component input {reason}"))
        })
        .collect()
}

/// Results travel in the opposite direction. Retain the producing artifact's
/// actual membership keys until its owned completion is acknowledged.
pub(super) fn admit_result_memberships(
    receiver: &TerminalModule,
    required: &BoundaryMachineDeclaration,
    provider: &TerminalModule,
    supplied: &BoundaryMachineDeclaration,
) -> Result<Memberships, String> {
    match (&supplied.result, &required.result) {
        (BoundaryMachineResult::Unit, BoundaryMachineResult::Unit) => Ok(Vec::new()),
        (BoundaryMachineResult::Scalar(left), BoundaryMachineResult::Scalar(right))
            if left == right =>
        {
            Ok(Vec::new())
        }
        (BoundaryMachineResult::Structural(left), BoundaryMachineResult::Structural(right)) => {
            if !left.projected_qualifications.is_empty()
                || !right.projected_qualifications.is_empty()
            {
                return Err("component result requires projected membership transfer".into());
            }
            admit_memberships(
                provider,
                left.structural_type,
                &left.qualifications,
                receiver,
                right.structural_type,
                &right.qualifications,
            )
            .map_err(|reason| format!("component result {reason}"))
        }
        _ => Err("component result kinds or scalar contracts differ".into()),
    }
}

fn admit_memberships(
    source_module: &TerminalModule,
    source_type: StructuralTypeId,
    source_required: &[StructuralDomainId],
    target_module: &TerminalModule,
    target_type: StructuralTypeId,
    target_required: &[StructuralDomainId],
) -> Result<Memberships, String> {
    let mut bindings = Vec::new();
    for source in source_module
        .structural_domains
        .iter()
        .filter(|row| row.carrier == source_type)
    {
        let mut matches = target_module.structural_domains.iter().filter(|row| {
            row.carrier == target_type && same_domain(source_module, source, target_module, row)
        });
        if let Some(destination) = matches.next() {
            if matches.next().is_some() {
                return Err("has ambiguous membership theories".into());
            }
            bindings.push((source.id, destination.id));
        }
    }
    let mut mapped = source_required
        .iter()
        .map(|source| {
            bindings
                .iter()
                .find(|(domain, _)| domain == source)
                .map(|(_, domain)| *domain)
                .ok_or_else(|| "membership theories differ".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    mapped.sort();
    if mapped != target_required {
        return Err("membership requirements differ".into());
    }
    Ok(bindings)
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
    if !source.complete_predicate_theory
        || !target.complete_predicate_theory
        || !same_name(source.identity, target.identity)
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
                let kind = match route {
                    StructuralEstablishmentRoute::CheckedRequirement { .. } => 0,
                    StructuralEstablishmentRoute::ExactMachine { .. } => 1,
                    StructuralEstablishmentRoute::BoundaryRequirement { .. } => 2,
                };
                let declaration = route.declaration();
                let identity = route.identity();
                // Invocation-local issuer coordinates cannot establish a portable
                // cross-artifact relation by spelling. The regular declaration join
                // retains that restriction for domain and carrier identities too.
                if module.identities.is_invocation_local(identity)
                    || module
                        .identities
                        .is_invocation_local(declaration.identity())
                {
                    return None;
                }
                let owner = module
                    .identities
                    .text(declaration.identity())
                    .filter(|name| !name.is_empty())?;
                module
                    .identities
                    .text(identity)
                    .filter(|name| !name.is_empty())
                    .map(|name| (kind, owner.to_owned(), name.to_owned()))
            })
            .collect::<Option<Vec<_>>>()?;
        rows.sort();
        Some(rows)
    };
    routes(left, source)
        .zip(routes(right, target))
        .is_some_and(|(a, b)| a == b)
}
