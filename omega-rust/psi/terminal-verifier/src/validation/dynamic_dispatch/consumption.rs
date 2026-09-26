//! Every dynamic site is consumed exactly by the dispatch or descriptor
//! that names it.

use super::super::ModuleError;
use super::invalid_indirect_dispatch;
use semantic_vocabulary::{MachineId, OperationId};
use std::collections::BTreeSet;
use terminal_psi::{OperationKind, TerminalModule};

/// Every rebound and stored descriptor is consumed by a dispatch, every
/// dynamic call and descriptor store has its dispatch or descriptor, and
/// every selection is consumed.
pub(super) fn require_every_dynamic_site_consumed(
    module: &TerminalModule,
    consumed_selections: &BTreeSet<(MachineId, u32)>,
    indirect_coordinates: &BTreeSet<(MachineId, OperationId)>,
    stored_dispatch_coordinates: &BTreeSet<(MachineId, OperationId)>,
    consumed_descriptors: &BTreeSet<(MachineId, u32)>,
    consumed_stored_descriptors: &BTreeSet<(MachineId, u32)>,
) -> Result<(), ModuleError> {
    let descriptors = &module.dynamic_dispatch.rebound_descriptors;
    let selections = &module.dynamic_dispatch.selections;
    for descriptor in descriptors {
        if !consumed_descriptors.contains(&(descriptor.owner, descriptor.ordinal)) {
            return Err(ModuleError::OrphanReboundDynamicDescriptor {
                owner: descriptor.owner,
                ordinal: descriptor.ordinal,
            });
        }
    }
    for descriptor in &module.dynamic_dispatch.stored_descriptors {
        if !consumed_stored_descriptors.contains(&(descriptor.owner, descriptor.ordinal)) {
            return Err(ModuleError::OrphanStoredDynamicDescriptor {
                owner: descriptor.owner,
                ordinal: descriptor.ordinal,
            });
        }
    }
    for machine in module.machines.iter() {
        for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
            if matches!(
                operation.kind,
                OperationKind::CallDynamicScalar { .. } | OperationKind::CallDynamicUnit { .. }
            ) && !indirect_coordinates.contains(&(machine.id, operation.id))
                && !stored_dispatch_coordinates.contains(&(machine.id, operation.id))
            {
                return Err(invalid_indirect_dispatch(machine.id, operation.id));
            }
            if matches!(operation.kind, OperationKind::StoreDynamicDescriptor { .. })
                && !module
                    .dynamic_dispatch
                    .stored_descriptors
                    .iter()
                    .any(|descriptor| {
                        descriptor.owner == machine.id
                            && descriptor.establishment_operation == operation.id
                    })
            {
                return Err(ModuleError::InvalidStoredDynamicDescriptor {
                    owner: machine.id,
                    ordinal: match operation.kind {
                        OperationKind::StoreDynamicDescriptor { descriptor_ordinal } => {
                            descriptor_ordinal
                        }
                        _ => unreachable!(),
                    },
                });
            }
        }
    }
    for selection in selections {
        if !consumed_selections.contains(&(selection.owner, selection.ordinal)) {
            return Err(ModuleError::OrphanDynamicConformanceSelection {
                owner: selection.owner,
                ordinal: selection.ordinal,
            });
        }
    }
    Ok(())
}
