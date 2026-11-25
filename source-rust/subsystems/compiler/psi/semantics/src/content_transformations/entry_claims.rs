//! Content projected from the exact owned structural claims of a callable input.
use crate::owned_structure::{ClaimTypes, claim_catalogue};
use semantic_vocabulary::{
    ContentAlgebraKind, ContentPlaceVersion, ContentProjectionExpression, ContentStructuralPlace,
};
use terminal_psi::{
    ClaimContentProjection, ContentEntryClaim, MachineContract, StructuralAccess,
    StructuralDomainDeclaration, StructuralPathQualification, StructuralTypeDeclaration,
    TerminalIdentityTable,
};

/// The parameter's declared type determines which structural paths own a
/// claim. Qualifying such a path adds its owner's projection to that same ID.
fn declared_entry_contents(
    types: &[StructuralTypeDeclaration],
    domains: &[StructuralDomainDeclaration],
    identities: &TerminalIdentityTable,
    contract: &MachineContract,
) -> Result<Vec<ContentEntryClaim>, &'static str> {
    let mut bindings = Vec::new();
    if !domains
        .iter()
        .any(|domain| domain.content_projection.is_some())
    {
        return Ok(bindings);
    }
    let mut claims_by_place = contract.entry_claims.iter().collect::<Vec<_>>();
    claims_by_place.sort_unstable_by_key(|entry| (entry.input, entry.path.as_slice()));
    if claims_by_place
        .windows(2)
        .any(|pair| pair[0].input == pair[1].input && pair[0].path == pair[1].path)
    {
        return Err("different structural claims name the same input path");
    }
    let mut claim_types = None;
    for formal in contract
        .structural_parameters
        .iter()
        .filter(|formal| formal.access == StructuralAccess::Owned)
    {
        let qualifications = formal
            .qualifications
            .iter()
            .map(|domain| (&[][..], domain))
            .chain(
                formal
                    .projected_qualifications
                    .iter()
                    .chain(&formal.additional_input_qualifications)
                    .map(|row| (row.path.as_slice(), &row.domain)),
            );
        let mut owners = Vec::new();
        for (path, id) in qualifications {
            let domain = domains
                .iter()
                .find(|domain| domain.id == *id)
                .ok_or("entry qualification has no declared domain")?;
            if let Some(owner) = &domain.content_projection {
                if owner.identity.domain.get() != domain.semantic_domain.get()
                    || owner.algebra.parameter.is_empty()
                    || owner.expression.report_fingerprint(&owner.algebra)
                        != owner.identity.projection_report_fingerprint
                    || !matches!(
                        (&owner.expression, owner.algebra.kind),
                        (
                            ContentProjectionExpression::CountedQuantity(_),
                            ContentAlgebraKind::CountedQuantity
                        ) | (
                            ContentProjectionExpression::IntervalSet(_),
                            ContentAlgebraKind::IntervalSet
                        )
                    )
                {
                    return Err(
                        "input qualification differs from its owner's projection or algebra",
                    );
                }
                owners.push((path, domain.carrier, owner));
            }
        }
        if owners.is_empty() {
            continue;
        }
        let catalogue = match &claim_types {
            Some(catalogue) => catalogue,
            None => claim_types.insert(ClaimTypes::new(types)?),
        };
        for path in claim_catalogue(
            catalogue,
            formal.structural_type,
            contract.entry_claims.len(),
        )? {
            let mut projections = Vec::new();
            for &(qualification, carrier, owner) in &owners {
                if StructuralPathQualification::selected_suffix(qualification, &path) != Some(&[]) {
                    continue;
                }
                if crate::runtime_structural_path_tip(types.iter(), formal.structural_type, &path)
                    .is_none_or(|tip| tip.id != carrier)
                {
                    return Err("entry content qualification belongs to a different carrier");
                }
                projections.push(ClaimContentProjection {
                    projection: owner.identity,
                    algebra: owner.algebra.clone(),
                });
            }
            if projections.is_empty() {
                continue;
            }
            projections.sort_unstable();
            projections.dedup();
            let position = claims_by_place
                .binary_search_by_key(&(formal.place, path.as_slice()), |entry| {
                    (entry.input, entry.path.as_slice())
                })
                .map_err(|_| "qualified input has no structural claim at its owned path")?;
            let entry = claims_by_place[position];
            bindings.push(ContentEntryClaim {
                claim: entry.claim,
                input: ContentStructuralPlace {
                    version: ContentPlaceVersion::Entry,
                    root: formal.place,
                    segments: super::content_path(
                        types.iter(),
                        identities,
                        formal.structural_type,
                        &path,
                    )
                    .ok_or("entry content has no exact named structural path")?,
                },
                projections,
            });
        }
    }
    bindings.sort_unstable_by_key(|binding| binding.claim);
    if bindings
        .windows(2)
        .any(|pair| pair[0].claim == pair[1].claim)
    {
        return Err("different qualified input claims share an identity");
    }
    Ok(bindings)
}

/// Install source-derived projections without consuming a separately issued
/// content-only binding. A retained row using an ordinary ID must agree.
pub fn publish_declared_entry_contents(
    types: &[StructuralTypeDeclaration],
    domains: &[StructuralDomainDeclaration],
    identities: &TerminalIdentityTable,
    contract: &mut MachineContract,
) -> Result<(), &'static str> {
    let mut entries = declared_entry_contents(types, domains, identities, contract)?;
    for old in &contract.content_entry_claims {
        if contract
            .entry_claims
            .binary_search_by_key(&old.claim, |entry| entry.claim)
            .is_ok()
        {
            if !entries.iter().any(|entry| entry == old) {
                return Err("retained content conflicts with its structural entry claim");
            }
        } else {
            entries.push(old.clone());
        }
    }
    entries.sort_unstable_by_key(|entry| entry.claim);
    contract.content_entry_claims = entries;
    Ok(())
}

/// Reconstruct bindings on ordinary structural claims. Independent content-only
/// entries retain their separate machine-local identities.
pub fn validate_declared_entry_contents(
    types: &[StructuralTypeDeclaration],
    domains: &[StructuralDomainDeclaration],
    identities: &TerminalIdentityTable,
    contract: &MachineContract,
) -> Result<(), &'static str> {
    let expected = declared_entry_contents(types, domains, identities, contract)?;
    let actual = contract.content_entry_claims.iter().filter(|binding| {
        contract
            .entry_claims
            .binary_search_by_key(&binding.claim, |entry| entry.claim)
            .is_ok()
    });
    if actual.ne(&expected) {
        return Err("entry content differs from its owned structural claim and qualification");
    }
    Ok(())
}

#[cfg(test)]
mod tests;
