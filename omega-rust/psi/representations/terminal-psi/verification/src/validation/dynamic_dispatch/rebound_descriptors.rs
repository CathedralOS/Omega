//! The rebound dynamic descriptors and the selections they consume.

use super::super::ModuleError;
use super::{dynamic_source_type_identity, invalid_descriptor};
use semantic_vocabulary::MachineId;
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{StructuralAccess, TerminalMachine, TerminalModule};

/// The rebound dynamic descriptors: coordinates are unique and ordered,
/// ordinals are dense per owner, and each descriptor rebinds a selection it
/// consumes to a known parameter.
pub(super) fn validate_rebound_descriptors(
    module: &TerminalModule,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    consumed_selections: &mut BTreeSet<(MachineId, u32)>,
) -> Result<(), ModuleError> {
    let selections = &module.dynamic_dispatch.selections;
    let descriptors = &module.dynamic_dispatch.rebound_descriptors;
    let mut descriptor_coordinates = BTreeSet::new();
    for descriptor in descriptors {
        if !descriptor_coordinates.insert((descriptor.owner, descriptor.ordinal)) {
            return Err(ModuleError::DuplicateReboundDynamicDescriptor {
                owner: descriptor.owner,
                ordinal: descriptor.ordinal,
            });
        }
    }
    if !descriptors
        .windows(2)
        .all(|pair| (pair[0].owner, pair[0].ordinal) < (pair[1].owner, pair[1].ordinal))
    {
        return Err(ModuleError::NonCanonicalReboundDynamicDescriptorOrder);
    }
    let mut expected_descriptor_ordinals = BTreeMap::<MachineId, u32>::new();
    for descriptor in descriptors {
        let expected = expected_descriptor_ordinals
            .entry(descriptor.owner)
            .or_default();
        if descriptor.ordinal != *expected {
            return Err(ModuleError::NonDenseReboundDynamicDescriptor {
                owner: descriptor.owner,
                expected: *expected,
                actual: descriptor.ordinal,
            });
        }
        *expected =
            expected
                .checked_add(1)
                .ok_or(ModuleError::InvalidReboundDynamicDescriptor {
                    owner: descriptor.owner,
                    ordinal: descriptor.ordinal,
                })?;
        let initial = selections
            .iter()
            .filter(|selection| {
                selection.owner == descriptor.owner
                    && selection.ordinal == descriptor.initial_selection_ordinal
            })
            .collect::<Vec<_>>();
        let rebound = selections
            .iter()
            .filter(|selection| {
                selection.owner == descriptor.owner
                    && selection.ordinal == descriptor.rebound_selection_ordinal
            })
            .collect::<Vec<_>>();
        let ([initial], [rebound]) = (initial.as_slice(), rebound.as_slice()) else {
            return Err(invalid_descriptor(descriptor.owner, descriptor.ordinal));
        };
        let initial_applications = module
            .closed_conformance_applications
            .iter()
            .filter(|application| {
                application.owner == initial.owner
                    && application.report_fingerprint
                        == initial.conformance_application_report_fingerprint
                    && application.commitment == initial.conformance_application_commitment
            })
            .collect::<Vec<_>>();
        let rebound_applications = module
            .closed_conformance_applications
            .iter()
            .filter(|application| {
                application.owner == rebound.owner
                    && application.report_fingerprint
                        == rebound.conformance_application_report_fingerprint
                    && application.commitment == rebound.conformance_application_commitment
            })
            .collect::<Vec<_>>();
        let ([initial_application], [rebound_application]) = (
            initial_applications.as_slice(),
            rebound_applications.as_slice(),
        ) else {
            return Err(invalid_descriptor(descriptor.owner, descriptor.ordinal));
        };
        let interfaces_match = initial_application.trait_identity
            == rebound_application.trait_identity
            && initial_application.trait_lifetime_arguments
                == rebound_application.trait_lifetime_arguments
            && initial_application.trait_arguments == rebound_application.trait_arguments
            && initial_application.telescope == rebound_application.telescope
            && initial_application.rows.len() == rebound_application.rows.len()
            && initial_application
                .rows
                .iter()
                .zip(&rebound_application.rows)
                .all(|(initial, rebound)| {
                    initial.declaring_trait_identity == rebound.declaring_trait_identity
                        && initial.public_requirement_identity
                            == rebound.public_requirement_identity
                        && initial.family_tuple == rebound.family_tuple
                        && initial.requirement_identity == rebound.requirement_identity
                });
        if descriptor.initial_selection_ordinal.checked_add(1)
            != Some(descriptor.rebound_selection_ordinal)
            || !interfaces_match
            || dynamic_source_type_identity(module, machines, initial)
                != dynamic_source_type_identity(module, machines, rebound)
            || initial.source.access != rebound.source.access
            || !matches!(
                initial.source.access,
                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
            )
        {
            return Err(invalid_descriptor(descriptor.owner, descriptor.ordinal));
        }
        consumed_selections.insert((descriptor.owner, descriptor.initial_selection_ordinal));
        consumed_selections.insert((descriptor.owner, descriptor.rebound_selection_ordinal));
    }
    Ok(())
}
