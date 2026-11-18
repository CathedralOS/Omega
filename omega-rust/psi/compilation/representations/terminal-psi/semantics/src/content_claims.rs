//! Content describes an existing owned claim; it does not issue another one.
//! Reconstruct the complete entry roster from the authenticated parameter
//! memberships and the same typed claim catalogue used by ordinary moves.

use semantic_vocabulary::{
    ClaimId, ContentPlaceVersion, ContentStructuralPlace, PlaceId, StructuralDomainId,
    StructuralTypeId,
};
use terminal_psi::{
    ClaimContentProjection, ContentEntryClaim, MachineContract, StructuralAccess,
    StructuralDomainDeclaration, StructuralPathQualification, StructuralPathSegment,
    StructuralTypeDeclaration, TerminalIdentityTable,
};

/// The already declared memberships of one owned occurrence. Claim bindings
/// are supplied independently, so desired qualifications cannot create custody.
pub struct Subject<'a> {
    pub place: PlaceId,
    pub carrier: StructuralTypeId,
    pub qualifications: &'a [StructuralDomainId],
    pub projected_qualifications: &'a [StructuralPathQualification],
}

pub struct ClaimProjections<'a> {
    types: &'a [StructuralTypeDeclaration],
    domains: Vec<&'a StructuralDomainDeclaration>,
    identities: &'a TerminalIdentityTable,
}

impl<'a> ClaimProjections<'a> {
    pub fn new(
        types: &'a [StructuralTypeDeclaration],
        domains: &'a [StructuralDomainDeclaration],
        identities: &'a TerminalIdentityTable,
    ) -> Result<Self, &'static str> {
        let mut domains = domains.iter().collect::<Vec<_>>();
        domains.sort_unstable_by_key(|domain| domain.id);
        if domains.windows(2).any(|pair| pair[0].id == pair[1].id) {
            return Err("content claim domains repeat an identity");
        }
        Ok(Self {
            types,
            domains,
            identities,
        })
    }

    pub fn entry_claims(
        &self,
        contract: &MachineContract,
    ) -> Result<Vec<ContentEntryClaim>, &'static str> {
        let mut claims = contract.entry_claims.iter().collect::<Vec<_>>();
        claims.sort_unstable_by(|left, right| {
            (left.input, &left.path).cmp(&(right.input, &right.path))
        });
        let mut rows = Vec::new();
        for parameter in &contract.structural_parameters {
            if parameter.access != StructuralAccess::Owned {
                continue;
            }
            let first = claims.partition_point(|claim| claim.input < parameter.place);
            let last = claims.partition_point(|claim| claim.input <= parameter.place);
            rows.extend(
                self.at_subject(
                    Subject {
                        place: parameter.place,
                        carrier: parameter.structural_type,
                        qualifications: &parameter.qualifications,
                        projected_qualifications: &parameter.projected_qualifications,
                    },
                    claims[first..last]
                        .iter()
                        .map(|claim| (claim.claim, claim.path.as_slice())),
                    ContentPlaceVersion::Entry,
                )?,
            );
        }
        rows.sort();
        Ok(rows)
    }

    pub fn at_subject<'path>(
        &self,
        subject: Subject<'_>,
        claims: impl Iterator<Item = (ClaimId, &'path [StructuralPathSegment])>,
        version: ContentPlaceVersion,
    ) -> Result<Vec<ContentEntryClaim>, &'static str> {
        let Self {
            types,
            domains,
            identities,
        } = self;
        let mut claims = claims.collect::<Vec<_>>();
        claims.sort_unstable_by(|left, right| left.1.cmp(right.1));
        if claims.windows(2).any(|pair| pair[0].1 == pair[1].1) {
            return Err("content claim entry repeats an owned path");
        }
        let mut rows = Vec::new();
        for (path, domain) in subject
            .qualifications
            .iter()
            .map(|domain| (&[][..], *domain))
            .chain(
                subject
                    .projected_qualifications
                    .iter()
                    .map(|row| (row.path.as_slice(), row.domain)),
            )
        {
            let domain = domains
                .binary_search_by_key(&domain, |row| row.id)
                .ok()
                .map(|position| domains[position])
                .ok_or("content claim membership has no domain declaration")?;
            let Some(projection) = &domain.content_projection else {
                continue;
            };
            let carrier = crate::runtime_structural_path_tip(types.iter(), subject.carrier, path)
                .ok_or("content claim membership has no exact structural carrier")?;
            if carrier.id != domain.carrier {
                return Err("content claim membership names another carrier application");
            }
            let claim = claims
                .binary_search_by(|(_, candidate)| (*candidate).cmp(path))
                .ok()
                .map(|position| &claims[position].0)
                .ok_or("content membership has no exact independently owned claim")?;
            rows.push(ContentEntryClaim {
                claim: *claim,
                input: ContentStructuralPlace {
                    version,
                    root: subject.place,
                    segments: crate::content_transformations::content_path(
                        types,
                        identities,
                        subject.carrier,
                        path,
                    )?,
                },
                projections: vec![ClaimContentProjection {
                    projection: projection.identity,
                    algebra: projection.algebra.clone(),
                }],
            });
        }
        rows.sort();
        let mut merged: Vec<ContentEntryClaim> = Vec::with_capacity(rows.len());
        for mut row in rows {
            if let Some(previous) = merged.last_mut()
                && previous.claim == row.claim
                && previous.input == row.input
            {
                previous.projections.append(&mut row.projections);
            } else {
                merged.push(row);
            }
        }
        for row in &mut merged {
            row.projections.sort();
            if row.projections.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err("content claim repeats an exact projection obligation");
            }
        }
        Ok(merged)
    }

    pub fn validate_entries(&self, contract: &MachineContract) -> Result<(), &'static str> {
        if contract.content_entry_claims != self.entry_claims(contract)? {
            return Err("content entry claims differ from their complete owned membership roster");
        }
        Ok(())
    }

    pub fn resolve_subject(
        &self,
        carrier: StructuralTypeId,
        subject: &ContentStructuralPlace,
    ) -> Result<Vec<StructuralPathSegment>, &'static str> {
        crate::content_transformations::structural_path(
            self.types.iter(),
            self.identities,
            carrier,
            &subject.segments,
        )
        .ok_or("content claim subject has no exact typed path")
    }

    pub fn decomposition(
        &self,
        subject: Subject<'_>,
        path: &[StructuralPathSegment],
    ) -> Result<Vec<ClaimContentProjection>, &'static str> {
        decomposition(
            self.types,
            |id| {
                self.domains
                    .binary_search_by_key(&id, |domain| domain.id)
                    .ok()
                    .map(|position| self.domains[position])
            },
            subject,
            path,
        )
    }
}

/// Ending this wrapper leaves each child's qualification and custody intact.
/// An ancestor qualification requires its own complete consuming operation.
pub fn decomposition<'a>(
    types: &[StructuralTypeDeclaration],
    domain: impl Fn(StructuralDomainId) -> Option<&'a StructuralDomainDeclaration>,
    subject: Subject<'_>,
    path: &[StructuralPathSegment],
) -> Result<Vec<ClaimContentProjection>, &'static str> {
    let mut retained = Vec::new();
    for (qualified, id) in subject.qualifications.iter().map(|id| (&[][..], id)).chain(
        subject
            .projected_qualifications
            .iter()
            .map(|row| (row.path.as_slice(), &row.domain)),
    ) {
        if !path.starts_with(qualified) {
            continue;
        }
        if qualified != path {
            return Err("decomposition crosses a qualified ancestor");
        }
        let domain = domain(*id).ok_or("decomposition qualification has no declaration")?;
        let carrier = crate::runtime_structural_path_tip(types.iter(), subject.carrier, path)
            .ok_or("decomposition qualification has no exact carrier")?;
        if carrier.id != domain.carrier {
            return Err("decomposition qualification names another carrier application");
        }
        let projection = domain
            .content_projection
            .as_ref()
            .ok_or("decomposition cannot settle an independent non-content qualification")?;
        retained.push(ClaimContentProjection {
            projection: projection.identity,
            algebra: projection.algebra.clone(),
        });
    }
    retained.sort();
    if retained.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("decomposition repeats a content obligation");
    }
    Ok(retained)
}
