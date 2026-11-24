//! Assign callee-save slots and stack coordinates before encoding.
use frame_admission::frame_layout::*;
use optimization_core::OptimizationWorkBudget;
use post_allocation_machine::PostAllocationMachinePlan;
use register_environment::{ValidatedTargetRegisterEnvironment, selected_abi_preservation};
use register_homes::{
    AllocatedCalleeSavedRequirementPlan, AllocatedCalleeSavedRequirements,
    FunctionAllocatedCalleeSavedRequirements,
};
use register_model::{RegisterUnitId, ValidatedPreservationStorageCatalog};
use std::sync::Arc;
use target::NativeTarget;
pub(crate) fn stage_non_authoritative_callee_save_storage(
    source: &AllocatedCalleeSavedRequirements,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: NonAuthoritativeCalleeSaveStoragePolicy,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedNonAuthoritativeCalleeSaveStorage, NonAuthoritativeCalleeSaveStorageError> {
    let plan = admit_storage_plan(
        source,
        environment,
        policy,
        budget,
        derive_storage_functions,
    )?;
    validate_non_authoritative_callee_save_storage(source, environment, plan)
}

fn derive_storage_functions(
    source: &AllocatedCalleeSavedRequirementPlan,
    catalog: &ValidatedPreservationStorageCatalog,
) -> Result<Vec<FunctionNonAuthoritativeCalleeSaveStorage>, NonAuthoritativeCalleeSaveStorageError>
{
    source
        .functions
        .iter()
        .map(|function| derive_storage_function(function, catalog))
        .collect()
}

fn derive_storage_function(
    source: &FunctionAllocatedCalleeSavedRequirements,
    catalog: &ValidatedPreservationStorageCatalog,
) -> Result<FunctionNonAuthoritativeCalleeSaveStorage, NonAuthoritativeCalleeSaveStorageError> {
    let mut slots = Vec::new();
    let mut extent = 0_u64;
    let mut max_alignment = 1_u64;
    for group in &catalog.catalog().groups {
        let modified_units = source
            .modified_units
            .iter()
            .filter(|requirement| group.preserved_units.contains(&requirement.unit))
            .cloned()
            .collect::<Vec<_>>();
        if modified_units.is_empty() {
            continue;
        }
        let alignment = group.alignment_bytes;
        let offset = align_up_storage(extent, alignment)?;
        extent = offset
            .checked_add(group.size_bytes)
            .ok_or(NonAuthoritativeCalleeSaveStorageError::StorageGeometryOverflow)?;
        max_alignment = max_alignment.max(group.alignment_bytes);
        slots.push(NonAuthoritativeCalleeSaveSlot {
            id: NonAuthoritativeCalleeSaveSlotId(
                u16::try_from(slots.len())
                    .map_err(|_| NonAuthoritativeCalleeSaveStorageError::StorageGeometryOverflow)?,
            ),
            storage_group: group.id,
            storage_view: group.storage_view,
            preserved_units: group.preserved_units.clone(),
            modified_units,
            abstract_offset_bytes: offset,
            size_bytes: group.size_bytes,
            alignment_bytes: group.alignment_bytes,
        });
    }
    if slots
        .iter()
        .map(|slot| slot.modified_units.len())
        .sum::<usize>()
        != source.modified_units.len()
    {
        let missing = source
            .modified_units
            .iter()
            .find(|requirement| {
                !slots
                    .iter()
                    .any(|slot| slot.modified_units.contains(requirement))
            })
            .map(|requirement| requirement.unit)
            .unwrap_or(RegisterUnitId(u16::MAX));
        return Err(NonAuthoritativeCalleeSaveStorageError::UnknownPreservedUnit(missing));
    }
    Ok(FunctionNonAuthoritativeCalleeSaveStorage {
        machine: source.machine,
        abstract_area_bytes: extent,
        abstract_area_alignment: max_alignment,
        slots,
    })
}

fn align_up_storage(
    value: u64,
    alignment: u64,
) -> Result<u64, NonAuthoritativeCalleeSaveStorageError> {
    let mask = alignment
        .checked_sub(1)
        .ok_or(NonAuthoritativeCalleeSaveStorageError::StorageGeometryOverflow)?;
    value
        .checked_add(mask)
        .map(|rounded| rounded & !mask)
        .ok_or(NonAuthoritativeCalleeSaveStorageError::StorageGeometryOverflow)
}

pub(crate) fn stage_target_frame_layout(
    machine: &PostAllocationMachinePlan,
    requirements: &AllocatedCalleeSavedRequirements,
    storage: &ValidatedNonAuthoritativeCalleeSaveStorage,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: TargetFrameLayoutPolicy,
) -> Result<ValidatedTargetFrameLayout, TargetFrameLayoutError> {
    let plan = derive_frame_layout(machine, requirements, storage, environment, policy)?;
    validate_target_frame_layout(machine, requirements, storage, environment, plan)
}

pub(crate) fn validate_target_frame_layout(
    machine: &PostAllocationMachinePlan,
    requirements: &AllocatedCalleeSavedRequirements,
    storage: &ValidatedNonAuthoritativeCalleeSaveStorage,
    environment: &ValidatedTargetRegisterEnvironment,
    candidate: TargetFrameLayoutPlan,
) -> Result<ValidatedTargetFrameLayout, TargetFrameLayoutError> {
    let demands = Arc::new(prepare_frame_demands(machine, environment)?);
    validate_target_frame_demands(demands, requirements, storage, environment, candidate)
}

fn derive_frame_layout(
    machine: &PostAllocationMachinePlan,
    requirements: &AllocatedCalleeSavedRequirements,
    storage: &ValidatedNonAuthoritativeCalleeSaveStorage,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: TargetFrameLayoutPolicy,
) -> Result<TargetFrameLayoutPlan, TargetFrameLayoutError> {
    let machine_plan = machine;
    let requirement_plan = requirements.plan();
    let storage_plan = storage.plan();
    if machine_plan.target != environment.target()
        || machine_plan.register_environment != environment.identity()
        || machine_plan.physical_register_model != environment.physical().identity()
        || requirement_plan.selected != machine_plan.selected
        || requirement_plan.homes != machine_plan.homes
        || requirement_plan.post_allocation_manifest != machine_plan.post_allocation_manifest
        || requirement_plan.register_environment != environment.identity()
        || requirement_plan.physical_register_model != environment.physical().identity()
        || requirement_plan.target != environment.target()
        || storage_plan.callee_saved_requirements != requirements.identity()
        || storage_plan.register_environment != environment.identity()
        || storage_plan.physical_register_model != environment.physical().identity()
        || storage_plan.target != environment.target()
        || storage_plan.abi != requirement_plan.abi
    {
        return Err(TargetFrameLayoutError::RootMismatch);
    }

    if machine_plan.functions.len() != requirement_plan.functions.len()
        || machine_plan.functions.len() != storage_plan.functions.len()
    {
        return Err(TargetFrameLayoutError::FunctionRosterMismatch);
    }

    let functions = machine_plan
        .functions
        .iter()
        .zip(&requirement_plan.functions)
        .zip(&storage_plan.functions)
        .map(
            |((machine_function, requirement_function), storage_function)| {
                if machine_function.machine != requirement_function.machine
                    || machine_function.machine != storage_function.machine
                {
                    return Err(TargetFrameLayoutError::FunctionRosterMismatch);
                }
                let calls = contains_call(machine_function);
                let loans = stable_address_loans(machine_function);
                let callee_save_slots = storage_function
                    .slots
                    .iter()
                    .map(|slot| CalleeSaveFrameSlot {
                        abstract_slot: slot.id,
                        storage_view: slot.storage_view,
                        frame_offset_bytes: slot.abstract_offset_bytes,
                        size_bytes: slot.size_bytes,
                        alignment_bytes: slot.alignment_bytes,
                    })
                    .collect::<Vec<_>>();
                function_layout(
                    environment,
                    requirement_plan.abi,
                    policy,
                    machine_function.machine,
                    calls,
                    body_writes_link_register(environment, machine_function)?,
                    &machine_function.outgoing_arguments,
                    &machine_function.local_storage_slots,
                    loans,
                    storage_function.abstract_area_bytes,
                    callee_save_slots,
                )
            },
        )
        .collect::<Result<Vec<_>, _>>()?;

    Ok(TargetFrameLayoutPlan {
        post_allocation_machine: machine.identity,
        callee_saved_requirements: requirements.identity(),
        callee_save_storage: storage.receipt().identity(),
        register_environment: environment.identity(),
        physical_register_model: environment.physical().identity(),
        target: environment.target(),
        abi: requirement_plan.abi,
        policy,
        functions,
    })
}

#[allow(clippy::too_many_arguments)]
fn function_layout(
    environment: &ValidatedTargetRegisterEnvironment,
    abi: FrameAbiPreservationConvention,
    policy: TargetFrameLayoutPolicy,
    machine: semantic_vocabulary::MachineId,
    contains_call: bool,
    writes_link_register: bool,
    outgoing_arguments: &[selected_instructions::SelectedOutgoingArgumentSlot],
    local_storage: &[selected_instructions::SelectedLocalStorageSlot],
    stable_address_loans: Vec<selected_instructions::LocalStorageSlotId>,
    callee_save_area_bytes: u64,
    mut callee_save_slots: Vec<CalleeSaveFrameSlot>,
) -> Result<FunctionTargetFrameLayout, TargetFrameLayoutError> {
    let call_site = call_site_stack_contract(environment, abi)
        .ok_or(TargetFrameLayoutError::UnsupportedTarget)?;
    let shadow_bytes = if contains_call && abi == FrameAbiPreservationConvention::MicrosoftX64 {
        32
    } else {
        0
    };
    let mut outgoing_extent = u64::from(shadow_bytes);
    for slot in outgoing_arguments {
        if !contains_call
            || slot.byte_size == 0
            || !slot.alignment.is_power_of_two()
            || slot.abi_stack_byte_offset < u32::from(shadow_bytes)
            || !slot.abi_stack_byte_offset.is_multiple_of(u32::from(
                slot.reservation_alignment(call_site.stack_alignment_bytes),
            ))
            || (slot.alignment > call_site.stack_alignment_bytes
                && slot.id.role != selected_instructions::OutgoingArgumentSlotRole::ValueCopy)
        {
            return Err(TargetFrameLayoutError::NonCanonicalLayout);
        }
        let end = slot
            .abi_stack_byte_offset
            .checked_add(
                slot.reserved_byte_size(call_site.stack_alignment_bytes)
                    .ok_or(TargetFrameLayoutError::GeometryOverflow)?,
            )
            .ok_or(TargetFrameLayoutError::GeometryOverflow)?;
        outgoing_extent = outgoing_extent.max(u64::from(end));
    }
    let outgoing_abi_area = machine_code::OutgoingAbiFrameArea {
        byte_size: outgoing_extent,
        shadow_bytes,
    };
    let unwind = frame_unwind_policy(environment.target())
        .ok_or(TargetFrameLayoutError::UnsupportedTarget)?;
    if unwind.abi != abi {
        return Err(TargetFrameLayoutError::UnsupportedTarget);
    }
    let preservation_alignment = callee_save_slots
        .iter()
        .map(|slot| slot.alignment_bytes)
        .max()
        .unwrap_or(8)
        .max(8);
    if !preservation_alignment.is_power_of_two() {
        return Err(TargetFrameLayoutError::NonCanonicalLayout);
    }
    let saved_link_bytes = match unwind.continuation {
        FrameContinuationCustody::LinkRegister {
            saved_size_bytes, ..
        } if contains_call
            || writes_link_register
            || policy == TargetFrameLayoutPolicy::CanonicalSavedReturnAddressFrameV1 =>
        {
            Some(u64::from(saved_size_bytes))
        }
        _ => None,
    };
    let low_preservation = match unwind.preservation {
        FramePreservationPlacement::AboveLocals => None,
        FramePreservationPlacement::BelowLocals => Some(low_preservation_area(
            outgoing_extent,
            preservation_alignment,
            callee_save_slots.is_empty(),
            callee_save_area_bytes,
            saved_link_bytes,
        )?),
    };
    let mut local_extent = low_preservation.map_or(outgoing_extent, |area| area.locals_offset);
    let mut local_storage_slots = Vec::with_capacity(local_storage.len());
    for (index, slot) in local_storage.iter().enumerate() {
        if slot.byte_size == 0
            || !slot.alignment.is_power_of_two()
            || local_storage[..index]
                .iter()
                .any(|earlier| earlier.id == slot.id)
        {
            return Err(TargetFrameLayoutError::NonCanonicalLayout);
        }
        let frame_offset_bytes = align_up_frame(
            local_extent,
            u64::from(slot.alignment.min(call_site.stack_alignment_bytes)),
        )?;
        let placed = machine_code::LocalStorageFrameSlot {
            id: slot.id,
            frame_offset_bytes,
            size_bytes: slot.byte_size,
            alignment_bytes: slot.alignment,
        };
        local_extent = frame_offset_bytes
            .checked_add(placed.reserved_size_bytes(call_site.stack_alignment_bytes))
            .ok_or(TargetFrameLayoutError::GeometryOverflow)?;
        local_storage_slots.push(placed);
    }
    if stable_address_loans
        .iter()
        .any(|loan| !local_storage_slots.iter().any(|slot| slot.id == *loan))
    {
        return Err(TargetFrameLayoutError::NonCanonicalLayout);
    }
    let preservation_offset = if let Some(area) = low_preservation {
        area.preservation_offset
    } else if local_storage_slots.is_empty() && callee_save_slots.is_empty() {
        outgoing_extent
    } else {
        align_up_frame(local_extent, preservation_alignment)?
    };
    for slot in &mut callee_save_slots {
        slot.frame_offset_bytes = slot
            .frame_offset_bytes
            .checked_add(preservation_offset)
            .ok_or(TargetFrameLayoutError::GeometryOverflow)?;
    }
    let used_area_bytes = if low_preservation.is_some() {
        local_extent
    } else {
        callee_save_area_bytes
            .checked_add(preservation_offset)
            .ok_or(TargetFrameLayoutError::GeometryOverflow)?
    };
    let locals_need_aligned_activation = local_storage.iter().any(|slot| {
        !call_site
            .entry_residue_bytes
            .is_multiple_of(u64::from(slot.alignment))
    });
    let stack_pointer = environment
        .physical()
        .model()
        .view_named(unwind.stack_pointer_view)
        .ok_or(TargetFrameLayoutError::MissingStackPointerView)?
        .id;
    let (frame_size_bytes, red_zone_resident_bytes, return_address) = match unwind.continuation {
        FrameContinuationCustody::CallerActivationStack {
            return_address_size_bytes,
        } => {
            let frame_size = if contains_call
                || locals_need_aligned_activation
                || (unwind.abi == FrameAbiPreservationConvention::MicrosoftX64
                    && used_area_bytes != 0)
            {
                align_to_residue(
                    used_area_bytes,
                    u64::from(call_site.stack_alignment_bytes),
                    call_site.entry_residue_bytes,
                )?
            } else {
                align_up_frame(used_area_bytes, 8)?
            };
            let red_zone_capacity = u64::from(
                selected_abi_preservation(environment)
                    .map_err(|_| TargetFrameLayoutError::UnsupportedTarget)?
                    .convention
                    .red_zone_bytes,
            );
            let red_zone_resident = if unwind.abi == FrameAbiPreservationConvention::SystemVAMD64
                && !contains_call
                && callee_save_slots.is_empty()
                && frame_size != 0
                && frame_size <= red_zone_capacity
                && local_storage.iter().all(|slot| {
                    matches!(
                        slot.id,
                        selected_instructions::LocalStorageSlotId::Spill { .. }
                    )
                }) {
                frame_size
            } else {
                0
            };
            (
                frame_size,
                red_zone_resident,
                ReturnAddressFrameCustody::CallerActivationStack {
                    post_prologue_offset_bytes: frame_size - red_zone_resident,
                    size_bytes: return_address_size_bytes,
                },
            )
        }
        FrameContinuationCustody::LinkRegister {
            view,
            saved_size_bytes,
        } => {
            let link = environment
                .physical()
                .model()
                .view_named(view)
                .ok_or(TargetFrameLayoutError::MissingLinkRegisterView)?
                .id;
            let saved_size = u64::from(saved_size_bytes);
            if saved_link_bytes.is_some() {
                let (link_offset, used) = match low_preservation {
                    // The link slot already sits below the locals.
                    Some(area) => (
                        area.link_offset
                            .ok_or(TargetFrameLayoutError::NonCanonicalLayout)?,
                        used_area_bytes,
                    ),
                    None => {
                        let link_offset = align_up_frame(used_area_bytes, saved_size)?;
                        (
                            link_offset,
                            link_offset
                                .checked_add(saved_size)
                                .ok_or(TargetFrameLayoutError::GeometryOverflow)?,
                        )
                    }
                };
                (
                    align_up_frame(used, u64::from(call_site.stack_alignment_bytes))?,
                    0,
                    ReturnAddressFrameCustody::SavedLinkRegister {
                        view: link,
                        frame_offset_bytes: link_offset,
                        size_bytes: saved_size_bytes,
                    },
                )
            } else {
                (
                    align_up_frame(used_area_bytes, u64::from(call_site.stack_alignment_bytes))?,
                    0,
                    ReturnAddressFrameCustody::LiveLinkRegister { view: link },
                )
            }
        }
    };

    if frame_size_bytes > MAX_FRAME_BYTES {
        return Err(TargetFrameLayoutError::FrameExceedsAddressableExtent { frame_size_bytes });
    }

    let committed_bytes = frame_size_bytes - red_zone_resident_bytes;
    let stack_probe = probe_plan(environment.target(), committed_bytes)?;
    let restores = unwind_restores(return_address, &callee_save_slots);

    Ok(FunctionTargetFrameLayout {
        machine,
        contains_call,
        stack_pointer,
        pre_call_stack_alignment: call_site.stack_alignment_bytes,
        frame_size_bytes,
        red_zone_resident_bytes,
        abi_stack_alignment_bytes: call_site.stack_alignment_bytes,
        outgoing_abi_area,
        local_storage_slots,
        stable_address_loans,
        callee_save_slots,
        return_address,
        stack_probe,
        unwind: FrameUnwindPlan {
            restores,
            released_bytes: committed_bytes,
            return_address,
        },
    })
}

#[derive(Debug, Clone, Copy)]
struct LowPreservationArea {
    preservation_offset: u64,
    link_offset: Option<u64>,
    locals_offset: u64,
}

/// Place preserved registers, then the saved link register, directly above
/// the outgoing-argument area; local storage starts after them.
fn low_preservation_area(
    outgoing_extent: u64,
    preservation_alignment: u64,
    no_preserved_registers: bool,
    callee_save_area_bytes: u64,
    saved_link_bytes: Option<u64>,
) -> Result<LowPreservationArea, TargetFrameLayoutError> {
    let preservation_offset = if no_preserved_registers {
        outgoing_extent
    } else {
        align_up_frame(outgoing_extent, preservation_alignment)?
    };
    let preservation_end = preservation_offset
        .checked_add(callee_save_area_bytes)
        .ok_or(TargetFrameLayoutError::GeometryOverflow)?;
    let (link_offset, locals_offset) = match saved_link_bytes {
        Some(size) => {
            let link = align_up_frame(preservation_end, size)?;
            (
                Some(link),
                link.checked_add(size)
                    .ok_or(TargetFrameLayoutError::GeometryOverflow)?,
            )
        }
        None => (None, preservation_end),
    };
    Ok(LowPreservationArea {
        preservation_offset,
        link_offset,
        locals_offset,
    })
}

fn probe_plan(
    target: NativeTarget,
    frame_size_bytes: u64,
) -> Result<StackProbePlan, TargetFrameLayoutError> {
    let interval_bytes =
        stack_commit_granule_bytes(target).ok_or(TargetFrameLayoutError::UnsupportedTarget)?;
    let touches = if frame_size_bytes > interval_bytes {
        u32::try_from(frame_size_bytes.div_ceil(interval_bytes))
            .map_err(|_| TargetFrameLayoutError::GeometryOverflow)?
    } else {
        0
    };
    Ok(StackProbePlan {
        interval_bytes,
        touches,
    })
}

fn align_up_frame(value: u64, alignment: u64) -> Result<u64, TargetFrameLayoutError> {
    let remainder = value % alignment;
    if remainder == 0 {
        return Ok(value);
    }
    value
        .checked_add(alignment - remainder)
        .ok_or(TargetFrameLayoutError::GeometryOverflow)
}

fn align_to_residue(
    value: u64,
    alignment: u64,
    residue: u64,
) -> Result<u64, TargetFrameLayoutError> {
    let current = value % alignment;
    let padding = (residue + alignment - current) % alignment;
    value
        .checked_add(padding)
        .ok_or(TargetFrameLayoutError::GeometryOverflow)
}

#[cfg(test)]
fn spill(register: u32) -> selected_instructions::SelectedLocalStorageSlot {
    selected_instructions::SelectedLocalStorageSlot {
        id: selected_instructions::LocalStorageSlotId::Spill {
            register: selected_instructions::VirtualRegisterId(register),
        },
        byte_size: 8,
        alignment: 8,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CalleeSaveFrameSlot, FrameAbiPreservationConvention, FrameContinuationCustody,
        FramePreservationPlacement, FrameUnwindPolicy, MAX_FRAME_BYTES, ReturnAddressFrameCustody,
        TargetFrameLayoutError, TargetFrameLayoutPolicy, call_site_stack_contract,
        frame_unwind_policy, function_layout, minimal_aligned_extent, save_region_is_disjoint,
        spill, stack_commit_granule_bytes,
    };
    use target::NativeTarget;

    macro_rules! row {
        ($a:expr => $b:expr) => {
            ($a, $b)
        };
    }

    macro_rules! t {
        ($target:expr => $abi:ident => $c:expr => $d:expr) => {
            ($target, FrameAbiPreservationConvention::$abi, $c, $d)
        };
        ($target:expr => $abi:ident => $c:expr) => {
            ($target, FrameAbiPreservationConvention::$abi, $c)
        };
    }

    #[derive(Clone, Copy)]
    enum CaseShape {
        Bare,
        Call,
        Link,
    }

    #[allow(clippy::too_many_arguments)]
    fn frame_with_saves(
        environment: &register_environment::ValidatedTargetRegisterEnvironment,
        abi: FrameAbiPreservationConvention,
        contains_call: bool,
        writes_link_register: bool,
        outgoing_arguments: &[selected_instructions::SelectedOutgoingArgumentSlot],
        local_storage: &[selected_instructions::SelectedLocalStorageSlot],
        callee_save_area_bytes: u64,
        callee_save_slots: Vec<CalleeSaveFrameSlot>,
    ) -> Result<machine_code::FunctionTargetFrameLayout, TargetFrameLayoutError> {
        function_layout(
            environment,
            abi,
            TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
            semantic_vocabulary::MachineId::new(1).unwrap(),
            contains_call,
            writes_link_register,
            outgoing_arguments,
            local_storage,
            Vec::new(),
            callee_save_area_bytes,
            callee_save_slots,
        )
    }

    fn frame(
        environment: &register_environment::ValidatedTargetRegisterEnvironment,
        abi: FrameAbiPreservationConvention,
        shape: CaseShape,
        storage: (
            &[selected_instructions::SelectedOutgoingArgumentSlot],
            &[selected_instructions::SelectedLocalStorageSlot],
        ),
    ) -> Result<machine_code::FunctionTargetFrameLayout, TargetFrameLayoutError> {
        let (outgoing_arguments, local_storage) = storage;
        frame_with_saves(
            environment,
            abi,
            matches!(shape, CaseShape::Call),
            matches!(shape, CaseShape::Link),
            outgoing_arguments,
            local_storage,
            0,
            Vec::new(),
        )
    }

    #[test]
    fn call_site_contract_binds_the_declared_convention_alignment() {
        for (target, abi, residue) in [
            (
                NativeTarget::linux_x64(),
                FrameAbiPreservationConvention::SystemVAMD64,
                8,
            ),
            (
                NativeTarget::windows_x64(),
                FrameAbiPreservationConvention::MicrosoftX64,
                8,
            ),
            (
                NativeTarget::uefi_x64(),
                FrameAbiPreservationConvention::MicrosoftX64,
                8,
            ),
            (
                NativeTarget::linux_arm64(),
                FrameAbiPreservationConvention::Aapcs64,
                0,
            ),
            (
                NativeTarget::macos_arm64(),
                FrameAbiPreservationConvention::DarwinAapcs64,
                0,
            ),
        ] {
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let contract = call_site_stack_contract(&environment, abi).unwrap();
            assert_eq!(
                contract.stack_alignment_bytes,
                register_environment::selected_abi_preservation(&environment)
                    .unwrap()
                    .convention
                    .stack_alignment,
                "{target:?}"
            );
            assert_eq!(contract.entry_residue_bytes, residue, "{target:?}");
        }
    }

    #[test]
    fn call_site_contract_fails_closed_on_undeclared_pairs() {
        let environment =
            register_environment::baseline_target_register_environment(NativeTarget::linux_x64())
                .unwrap();
        for abi in [
            FrameAbiPreservationConvention::Aapcs64,
            FrameAbiPreservationConvention::DarwinAapcs64,
        ] {
            assert_eq!(call_site_stack_contract(&environment, abi), None, "{abi:?}");
        }
    }

    #[test]
    fn stack_commit_matrix_declares_one_row_per_supported_pair() {
        for (target, expected) in [
            (NativeTarget::linux_x64(), 4_096),
            (NativeTarget::windows_x64(), 4_096),
            (NativeTarget::uefi_x64(), 4_096),
            (NativeTarget::linux_arm64(), 4_096),
            (NativeTarget::macos_arm64(), 16_384),
            (NativeTarget::macos_x64(), 4_096),
        ] {
            assert_eq!(
                stack_commit_granule_bytes(target),
                Some(expected),
                "{target:?}"
            );
        }
    }

    #[test]
    fn windows_arm64_commits_stack_in_four_kibibyte_pages() {
        assert_eq!(
            stack_commit_granule_bytes(NativeTarget::windows_arm64()),
            Some(4_096)
        );
    }

    #[test]
    fn unwind_matrix_declares_one_row_per_supported_pair() {
        let caller_stack = FrameContinuationCustody::CallerActivationStack {
            return_address_size_bytes: 8,
        };
        let link = FrameContinuationCustody::LinkRegister {
            view: "x30",
            saved_size_bytes: 8,
        };
        for (target, expected) in [
            (
                NativeTarget::linux_x64(),
                FrameUnwindPolicy {
                    abi: FrameAbiPreservationConvention::SystemVAMD64,
                    stack_pointer_view: "rsp",
                    continuation: caller_stack,
                    preservation: FramePreservationPlacement::AboveLocals,
                },
            ),
            (
                NativeTarget::macos_x64(),
                FrameUnwindPolicy {
                    abi: FrameAbiPreservationConvention::SystemVAMD64,
                    stack_pointer_view: "rsp",
                    continuation: caller_stack,
                    preservation: FramePreservationPlacement::AboveLocals,
                },
            ),
            (
                NativeTarget::windows_x64(),
                FrameUnwindPolicy {
                    abi: FrameAbiPreservationConvention::MicrosoftX64,
                    stack_pointer_view: "rsp",
                    continuation: caller_stack,
                    preservation: FramePreservationPlacement::AboveLocals,
                },
            ),
            (
                NativeTarget::uefi_x64(),
                FrameUnwindPolicy {
                    abi: FrameAbiPreservationConvention::MicrosoftX64,
                    stack_pointer_view: "rsp",
                    continuation: caller_stack,
                    preservation: FramePreservationPlacement::AboveLocals,
                },
            ),
            (
                NativeTarget::linux_arm64(),
                FrameUnwindPolicy {
                    abi: FrameAbiPreservationConvention::Aapcs64,
                    stack_pointer_view: "sp",
                    continuation: link,
                    preservation: FramePreservationPlacement::AboveLocals,
                },
            ),
            (
                NativeTarget::macos_arm64(),
                FrameUnwindPolicy {
                    abi: FrameAbiPreservationConvention::DarwinAapcs64,
                    stack_pointer_view: "sp",
                    continuation: link,
                    preservation: FramePreservationPlacement::AboveLocals,
                },
            ),
        ] {
            assert_eq!(frame_unwind_policy(target), Some(expected), "{target:?}");
        }
    }

    #[test]
    fn windows_arm64_unwinds_through_the_link_register_with_x18_reserved() {
        assert_eq!(
            frame_unwind_policy(NativeTarget::windows_arm64()),
            Some(FrameUnwindPolicy {
                abi: FrameAbiPreservationConvention::DarwinAapcs64,
                stack_pointer_view: "sp",
                continuation: FrameContinuationCustody::LinkRegister {
                    view: "x30",
                    saved_size_bytes: 8,
                },
                preservation: FramePreservationPlacement::BelowLocals,
            })
        );
    }

    #[test]
    fn continuation_admits_only_its_own_custody_family() {
        let caller_stack = FrameContinuationCustody::CallerActivationStack {
            return_address_size_bytes: 8,
        };
        let link = FrameContinuationCustody::LinkRegister {
            view: "x30",
            saved_size_bytes: 8,
        };
        let view = register_model::RegisterViewId(7);
        assert!(
            caller_stack.admits(ReturnAddressFrameCustody::CallerActivationStack {
                post_prologue_offset_bytes: 0,
                size_bytes: 8,
            })
        );
        assert!(!caller_stack.admits(ReturnAddressFrameCustody::LiveLinkRegister { view }));
        assert!(
            !caller_stack.admits(ReturnAddressFrameCustody::SavedLinkRegister {
                view,
                frame_offset_bytes: 0,
                size_bytes: 8,
            })
        );
        assert!(link.admits(ReturnAddressFrameCustody::LiveLinkRegister { view }));
        assert!(link.admits(ReturnAddressFrameCustody::SavedLinkRegister {
            view,
            frame_offset_bytes: 0,
            size_bytes: 8,
        }));
        assert!(
            !link.admits(ReturnAddressFrameCustody::CallerActivationStack {
                post_prologue_offset_bytes: 0,
                size_bytes: 8,
            })
        );
    }

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

    #[test]
    fn compiler_spill_slots_expand_final_frame_without_aliasing_outgoing_or_other_locals() {
        for target in [
            NativeTarget::windows_x64(),
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
        ] {
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let abi = register_environment::selected_abi_preservation(&environment)
                .unwrap()
                .kind;
            let slots = [0, 1].map(spill);
            let layout = function_layout(
                &environment,
                abi,
                TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
                semantic_vocabulary::MachineId::new(1).unwrap(),
                true,
                false,
                &[],
                &slots,
                Vec::new(),
                0,
                Vec::new(),
            )
            .unwrap();
            let [first, second] = layout.local_storage_slots.as_slice() else {
                panic!("expected a frame slot for each of the two spilled registers");
            };
            assert!(first.frame_offset_bytes >= layout.outgoing_abi_area.byte_size);
            assert_eq!(second.frame_offset_bytes, first.frame_offset_bytes + 8);
            assert!(second.frame_offset_bytes + 8 <= layout.frame_size_bytes);
            assert_eq!(
                [first.id, second.id],
                [slots[0].id, slots[1].id],
                "each spill offset belongs to the register whose value it holds"
            );
            assert!(
                function_layout(
                    &environment,
                    abi,
                    TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
                    semantic_vocabulary::MachineId::new(1).unwrap(),
                    true,
                    false,
                    &[],
                    &[slots[0].clone(), slots[0].clone()],
                    Vec::new(),
                    0,
                    Vec::new()
                )
                .is_err()
            );
        }
    }

    #[test]
    fn owned_overaligned_homes_keep_fixed_x64_stack_geometry() {
        use selected_instructions::{
            LocalStorageSlotId, OutgoingArgumentSlotId, OutgoingArgumentSlotRole,
            SelectedLocalStorageSlot, SelectedOutgoingArgumentSlot,
        };
        use semantic_vocabulary::{OperationId, PlaceId};
        for target in [NativeTarget::linux_x64(), NativeTarget::windows_x64()] {
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let abi = register_environment::selected_abi_preservation(&environment)
                .unwrap()
                .kind;
            let outgoing = SelectedOutgoingArgumentSlot {
                id: OutgoingArgumentSlotId {
                    role: OutgoingArgumentSlotRole::ValueCopy,
                    invocation_site: OperationId::new(11).unwrap().into(),
                    argument_index: 0,
                },
                byte_size: 32,
                alignment: 32,
                abi_stack_byte_offset: if target == NativeTarget::windows_x64() {
                    32
                } else {
                    0
                },
            };
            let local = SelectedLocalStorageSlot {
                id: LocalStorageSlotId::Structural {
                    operation: OperationId::new(12).unwrap(),
                    place: PlaceId::new(1).unwrap(),
                },
                byte_size: 64,
                alignment: 64,
            };
            let layout = frame(
                &environment,
                abi,
                CaseShape::Call,
                (
                    std::slice::from_ref(&outgoing),
                    std::slice::from_ref(&local),
                ),
            )
            .unwrap();
            assert_eq!(layout.abi_stack_alignment_bytes, 16);
            assert_eq!(layout.frame_size_bytes % 16, 8);
            assert_eq!(layout.red_zone_resident_bytes, 0);
            let placed = &layout.local_storage_slots[0];
            let outgoing_end = u64::from(outgoing.abi_stack_byte_offset)
                + u64::from(outgoing.reserved_byte_size(16).unwrap());
            assert!(placed.frame_offset_bytes >= outgoing_end);
            assert!(
                placed.frame_offset_bytes + placed.reserved_size_bytes(16)
                    <= layout.frame_size_bytes
            );
            for (start, size, alignment, reserved) in [
                (
                    u64::from(outgoing.abi_stack_byte_offset),
                    32_u64,
                    32_u64,
                    u64::from(outgoing.reserved_byte_size(16).unwrap()),
                ),
                (
                    placed.frame_offset_bytes,
                    64,
                    64,
                    placed.reserved_size_bytes(16),
                ),
            ] {
                for stack_residue in (0..alignment).step_by(16) {
                    let base = stack_residue + start;
                    let address = (base + alignment - 1) & !(alignment - 1);
                    assert_eq!(address % alignment, 0);
                    assert!(address >= base && address + size <= base + reserved);
                }
            }
            let mut invalid = outgoing;
            invalid.id.role = OutgoingArgumentSlotRole::Argument;
            assert_eq!(
                frame(&environment, abi, CaseShape::Call, (&[invalid], &[])),
                Err(TargetFrameLayoutError::NonCanonicalLayout),
            );
        }
    }

    #[test]
    fn narrow_outgoing_payload_aligns_preservation_without_activation_locals() {
        for (target, abi, stack_offset, save_name) in [
            t!(NativeTarget::linux_x64() => SystemVAMD64 => 0 => "rbx"),
            t!(NativeTarget::windows_x64() => MicrosoftX64 => 32 => "rbx"),
            t!(NativeTarget::linux_arm64() => Aapcs64 => 0 => "x19"),
            t!(NativeTarget::macos_arm64() => DarwinAapcs64 => 0 => "x19"),
        ] {
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let outgoing = selected_instructions::SelectedOutgoingArgumentSlot {
                id: selected_instructions::OutgoingArgumentSlotId {
                    role: selected_instructions::OutgoingArgumentSlotRole::Argument,
                    invocation_site: semantic_vocabulary::OperationId::new(11).unwrap().into(),
                    argument_index: 8,
                },
                byte_size: 4,
                alignment: 4,
                abi_stack_byte_offset: stack_offset,
            };
            let save = CalleeSaveFrameSlot {
                abstract_slot: machine_code::NonAuthoritativeCalleeSaveSlotId(0),
                storage_view: environment
                    .physical()
                    .model()
                    .view_named(save_name)
                    .unwrap()
                    .id,
                frame_offset_bytes: 0,
                size_bytes: 8,
                alignment_bytes: 8,
            };
            let layout = frame_with_saves(
                &environment,
                abi,
                true,
                false,
                std::slice::from_ref(&outgoing),
                &[],
                8,
                vec![save],
            )
            .unwrap();
            let outgoing_end = u64::from(stack_offset) + 4;
            assert_eq!(layout.outgoing_abi_area.byte_size, outgoing_end);
            assert!(layout.local_storage_slots.is_empty());
            let saved = &layout.callee_save_slots[0];
            assert_eq!(saved.frame_offset_bytes, outgoing_end + 4);
            assert!(
                saved
                    .frame_offset_bytes
                    .is_multiple_of(saved.alignment_bytes)
            );
            assert!(saved.frame_offset_bytes + saved.size_bytes <= layout.frame_size_bytes);

            let without_save =
                frame(&environment, abi, CaseShape::Call, (&[outgoing], &[])).unwrap();
            assert_eq!(without_save.outgoing_abi_area.byte_size, outgoing_end);
            assert!(without_save.callee_save_slots.is_empty());
        }
    }

    #[test]
    fn activation_locals_are_not_outgoing_abi_storage_on_any_host() {
        let id = selected_instructions::LocalStorageSlotId::Structural {
            operation: semantic_vocabulary::OperationId::new(7).unwrap(),
            place: semantic_vocabulary::PlaceId::new(11).unwrap(),
        };
        let local = selected_instructions::SelectedLocalStorageSlot {
            id,
            byte_size: 24,
            alignment: 8,
        };
        for target in [
            NativeTarget::windows_x64(),
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
        ] {
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let abi = register_environment::selected_abi_preservation(&environment)
                .unwrap()
                .kind;
            let layout = frame(
                &environment,
                abi,
                CaseShape::Call,
                (&[], std::slice::from_ref(&local)),
            )
            .unwrap();
            let outgoing = if abi == FrameAbiPreservationConvention::MicrosoftX64 {
                32
            } else {
                0
            };
            assert_eq!(layout.outgoing_abi_area.byte_size, outgoing);
            assert_eq!(layout.local_storage_slots[0].id, id);
            assert_eq!(layout.local_storage_slots[0].frame_offset_bytes, outgoing);
            assert_eq!(layout.local_storage_slots[0].size_bytes, 24);
            assert!(layout.frame_size_bytes >= outgoing + 24);
            assert!(
                frame(
                    &environment,
                    abi,
                    CaseShape::Call,
                    (&[], &[local.clone(), local.clone()])
                )
                .is_err()
            );
        }
    }

    #[test]
    fn outgoing_pointer_slots_reserve_storage_on_every_host_abi() {
        for (target, abi, offset) in [
            t!(NativeTarget::windows_x64() => MicrosoftX64 => 32),
            t!(NativeTarget::linux_x64() => SystemVAMD64 => 0),
            t!(NativeTarget::linux_arm64() => Aapcs64 => 0),
            t!(NativeTarget::macos_arm64() => DarwinAapcs64 => 0),
        ] {
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let slot = selected_instructions::SelectedOutgoingArgumentSlot {
                id: selected_instructions::OutgoingArgumentSlotId {
                    role: selected_instructions::OutgoingArgumentSlotRole::Argument,
                    invocation_site: semantic_vocabulary::OperationId::new(11).unwrap().into(),
                    argument_index: 8,
                },
                byte_size: 8,
                alignment: 8,
                abi_stack_byte_offset: offset,
            };
            let layout = frame(
                &environment,
                abi,
                CaseShape::Call,
                (std::slice::from_ref(&slot), &[]),
            )
            .unwrap();
            assert_eq!(layout.outgoing_abi_area.byte_size, u64::from(offset) + 8);
            assert!(layout.frame_size_bytes >= u64::from(offset) + 8);
            assert!(frame(&environment, abi, CaseShape::Bare, (&[slot], &[])).is_err());
        }
    }

    #[test]
    fn large_frames_commit_through_an_exact_probe_roster() {
        let local = selected_instructions::SelectedLocalStorageSlot {
            id: selected_instructions::LocalStorageSlotId::Structural {
                operation: semantic_vocabulary::OperationId::new(7).unwrap(),
                place: semantic_vocabulary::PlaceId::new(11).unwrap(),
            },
            byte_size: 5_000,
            alignment: 8,
        };
        for (target, abi, interval, touches) in [
            t!(NativeTarget::linux_x64() => SystemVAMD64 => 4_096_u64 => 2_u32),
            t!(NativeTarget::windows_x64() => MicrosoftX64 => 4_096 => 2),
            t!(NativeTarget::uefi_x64() => MicrosoftX64 => 4_096 => 2),
            t!(NativeTarget::linux_arm64() => Aapcs64 => 4_096 => 2),
            t!(NativeTarget::macos_arm64() => DarwinAapcs64 => 16_384 => 0),
        ] {
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let layout = frame(
                &environment,
                abi,
                CaseShape::Bare,
                (&[], std::slice::from_ref(&local)),
            )
            .unwrap();
            assert_eq!(layout.stack_probe.interval_bytes, interval, "{target:?}");
            assert_eq!(layout.stack_probe.touches, touches, "{target:?}");
            let small = selected_instructions::SelectedLocalStorageSlot {
                byte_size: 24,
                ..local.clone()
            };
            let small_layout = frame(
                &environment,
                abi,
                CaseShape::Bare,
                (&[], std::slice::from_ref(&small)),
            )
            .unwrap();
            assert_eq!(
                small_layout.stack_probe.interval_bytes, interval,
                "{target:?}"
            );
            assert_eq!(small_layout.stack_probe.touches, 0, "{target:?}");
        }
    }

    #[test]
    fn frames_past_the_shared_displacement_reach_are_refused_by_name() {
        for (target, abi) in [
            row!(NativeTarget::linux_x64() => FrameAbiPreservationConvention::SystemVAMD64),
            row!(NativeTarget::windows_x64() => FrameAbiPreservationConvention::MicrosoftX64),
            row!(NativeTarget::linux_arm64() => FrameAbiPreservationConvention::Aapcs64),
            row!(NativeTarget::macos_arm64() => FrameAbiPreservationConvention::DarwinAapcs64),
        ] {
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let layout = |byte_size: u32| {
                frame(
                    &environment,
                    abi,
                    CaseShape::Bare,
                    (
                        &[],
                        &[selected_instructions::SelectedLocalStorageSlot {
                            id: selected_instructions::LocalStorageSlotId::Structural {
                                operation: semantic_vocabulary::OperationId::new(7).unwrap(),
                                place: semantic_vocabulary::PlaceId::new(11).unwrap(),
                            },
                            byte_size,
                            alignment: 8,
                        }],
                    ),
                )
            };
            assert!(layout(33_554_440).unwrap().frame_size_bytes >= 33_554_440);
            assert!(layout(0x7fff_f000).unwrap().frame_size_bytes <= MAX_FRAME_BYTES);
            assert!(matches!(
                layout(0x8000_0000),
                Err(TargetFrameLayoutError::FrameExceedsAddressableExtent {
                    frame_size_bytes
                }) if frame_size_bytes > MAX_FRAME_BYTES
            ));
        }
    }

    #[test]
    fn system_v_leaf_spill_storage_inside_the_red_zone_commits_nothing() {
        let environment =
            register_environment::baseline_target_register_environment(NativeTarget::linux_x64())
                .unwrap();
        for slots in [
            vec![spill(0)],
            vec![spill(0), spill(1), spill(2)],
            (0..16).map(spill).collect::<Vec<_>>(),
        ] {
            let extent = u64::try_from(slots.len()).unwrap() * 8;
            let layout = frame(
                &environment,
                FrameAbiPreservationConvention::SystemVAMD64,
                CaseShape::Bare,
                (&[], &slots),
            )
            .unwrap();
            assert_eq!(layout.frame_size_bytes, extent);
            assert_eq!(layout.red_zone_resident_bytes, extent);
            assert_eq!(
                layout.return_address,
                ReturnAddressFrameCustody::CallerActivationStack {
                    post_prologue_offset_bytes: 0,
                    size_bytes: 8,
                }
            );
            assert_eq!(layout.stack_probe.touches, 0);
        }
        let over: Vec<_> = (0..17).map(spill).collect();
        let layout = frame(
            &environment,
            FrameAbiPreservationConvention::SystemVAMD64,
            CaseShape::Bare,
            (&[], &over),
        )
        .unwrap();
        assert_eq!(layout.red_zone_resident_bytes, 0);
        assert_eq!(layout.frame_size_bytes, 136);
        assert_eq!(
            layout.return_address,
            ReturnAddressFrameCustody::CallerActivationStack {
                post_prologue_offset_bytes: 136,
                size_bytes: 8,
            }
        );
    }

    #[test]
    fn red_zone_residency_requires_leaf_spill_only_sysv_storage() {
        let structural = selected_instructions::SelectedLocalStorageSlot {
            id: selected_instructions::LocalStorageSlotId::Structural {
                operation: semantic_vocabulary::OperationId::new(7).unwrap(),
                place: semantic_vocabulary::PlaceId::new(11).unwrap(),
            },
            byte_size: 8,
            alignment: 8,
        };
        let sysv =
            register_environment::baseline_target_register_environment(NativeTarget::linux_x64())
                .unwrap();
        for (contains_call, locals, save_area, saves) in [
            (true, vec![spill(0)], 0, Vec::new()),
            (false, vec![structural.clone()], 0, Vec::new()),
            (false, vec![spill(0), structural.clone()], 0, Vec::new()),
            (
                false,
                vec![spill(0)],
                8,
                vec![CalleeSaveFrameSlot {
                    abstract_slot: machine_code::NonAuthoritativeCalleeSaveSlotId(0),
                    storage_view: sysv.physical().model().view_named("rbx").unwrap().id,
                    frame_offset_bytes: 0,
                    size_bytes: 8,
                    alignment_bytes: 8,
                }],
            ),
        ] {
            let layout = frame_with_saves(
                &sysv,
                FrameAbiPreservationConvention::SystemVAMD64,
                contains_call,
                false,
                &[],
                &locals,
                save_area,
                saves,
            )
            .unwrap();
            assert_eq!(layout.red_zone_resident_bytes, 0, "{locals:?}");
            assert!(layout.frame_size_bytes != 0);
            assert_eq!(
                layout.return_address,
                ReturnAddressFrameCustody::CallerActivationStack {
                    post_prologue_offset_bytes: layout.frame_size_bytes,
                    size_bytes: 8,
                }
            );
        }
        for (target, abi) in [
            row!(NativeTarget::windows_x64() => FrameAbiPreservationConvention::MicrosoftX64),
            row!(NativeTarget::uefi_x64() => FrameAbiPreservationConvention::MicrosoftX64),
            row!(NativeTarget::linux_arm64() => FrameAbiPreservationConvention::Aapcs64),
            row!(NativeTarget::macos_arm64() => FrameAbiPreservationConvention::DarwinAapcs64),
        ] {
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let layout = frame(&environment, abi, CaseShape::Bare, (&[], &[spill(0)])).unwrap();
            assert_eq!(layout.red_zone_resident_bytes, 0, "{target:?}");
            assert!(layout.frame_size_bytes != 0, "{target:?}");
        }
        let empty = frame(
            &sysv,
            FrameAbiPreservationConvention::SystemVAMD64,
            CaseShape::Bare,
            (&[], &[]),
        )
        .unwrap();
        assert_eq!(empty.frame_size_bytes, 0);
        assert_eq!(empty.red_zone_resident_bytes, 0);
    }

    #[test]
    fn unwind_matrix_fails_closed_on_conventions_the_pair_does_not_declare() {
        for (target, abi) in [
            row!(NativeTarget::linux_x64() => FrameAbiPreservationConvention::MicrosoftX64),
            row!(NativeTarget::windows_x64() => FrameAbiPreservationConvention::SystemVAMD64),
            row!(NativeTarget::uefi_x64() => FrameAbiPreservationConvention::SystemVAMD64),
            row!(NativeTarget::linux_arm64() => FrameAbiPreservationConvention::DarwinAapcs64),
            row!(NativeTarget::macos_arm64() => FrameAbiPreservationConvention::Aapcs64),
        ] {
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            assert_eq!(
                frame(&environment, abi, CaseShape::Bare, (&[], &[])),
                Err(TargetFrameLayoutError::UnsupportedTarget),
                "{target:?} must refuse the foreign convention {abi:?}"
            );
        }
    }

    #[test]
    fn windows_frames_separate_shadow_space_from_preservation_storage() {
        let environment =
            register_environment::baseline_target_register_environment(NativeTarget::windows_x64())
                .unwrap();
        for (area, extent) in [(0, 0), (8, 8), (16, 24)] {
            let layout = frame_with_saves(
                &environment,
                FrameAbiPreservationConvention::MicrosoftX64,
                false,
                false,
                &[],
                &[],
                area,
                Vec::new(),
            )
            .unwrap();
            assert_eq!(layout.frame_size_bytes, extent);
            assert_eq!(layout.outgoing_abi_area.byte_size, 0);
            assert_eq!(layout.outgoing_abi_area.shadow_bytes, 0);
            assert_eq!(
                layout.return_address,
                ReturnAddressFrameCustody::CallerActivationStack {
                    post_prologue_offset_bytes: extent,
                    size_bytes: 8,
                }
            );
        }
        for (area, expected_extent) in [(0, 40), (8, 40), (16, 56)] {
            let slots = if area == 0 {
                Vec::new()
            } else {
                vec![CalleeSaveFrameSlot {
                    abstract_slot: machine_code::NonAuthoritativeCalleeSaveSlotId(0),
                    storage_view: environment.physical().model().view_named("rbx").unwrap().id,
                    frame_offset_bytes: 0,
                    size_bytes: area,
                    alignment_bytes: 8,
                }]
            };
            let layout = frame_with_saves(
                &environment,
                FrameAbiPreservationConvention::MicrosoftX64,
                true,
                false,
                &[],
                &[],
                area,
                slots,
            )
            .unwrap();
            assert_eq!(layout.outgoing_abi_area.byte_size, 32);
            assert_eq!(layout.outgoing_abi_area.shadow_bytes, 32);
            assert_eq!(layout.frame_size_bytes, expected_extent);
            assert!(
                layout
                    .callee_save_slots
                    .iter()
                    .all(|slot| slot.frame_offset_bytes == 32)
            );
        }
        assert_eq!(
            frame_with_saves(
                &environment,
                FrameAbiPreservationConvention::MicrosoftX64,
                true,
                false,
                &[],
                &[],
                u64::MAX,
                Vec::new()
            ),
            Err(TargetFrameLayoutError::GeometryOverflow)
        );
    }

    #[test]
    fn windows_arm64_keeps_saves_and_link_below_large_locals() {
        // 100 spill slots put the locals past the 504 bytes a Windows ARM64
        // unwind code can reach, so the saves must sit below them.
        let environment = register_environment::baseline_target_register_environment(
            NativeTarget::windows_arm64(),
        )
        .unwrap();
        let model = environment.physical().model();
        let link = model.view_named("x30").unwrap().id;
        let x19 = model.view_named("x19").unwrap().id;
        let locals = (0..100).map(spill).collect::<Vec<_>>();
        let save = CalleeSaveFrameSlot {
            abstract_slot: machine_code::NonAuthoritativeCalleeSaveSlotId(0),
            storage_view: x19,
            frame_offset_bytes: 0,
            size_bytes: 8,
            alignment_bytes: 8,
        };
        let layout = frame_with_saves(
            &environment,
            FrameAbiPreservationConvention::DarwinAapcs64,
            true,
            false,
            &[],
            &locals,
            8,
            vec![save],
        )
        .unwrap();
        assert_eq!(layout.callee_save_slots[0].frame_offset_bytes, 0);
        assert_eq!(
            layout.return_address,
            ReturnAddressFrameCustody::SavedLinkRegister {
                view: link,
                frame_offset_bytes: 8,
                size_bytes: 8,
            }
        );
        assert_eq!(layout.local_storage_slots[0].frame_offset_bytes, 16);
        assert_eq!(
            layout.local_storage_slots[99].frame_offset_bytes,
            16 + 99 * 8
        );
        assert_eq!(layout.frame_size_bytes, 816);
        assert_eq!(layout.unwind.released_bytes, 816);

        // The same body on macOS keeps its saves above the locals.
        let darwin =
            register_environment::baseline_target_register_environment(NativeTarget::macos_arm64())
                .unwrap();
        let darwin_layout = frame_with_saves(
            &darwin,
            FrameAbiPreservationConvention::DarwinAapcs64,
            true,
            false,
            &[],
            &locals,
            8,
            vec![CalleeSaveFrameSlot {
                storage_view: darwin.physical().model().view_named("x19").unwrap().id,
                ..save
            }],
        )
        .unwrap();
        assert_eq!(darwin_layout.local_storage_slots[0].frame_offset_bytes, 0);
        assert_eq!(darwin_layout.callee_save_slots[0].frame_offset_bytes, 800);
        assert_eq!(darwin_layout.frame_size_bytes, 816);
    }

    #[test]
    fn allocated_link_register_write_gets_a_leaf_frame_slot() {
        for target in [NativeTarget::macos_arm64(), NativeTarget::linux_arm64()] {
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let link = environment.physical().model().view_named("x30").unwrap().id;
            let abi = register_environment::selected_abi_preservation(&environment)
                .unwrap()
                .kind;
            let layout = frame(&environment, abi, CaseShape::Link, (&[], &[])).unwrap();
            assert!(!layout.contains_call);
            assert_eq!(layout.frame_size_bytes, 16);
            assert_eq!(
                layout.return_address,
                ReturnAddressFrameCustody::SavedLinkRegister {
                    view: link,
                    frame_offset_bytes: 0,
                    size_bytes: 8,
                }
            );
        }
    }
}
