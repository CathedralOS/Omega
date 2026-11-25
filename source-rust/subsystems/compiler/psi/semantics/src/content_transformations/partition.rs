//! Obligations retained when a consuming pattern opens a content-bearing record.
use semantic_vocabulary::{ContentPlaceVersion, ContentTerm, Proposition};
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
