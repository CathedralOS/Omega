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
        if row.machine != source.machine
            || row.kind != AllocatedCalleeSavedFunctionKind::Ordinary
            || row.contains_call != calls
            || row.pre_call_stack_alignment != 16
            || row.abi_stack_alignment_bytes != 16
            || row.callee_save_slots.len() != storage.slots.len()
            || row
                .callee_save_slots
                .iter()
                .zip(&storage.slots)
                .any(|(placed, abstract_slot)| {
                    placed.abstract_slot != abstract_slot.id
                        || placed.storage_view != abstract_slot.storage_view
                        || placed.frame_offset_bytes != abstract_slot.abstract_offset_bytes
                        || placed.size_bytes != abstract_slot.size_bytes
                        || placed.alignment_bytes != abstract_slot.alignment_bytes
                })
        {
            return Err(Error::NonCanonicalLayout);
        }
        let area = storage.abstract_area_bytes;
        let physical = environment.physical().model();
        match (environment.target().architecture, required.abi) {
            (
                Architecture::X86_64,
                convention @ (FrameAbiPreservationConvention::SystemVAMD64
                | FrameAbiPreservationConvention::MicrosoftX64),
            ) if !calls || convention == FrameAbiPreservationConvention::SystemVAMD64 => {
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

#[cfg(test)]
mod tests {
    use super::minimal_aligned_extent;

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
