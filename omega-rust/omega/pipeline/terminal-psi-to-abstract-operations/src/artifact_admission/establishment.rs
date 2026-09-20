//! Provider establishment supplies bound at the executable input boundary.
//!
//! A `Placed<P, T>` direct-entry input is custody, not data: the roster row
//! seals the placement the view interprets, and neither the roster nor a
//! pointer is the authority that lends the referent. The ordinary
//! establishment route supplies one [`TerminalPlacedViewEstablishment`] per
//! declared direct-entry roster row — the provider's loan of the exact
//! qualified backing — and admission binds each supply to its row so the
//! realized executable's entry boundary can lend it for the invocation's
//! duration. The loan is invocation-scoped: the bound set is retained as
//! executable-entry evidence, and on failure the supply returns to the
//! provider unbound.
//!
//! Every supplied `input` must equal its declared roster row exactly —
//! machine, position, declaration identities, access and binding flags, and
//! both sealed placement identities — so a stale layout, qualified backing,
//! range, rights, or occurrence answers no row here and rejects before
//! access. A row left without a supply still fails custody, and an
//! exclusive-borrow referent may not overlap another established referent.
//!
//! The loan's referent is module-visible custody, not a bare host carrier:
//! its declared structural type must be a type the artifact declares, its
//! path must resolve through that declared shape graph to a real place, and
//! every domain qualification it asserts must be a domain the artifact
//! declares over that carrier. A supply whose backing, range, or
//! qualifications cannot rejoin the module's own catalogs is a stale or
//! substituted establishment and rejects before access.

pub use terminal_interpreter::TerminalPlacedViewEstablishment;

use super::ArtifactLoweringError;
use std::collections::BTreeSet;
use terminal_psi::{StructuralAccess, TerminalModule};

const fn exclusive(access: StructuralAccess) -> bool {
    matches!(
        access,
        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
    )
}

fn referent_overlaps(
    left: &terminal_interpreter::TerminalStructuralValue,
    right: &terminal_interpreter::TerminalStructuralValue,
) -> bool {
    left.opaque_identity == right.opaque_identity
        && (left.path.starts_with(&right.path) || right.path.starts_with(&left.path))
}

/// Join each direct-entry placed-view roster row to one supplied provider
/// establishment and return the bound set in roster order.
///
/// Only rows on the module's entry machine are entry inputs; a row on any
/// other machine declares a call-bound input whose custody arrives through
/// its caller, a route the executable input boundary does not carry — such
/// modules keep failing closed beside the establishment-less entrance. The
/// interpreter additionally checks each exclusive referent against the
/// invocation's structural arguments; this boundary admits no argument
/// channel, so establishments are the only live carriers to check.
pub(super) fn establish_native_placed_view_inputs(
    module: &TerminalModule,
    establishments: &[TerminalPlacedViewEstablishment],
) -> Result<Vec<TerminalPlacedViewEstablishment>, ArtifactLoweringError> {
    if module
        .placed_view_inputs
        .iter()
        .any(|row| row.machine != module.entry)
    {
        // A subordinate input is bound by its call, not by the executable's
        // host: this input boundary cannot carry it and there is nothing to
        // answer it with yet.
        return Err(ArtifactLoweringError::PlacedViewInputsRequireCustodyLowering);
    }
    let entry_rows = module
        .placed_view_inputs
        .iter()
        .filter(|row| row.machine == module.entry)
        .collect::<Vec<_>>();
    // Every supply must answer one declared entry row exactly; custody the
    // artifact never demanded cannot be bound at its input boundary.
    let mut seen = BTreeSet::new();
    for establishment in establishments {
        if !entry_rows.contains(&&establishment.input) {
            return Err(ArtifactLoweringError::PlacedViewEstablishmentUnexpected {
                machine: establishment.input.machine,
                position: establishment.input.position,
            });
        }
        if !seen.insert(&establishment.input) {
            return Err(ArtifactLoweringError::PlacedViewEstablishmentDuplicate {
                machine: establishment.input.machine,
                position: establishment.input.position,
            });
        }
        let referent = &establishment.referent;
        terminal_semantics::validate_placed_view_referent(
            module,
            referent.structural_type,
            &referent.path,
            &referent.qualifications,
        )
        .map_err(|error| match error {
            terminal_semantics::PlacedViewReferentError::QualificationsNonCanonical => {
                ArtifactLoweringError::PlacedViewEstablishmentQualificationsNonCanonical
            }
            terminal_semantics::PlacedViewReferentError::BackingUndeclared(carrier) => {
                ArtifactLoweringError::PlacedViewEstablishmentBackingUndeclared(carrier)
            }
            terminal_semantics::PlacedViewReferentError::RangeUnresolved => {
                ArtifactLoweringError::PlacedViewEstablishmentRangeUnresolved
            }
            terminal_semantics::PlacedViewReferentError::QualificationUndeclared(domain) => {
                ArtifactLoweringError::PlacedViewEstablishmentQualificationUndeclared(domain)
            }
            terminal_semantics::PlacedViewReferentError::QualificationCarrier(domain) => {
                ArtifactLoweringError::PlacedViewEstablishmentQualificationCarrier(domain)
            }
        })?;
    }
    let mut bound: Vec<(
        &terminal_psi::TerminalPlacedViewInput,
        &TerminalPlacedViewEstablishment,
    )> = Vec::with_capacity(entry_rows.len());
    for row in entry_rows {
        let Some(establishment) = establishments
            .iter()
            .find(|establishment| establishment.input == *row)
        else {
            return Err(ArtifactLoweringError::PlacedViewInputsRequireCustodyLowering);
        };
        for (previous_row, previous) in &bound {
            if (exclusive(row.access) || exclusive(previous_row.access))
                && referent_overlaps(&establishment.referent, &previous.referent)
            {
                return Err(ArtifactLoweringError::PlacedViewEstablishmentAliasing(
                    establishment.referent.opaque_identity,
                ));
            }
        }
        bound.push((row, establishment));
    }
    Ok(bound
        .into_iter()
        .map(|(_, establishment)| establishment.clone())
        .collect())
}
