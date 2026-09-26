//! Check submitted geometry without constructing a replacement frame plan.

use selected_instructions::MachineAlternativeFamily;

use crate::frame_layout::{
    FrameAbiPreservationConvention, FrameContinuationCustody, FrameUnwindRestore,
    ReturnAddressFrameCustody, StagedOptimizedPostAllocationMachinePlan,
    TargetFrameLayoutError as Error, TargetFrameLayoutPlan, TargetFrameLayoutPolicy,
    ValidatedAllocatedCalleeSavedRequirements, ValidatedNonAuthoritativeCalleeSaveStorage,
    ValidatedTargetRegisterEnvironment, call_site::call_site_stack_contract,
    stack_commit::stack_commit_granule_bytes, unwind::frame_unwind_policy,
};

pub(super) fn validate_layout(
    machine: &StagedOptimizedPostAllocationMachinePlan,
    requirements: &ValidatedAllocatedCalleeSavedRequirements,
    storage: &ValidatedNonAuthoritativeCalleeSaveStorage,
    environment: &ValidatedTargetRegisterEnvironment,
    candidate: &TargetFrameLayoutPlan,
) -> Result<(), Error> {
    let current = machine.machine().plan();
    let required = requirements.plan();
    let saved = storage.plan();
    if current.target != environment.target()
        || current.register_environment != environment.identity()
        || current.physical_register_model != environment.physical().identity()
        || required.selected != current.selected
        || required.homes != current.homes
        || required.post_allocation_manifest != current.post_allocation_manifest
        || required.register_environment != environment.identity()
        || required.physical_register_model != environment.physical().identity()
        || required.target != environment.target()
        || saved.callee_saved_requirements != requirements.receipt().identity()
        || saved.register_environment != environment.identity()
        || saved.physical_register_model != environment.physical().identity()
        || saved.target != environment.target()
        || saved.abi != required.abi
    {
        return Err(Error::RootMismatch);
    }

    if current.functions.len() != required.functions.len()
        || current.functions.len() != saved.functions.len()
    {
        return Err(Error::FunctionRosterMismatch);
    }
    if candidate.functions.len() != current.functions.len() {
        return Err(Error::NonCanonicalLayout);
    }
    // The call-site stack contract resolves from the same target-owned
    // declarations the producer used — the selected convention's declared
    // stack alignment and the (architecture, convention) call-entry residue —
    // so a submitted row cannot record a borrowed constant or an undeclared
    // pair's geometry.
    let call_site =
        call_site_stack_contract(environment, required.abi).ok_or(Error::UnsupportedTarget)?;
    for (((source, requirement), storage), row) in current
        .functions
        .iter()
        .zip(&required.functions)
        .zip(&saved.functions)
        .zip(&candidate.functions)
    {
        if source.machine != requirement.machine || source.machine != storage.machine {
            return Err(Error::FunctionRosterMismatch);
        }
        let calls = source
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| {
                matches!(
                    instruction.alternative.key.family,
                    MachineAlternativeFamily::CallScalar
                        | MachineAlternativeFamily::CallUnit
                        | MachineAlternativeFamily::CallAggregate
                        | MachineAlternativeFamily::NormalizedForeignCall
                )
            });
        let shadow = if calls && required.abi == FrameAbiPreservationConvention::MicrosoftX64 {
            32_u64
        } else {
            0
        };
        let mut outgoing = shadow;
        for (index, slot) in source.outgoing_arguments.iter().enumerate() {
            let end = u64::from(slot.abi_stack_byte_offset) + u64::from(slot.byte_size);
            if !calls
                || slot.byte_size == 0
                || !slot.alignment.is_power_of_two()
                || u64::from(slot.abi_stack_byte_offset) < shadow
                || !slot
                    .abi_stack_byte_offset
                    .is_multiple_of(u32::from(slot.alignment))
                || end > u64::from(u32::MAX)
                || source.outgoing_arguments[..index].iter().any(|earlier| {
                    earlier.id == slot.id
                        || (earlier.id.operation == slot.id.operation
                            && u64::from(earlier.abi_stack_byte_offset) < end
                            && u64::from(slot.abi_stack_byte_offset)
                                < u64::from(earlier.abi_stack_byte_offset)
                                    + u64::from(earlier.byte_size))
                })
            {
                return Err(Error::NonCanonicalLayout);
            }
            outgoing = outgoing.max(end);
        }
        if row.local_storage_slots.len() != source.local_storage_slots.len() {
            return Err(Error::NonCanonicalLayout);
        }
        let mut local_extent = outgoing;
        for (index, (local, placed)) in source
            .local_storage_slots
            .iter()
            .zip(&row.local_storage_slots)
            .enumerate()
        {
            if local.byte_size == 0
                || !local.alignment.is_power_of_two()
                || local.alignment > 8
                || source.local_storage_slots[..index]
                    .iter()
                    .any(|earlier| earlier.id == local.id)
                || placed.id != local.id
                || placed.size_bytes != local.byte_size
                || placed.alignment_bytes != local.alignment
                || !minimal_aligned_extent(
                    local_extent,
                    placed.frame_offset_bytes,
                    u64::from(local.alignment),
                    0,
                )
            {
                return Err(Error::NonCanonicalLayout);
            }
            local_extent = placed
                .frame_offset_bytes
                .checked_add(u64::from(placed.size_bytes))
                .ok_or(Error::GeometryOverflow)?;
        }
        // The stable-address-loan roster is recovered from the physical
        // address operations themselves, never from the producer's claims:
        // a materialized frame address on an activation-local non-spill
        // slot loans that coordinate to the pointer's observers, and no
        // other access does. Allocator spill materializations are private
        // reload windows consumed inside the same window, so they cannot
        // appear. The canonical roster is ascending and duplicate-free,
        // which exact equality with the recovered set enforces.
        let stable_address_loans = source
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match instruction.address {
                Some(physical_instructions::PhysicalAddressOperation::FrameAddress {
                    slot: selected_instructions::FrameStorageSlotId::Local(slot),
                    ..
                }) if !matches!(
                    slot,
                    selected_instructions::LocalStorageSlotId::Spill { .. }
                ) =>
                {
                    Some(slot)
                }
                _ => None,
            })
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if row.stable_address_loans != stable_address_loans
            || stable_address_loans
                .iter()
                .any(|loan| !row.local_storage_slots.iter().any(|slot| slot.id == *loan))
        {
            return Err(Error::NonCanonicalLayout);
        }
        let preservation_offset =
            if source.local_storage_slots.is_empty() && storage.slots.is_empty() {
                outgoing
            } else {
                let alignment = storage
                    .slots
                    .iter()
                    .map(|slot| slot.alignment_bytes)
                    .max()
                    .unwrap_or(8)
                    .max(8);
                if !alignment.is_power_of_two() {
                    return Err(Error::NonCanonicalLayout);
                }
                local_extent
                    .checked_add((alignment - local_extent % alignment) % alignment)
                    .ok_or(Error::GeometryOverflow)?
            };
        let area = storage
            .abstract_area_bytes
            .checked_add(preservation_offset)
            .ok_or(Error::GeometryOverflow)?;
        // A red-zone-resident frame is canonical only as a whole: either no
        // bytes live below the unadjusted stack pointer, or the entire
        // addressed extent does and nothing is committed. Residency also
        // requires the leaf System V form, no preservation storage, and only
        // allocator spill slots, so every below-RSP access still resolves
        // through signed frame displacements.
        let resident = row.red_zone_resident_bytes;
        let committed = row
            .frame_size_bytes
            .checked_sub(resident)
            .filter(|_| resident == 0 || resident == row.frame_size_bytes)
            .ok_or(Error::NonCanonicalLayout)?;
        if resident != 0 {
            let capacity = u64::from(
                register_environment::selected_abi_preservation(environment)
                    .map_err(|_| Error::UnsupportedTarget)?
                    .convention
                    .red_zone_bytes,
            );
            if required.abi != FrameAbiPreservationConvention::SystemVAMD64
                || calls
                || !storage.slots.is_empty()
                || row.frame_size_bytes > capacity
                || source.local_storage_slots.iter().any(|slot| {
                    !matches!(
                        slot.id,
                        selected_instructions::LocalStorageSlotId::Spill { .. }
                    )
                })
            {
                return Err(Error::NonCanonicalLayout);
            }
        }
        // The probe roster is checked as a coverage bound over the submitted
        // committed extent, not by invoking the producer's plan computation.
        // Red-zone-resident bytes are already below the unadjusted stack
        // pointer and never enter the commit schedule. The granule itself
        // resolves through the same declared stack-commit matrix the producer
        // used, so an undeclared pair cannot inherit a borrowed interval.
        let probe_interval =
            stack_commit_granule_bytes(environment.target()).ok_or(Error::UnsupportedTarget)?;
        let touches = u64::from(row.stack_probe.touches);
        if row.stack_probe.interval_bytes != probe_interval
            || (touches == 0) != (committed <= probe_interval)
            || (touches != 0
                && !(touches
                    .checked_mul(probe_interval)
                    .is_some_and(|covered| covered >= committed)
                    && touches
                        .checked_sub(1)
                        .and_then(|earlier| earlier.checked_mul(probe_interval))
                        .is_some_and(|uncovered| uncovered < committed)))
        {
            return Err(Error::NonCanonicalLayout);
        }
        if row.machine != source.machine
            || row.contains_call != calls
            || row.pre_call_stack_alignment != call_site.stack_alignment_bytes
            || row.abi_stack_alignment_bytes != call_site.stack_alignment_bytes
            || row.outgoing_abi_area.byte_size != outgoing
            || u64::from(row.outgoing_abi_area.shadow_bytes) != shadow
            || row.callee_save_slots.len() != storage.slots.len()
            || row
                .callee_save_slots
                .iter()
                .zip(&storage.slots)
                .any(|(placed, abstract_slot)| {
                    placed.abstract_slot != abstract_slot.id
                        || placed.storage_view != abstract_slot.storage_view
                        || placed.frame_offset_bytes.checked_sub(preservation_offset)
                            != Some(abstract_slot.abstract_offset_bytes)
                        || placed.size_bytes != abstract_slot.size_bytes
                        || placed.alignment_bytes != abstract_slot.alignment_bytes
                        || !save_region_is_disjoint(
                            placed.frame_offset_bytes,
                            placed.size_bytes,
                            placed.alignment_bytes,
                            preservation_offset,
                            area,
                        )
                })
        {
            return Err(Error::NonCanonicalLayout);
        }
        let physical = environment.physical().model();
        // The replay resolves the same declared unwind row the producer used.
        // An undeclared pair fails closed, and a submitted plan whose pinned
        // convention is not the row's has drifted off the pair and fails
        // closed rather than replaying under a shared architecture arm. The
        // recorded return-address custody must then be an instance of the
        // row's declared continuation mechanism before its coordinates are
        // checked for canonicity.
        let unwind = frame_unwind_policy(environment.target()).ok_or(Error::UnsupportedTarget)?;
        if unwind.abi != required.abi {
            return Err(Error::UnsupportedTarget);
        }
        let stack = physical
            .view_named(unwind.stack_pointer_view)
            .ok_or(Error::MissingStackPointerView)?
            .id;
        if row.stack_pointer != stack || !unwind.continuation.admits(row.return_address) {
            return Err(Error::NonCanonicalLayout);
        }
        match unwind.continuation {
            FrameContinuationCustody::CallerActivationStack {
                return_address_size_bytes,
            } => {
                let (alignment, residue) = if calls
                    || (required.abi == FrameAbiPreservationConvention::MicrosoftX64 && area != 0)
                {
                    (
                        u64::from(call_site.stack_alignment_bytes),
                        call_site.entry_residue_bytes,
                    )
                } else {
                    (8, 0)
                };
                if !minimal_aligned_extent(area, row.frame_size_bytes, alignment, residue)
                    || row.return_address
                        != (ReturnAddressFrameCustody::CallerActivationStack {
                            post_prologue_offset_bytes: committed,
                            size_bytes: return_address_size_bytes,
                        })
                {
                    return Err(Error::NonCanonicalLayout);
                }
            }
            FrameContinuationCustody::LinkRegister {
                view,
                saved_size_bytes,
            } => {
                let link = physical
                    .view_named(view)
                    .ok_or(Error::MissingLinkRegisterView)?
                    .id;
                let saved_size = u64::from(saved_size_bytes);
                if calls
                    || candidate.policy
                        == TargetFrameLayoutPolicy::CanonicalSavedReturnAddressFrameV1
                {
                    let ReturnAddressFrameCustody::SavedLinkRegister {
                        view,
                        frame_offset_bytes,
                        size_bytes,
                    } = row.return_address
                    else {
                        return Err(Error::NonCanonicalLayout);
                    };
                    let used = frame_offset_bytes
                        .checked_add(saved_size)
                        .ok_or(Error::GeometryOverflow)?;
                    if view != link
                        || size_bytes != saved_size_bytes
                        || !minimal_aligned_extent(area, frame_offset_bytes, saved_size, 0)
                        || !minimal_aligned_extent(
                            used,
                            row.frame_size_bytes,
                            u64::from(call_site.stack_alignment_bytes),
                            0,
                        )
                    {
                        return Err(Error::NonCanonicalLayout);
                    }
                } else if row.return_address
                    != (ReturnAddressFrameCustody::LiveLinkRegister { view: link })
                    || !minimal_aligned_extent(
                        area,
                        row.frame_size_bytes,
                        u64::from(call_site.stack_alignment_bytes),
                        0,
                    )
                {
                    return Err(Error::NonCanonicalLayout);
                }
            }
        }
        // The unwind roster is recovered from the pieces already bound to
        // the inputs, never by trusting the candidate's own record: a saved
        // link register — whose custody and coordinates were just checked
        // canonically above — heads the roster, then every preservation
        // slot in reverse save order, so the roster is strictly descending
        // frame offsets. The release is exactly the committed extent and
        // the restated custody is the checked `return_address`. Exact
        // equality leaves the producer's roster non-authoritative.
        let mut restores = Vec::with_capacity(row.callee_save_slots.len() + 1);
        if let ReturnAddressFrameCustody::SavedLinkRegister {
            view,
            frame_offset_bytes,
            size_bytes,
        } = row.return_address
        {
            restores.push(FrameUnwindRestore {
                view,
                frame_offset_bytes,
                size_bytes: u64::from(size_bytes),
            });
        }
        restores.extend(
            row.callee_save_slots
                .iter()
                .rev()
                .map(|slot| FrameUnwindRestore {
                    view: slot.storage_view,
                    frame_offset_bytes: slot.frame_offset_bytes,
                    size_bytes: slot.size_bytes,
                }),
        );
        if row.unwind.restores != restores
            || row.unwind.released_bytes != committed
            || row.unwind.return_address != row.return_address
        {
            return Err(Error::NonCanonicalLayout);
        }
    }
    Ok(())
}

// Minimality is checked as a bound and congruence, not by invoking the
// producer's padding/alignment calculation. Subtraction cannot wrap.
fn minimal_aligned_extent(used: u64, extent: u64, alignment: u64, residue: u64) -> bool {
    extent
        .checked_sub(used)
        .is_some_and(|padding| padding < alignment)
        && extent % alignment == residue
}

fn save_region_is_disjoint(
    offset: u64,
    size: u64,
    alignment: u64,
    outgoing: u64,
    end: u64,
) -> bool {
    size != 0
        && alignment != 0
        && offset.is_multiple_of(alignment)
        && offset >= outgoing
        && offset.checked_add(size).is_some_and(|limit| limit <= end)
}

#[cfg(test)]
mod tests {
    use super::{minimal_aligned_extent, save_region_is_disjoint};

    #[test]
    fn saves_cannot_overlap_the_outgoing_abi_area_or_escape_storage() {
        assert!(save_region_is_disjoint(8, 8, 8, 4, 16));
        assert!(!save_region_is_disjoint(4, 8, 8, 4, 12));
        assert!(save_region_is_disjoint(32, 8, 8, 32, 40));
        assert!(save_region_is_disjoint(48, 16, 16, 32, 64));
        for (offset, size, alignment, end) in [
            (24, 8, 8, 40),
            (32, 16, 8, 40),
            (33, 8, 8, 48),
            (32, 8, 0, 40),
            (32, 0, 8, 40),
            (u64::MAX - 7, 16, 8, u64::MAX),
        ] {
            assert!(!save_region_is_disjoint(offset, size, alignment, 32, end));
        }
    }

    #[test]
    fn submitted_padding_must_be_minimal_and_have_the_abi_residue() {
        assert!(minimal_aligned_extent(9, 16, 8, 0));
        assert!(minimal_aligned_extent(16, 24, 16, 8));
        assert!(!minimal_aligned_extent(9, 8, 8, 0));
        assert!(!minimal_aligned_extent(9, 24, 8, 0));
        assert!(!minimal_aligned_extent(16, 16, 16, 8));
        assert!(!minimal_aligned_extent(u64::MAX, 0, 16, 0));
        assert!(minimal_aligned_extent(u64::MAX - 7, u64::MAX - 7, 16, 8));
    }
}
