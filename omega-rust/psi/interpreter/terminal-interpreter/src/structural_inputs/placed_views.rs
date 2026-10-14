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
//! custody. Ordinary helpers reborrow that live occurrence through their exact
//! retained formal; suspended callers keep the original backing pinned until
//! return. No helper creates a second host establishment or remints authority.

use crate::errors::{TerminalInterpretError, malformed};
use crate::values::{StructuralRuntimePlace, TerminalStructuralValue};
use semantic_vocabulary::{MachineId, StructuralDomainId, StructuralTypeId};
use std::collections::BTreeMap;
use terminal_psi::{StructuralAccess, TerminalModule, TerminalPlacedViewInput};

pub use terminal_installation::TerminalPlacedViewEstablishment;

/// A live placed-view presentation under a retained formal's placement seal.
/// Entry lends the original qualified backing; helper frames reborrow that same
/// occurrence and return it to the suspended caller without reminting custody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlacedViewOccurrence {
    /// A checked local RAM borrow stays distinct from a host establishment.
    pub(crate) ram: Option<crate::reference::PlacedRamReference>,
    /// Original view carrier presented by the current frame.
    pub(crate) view: StructuralRuntimePlace,
    /// Runtime place of the lent referent backing.
    pub(crate) referent: StructuralRuntimePlace,
    /// The referent's declared structural type identity.
    pub(crate) structural_type: StructuralTypeId,
    /// Domain qualifications the root installation asserts on the backing.
    pub(crate) qualifications: Vec<StructuralDomainId>,
}

const fn exclusive(access: StructuralAccess) -> bool {
    matches!(access, terminal_psi::exclusive_borrow_pair_pattern!())
}

fn overlaps(left: &StructuralRuntimePlace, right: &StructuralRuntimePlace) -> bool {
    left.opaque_identity == right.opaque_identity
        && (left.path.starts_with(&right.path) || right.path.starts_with(&left.path))
}

/// Resolve the retained formal coordinate against the machine's exact initial
/// state. Ordinary block transport carries later states under the same loan.
fn input_parameter<'module>(
    module: &'module TerminalModule,
    row: &TerminalPlacedViewInput,
) -> Result<&'module terminal_psi::StructuralParameterDeclaration, TerminalInterpretError> {
    terminal_semantics::placed_view_input_parameter(module, row)
        .map_err(TerminalInterpretError::PlacedViewInput)
}

/// Bind only the installed entry's actual ingress, before effects. The view
/// parameter is the presentation of this same loan, not a second borrowing of
/// its backing; every other ordinary argument participates in alias checking.
pub(crate) fn establish_placed_view_inputs(
    module: &TerminalModule,
    entry: MachineId,
    establishments: &[TerminalPlacedViewEstablishment],
    structural_arguments: &[TerminalStructuralValue],
) -> Result<BTreeMap<TerminalPlacedViewInput, PlacedViewOccurrence>, TerminalInterpretError> {
    let entry_rows = module
        .placed_view_inputs
        .iter()
        .filter(|row| row.machine == entry)
        .map(|row| input_parameter(module, row).map(|_| row))
        .collect::<Result<Vec<_>, _>>()?;
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
        let referent = &establishment.referent;
        // Artifact verification cannot validate freshly supplied runtime backing.
        // Rejoin it before creating any invocation-scoped occurrence, using the
        // same declaration relation as native input admission.
        terminal_semantics::validate_placed_view_referent(
            module,
            referent.structural_type,
            &referent.path,
            &referent.qualifications,
        )
        .map_err(|error| match error {
            terminal_semantics::PlacedViewReferentError::QualificationsNonCanonical => {
                TerminalInterpretError::StructuralQualificationsNonCanonical
            }
            error => TerminalInterpretError::PlacedViewReferent(error),
        })?;
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
        let parameter = input_parameter(module, row)?;
        let view = structural_arguments
            .get(row.runtime_position as usize)
            .filter(|view| view.structural_type == parameter.structural_type)
            .ok_or_else(malformed)?;
        // One descriptor occurrence cannot designate competing original backings.
        // Shared presentations may repeat only the same complete association.
        if occurrences.iter().any(|(previous_row, previous)| {
            previous.view == StructuralRuntimePlace::from(view)
                && (previous.referent != referent
                    || previous.structural_type != establishment.referent.structural_type
                    || previous.qualifications != establishment.referent.qualifications
                    || previous_row.view_identity != row.view_identity
                    || previous_row.policy_identity != row.policy_identity
                    || previous_row.policy_plan_machine_identity
                        != row.policy_plan_machine_identity
                    || previous_row.schema_identity != row.schema_identity
                    || previous_row.placement_commitment != row.placement_commitment
                    || previous_row.placement_report_fingerprint
                        != row.placement_report_fingerprint)
        }) {
            return Err(
                TerminalInterpretError::PlacedViewInputEstablishmentAliasing(view.opaque_identity),
            );
        }
        if structural_arguments
            .iter()
            .zip(
                &module
                    .machine(entry)
                    .ok_or_else(malformed)?
                    .structural_parameters,
            )
            .enumerate()
            .filter(|(position, _)| *position != row.runtime_position as usize)
            .any(|(_, (argument, formal))| {
                (exclusive(row.access)
                    || exclusive(formal.access)
                    || formal.access == StructuralAccess::Owned)
                    && overlaps(&referent, &StructuralRuntimePlace::from(argument))
            })
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
                    ram: None,
                    view: StructuralRuntimePlace::from(view),
                    referent,
                    structural_type: establishment.referent.structural_type,
                    qualifications: establishment.referent.qualifications.clone(),
                },
            )
            .is_some()
        {
            return Err(malformed());
        }
    }
    Ok(occurrences)
}

impl crate::execution::TerminalExecution<'_> {
    /// An ordinary call reborrows an existing established occurrence. Formal
    /// type equality alone cannot supply its backing: the resolved operand must
    /// name the caller's live view and the complete placement seal must agree.
    pub(crate) fn bind_call_placed_views(
        &self,
        callee: MachineId,
        arguments: &[terminal_psi::StructuralArgument],
        values: &BTreeMap<semantic_vocabulary::PlaceId, TerminalStructuralValue>,
    ) -> Result<BTreeMap<TerminalPlacedViewInput, PlacedViewOccurrence>, TerminalInterpretError>
    {
        let module = &self.program.catalogs;
        let mut bound: BTreeMap<TerminalPlacedViewInput, PlacedViewOccurrence> = BTreeMap::new();
        for row in module
            .placed_view_inputs
            .iter()
            .filter(|row| row.machine == callee)
        {
            let parameter = input_parameter(module, row)?;
            let argument = arguments
                .get(row.runtime_position as usize)
                .ok_or_else(malformed)?;
            let (resolved, ram) = self.resolve_live_placed_projection(argument)?;
            let presented = values.get(&parameter.place).ok_or_else(malformed)?;
            if argument.access != row.access
                || presented.structural_type != parameter.structural_type
            {
                return Err(malformed());
            }
            let occurrence = if let Some(mut ram) = ram {
                self.validate_placed_ram_home(&ram)?;
                let plan = module
                    .placed_view_plans
                    .iter()
                    .find(|plan| plan.carrier == ram.correspondence.carrier)
                    .ok_or_else(malformed)?;
                if ram.selected_field.is_some()
                    || ram.presentation != resolved
                    || ram.correspondence.carrier != parameter.structural_type
                    || !crate::reference::access_covers(ram.access, row.access)
                    || (
                        row.view_identity,
                        row.policy_identity,
                        row.policy_plan_machine_identity,
                        row.schema_identity,
                        row.placement_commitment,
                        row.placement_report_fingerprint,
                    ) != (
                        plan.view_identity,
                        plan.policy_identity,
                        plan.policy_plan_machine_identity,
                        plan.schema_identity,
                        plan.placement.content_interpretation().commitment(),
                        plan.placement.identity().compatibility_fingerprint(),
                    )
                {
                    return Err(malformed());
                }
                ram.access = row.access;
                ram.presentation = presented.clone();
                PlacedViewOccurrence {
                    view: StructuralRuntimePlace::from(presented),
                    referent: StructuralRuntimePlace::from(&ram.source),
                    structural_type: ram.source.structural_type,
                    qualifications: ram.source.qualifications.clone(),
                    ram: Some(ram),
                }
            } else {
                let source = self
                    .placed_view_occurrences
                    .iter()
                    .find(|(source, occurrence)| {
                        occurrence.view == StructuralRuntimePlace::from(&resolved)
                            && source.view_identity == row.view_identity
                            && source.policy_identity == row.policy_identity
                            && source.policy_plan_machine_identity
                                == row.policy_plan_machine_identity
                            && source.schema_identity == row.schema_identity
                            && source.placement_commitment == row.placement_commitment
                            && source.placement_report_fingerprint
                                == row.placement_report_fingerprint
                            && match (source.access, row.access) {
                                (
                                    StructuralAccess::MutableBorrow,
                                    StructuralAccess::SharedBorrow
                                    | StructuralAccess::MutableBorrow
                                    | StructuralAccess::WriteOnlyBorrow,
                                ) => true,
                                (left, right) => left == right,
                            }
                    })
                    .ok_or(TerminalInterpretError::PlacedViewInputsRequireCustody)?;
                let mut occurrence = source.1.clone();
                occurrence.view = StructuralRuntimePlace::from(presented);
                if let Some(ram) = &mut occurrence.ram {
                    self.validate_placed_ram_home(ram)?;
                    if !crate::reference::access_covers(ram.access, row.access) {
                        return Err(malformed());
                    }
                    ram.access = row.access;
                    ram.presentation = presented.clone();
                }
                occurrence
            };
            for (other_row, other) in &bound {
                if (exclusive(row.access) || exclusive(other_row.access))
                    && overlaps(&occurrence.referent, &other.referent)
                {
                    return Err(
                        TerminalInterpretError::PlacedViewInputEstablishmentAliasing(
                            occurrence.referent.opaque_identity,
                        ),
                    );
                }
            }
            if values.iter().any(|(place, value)| {
                *place != parameter.place
                    && module.machine(callee).is_some_and(|machine| {
                        machine
                            .structural_parameters
                            .iter()
                            .find(|formal| formal.place == *place)
                            .is_some_and(|formal| {
                                (exclusive(row.access)
                                    || exclusive(formal.access)
                                    || formal.access == StructuralAccess::Owned)
                                    && overlaps(
                                        &occurrence.referent,
                                        &StructuralRuntimePlace::from(value),
                                    )
                            })
                    })
            }) {
                return Err(
                    TerminalInterpretError::PlacedViewInputEstablishmentAliasing(
                        occurrence.referent.opaque_identity,
                    ),
                );
            }
            bound.insert(row.clone(), occurrence);
        }
        Ok(bound)
    }
}
