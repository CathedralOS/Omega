//! Establish whether the same package storage is still live between two uses.
use super::*;

// A saved descriptor may be reused across blocks or intervening writes when
// every incoming path retains this package's storage. Reuse the descriptor
// replay that already accounts for field overlap, call write frames and loans.
pub(super) fn same_storage_version(
    module: &TerminalModule,
    machine: &TerminalMachine,
    opened: &TerminalPackageOpening,
    projection: OperationId,
    call: OperationId,
    materialized: Option<semantic_vocabulary::PlaceId>,
) -> bool {
    use crate::descriptors::descriptor_lifetimes::descriptor_location;
    use crate::descriptors::descriptor_results::{
        DescriptorPayloadTarget, require_unchanged_observed_storage,
        terminal_borrowed_storage_graph_from,
    };
    let position = |id| {
        machine.blocks.iter().find_map(|block| {
            let index = block.operations.iter().position(|op| op.id == id)?;
            Some((block.id, u32::try_from(index).ok()?))
        })
    };
    let (Some(read), Some(use_site)) = (position(projection), position(call)) else {
        return false;
    };
    if read.0 == use_site.0 && read.1 < use_site.1 {
        let Some(block) = machine.block(&read.0) else {
            return false;
        };
        let between = &block.operations[read.1 as usize + 1..use_site.1 as usize];
        if between.iter().all(|operation| match &operation.kind {
            OperationKind::PrimitiveScalarRead { .. }
            | OperationKind::BooleanConstant { .. }
            | OperationKind::IntegerConstant { .. } => true,
            // Calling the stored machine on a fresh reborrow can mutate its
            // pointee, while neither stored field was given to the callee
            // for replacement. Other effects still require ordinary replay.
            OperationKind::CallMachine {
                contract,
                structural_arguments,
                ..
            } => module
                .execution
                .callables
                .contract(*contract)
                .is_some_and(|contract| {
                    let sites = &module.execution.packages.sites;
                    let same_opening = sites
                        .binary_search_by_key(&(machine.id, operation.id), |s| {
                            (s.machine, s.operation)
                        })
                        .ok()
                        .and_then(|index| match sites[index].event {
                            TerminalPackageEvent::Invoke { opening: index } => {
                                opening(module, machine.id, index).ok()
                            }
                            _ => None,
                        })
                        == Some(opened);
                    same_opening
                        && !structural_arguments.is_empty()
                        && contract.envelope.service_reach.is_empty()
                        && contract.envelope.invocations.is_empty()
                        && !contract.envelope.suspends
                        && !contract.envelope.blocks
                        && structural_arguments.len() == contract.structural_parameters.len()
                        && structural_arguments
                            .iter()
                            .zip(&contract.structural_parameters)
                            .all(|(arg, formal)| {
                                arg.access != StructuralAccess::Owned
                                    && formal.access == arg.access
                                    && opened.witnesses.contains(&formal.structural_type)
                                    && opening::subject_field(
                                        module, machine, operation, opened, arg,
                                    )
                                    .is_some()
                            })
                }),
            _ => false,
        }) {
            return true;
        }
    }
    let Ok(graph) = terminal_borrowed_storage_graph_from(module, [machine.id], &[]) else {
        return false;
    };
    let unchanged = |place, path: &[StructuralPathSegment]| {
        let Some(root) = machine.structural_place_type(place) else {
            return false;
        };
        let Some(location) =
            descriptor_location(&module.types.structural_types, place, root, path, None)
        else {
            return false;
        };
        require_unchanged_observed_storage(
            &module.types.structural_types,
            &graph,
            machine.id,
            read,
            use_site,
            &DescriptorPayloadTarget::current(location),
        )
        .is_ok()
    };
    unchanged(opened.subject.place, &opened.subject.path)
        && materialized.is_none_or(|place| unchanged(place, &[]))
}
