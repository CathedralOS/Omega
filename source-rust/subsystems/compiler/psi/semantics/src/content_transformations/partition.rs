//! Obligations retained when a consuming pattern opens a content-bearing record.
mod intervals;
mod returns;

use semantic_vocabulary::{
    ClaimId, ContentAlgebraKind, ContentConservation, ContentPlaceVersion,
    ContentProjectionExpression, ContentProjectionIdentity, ContentStructuralPlace, ContentTerm,
    EdgeId, PlaceId, Proposition, StructuralDomainId,
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
            Exchange::read(law).is_some_and(|exchange| {
                exchange.projection == owner.identity
                    && *law.algebra() == owner.algebra
                    && exchange
                        .inputs
                        .iter()
                        .any(|input| input.root == formal.place)
                    && exchange
                        .outputs
                        .iter()
                        .all(|output| output.root == result.place)
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

/// The term order is canonical. Entry/current ownership determines the
/// direction, so the same equation describes splitting and joining.
struct Exchange<'a> {
    projection: ContentProjectionIdentity,
    inputs: Vec<&'a ContentStructuralPlace>,
    outputs: Vec<&'a ContentStructuralPlace>,
}

impl<'a> Exchange<'a> {
    fn read(law: &'a ContentConservation) -> Option<Self> {
        let (
            ContentTerm::Projection {
                projection,
                subject,
            },
            ContentTerm::Separate(parts),
        ) = (law.left(), law.right())
        else {
            return None;
        };
        if !is_interval_partition(law) || parts.len() < 2 {
            return None;
        }
        let pieces = parts
            .iter()
            .map(|term| match term {
                ContentTerm::Projection {
                    projection: id,
                    subject: piece,
                } if id == projection && piece.version != subject.version => Some(piece),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        let (inputs, outputs) = match subject.version {
            ContentPlaceVersion::Entry => (vec![subject], pieces),
            ContentPlaceVersion::Current => (pieces, vec![subject]),
        };
        inputs
            .iter()
            .all(|place| place.segments.is_empty())
            .then_some(Self {
                projection: *projection,
                inputs,
                outputs,
            })
    }
}

pub(crate) type Casts = std::collections::BTreeSet<(EdgeId, PlaceId, StructuralDomainId)>;

pub(super) struct Proof {
    pub(super) inputs: Vec<ClaimId>,
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

/// Derive the geometry from the field initializers on each actual return.
/// The equation identifies which owned inputs and result fields to compare.
pub(super) fn checked_return(
    module: &TerminalModule,
    machine: &TerminalMachine,
    law: &ContentConservation,
) -> Result<Proof, &'static str> {
    let exchange = Exchange::read(law).ok_or(
        "partition requires an interval projection between owned inputs and returned pieces",
    )?;
    let projection = exchange.projection;
    let carrier = machine
        .structural_parameter(exchange.inputs[0].root)
        .ok_or("partition input must name an owned formal")?
        .structural_type;
    let domain =
        crate::exactly_one::exactly_one(module.types.structural_domains.iter(), |domain| {
            domain.carrier == carrier
                && domain.content_projection.as_ref().is_some_and(|owner| {
                    owner.identity == projection && owner.algebra == *law.algebra()
                })
        })
        .ok_or("partition input has no exact owner interval projection")?;
    let mut inputs = Vec::with_capacity(exchange.inputs.len());
    let mut claims = Vec::with_capacity(exchange.inputs.len());
    for entry in exchange.inputs {
        let formal = machine
            .structural_parameter(entry.root)
            .filter(|formal| {
                formal.access == StructuralAccess::Owned
                    && formal.structural_type == domain.carrier
                    && formal.qualifications.contains(&domain.id)
            })
            .ok_or("partition input must own the projection on its exact carrier")?;
        let claim =
            crate::exactly_one::exactly_one(machine.content_entry_claims.iter(), |binding| {
                binding.input == *entry
                    && binding
                        .projections
                        .iter()
                        .any(|p| p.projection == projection && p.algebra == *law.algebra())
            })
            .ok_or("partition needs the consumed entry carrying its exact projection")?;
        inputs.push(formal.place);
        claims.push(claim.claim);
    }
    inputs.sort_unstable();
    claims.sort_unstable();
    if inputs.windows(2).any(|pair| pair[0] == pair[1])
        || claims.windows(2).any(|pair| pair[0] == pair[1])
    {
        return Err("partition consumes the same entry claim more than once");
    }
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
    for subject in exchange.outputs {
        if subject.root != result.place {
            return Err("partition changes its returned subject");
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
    let mut declared = result
        .projected_qualifications
        .iter()
        .filter(|q| q.domain == domain.id)
        .map(|q| q.path.clone())
        .collect::<std::collections::BTreeSet<_>>();
    if result.qualifications.contains(&domain.id) {
        declared.insert(Vec::new());
    }
    let unaccounted = machine.membership_ensures.iter().any(|fact| matches!(fact,
        terminal_psi::RuntimeDomainMembership::Structural { root, path, version: ContentPlaceVersion::Current, domain: id }
        if *root == result.place && *id == domain.id && !canonical.contains(path)
    ));
    if unaccounted
        || output
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            != declared
    {
        return Err("partition must cover every returned occurrence of its domain");
    }
    let arrivals = returns::check(module, machine, &inputs, &output, interval)?;
    Ok(Proof {
        inputs: claims,
        domain: domain.id,
        arrivals,
    })
}
