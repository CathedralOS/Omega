//! The module's dynamic conformance selections.

use super::super::ModuleError;
use semantic_vocabulary::MachineId;
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{TerminalMachine, TerminalModule};

/// The module's dynamic conformance selections: coordinates are unique,
/// rows are canonically ordered, ordinals are dense per owner, and each
/// selection names a known machine and conformance.
pub(super) fn validate_conformance_selections(
    module: &TerminalModule,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
) -> Result<(), ModuleError> {
    let selections = &module.dynamic_dispatch.selections;
    let mut selection_coordinates = BTreeSet::new();
    for selection in selections {
        if !selection_coordinates.insert((selection.owner, selection.ordinal)) {
            return Err(ModuleError::DuplicateDynamicConformanceSelection {
                owner: selection.owner,
                ordinal: selection.ordinal,
            });
        }
    }
    if !selections
        .windows(2)
        .all(|pair| (pair[0].owner, pair[0].ordinal) < (pair[1].owner, pair[1].ordinal))
    {
        return Err(ModuleError::NonCanonicalDynamicConformanceSelectionOrder);
    }
    let mut expected_ordinals = BTreeMap::<MachineId, u32>::new();
    for selection in selections {
        let expected = expected_ordinals.entry(selection.owner).or_default();
        if selection.ordinal != *expected {
            return Err(ModuleError::NonDenseDynamicConformanceSelection {
                owner: selection.owner,
                expected: *expected,
                actual: selection.ordinal,
            });
        }
        *expected =
            expected
                .checked_add(1)
                .ok_or(ModuleError::InvalidDynamicConformanceSelection {
                    owner: selection.owner,
                    ordinal: selection.ordinal,
                })?;
        let application_count = module
            .closed_conformance_applications
            .iter()
            .filter(|application| {
                application.owner == selection.owner
                    && application.report_fingerprint
                        == selection.conformance_application_report_fingerprint
                    && application.commitment == selection.conformance_application_commitment
            })
            .count();
        if !machines.contains_key(&selection.owner)
            || selection.conformance_application_report_fingerprint == 0
            || selection.conformance_application_commitment.is_zero()
            || application_count != 1
        {
            return Err(ModuleError::InvalidDynamicConformanceSelection {
                owner: selection.owner,
                ordinal: selection.ordinal,
            });
        }
    }
    Ok(())
}
