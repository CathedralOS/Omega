//! Established placed-view inputs at the interpretation boundary.
//!
//! A `Placed<P, T>` direct-entry input is custody, not data: the roster row
//! seals the placement the view interprets, and no scalar, structural, or
//! byte-sequence argument channel can lend the referent it names. The ordinary
//! establishment route supplies one [`TerminalPlacedViewEstablishment`] per
//! direct-entry roster row — the provider's loan of the exact qualified
//! backing — and interpretation binds it as a live occurrence for the entry
//! invocation's duration. A roster or a pointer alone is not this authority:
//! an establishment that does not exactly rejoin a declared row is rejected
//! rather than answered, and an entry row left without one still fails
//! custody.

use crate::terminal_interpreter::errors::TerminalInterpretError;
use crate::terminal_interpreter::values::{StructuralRuntimePlace, TerminalStructuralValue};
use semantic_vocabulary::{StructuralDomainId, StructuralTypeId};
use std::collections::BTreeMap;
use terminal_psi::{StructuralAccess, TerminalModule, TerminalPlacedViewInput};

/// One established placed-view input supplied beside the ordinary structural
/// arguments.
///
/// `input` must equal its entry roster row exactly — machine, position, every
/// declaration identity, access and binding flags, and both sealed placement
/// identities — so a stale or substituted row cannot answer a different
/// declared input. `referent` is the qualified backing carrier the provider
/// lends for the invocation's duration: the borrowed establishment's parent
/// claim. Qualification IDs are semantic runtime facts supplied by the root
/// installation, the same as on structural arguments; Psi treats the carrier
/// as an opaque occurrence root, never an address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalPlacedViewEstablishment {
    pub input: TerminalPlacedViewInput,
    pub referent: TerminalStructuralValue,
}

/// A live placed-view occurrence: the referent a direct-entry roster row
/// names, bound for the entry invocation's duration under the row's sealed
/// custody. The occurrence records the qualified backing the provider lent;
/// retirement at entry completion releases the loan without reminting
/// custody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlacedViewOccurrence {
    /// Runtime place of the lent referent backing.
    pub(crate) referent: StructuralRuntimePlace,
    /// The referent's declared structural type identity.
    pub(crate) structural_type: StructuralTypeId,
    /// Domain qualifications the root installation asserts on the backing.
    pub(crate) qualifications: Vec<StructuralDomainId>,
}

const fn exclusive(access: StructuralAccess) -> bool {
    matches!(
        access,
        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
    )
}

fn overlaps(left: &StructuralRuntimePlace, right: &StructuralRuntimePlace) -> bool {
    left.opaque_identity == right.opaque_identity
        && (left.path.starts_with(&right.path) || right.path.starts_with(&left.path))
}

/// Bind each direct-entry placed-view input to one supplied establishment.
///
/// Only rows on the module's entry machine are entry inputs; a roster row on
/// any other machine declares a call-bound input whose custody arrives through
/// the caller, a route interpretation does not yet route — such modules keep
/// failing closed. Every entry row must rejoin exactly one establishment; a
/// supply naming no declared entry row, or overlapping another exclusive
/// referent or a structural argument, rejects before the entry machine runs.
pub(super) fn establish_placed_view_inputs(
    module: &TerminalModule,
    establishments: &[TerminalPlacedViewEstablishment],
    structural_arguments: &[TerminalStructuralValue],
) -> Result<BTreeMap<TerminalPlacedViewInput, PlacedViewOccurrence>, TerminalInterpretError> {
    let entry_rows = module
        .placed_view_inputs
        .iter()
        .filter(|row| row.machine == module.entry)
        .collect::<Vec<_>>();
    if module
        .placed_view_inputs
        .iter()
        .any(|row| row.machine != module.entry)
    {
        // A subordinate input is bound by its call, not by the host: this
        // input boundary cannot carry it and there is nothing to answer it
        // with yet.
        return Err(TerminalInterpretError::PlacedViewInputsRequireCustody);
    }
    // Every supply must answer one declared entry row exactly; custody the
    // artifact never demanded cannot be bound at its input boundary.
    let mut seen = std::collections::BTreeSet::new();
    for establishment in establishments {
        if !entry_rows.contains(&&establishment.input) {
            return Err(
                TerminalInterpretError::PlacedViewInputEstablishmentUnexpected {
                    machine: establishment.input.machine,
                    position: establishment.input.position,
                },
            );
        }
        if !seen.insert(&establishment.input) {
            return Err(
                TerminalInterpretError::PlacedViewInputEstablishmentDuplicate {
                    machine: establishment.input.machine,
                    position: establishment.input.position,
                },
            );
        }
        if establishment
            .referent
            .qualifications
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(TerminalInterpretError::StructuralQualificationsNonCanonical);
        }
    }
    let mut occurrences: BTreeMap<TerminalPlacedViewInput, PlacedViewOccurrence> = BTreeMap::new();
    for row in entry_rows {
        let Some(establishment) = establishments
            .iter()
            .find(|establishment| establishment.input == *row)
        else {
            return Err(TerminalInterpretError::PlacedViewInputsRequireCustody);
        };
        let referent = StructuralRuntimePlace::from(&establishment.referent);
        for (previous_row, previous) in &occurrences {
            if (exclusive(row.access) || exclusive(previous_row.access))
                && overlaps(&referent, &previous.referent)
            {
                return Err(
                    TerminalInterpretError::PlacedViewInputEstablishmentAliasing(
                        establishment.referent.opaque_identity,
                    ),
                );
            }
        }
        if exclusive(row.access)
            && structural_arguments
                .iter()
                .map(StructuralRuntimePlace::from)
                .any(|argument| overlaps(&referent, &argument))
        {
            return Err(
                TerminalInterpretError::PlacedViewInputEstablishmentAliasing(
                    establishment.referent.opaque_identity,
                ),
            );
        }
        if occurrences
            .insert(
                row.clone(),
                PlacedViewOccurrence {
                    referent,
                    structural_type: establishment.referent.structural_type,
                    qualifications: establishment.referent.qualifications.clone(),
                },
            )
            .is_some()
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
    }
    Ok(occurrences)
}
