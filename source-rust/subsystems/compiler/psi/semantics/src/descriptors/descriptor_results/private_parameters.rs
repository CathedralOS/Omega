//! A scalar-only boundary observation can leave a caller's unexposed, owned input intact.
//! Other stores and other callers continue to use the ordinary conservative write frame.

use super::{DescriptorPosition, PlaceId};
use terminal_psi::{
    OperationKind, StructuralAccess, StructuralStorageUse, TerminalMachine, TerminalModule,
};

/// All approved observations share the machine's same unexposed input roots.
/// Both collections contain sorted declared identities, independent of their numbering.
#[derive(Default)]
pub struct PrivateObservationInputs {
    observations: Vec<DescriptorPosition>,
    roots: Vec<PlaceId>,
}

impl PrivateObservationInputs {
    pub(super) fn at(&self, position: DescriptorPosition) -> &[PlaceId] {
        if self.observations.binary_search(&position).is_ok() {
            &self.roots
        } else {
            &[]
        }
    }
}

pub(super) fn across_closed_observations(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> PrivateObservationInputs {
    let mut observations = Vec::new();
    for block in &machine.blocks {
        for (index, operation) in block.operations.iter().enumerate() {
            let OperationKind::BoundaryCall {
                boundary,
                arguments,
                structural_arguments,
                completion_receipts,
                ..
            } = &operation.kind
            else {
                continue;
            };
            let Some(contract) = module.boundary_machine(*boundary) else {
                continue;
            };
            if arguments.is_empty()
                && structural_arguments.is_empty()
                && completion_receipts.is_empty()
                && contract.parameter_order.is_empty()
                && contract.published_service_ceiling.is_empty()
                && contract.fixed_service_reach.is_empty()
                && contract.published_synchronous_entries.is_empty()
                && contract.attachment.is_none()
                && contract.reference_outputs.is_empty()
                && contract.content_guarantees.is_empty()
                && contract.program_local_root_introductions.is_empty()
                && contract.result_reference_regions.is_empty()
                && contract.declaration.as_ref().is_some_and(|decl| {
                    !decl.source.published_may_suspend && !decl.source.published_may_block
                })
                && matches!(
                    contract.result,
                    terminal_psi::BoundaryMachineResult::Unit
                        | terminal_psi::BoundaryMachineResult::Scalar(_)
                )
                && let Ok(index) = u32::try_from(index)
            {
                observations.push((block.id, index));
            }
        }
    }
    if observations.is_empty() {
        return PrivateObservationInputs::default();
    }
    let mut candidates: Vec<_> = machine
        .structural_parameters
        .iter()
        .filter(|formal| formal.access == StructuralAccess::Owned)
        .filter_map(|formal| {
            // Handles and views may reach mutable state outside this invocation.
            let ty = module.structural_type(formal.structural_type)?;
            let terminal_psi::StructuralTypeShape::Record { fields, .. } = &ty.shape else {
                return None;
            };
            fields
                .iter()
                .all(|field| {
                    matches!(
                        field.field_type,
                        terminal_psi::StructuralFieldType::Scalar(_)
                    )
                })
                .then_some((formal.place, true))
        })
        .collect();
    candidates.sort_unstable();
    candidates.dedup();
    let mut expose = |place: PlaceId| {
        if let Ok(index) = candidates.binary_search_by_key(&place, |(place, _)| *place) {
            candidates[index].1 = false;
        }
    };
    for block in &machine.blocks {
        for operation in &block.operations {
            if let OperationKind::MoveStructuralValue { source } = &operation.kind
                && source.access == StructuralAccess::Owned
                && source.path.is_empty()
            {
                // Transferring the complete value cannot expose its previous home.
                continue;
            }
            for (storage, _) in operation.kind.storage_uses() {
                let root = match storage {
                    StructuralStorageUse::Type(_) => continue,
                    StructuralStorageUse::Place(root)
                    | StructuralStorageUse::Canonical { root, .. }
                    | StructuralStorageUse::ObservedField { root, .. } => root,
                    StructuralStorageUse::Projection(projected)
                    | StructuralStorageUse::Element {
                        descriptor: projected,
                        ..
                    } => projected.root,
                };
                expose(root);
            }
        }
        for projection in block.terminator.storage_uses() {
            expose(projection.root);
        }
    }
    let roots: Vec<_> = candidates
        .into_iter()
        .filter_map(|(place, private)| private.then_some(place))
        .collect();
    if roots.is_empty() {
        return PrivateObservationInputs::default();
    }
    observations.sort_unstable();
    observations.dedup();
    PrivateObservationInputs {
        observations,
        roots,
    }
}
