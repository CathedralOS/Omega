//! Frame-row geometry: the projections every reader of a laid-out row resolves
//! here instead of restating. Each reads the row's own recorded authority — the
//! post-allocation instruction stream, the custody decision, the storage slots
//! — so the independent replay stays independent of the producer while both
//! recover the same value from the same evidence.

use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId, MachineAlternativeFamily};

use super::{
    CalleeSaveFrameSlot, FrameUnwindRestore, FunctionTargetFrameLayout, ReturnAddressFrameCustody,
};

/// Whether the body executes any call, including a normalized foreign call.
pub(super) fn contains_call(
    function: &physical_instructions::PostAllocationMachineFunction,
) -> bool {
    function.instructions().any(|instruction| {
        matches!(
            instruction.alternative.key.family,
            MachineAlternativeFamily::CallScalar
                | MachineAlternativeFamily::InvokeNoNormal
                | MachineAlternativeFamily::CallUnit
                | MachineAlternativeFamily::CallAggregate
                | MachineAlternativeFamily::NormalizedForeignCall
        )
    })
}

/// The activation-local slots whose stable addresses the body materializes, in
/// canonical ascending order without duplicates.
///
/// A frame-address materialization on an activation-local slot loans that
/// slot's stable coordinate to whatever observes the pointer. Allocator spill
/// slots only gain the private reload materializations allocation inserts,
/// which are consumed inside the same window, so they are never loans. The
/// roster is closed over declared storage: an address on an undeclared slot is
/// admitted by neither reader.
pub(super) fn stable_address_loans(
    function: &physical_instructions::PostAllocationMachineFunction,
) -> Vec<LocalStorageSlotId> {
    function
        .instructions()
        .filter_map(|instruction| match instruction.address {
            Some(physical_instructions::PhysicalAddressOperation::FrameAddress {
                slot: FrameStorageSlotId::Local(slot),
                ..
            }) if !matches!(slot, LocalStorageSlotId::Spill { .. }) => Some(slot),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// The epilogue's restoration roster: a saved link register sits at the highest
/// frame coordinate and is restored first, then preservation slots in reverse
/// save order, so roster order is strictly descending frame offset.
pub(super) fn unwind_restores(
    return_address: ReturnAddressFrameCustody,
    callee_save_slots: &[CalleeSaveFrameSlot],
) -> Vec<FrameUnwindRestore> {
    let mut restores = Vec::with_capacity(callee_save_slots.len() + 1);
    if let ReturnAddressFrameCustody::SavedLinkRegister {
        view,
        frame_offset_bytes,
        size_bytes,
    } = return_address
    {
        restores.push(FrameUnwindRestore {
            view,
            frame_offset_bytes,
            size_bytes: u64::from(size_bytes),
        });
    }
    restores.extend(
        callee_save_slots
            .iter()
            .rev()
            .map(|slot| FrameUnwindRestore {
                view: slot.storage_view,
                frame_offset_bytes: slot.frame_offset_bytes,
                size_bytes: slot.size_bytes,
            }),
    );
    restores
}

/// The committed extent of one laid-out row: `frame_size_bytes` minus the
/// red-zone-resident bytes, admitted only for a canonical residency shape.
/// Either nothing is resident, or the whole addressed extent is and the row is
/// a leaf with no preservation storage and only allocator spill homes.
pub(crate) fn committed_extent(row: &FunctionTargetFrameLayout) -> Option<u64> {
    let resident = row.red_zone_resident_bytes;
    let committed = row.frame_size_bytes.checked_sub(resident)?;
    if resident == 0 {
        return Some(committed);
    }
    (resident == row.frame_size_bytes
        && !row.contains_call
        && row.callee_save_slots.is_empty()
        && row
            .local_storage_slots
            .iter()
            .all(|slot| matches!(slot.id, LocalStorageSlotId::Spill { .. })))
    .then_some(committed)
}
