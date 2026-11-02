//! Preserve existing membership evidence when a descriptor changes presentation.
//!
//! Stored and canonical descriptors name different structural carriers. Copying
//! their address and extent preserves the observed value, but a domain ID still
//! names its exact carrier. Rebind only the same semantic theory; this operation
//! creates no source evidence and grants no access, custody or lifetime.

use semantic_vocabulary::{StructuralDomainId, StructuralTypeId};
use terminal_psi::{
    StructuralDomainDeclaration, StructuralPathQualification, StructuralTypeDeclaration,
};

pub fn present_structural_memberships<'types, 'domains>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'domains StructuralDomainDeclaration> + Clone,
    destination: StructuralTypeId,
    whole: &mut Vec<StructuralDomainId>,
    projected: &mut Vec<StructuralPathQualification>,
) -> Result<(), &'static str> {
    let present = |domain: StructuralDomainId, carrier: StructuralTypeId| {
        let source = domains
            .clone()
            .find(|row| row.id == domain)
            .ok_or("membership presentation has no source domain")?;
        if source.carrier == carrier {
            return Ok(domain);
        }
        if crate::borrowed_referent_presentation(types.clone(), source.carrier, carrier)
            != Some(crate::BorrowedReferentPresentation::Unchanged)
        {
            return Err("membership presentation changes its observed carrier");
        }
        let mut matches = domains.clone().filter(|row| {
            row.carrier == carrier
                && row.semantic_domain == source.semantic_domain
                && same_presented_theory(domains.clone(), source, row)
        });
        let destination = matches
            .next()
            .ok_or("membership presentation has no identical destination theory")?;
        if matches.next().is_some() {
            return Err("membership presentation has ambiguous destination theories");
        }
        Ok(destination.id)
    };
    for domain in whole.iter_mut() {
        *domain = present(*domain, destination)?;
    }
    for row in projected.iter_mut() {
        let carrier = crate::qualification_path_carrier(types.clone(), destination, &row.path)
            .ok_or("membership presentation has no destination path carrier")?;
        row.domain = present(row.domain, carrier)?;
    }
    whole.sort();
    whole.dedup();
    projected.sort();
    projected.dedup();
    Ok(())
}

/// A presentation changes carrier coordinates, not the dependency theory.
/// Resolve every prerequisite on both carriers instead of donating source
/// parent rows to the destination descriptor.
fn same_presented_theory<'domains>(
    domains: impl Iterator<Item = &'domains StructuralDomainDeclaration> + Clone,
    source: &'domains StructuralDomainDeclaration,
    destination: &'domains StructuralDomainDeclaration,
) -> bool {
    let resolve = |semantic_domain, carrier| {
        let mut matches = domains
            .clone()
            .filter(|row| row.semantic_domain == semantic_domain && row.carrier == carrier);
        let row = matches.next()?;
        matches.next().is_none().then_some(row)
    };
    if resolve(source.semantic_domain, source.carrier).map(|row| row.id) != Some(source.id)
        || resolve(destination.semantic_domain, destination.carrier).map(|row| row.id)
            != Some(destination.id)
    {
        return false;
    }
    let mut pending = vec![(source, destination, false)];
    let mut active = Vec::new();
    let mut visited = Vec::new();
    while let Some((source, destination, completed)) = pending.pop() {
        if completed {
            if active.pop() != Some(source.id) {
                return false;
            }
            visited.push(source.id);
            continue;
        }
        if active.contains(&source.id) {
            return false;
        }
        if visited.contains(&source.id) {
            continue;
        }
        if source.semantic_domain != destination.semantic_domain
            || source.complete_predicate_theory != destination.complete_predicate_theory
            || source.predicate != destination.predicate
            || source.content_projection != destination.content_projection
            || source.byte_predicate != destination.byte_predicate
            || source.establishment_routes != destination.establishment_routes
        {
            return false;
        }
        let (Some(source_required), Some(destination_required)) = (
            &source.required_memberships,
            &destination.required_memberships,
        ) else {
            return false;
        };
        if source_required != destination_required
            || !source_required.windows(2).all(|pair| pair[0] < pair[1])
        {
            return false;
        }
        active.push(source.id);
        pending.push((source, destination, true));
        for required in source_required.iter().rev() {
            let Some(source_parent) = resolve(*required, source.carrier) else {
                return false;
            };
            let Some(destination_parent) = resolve(*required, destination.carrier) else {
                return false;
            };
            pending.push((source_parent, destination_parent, false));
        }
    }
    true
}
