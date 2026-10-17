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
                && row.predicate == source.predicate
                && row.content_projection == source.content_projection
                && row.byte_predicate == source.byte_predicate
                && row.establishment_routes == source.establishment_routes
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
