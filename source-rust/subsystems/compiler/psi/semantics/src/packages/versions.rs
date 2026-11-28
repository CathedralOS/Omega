//! Establish whether the same package storage is still live between two uses.
use super::*;

// A saved descriptor may be reused across blocks or intervening writes when
// every incoming path retains this package's storage. Reuse the descriptor
// replay that already accounts for field overlap, call write frames and loans.
pub(super) fn same_storage_version(
    module: &TerminalModule,
    machine: &TerminalMachine,
    subject: &StructuralArgument,
    projection: OperationId,
    call: OperationId,
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
    if read.0 == use_site.0 && read.1.checked_add(1) == Some(use_site.1) {
        return true;
    }
    let Some(root) = machine.structural_place_type(subject.place) else {
        return false;
    };
    let Some(location) = descriptor_location(
        &module.types.structural_types,
        subject.place,
        root,
        &subject.path,
        None,
    ) else {
        return false;
    };
    let Ok(graph) = terminal_borrowed_storage_graph_from(module, [machine.id], &[]) else {
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
}
