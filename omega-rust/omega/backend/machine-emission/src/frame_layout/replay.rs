//! Check submitted geometry without constructing a replacement frame plan.

use selected_instructions::MachineAlternativeFamily;
use target::Architecture;

use crate::frame_layout::{
    AllocatedCalleeSavedFunctionKind, FrameAbiPreservationConvention, ReturnAddressFrameCustody,
    StagedOptimizedPostAllocationMachinePlan, TargetFrameLayoutError as Error,
    TargetFrameLayoutPlan, TargetFrameLayoutPolicy, ValidatedAllocatedCalleeSavedRequirements,
    ValidatedNonAuthoritativeCalleeSaveStorage, ValidatedTargetRegisterEnvironment,
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
    if !current.structural_unit_functions.is_empty()
        || required
            .functions
            .iter()
            .any(|row| row.kind != AllocatedCalleeSavedFunctionKind::Ordinary)
        || saved
            .functions
            .iter()
            .any(|row| row.kind != AllocatedCalleeSavedFunctionKind::Ordinary)
    {
        return Err(Error::StructuralFunctionUnsupported);
    }
    if current.functions.len() != required.functions.len()
        || current.functions.len() != saved.functions.len()
    {
        return Err(Error::FunctionRosterMismatch);
    }
    if candidate.functions.len() != current.functions.len() {
        return Err(Error::NonCanonicalLayout);
    }
    for (((source, requirement), storage), row) in current
        .functions
        .iter()
        .zip(&required.functions)
        .zip(&saved.functions)
        .zip(&candidate.functions)
    {
        if source.machine != requirement.machine
            || source.machine != storage.machine
            || requirement.kind != storage.kind
        {
            return Err(Error::FunctionRosterMismatch);
        }
        let calls = source
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| {
                instruction.alternative.key.family == MachineAlternativeFamily::CallI64
            });
        let outgoing = if calls && required.abi == FrameAbiPreservationConvention::MicrosoftX64 {
            32_u64
        } else {
            0
        };
        let area = storage
            .abstract_area_bytes
            .checked_add(outgoing)
            .ok_or(Error::GeometryOverflow)?;
        if row.machine != source.machine
            || row.kind != AllocatedCalleeSavedFunctionKind::Ordinary
            || row.contains_call != calls
            || row.pre_call_stack_alignment != 16
            || row.abi_stack_alignment_bytes != 16
            || row.outgoing_abi_area.byte_size != outgoing
            || u64::from(row.outgoing_abi_area.shadow_bytes) != outgoing
            || row.callee_save_slots.len() != storage.slots.len()
            || row
                .callee_save_slots
                .iter()
                .zip(&storage.slots)
                .any(|(placed, abstract_slot)| {
                    placed.abstract_slot != abstract_slot.id
                        || placed.storage_view != abstract_slot.storage_view
                        || placed.frame_offset_bytes.checked_sub(outgoing)
                            != Some(abstract_slot.abstract_offset_bytes)
                        || placed.size_bytes != abstract_slot.size_bytes
                        || placed.alignment_bytes != abstract_slot.alignment_bytes
                        || !save_region_is_disjoint(
                            placed.frame_offset_bytes,
                            placed.size_bytes,
                            placed.alignment_bytes,
                            outgoing,
                            area,
                        )
                })
        {
            return Err(Error::NonCanonicalLayout);
        }
        let physical = environment.physical().model();
        match (environment.target().architecture, required.abi) {
            (
                Architecture::X86_64,
                convention @ (FrameAbiPreservationConvention::SystemVAMD64
                | FrameAbiPreservationConvention::MicrosoftX64),
            ) => {
                let stack = physical
                    .view_named("rsp")
                    .ok_or(Error::MissingStackPointerView)?
                    .id;
                let (alignment, residue) = if calls
                    || (convention == FrameAbiPreservationConvention::MicrosoftX64 && area != 0)
                {
                    (16, 8)
                } else {
                    (8, 0)
                };
                if row.stack_pointer != stack
                    || !minimal_aligned_extent(area, row.frame_size_bytes, alignment, residue)
                    || row.return_address
                        != (ReturnAddressFrameCustody::CallerActivationStack {
                            post_prologue_offset_bytes: row.frame_size_bytes,
                            size_bytes: 8,
                        })
                {
                    return Err(Error::NonCanonicalLayout);
                }
            }
            (
                Architecture::Aarch64,
                FrameAbiPreservationConvention::Aapcs64
                | FrameAbiPreservationConvention::DarwinAapcs64,
            ) => {
                let stack = physical
                    .view_named("sp")
                    .ok_or(Error::MissingStackPointerView)?
                    .id;
                let link = physical
                    .view_named("x30")
                    .ok_or(Error::MissingLinkRegisterView)?
                    .id;
                if row.stack_pointer != stack {
                    return Err(Error::NonCanonicalLayout);
                }
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
                        .checked_add(8)
                        .ok_or(Error::GeometryOverflow)?;
                    if view != link
                        || size_bytes != 8
                        || !minimal_aligned_extent(area, frame_offset_bytes, 8, 0)
                        || !minimal_aligned_extent(used, row.frame_size_bytes, 16, 0)
                    {
                        return Err(Error::NonCanonicalLayout);
                    }
                } else if row.return_address
                    != (ReturnAddressFrameCustody::LiveLinkRegister { view: link })
                    || !minimal_aligned_extent(area, row.frame_size_bytes, 16, 0)
                {
                    return Err(Error::NonCanonicalLayout);
                }
            }
            _ => return Err(Error::UnsupportedTarget),
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
