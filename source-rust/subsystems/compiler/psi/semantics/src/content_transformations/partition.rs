//! Obligations retained when a consuming pattern opens a content-bearing record.
mod intervals;
mod returns;

use semantic_vocabulary::{
    ClaimId, ContentAlgebraKind, ContentConservation, ContentPlaceVersion,
    ContentProjectionExpression, ContentTerm, EdgeId, PlaceId, Proposition, StructuralDomainId,
};
use terminal_psi::{StructuralAccess, StructuralArgument, TerminalMachine, TerminalModule};

/// Opening an entry wrapper changes its storage disposition. Each exact root
/// qualification must still be accounted for by a normal-return law. The
/// checker proves those laws independently against the actual returned pieces.
pub(crate) fn require_decomposition_laws(
    module: &TerminalModule,
    machine: &TerminalMachine,
    subject: &StructuralArgument,
) -> Result<(), &'static str> {
    let formal = machine
        .structural_parameter(subject.place)
        .filter(|formal| {
            formal.access == StructuralAccess::Owned
                && subject.path.is_empty()
                && !formal.qualifications.is_empty()
                && formal.projected_qualifications.is_empty()
                && formal.additional_input_qualifications.is_empty()
        })
        .ok_or("qualified decomposition requires its exact owned content input")?;
    let result = machine
        .result
        .structural()
        .ok_or("qualified decomposition requires an owned structural result")?;
    for id in &formal.qualifications {
        let domain = module
            .structural_domain(*id)
            .ok_or("qualified decomposition refers to an absent domain")?;
        let owner = domain
            .content_projection
            .as_ref()
            .filter(|_| {
                domain.carrier == formal.structural_type
                    && domain.byte_predicate.is_none()
                    && domain.access_policy.is_none()
            })
            .ok_or("qualified decomposition has a domain without a conserved content projection")?;
        let covered = machine.ensures.iter().any(|clause| {
            let Proposition::ContentConservation(law) = &clause.proposition else {
                return false;
            };
            // The canonical term order puts a projection before a separation.
            let (ContentTerm::Projection { projection, subject: entry }, ContentTerm::Separate(parts)) =
                (law.left(), law.right())
            else {
                return false;
            };
            *projection == owner.identity
                && *law.algebra() == owner.algebra
                && entry.version == ContentPlaceVersion::Entry
                && entry.root == formal.place
                && entry.segments.is_empty()
                && parts.len() >= 2
                && parts.iter().all(|part| {
                    matches!(part, ContentTerm::Projection { projection, subject: piece }
                        if *projection == owner.identity && piece.version == ContentPlaceVersion::Current
                            && piece.root == result.place && !piece.segments.is_empty())
                })
        });
        if !covered {
            return Err(
                "qualified decomposition requires a partition law for each independent input projection",
            );
        }
    }
    Ok(())
}

pub(super) fn is_interval_partition(law: &ContentConservation) -> bool {
    law.algebra().kind == ContentAlgebraKind::IntervalSet
        && matches!(law.right(), ContentTerm::Separate(_))
}

pub(crate) type Casts = std::collections::BTreeSet<(EdgeId, PlaceId, StructuralDomainId)>;

pub(super) struct Proof {
    pub(super) input: ClaimId,
    domain: StructuralDomainId,
    arrivals: std::collections::BTreeSet<(EdgeId, PlaceId)>,
}

/// Recompute each transferred projection once for this body's cast sites.
pub(crate) fn checked_casts(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Result<Casts, &'static str> {
    let mut casts = Casts::new();
    for clause in &machine.ensures {
        if let Proposition::ContentConservation(law) = &clause.proposition
            && is_interval_partition(law)
        {
            let proof = checked_return(module, machine, law)?;
            casts.extend(
                proof
                    .arrivals
                    .into_iter()
                    .map(|(edge, place)| (edge, place, proof.domain)),
            );
        }
    }
    if !casts.is_empty() {
        super::forwarding::validate_claims(module, machine)?;
    }
    Ok(casts)
}

/// A partition transfers one entry claim to all returned pieces bearing its
/// projection. Geometry comes from the checked field initializers and actual
/// control-flow arrivals. The source guarantee only names the terms to check.
pub(super) fn checked_return(
    module: &TerminalModule,
    machine: &TerminalMachine,
    law: &ContentConservation,
) -> Result<Proof, &'static str> {
    let (
        ContentTerm::Projection {
            projection,
            subject: entry,
        },
        ContentTerm::Separate(pieces),
    ) = (law.left(), law.right())
    else {
        return Err("partition needs an entry projection and separated result pieces");
    };
    if law.algebra().kind != ContentAlgebraKind::IntervalSet
        || entry.version != ContentPlaceVersion::Entry
        || !entry.segments.is_empty()
    {
        return Err("partition requires an interval projection on its original input");
    }
    let claim = crate::exactly_one::exactly_one(machine.content_entry_claims.iter(), |binding| {
        binding.input == *entry
            && binding
                .projections
                .iter()
                .any(|p| p.projection == *projection && p.algebra == *law.algebra())
    })
    .ok_or("partition needs the one consumed entry carrying its exact projection")?;
    let input = machine
        .structural_parameter(entry.root)
        .filter(|formal| formal.access == StructuralAccess::Owned)
        .ok_or("partition input is not an owned formal")?;
    let domain =
        crate::exactly_one::exactly_one(module.types.structural_domains.iter(), |domain| {
            domain.carrier == input.structural_type
                && domain.content_projection.as_ref().is_some_and(|owner| {
                    owner.identity == *projection && owner.algebra == *law.algebra()
                })
        })
        .ok_or("partition input has no exact owner interval projection")?;
    let Some(ContentProjectionExpression::IntervalSet(intervals)) = domain
        .content_projection
        .as_ref()
        .map(|owner| &owner.expression)
    else {
        return Err("partition requires an interval-set owner");
    };
    let [interval] = intervals.as_slice() else {
        return Err("partition requires a single input interval");
    };
    let result = machine
        .result
        .structural()
        .ok_or("partition has no owned result")?;
    let mut output = Vec::new();
    let mut canonical = std::collections::BTreeSet::new();
    for piece in pieces {
        let ContentTerm::Projection {
            projection: owner,
            subject,
        } = piece
        else {
            return Err("partition result is not a projection");
        };
        if owner != projection
            || subject.version != ContentPlaceVersion::Current
            || subject.root != result.place
        {
            return Err("partition changes the owner projection or returned subject");
        }
        let path = super::structural_path(
            module.types.structural_types.iter(),
            &module.identities,
            result.structural_type,
            &subject.segments,
        )
        .ok_or("partition refers to an unknown returned field")?;
        let (field, carrier) = crate::canonical_static_projection(
            module.types.structural_types.iter(),
            result.structural_type,
            &path,
        )
        .ok_or("partition output must be a static structural place")?;
        if carrier.id != domain.carrier || !canonical.insert(field) {
            return Err("partition requires distinct results of the exact input carrier");
        }
        output.push(path);
    }
    let declared = result
        .projected_qualifications
        .iter()
        .filter(|q| q.domain == domain.id)
        .map(|q| q.path.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let unaccounted = machine.membership_ensures.iter().any(|fact| matches!(fact,
        terminal_psi::RuntimeDomainMembership::Structural { root, path, version: ContentPlaceVersion::Current, domain: id }
        if *root == result.place && *id == domain.id && !canonical.contains(path)
    ));
    if unaccounted
        || result.qualifications.contains(&domain.id)
        || output
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            != declared
    {
        return Err("partition must cover every returned occurrence of its domain");
    }
    let arrivals = returns::check(module, machine, entry.root, &output, interval)?;
    Ok(Proof {
        input: claim.claim,
        domain: domain.id,
        arrivals,
    })
}
