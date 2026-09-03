//! Rebound named-dynamic descriptor and indirect-call emission.

use omega_assigned_target_operations::{
    AssignedAggregateCopy, AssignedFunction, AssignedUnitOperation,
};
use omega_machine_code::{
    DynamicCallRecord, DynamicInstanceMaterializationRecord, DynamicTableAddressEncoding,
    DynamicTableAddressMaterialization, DynamicTraitDescriptorAbiRecord,
    InternalUnitCallArgumentRecord, StoredDynamicCallRecord,
    StoredDynamicDescriptorMaterializationRecord, UnitCallStackEvidence,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};

use super::scalar_call::emit_unit_scalar_result;
use super::{
    Aarch64UnitParameterHome, X86UnitParameterHome, aarch64_outgoing_placement_extent, align_u32,
    emit_aarch64_aggregate_copy_from_home, emit_x86_64_aggregate_copy_from_home,
    outgoing_placement_extent,
};
use crate::{
    EmissionError, aarch64_load_base, aarch64_store_base, aarch64_unit_memory_access,
    aarch64_unit_stack_access, append_aarch64_instructions, emit_aarch64_adjust_sp,
    emit_aarch64_sp_address, emit_x86_64_adjust_sp, emit_x86_64_memory_load_width,
    emit_x86_64_stack_load_width, emit_x86_64_stack_store_width, stack_adjustment_pair,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_stored_descriptor(
    operation: &AssignedUnitOperation,
    owner: psi_core::MachineId,
    target: NativeTarget,
    functions: &[AssignedFunction],
    x86_homes: &[X86UnitParameterHome],
    aarch64_homes: &[Aarch64UnitParameterHome],
    bytes: &mut Vec<u8>,
    operation_ordinal: usize,
    code_offset: usize,
) -> Result<StoredDynamicDescriptorMaterializationRecord, EmissionError> {
    let AssignedUnitOperation::StoreDynamicDescriptor {
        psi_operation,
        stored,
        descriptor_abi,
        descriptor_home_byte_offset,
        source_copy,
    } = operation
    else {
        unreachable!("stored-descriptor router supplied another operation")
    };
    let invalid = || EmissionError::InvalidStoredDynamicDescriptorCustody(*psi_operation);
    if !stored.has_complete_custody(owner, *psi_operation)
        || source_copy.access == psi_terminal::StructuralAccess::Owned
        || !copy_matches_selection(source_copy, &stored.selection)
        || stored
            .application
            .realization_callables
            .iter()
            .any(|callable| {
                functions
                    .iter()
                    .filter(|function| function.machine == callable.machine)
                    .count()
                    != 1
            })
    {
        return Err(invalid());
    }
    let descriptor = descriptor_record(*descriptor_abi, target, &invalid)?;
    let descriptor_word_byte_size =
        u16::try_from(descriptor.word_byte_size).map_err(|_| invalid())?;
    let (instance, table_address) = match target.architecture {
        Architecture::X86_64 => {
            let instance = emit_x86_64_instance(
                bytes,
                *descriptor_home_byte_offset,
                source_copy,
                x86_homes,
                stored.selection.ordinal,
            )?;
            let table_offset = bytes.len();
            bytes.extend_from_slice(&[0x4c, 0x8d, 0x15]);
            let relocation_offset = bytes.len();
            bytes.extend_from_slice(&0_i32.to_le_bytes());
            emit_x86_64_stack_store_width(
                bytes,
                10,
                descriptor_home_byte_offset
                    .checked_add(descriptor.table_byte_offset)
                    .ok_or_else(invalid)?,
                descriptor_word_byte_size,
            )?;
            (
                instance,
                DynamicTableAddressMaterialization {
                    code_offset: table_offset,
                    byte_count: bytes.len() - table_offset,
                    encoding: DynamicTableAddressEncoding::X86_64Relative32 { relocation_offset },
                },
            )
        }
        Architecture::Aarch64 => {
            let instance = emit_aarch64_instance(
                bytes,
                *descriptor_home_byte_offset,
                source_copy,
                aarch64_homes,
                stored.selection.ordinal,
            )?;
            let table_offset = bytes.len();
            let page_relocation_offset = bytes.len();
            bytes.extend_from_slice(&(0x9000_0000 | 10_u32).to_le_bytes());
            let page_offset_relocation_offset = bytes.len();
            bytes.extend_from_slice(&(0x9100_0000 | (10_u32 << 5) | 10_u32).to_le_bytes());
            bytes.extend_from_slice(
                &aarch64_unit_stack_access(
                    aarch64_store_base(descriptor_word_byte_size)?,
                    10,
                    descriptor_home_byte_offset
                        .checked_add(descriptor.table_byte_offset)
                        .ok_or_else(invalid)?,
                    descriptor_word_byte_size,
                )?
                .to_le_bytes(),
            );
            (
                instance,
                DynamicTableAddressMaterialization {
                    code_offset: table_offset,
                    byte_count: bytes.len() - table_offset,
                    encoding: DynamicTableAddressEncoding::Aarch64PageAddress {
                        page_relocation_offset,
                        page_offset_relocation_offset,
                    },
                },
            )
        }
    };
    Ok(StoredDynamicDescriptorMaterializationRecord {
        psi_operation: *psi_operation,
        stored: stored.clone(),
        descriptor_abi: descriptor,
        descriptor_home_byte_offset: *descriptor_home_byte_offset,
        instance,
        table_address,
        operation_ordinal,
        code_offset,
        byte_count: bytes.len() - code_offset,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_stored_dynamic_call(
    operation: &AssignedUnitOperation,
    owner: psi_core::MachineId,
    target: NativeTarget,
    functions: &[AssignedFunction],
    bytes: &mut Vec<u8>,
    establishments: &[StoredDynamicDescriptorMaterializationRecord],
    operation_ordinal: usize,
    code_offset: usize,
) -> Result<StoredDynamicCallRecord, EmissionError> {
    let AssignedUnitOperation::StoredDynamicScalarCall {
        psi_operation,
        result,
        dynamic_dispatch,
        call_plan,
        result_home,
        descriptor_abi,
        descriptor_home_byte_offset,
        source_copy,
        ..
    } = operation
    else {
        unreachable!("stored-call router supplied another operation")
    };
    let invalid = || EmissionError::InvalidStoredDynamicCallCustody(*psi_operation);
    let descriptor = descriptor_record(*descriptor_abi, target, &invalid)?;
    let matching = establishments
        .iter()
        .filter(|establishment| {
            establishment.stored == dynamic_dispatch.stored
                && establishment.descriptor_abi == descriptor
                && establishment.descriptor_home_byte_offset == *descriptor_home_byte_offset
                && establishment.instance.source == target_argument(source_copy)
                && establishment.operation_ordinal < operation_ordinal
        })
        .collect::<Vec<_>>();
    let [establishment] = matching.as_slice() else {
        return Err(invalid());
    };
    if result.value != result_home.source_value
        || result.scalar_type != result_home.scalar_type
        || result_home.defining_operation != *psi_operation
        || !dynamic_dispatch.has_complete_custody(owner, *psi_operation)
        || call_plan.result.as_ref().map(|placement| placement.shape) != Some(result_home.shape)
        || call_plan.parameters.as_slice() != std::slice::from_ref(&source_copy.destination)
        || source_copy.access == psi_terminal::StructuralAccess::Owned
        || !copy_matches_selection(source_copy, &dynamic_dispatch.stored.selection)
        || functions
            .iter()
            .filter(|function| function.machine == dynamic_dispatch.dispatch.realization)
            .count()
            != 1
    {
        return Err(invalid());
    }
    let selected_row = dynamic_dispatch
        .stored
        .application
        .rows
        .iter()
        .position(|row| {
            row.declaring_trait_identity == dynamic_dispatch.dispatch.declaring_trait_identity
                && row.public_requirement_identity
                    == dynamic_dispatch.dispatch.public_requirement_identity
                && row.requirement_identity == dynamic_dispatch.dispatch.requirement_identity
                && row.realization_identity == dynamic_dispatch.dispatch.realization_identity
                && row.realization_callable_identity.as_deref()
                    == Some(
                        dynamic_dispatch
                            .dispatch
                            .realization_callable_identity
                            .as_str(),
                    )
        })
        .ok_or_else(invalid)?;
    let callable = dynamic_dispatch
        .stored
        .application
        .realization_callables
        .iter()
        .find(|callable| {
            callable.machine == dynamic_dispatch.dispatch.realization
                && callable.source_callable_identity
                    == dynamic_dispatch.dispatch.realization_callable_identity
        })
        .ok_or_else(invalid)?;
    let callable_result_matches = match (callable.result, result.scalar_type) {
        (psi_terminal::ClosedConformanceCallableResult::Bool, psi_core::ScalarType::Boolean) => {
            true
        }
        (
            psi_terminal::ClosedConformanceCallableResult::I32,
            psi_core::ScalarType::Integer(integer),
        ) => psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 32)
            .is_ok_and(|expected| integer == expected),
        _ => false,
    };
    if !callable_result_matches {
        return Err(invalid());
    }
    let selected_table_byte_offset = u32::try_from(selected_row)
        .ok()
        .and_then(|slot| slot.checked_mul(descriptor.word_byte_size))
        .ok_or_else(invalid)?;
    let (argument, call_offset, call_width, stack) = match target.architecture {
        Architecture::X86_64 => emit_x86_64_stored_call(
            bytes,
            target,
            *psi_operation,
            *descriptor_home_byte_offset,
            source_copy,
            call_plan,
            selected_table_byte_offset,
        )?,
        Architecture::Aarch64 => emit_aarch64_stored_call(
            bytes,
            *psi_operation,
            *descriptor_home_byte_offset,
            source_copy,
            call_plan,
            selected_table_byte_offset,
        )?,
    };
    let result_record = emit_unit_scalar_result(
        bytes,
        target.architecture,
        *psi_operation,
        call_plan,
        *result_home,
    )?;
    Ok(StoredDynamicCallRecord {
        establishment: (*establishment).clone(),
        psi_operation: *psi_operation,
        dynamic_dispatch: dynamic_dispatch.clone(),
        call_plan: call_plan.clone(),
        result: result_record,
        argument,
        selected_table_byte_offset,
        indirect_call_offset: call_offset,
        indirect_call_byte_count: call_width,
        unit_stack: stack,
        operation_ordinal,
        code_offset,
        byte_count: bytes.len() - code_offset,
    })
}

fn descriptor_record(
    descriptor_abi: omega_assigned_target_operations::AssignedDynamicTraitDescriptorAbi,
    target: NativeTarget,
    invalid: &impl Fn() -> EmissionError,
) -> Result<DynamicTraitDescriptorAbiRecord, EmissionError> {
    let descriptor = DynamicTraitDescriptorAbiRecord {
        instance_byte_offset: descriptor_abi.instance_offset(),
        table_byte_offset: descriptor_abi.table_offset(),
        word_byte_size: descriptor_abi.word_size(),
        total_byte_size: descriptor_abi.total_size(),
        byte_alignment: descriptor_abi.align(),
    };
    let pointer_size = u32::try_from(target.pointer_size).map_err(|_| invalid())?;
    let pointer_alignment = u32::try_from(target.pointer_alignment).map_err(|_| invalid())?;
    if descriptor.instance_byte_offset != 0
        || descriptor.table_byte_offset != pointer_size
        || descriptor.word_byte_size != pointer_size
        || descriptor.total_byte_size != pointer_size.checked_mul(2).ok_or_else(invalid)?
        || descriptor.byte_alignment != pointer_alignment
    {
        return Err(invalid());
    }
    Ok(descriptor)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_dynamic_call(
    operation: &AssignedUnitOperation,
    owner: psi_core::MachineId,
    target: NativeTarget,
    functions: &[AssignedFunction],
    x86_homes: &[X86UnitParameterHome],
    aarch64_homes: &[Aarch64UnitParameterHome],
    bytes: &mut Vec<u8>,
    operation_ordinal: usize,
    code_offset: usize,
) -> Result<DynamicCallRecord, EmissionError> {
    let (
        psi_operation,
        dynamic_dispatch,
        call_plan,
        result,
        descriptor_abi,
        descriptor_home_byte_offset,
        initial_copy,
        rebound_copy,
    ) = match operation {
        AssignedUnitOperation::DynamicScalarCall {
            psi_operation,
            result,
            dynamic_dispatch,
            call_plan,
            result_home,
            descriptor_abi,
            descriptor_home_byte_offset,
            initial_copy,
            rebound_copy,
            ..
        } => (
            psi_operation,
            dynamic_dispatch,
            call_plan,
            Some((result, result_home)),
            descriptor_abi,
            descriptor_home_byte_offset,
            initial_copy,
            rebound_copy,
        ),
        AssignedUnitOperation::DynamicUnitCall {
            psi_operation,
            dynamic_dispatch,
            call_plan,
            descriptor_abi,
            descriptor_home_byte_offset,
            initial_copy,
            rebound_copy,
            ..
        } => (
            psi_operation,
            dynamic_dispatch,
            call_plan,
            None,
            descriptor_abi,
            descriptor_home_byte_offset,
            initial_copy,
            rebound_copy,
        ),
        _ => unreachable!("dynamic-call router supplied another operation"),
    };
    let invalid = || EmissionError::InvalidDynamicCallCustody(*psi_operation);
    if result.is_some_and(|(result, result_home)| {
        !matches!(
            result.scalar_type,
            psi_core::ScalarType::Boolean | psi_core::ScalarType::Integer(_)
        ) || result.scalar_type != result_home.scalar_type
            || result.value != result_home.source_value
            || result_home.defining_operation != *psi_operation
    }) || !dynamic_dispatch.has_complete_application_custody(owner, *psi_operation)
        || call_plan.result.as_ref().map(|placement| placement.shape)
            != result.map(|(_, home)| home.shape)
        || call_plan.parameters.as_slice() != std::slice::from_ref(&initial_copy.destination)
        || initial_copy.destination != rebound_copy.destination
        || initial_copy.shape != rebound_copy.shape
        || initial_copy.structural_type != rebound_copy.structural_type
        || initial_copy.access == psi_terminal::StructuralAccess::Owned
        || rebound_copy.access == psi_terminal::StructuralAccess::Owned
        || !copy_matches_selection(initial_copy, &dynamic_dispatch.initial)
        || !copy_matches_selection(rebound_copy, &dynamic_dispatch.rebound)
        || functions
            .iter()
            .filter(|function| function.machine == dynamic_dispatch.dispatch.realization)
            .count()
            != 1
    {
        return Err(invalid());
    }
    let selected_row = dynamic_dispatch
        .application
        .rows
        .iter()
        .position(|row| {
            row.declaring_trait_identity == dynamic_dispatch.dispatch.declaring_trait_identity
                && row.public_requirement_identity
                    == dynamic_dispatch.dispatch.public_requirement_identity
                && row.requirement_identity == dynamic_dispatch.dispatch.requirement_identity
                && row.realization_identity == dynamic_dispatch.dispatch.realization_identity
                && row.realization_callable_identity.as_deref()
                    == Some(
                        dynamic_dispatch
                            .dispatch
                            .realization_callable_identity
                            .as_str(),
                    )
        })
        .ok_or_else(invalid)?;
    let selected_callable = dynamic_dispatch
        .application
        .realization_callables
        .iter()
        .find(|callable| {
            callable.machine == dynamic_dispatch.dispatch.realization
                && callable.source_callable_identity
                    == dynamic_dispatch.dispatch.realization_callable_identity
        })
        .ok_or_else(invalid)?;
    let emitted_result = result.map(|(result, _)| result.scalar_type);
    let callable_result_matches = match (selected_callable.result, emitted_result) {
        (psi_terminal::ClosedConformanceCallableResult::Unit, None) => true,
        (
            psi_terminal::ClosedConformanceCallableResult::I32,
            Some(psi_core::ScalarType::Integer(integer)),
        ) => psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 32)
            .is_ok_and(|expected| integer == expected),
        (
            psi_terminal::ClosedConformanceCallableResult::Bool,
            Some(psi_core::ScalarType::Boolean),
        ) => true,
        _ => false,
    };
    if !callable_result_matches {
        return Err(invalid());
    }
    if dynamic_dispatch
        .application
        .realization_callables
        .iter()
        .any(|callable| {
            functions
                .iter()
                .filter(|function| function.machine == callable.machine)
                .count()
                != 1
        })
    {
        return Err(invalid());
    }
    let selected_table_byte_offset = u32::try_from(selected_row)
        .ok()
        .and_then(|slot| slot.checked_mul(descriptor_abi.word_size()))
        .ok_or_else(invalid)?;
    let descriptor = DynamicTraitDescriptorAbiRecord {
        instance_byte_offset: descriptor_abi.instance_offset(),
        table_byte_offset: descriptor_abi.table_offset(),
        word_byte_size: descriptor_abi.word_size(),
        total_byte_size: descriptor_abi.total_size(),
        byte_alignment: descriptor_abi.align(),
    };
    if descriptor.instance_byte_offset != 0
        || descriptor.table_byte_offset != 8
        || descriptor.word_byte_size != 8
        || descriptor.total_byte_size != 16
        || descriptor.byte_alignment != 8
    {
        return Err(invalid());
    }

    let (
        initial_instance,
        table_address,
        rebound_instance,
        argument,
        call_offset,
        call_width,
        stack,
    ) = match target.architecture {
        Architecture::X86_64 => emit_x86_64_dynamic_call(
            bytes,
            target,
            *psi_operation,
            *descriptor_home_byte_offset,
            initial_copy,
            rebound_copy,
            call_plan,
            selected_table_byte_offset,
            dynamic_dispatch.initial.ordinal,
            dynamic_dispatch.rebound.ordinal,
            x86_homes,
        )?,
        Architecture::Aarch64 => emit_aarch64_dynamic_call(
            bytes,
            *psi_operation,
            *descriptor_home_byte_offset,
            initial_copy,
            rebound_copy,
            call_plan,
            selected_table_byte_offset,
            dynamic_dispatch.initial.ordinal,
            dynamic_dispatch.rebound.ordinal,
            aarch64_homes,
        )?,
    };
    let result_record = result
        .map(|(_, result_home)| {
            emit_unit_scalar_result(
                bytes,
                target.architecture,
                *psi_operation,
                call_plan,
                *result_home,
            )
        })
        .transpose()?;
    Ok(DynamicCallRecord {
        psi_operation: *psi_operation,
        dynamic_dispatch: dynamic_dispatch.clone(),
        call_plan: call_plan.clone(),
        result: result_record,
        descriptor_abi: descriptor,
        descriptor_home_byte_offset: *descriptor_home_byte_offset,
        initial_instance,
        table_address,
        rebound_instance,
        argument,
        selected_table_byte_offset,
        indirect_call_offset: call_offset,
        indirect_call_byte_count: call_width,
        unit_stack: stack,
        operation_ordinal,
        code_offset,
        byte_count: bytes.len() - code_offset,
    })
}

fn copy_matches_selection(
    copy: &AssignedAggregateCopy,
    selection: &psi_terminal::TerminalDynamicConformanceSelection,
) -> bool {
    copy.place == selection.source.place
        && copy.access == selection.source.access
        && copy.path == selection.source.path
}

type DynamicEmission = (
    DynamicInstanceMaterializationRecord,
    DynamicTableAddressMaterialization,
    DynamicInstanceMaterializationRecord,
    InternalUnitCallArgumentRecord,
    usize,
    usize,
    UnitCallStackEvidence,
);

#[allow(clippy::too_many_arguments)]
fn emit_x86_64_dynamic_call(
    bytes: &mut Vec<u8>,
    target: NativeTarget,
    operation: psi_core::OperationId,
    descriptor_offset: u32,
    initial: &AssignedAggregateCopy,
    rebound: &AssignedAggregateCopy,
    call_plan: &omega_calling_conventions::CallPlan,
    selected_table_byte_offset: u32,
    initial_selection_ordinal: u32,
    rebound_selection_ordinal: u32,
    homes: &[X86UnitParameterHome],
) -> Result<DynamicEmission, EmissionError> {
    let initial_instance = emit_x86_64_instance(
        bytes,
        descriptor_offset,
        initial,
        homes,
        initial_selection_ordinal,
    )?;
    let table_address_offset = bytes.len();
    bytes.extend_from_slice(&[0x4c, 0x8d, 0x15]); // lea r10, [rip + disp32]
    let relocation_offset = bytes.len();
    bytes.extend_from_slice(&0_i32.to_le_bytes());
    emit_x86_64_stack_store_width(bytes, 10, descriptor_offset + 8, 8)?;
    let table_address = DynamicTableAddressMaterialization {
        code_offset: table_address_offset,
        byte_count: bytes.len() - table_address_offset,
        encoding: DynamicTableAddressEncoding::X86_64Relative32 { relocation_offset },
    };
    let rebound_instance = emit_x86_64_instance(
        bytes,
        descriptor_offset,
        rebound,
        homes,
        rebound_selection_ordinal,
    )?;

    let outgoing_bytes = outgoing_placement_extent(&rebound.destination)?.max(
        if target.object_format == ObjectFormat::Coff {
            32
        } else {
            0
        },
    );
    let padding = (8 + 16 - (outgoing_bytes % 16)) % 16;
    let call_stack_bytes = outgoing_bytes
        .checked_add(padding)
        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
    let allocation = if call_stack_bytes == 0 {
        None
    } else {
        let offset = bytes.len();
        emit_x86_64_adjust_sp(bytes, call_stack_bytes, false);
        Some((offset, bytes.len() - offset))
    };
    let argument_offset = bytes.len();
    let descriptor_home = X86UnitParameterHome {
        place: rebound.place,
        shape: rebound.shape,
        source: rebound.source.clone(),
        byte_offset: descriptor_offset,
        indirect: true,
    };
    emit_x86_64_descriptor_argument(bytes, rebound, &descriptor_home, call_stack_bytes)?;
    let argument = argument_record(
        rebound,
        descriptor_offset,
        call_stack_bytes,
        argument_offset,
        bytes,
    );
    let table_home = call_stack_bytes
        .checked_add(descriptor_offset)
        .and_then(|offset| offset.checked_add(8))
        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
    emit_x86_64_stack_load_width(bytes, 11, table_home, 8)?;
    emit_x86_64_memory_load_width(bytes, 11, 11, selected_table_byte_offset, 8)?;
    let call_offset = bytes.len();
    bytes.extend_from_slice(&[0x41, 0xff, 0xd3]); // call r11
    let release = if call_stack_bytes == 0 {
        None
    } else {
        let offset = bytes.len();
        emit_x86_64_adjust_sp(bytes, call_stack_bytes, true);
        Some((offset, bytes.len() - offset))
    };
    if call_plan.parameters.as_slice() != std::slice::from_ref(&rebound.destination) {
        return Err(EmissionError::InvalidDynamicCallCustody(operation));
    }
    Ok((
        initial_instance,
        table_address,
        rebound_instance,
        argument,
        call_offset,
        3,
        UnitCallStackEvidence {
            outbound: stack_adjustment_pair(call_stack_bytes, allocation, release),
        },
    ))
}

fn emit_x86_64_stored_call(
    bytes: &mut Vec<u8>,
    target: NativeTarget,
    operation: psi_core::OperationId,
    descriptor_offset: u32,
    source: &AssignedAggregateCopy,
    call_plan: &omega_calling_conventions::CallPlan,
    selected_table_byte_offset: u32,
) -> Result<
    (
        InternalUnitCallArgumentRecord,
        usize,
        usize,
        UnitCallStackEvidence,
    ),
    EmissionError,
> {
    let outgoing_bytes = outgoing_placement_extent(&source.destination)?.max(
        if target.object_format == ObjectFormat::Coff {
            32
        } else {
            0
        },
    );
    let padding = (8 + 16 - (outgoing_bytes % 16)) % 16;
    let call_stack_bytes = outgoing_bytes
        .checked_add(padding)
        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
    let allocation = if call_stack_bytes == 0 {
        None
    } else {
        let offset = bytes.len();
        emit_x86_64_adjust_sp(bytes, call_stack_bytes, false);
        Some((offset, bytes.len() - offset))
    };
    let argument_offset = bytes.len();
    let descriptor_home = X86UnitParameterHome {
        place: source.place,
        shape: source.shape,
        source: source.source.clone(),
        byte_offset: descriptor_offset,
        indirect: true,
    };
    emit_x86_64_descriptor_argument(bytes, source, &descriptor_home, call_stack_bytes)?;
    let argument = argument_record(
        source,
        descriptor_offset,
        call_stack_bytes,
        argument_offset,
        bytes,
    );
    let table_home = call_stack_bytes
        .checked_add(descriptor_offset)
        .and_then(|offset| offset.checked_add(8))
        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
    emit_x86_64_stack_load_width(bytes, 11, table_home, 8)?;
    emit_x86_64_memory_load_width(bytes, 11, 11, selected_table_byte_offset, 8)?;
    let call_offset = bytes.len();
    bytes.extend_from_slice(&[0x41, 0xff, 0xd3]);
    let release = if call_stack_bytes == 0 {
        None
    } else {
        let offset = bytes.len();
        emit_x86_64_adjust_sp(bytes, call_stack_bytes, true);
        Some((offset, bytes.len() - offset))
    };
    if call_plan.parameters.as_slice() != std::slice::from_ref(&source.destination) {
        return Err(EmissionError::InvalidStoredDynamicCallCustody(operation));
    }
    Ok((
        argument,
        call_offset,
        3,
        UnitCallStackEvidence {
            outbound: stack_adjustment_pair(call_stack_bytes, allocation, release),
        },
    ))
}

fn emit_x86_64_instance(
    bytes: &mut Vec<u8>,
    descriptor_offset: u32,
    copy: &AssignedAggregateCopy,
    homes: &[X86UnitParameterHome],
    selection_ordinal: u32,
) -> Result<DynamicInstanceMaterializationRecord, EmissionError> {
    let home = homes
        .iter()
        .find(|home| home.place == copy.place)
        .ok_or(EmissionError::MissingUnitParameterHome(copy.place))?;
    if home.source != copy.source
        || copy
            .source_byte_offset
            .checked_add(u32::from(copy.shape.byte_size))
            .is_none_or(|end| end > u32::from(home.shape.byte_size))
    {
        return Err(EmissionError::UnitParameterHomeMismatch(copy.place));
    }
    let code_offset = bytes.len();
    if home.indirect {
        emit_x86_64_stack_load_width(bytes, 11, home.byte_offset, 8)?;
    } else {
        emit_x86_64_stack_address(bytes, 11, home.byte_offset)?;
    }
    emit_x86_64_stack_store_width(bytes, 11, descriptor_offset, 8)?;
    Ok(DynamicInstanceMaterializationRecord {
        selection_ordinal,
        source: target_argument(copy),
        source_home_byte_offset: home.byte_offset,
        source_home_indirect: home.indirect,
        code_offset,
        byte_count: bytes.len() - code_offset,
    })
}

pub(super) fn emit_x86_64_stack_address(
    bytes: &mut Vec<u8>,
    register: u8,
    byte_offset: u32,
) -> Result<(), EmissionError> {
    bytes.push(0x48 | (((register >> 3) & 1) << 2));
    bytes.push(0x8d);
    if byte_offset == 0 {
        bytes.extend_from_slice(&[0x04 | ((register & 7) << 3), 0x24]);
    } else if byte_offset <= i8::MAX as u32 {
        bytes.extend_from_slice(&[0x44 | ((register & 7) << 3), 0x24, byte_offset as u8]);
    } else {
        bytes.extend_from_slice(&[0x84 | ((register & 7) << 3), 0x24]);
        bytes.extend_from_slice(&byte_offset.to_le_bytes());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn emit_aarch64_dynamic_call(
    bytes: &mut Vec<u8>,
    operation: psi_core::OperationId,
    descriptor_offset: u32,
    initial: &AssignedAggregateCopy,
    rebound: &AssignedAggregateCopy,
    call_plan: &omega_calling_conventions::CallPlan,
    selected_table_byte_offset: u32,
    initial_selection_ordinal: u32,
    rebound_selection_ordinal: u32,
    homes: &[Aarch64UnitParameterHome],
) -> Result<DynamicEmission, EmissionError> {
    let initial_instance = emit_aarch64_instance(
        bytes,
        descriptor_offset,
        initial,
        homes,
        initial_selection_ordinal,
    )?;
    let table_address_offset = bytes.len();
    let page_relocation_offset = bytes.len();
    bytes.extend_from_slice(&(0x9000_0000 | 10_u32).to_le_bytes()); // adrp x10
    let page_offset_relocation_offset = bytes.len();
    bytes.extend_from_slice(&(0x9100_0000 | (10_u32 << 5) | 10_u32).to_le_bytes());
    bytes.extend_from_slice(
        &aarch64_unit_stack_access(aarch64_store_base(8)?, 10, descriptor_offset + 8, 8)?
            .to_le_bytes(),
    );
    let table_address = DynamicTableAddressMaterialization {
        code_offset: table_address_offset,
        byte_count: bytes.len() - table_address_offset,
        encoding: DynamicTableAddressEncoding::Aarch64PageAddress {
            page_relocation_offset,
            page_offset_relocation_offset,
        },
    };
    let rebound_instance = emit_aarch64_instance(
        bytes,
        descriptor_offset,
        rebound,
        homes,
        rebound_selection_ordinal,
    )?;
    let call_stack_bytes = align_u32(aarch64_outgoing_placement_extent(&rebound.destination)?, 16)?;
    let allocation = if call_stack_bytes == 0 {
        None
    } else {
        let mut instructions = Vec::new();
        let offset = bytes.len();
        emit_aarch64_adjust_sp(&mut instructions, call_stack_bytes, false)?;
        append_aarch64_instructions(bytes, instructions);
        Some((offset, 4))
    };
    let argument_offset = bytes.len();
    let descriptor_home = Aarch64UnitParameterHome {
        place: rebound.place,
        shape: rebound.shape,
        source: rebound.source.clone(),
        byte_offset: descriptor_offset,
        indirect: true,
    };
    let mut instructions = Vec::new();
    emit_aarch64_descriptor_argument(
        &mut instructions,
        rebound,
        &descriptor_home,
        call_stack_bytes,
    )?;
    append_aarch64_instructions(bytes, instructions);
    let argument = argument_record(
        rebound,
        descriptor_offset,
        call_stack_bytes,
        argument_offset,
        bytes,
    );
    let table_home = call_stack_bytes
        .checked_add(descriptor_offset)
        .and_then(|offset| offset.checked_add(8))
        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
    bytes.extend_from_slice(
        &aarch64_unit_stack_access(aarch64_load_base(8)?, 9, table_home, 8)?.to_le_bytes(),
    );
    bytes.extend_from_slice(
        &aarch64_unit_memory_access(aarch64_load_base(8)?, 9, 9, selected_table_byte_offset, 8)?
            .to_le_bytes(),
    );
    let call_offset = bytes.len();
    bytes.extend_from_slice(&(0xd63f_0000 | (9_u32 << 5)).to_le_bytes()); // blr x9
    let release = if call_stack_bytes == 0 {
        None
    } else {
        let mut instructions = Vec::new();
        let offset = bytes.len();
        emit_aarch64_adjust_sp(&mut instructions, call_stack_bytes, true)?;
        append_aarch64_instructions(bytes, instructions);
        Some((offset, 4))
    };
    if call_plan.parameters.as_slice() != std::slice::from_ref(&rebound.destination) {
        return Err(EmissionError::InvalidDynamicCallCustody(operation));
    }
    Ok((
        initial_instance,
        table_address,
        rebound_instance,
        argument,
        call_offset,
        4,
        UnitCallStackEvidence {
            outbound: stack_adjustment_pair(call_stack_bytes, allocation, release),
        },
    ))
}

fn emit_aarch64_stored_call(
    bytes: &mut Vec<u8>,
    operation: psi_core::OperationId,
    descriptor_offset: u32,
    source: &AssignedAggregateCopy,
    call_plan: &omega_calling_conventions::CallPlan,
    selected_table_byte_offset: u32,
) -> Result<
    (
        InternalUnitCallArgumentRecord,
        usize,
        usize,
        UnitCallStackEvidence,
    ),
    EmissionError,
> {
    let call_stack_bytes = align_u32(aarch64_outgoing_placement_extent(&source.destination)?, 16)?;
    let allocation = if call_stack_bytes == 0 {
        None
    } else {
        let mut instructions = Vec::new();
        let offset = bytes.len();
        emit_aarch64_adjust_sp(&mut instructions, call_stack_bytes, false)?;
        append_aarch64_instructions(bytes, instructions);
        Some((offset, 4))
    };
    let argument_offset = bytes.len();
    let descriptor_home = Aarch64UnitParameterHome {
        place: source.place,
        shape: source.shape,
        source: source.source.clone(),
        byte_offset: descriptor_offset,
        indirect: true,
    };
    let mut instructions = Vec::new();
    emit_aarch64_descriptor_argument(
        &mut instructions,
        source,
        &descriptor_home,
        call_stack_bytes,
    )?;
    append_aarch64_instructions(bytes, instructions);
    let argument = argument_record(
        source,
        descriptor_offset,
        call_stack_bytes,
        argument_offset,
        bytes,
    );
    let table_home = call_stack_bytes
        .checked_add(descriptor_offset)
        .and_then(|offset| offset.checked_add(8))
        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
    bytes.extend_from_slice(
        &aarch64_unit_stack_access(aarch64_load_base(8)?, 9, table_home, 8)?.to_le_bytes(),
    );
    bytes.extend_from_slice(
        &aarch64_unit_memory_access(aarch64_load_base(8)?, 9, 9, selected_table_byte_offset, 8)?
            .to_le_bytes(),
    );
    let call_offset = bytes.len();
    bytes.extend_from_slice(&(0xd63f_0000 | (9_u32 << 5)).to_le_bytes());
    let release = if call_stack_bytes == 0 {
        None
    } else {
        let mut instructions = Vec::new();
        let offset = bytes.len();
        emit_aarch64_adjust_sp(&mut instructions, call_stack_bytes, true)?;
        append_aarch64_instructions(bytes, instructions);
        Some((offset, 4))
    };
    if call_plan.parameters.as_slice() != std::slice::from_ref(&source.destination) {
        return Err(EmissionError::InvalidStoredDynamicCallCustody(operation));
    }
    Ok((
        argument,
        call_offset,
        4,
        UnitCallStackEvidence {
            outbound: stack_adjustment_pair(call_stack_bytes, allocation, release),
        },
    ))
}

fn emit_x86_64_descriptor_argument(
    bytes: &mut Vec<u8>,
    copy: &AssignedAggregateCopy,
    home: &X86UnitParameterHome,
    call_stack_bytes: u32,
) -> Result<(), EmissionError> {
    let [omega_calling_conventions::ValueLocation::Indirect { pointer, .. }] =
        copy.destination.locations.as_slice()
    else {
        return emit_x86_64_aggregate_copy_from_home(bytes, copy, home, call_stack_bytes);
    };
    let source_home = call_stack_bytes
        .checked_add(home.byte_offset)
        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
    let register = match *pointer {
        omega_calling_conventions::IndirectPointerLocation::Register(register) => {
            super::x86_unit_register(register)?
        }
        omega_calling_conventions::IndirectPointerLocation::Stack { .. } => 11,
    };
    emit_x86_64_stack_load_width(bytes, register, source_home, 8)?;
    if copy.source_byte_offset != 0 {
        bytes.extend_from_slice(&[0x48 | ((register >> 3) & 1), 0x81, 0xc0 | (register & 7)]);
        bytes.extend_from_slice(&copy.source_byte_offset.to_le_bytes());
    }
    if let omega_calling_conventions::IndirectPointerLocation::Stack {
        stack_byte_offset, ..
    } = *pointer
    {
        emit_x86_64_stack_store_width(bytes, register, stack_byte_offset, 8)?;
    }
    Ok(())
}

fn emit_aarch64_descriptor_argument(
    instructions: &mut Vec<u32>,
    copy: &AssignedAggregateCopy,
    home: &Aarch64UnitParameterHome,
    call_stack_bytes: u32,
) -> Result<(), EmissionError> {
    let [omega_calling_conventions::ValueLocation::Indirect { pointer, .. }] =
        copy.destination.locations.as_slice()
    else {
        return emit_aarch64_aggregate_copy_from_home(instructions, copy, home, call_stack_bytes);
    };
    let source_home = call_stack_bytes
        .checked_add(home.byte_offset)
        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
    let register = match *pointer {
        omega_calling_conventions::IndirectPointerLocation::Register(register) => {
            super::aarch64_unit_register(register)?
        }
        omega_calling_conventions::IndirectPointerLocation::Stack { .. } => 9,
    };
    instructions.push(aarch64_unit_stack_access(
        aarch64_load_base(8)?,
        register,
        source_home,
        8,
    )?);
    if copy.source_byte_offset > 0xfff {
        return Err(EmissionError::UnitCallStackAreaNotEncodable);
    }
    if copy.source_byte_offset != 0 {
        instructions.push(
            0x9100_0000
                | (copy.source_byte_offset << 10)
                | (u32::from(register) << 5)
                | u32::from(register),
        );
    }
    if let omega_calling_conventions::IndirectPointerLocation::Stack {
        stack_byte_offset, ..
    } = *pointer
    {
        instructions.push(aarch64_unit_stack_access(
            aarch64_store_base(8)?,
            register,
            stack_byte_offset,
            8,
        )?);
    }
    Ok(())
}

fn emit_aarch64_instance(
    bytes: &mut Vec<u8>,
    descriptor_offset: u32,
    copy: &AssignedAggregateCopy,
    homes: &[Aarch64UnitParameterHome],
    selection_ordinal: u32,
) -> Result<DynamicInstanceMaterializationRecord, EmissionError> {
    let home = homes
        .iter()
        .find(|home| home.place == copy.place)
        .ok_or(EmissionError::MissingUnitParameterHome(copy.place))?;
    if home.source != copy.source
        || copy
            .source_byte_offset
            .checked_add(u32::from(copy.shape.byte_size))
            .is_none_or(|end| end > u32::from(home.shape.byte_size))
    {
        return Err(EmissionError::UnitParameterHomeMismatch(copy.place));
    }
    let code_offset = bytes.len();
    let mut instructions = Vec::new();
    if home.indirect {
        instructions.push(aarch64_unit_stack_access(
            aarch64_load_base(8)?,
            9,
            home.byte_offset,
            8,
        )?);
    } else {
        emit_aarch64_sp_address(&mut instructions, 9, home.byte_offset)?;
    }
    instructions.push(aarch64_unit_stack_access(
        aarch64_store_base(8)?,
        9,
        descriptor_offset,
        8,
    )?);
    append_aarch64_instructions(bytes, instructions);
    Ok(DynamicInstanceMaterializationRecord {
        selection_ordinal,
        source: target_argument(copy),
        source_home_byte_offset: home.byte_offset,
        source_home_indirect: home.indirect,
        code_offset,
        byte_count: bytes.len() - code_offset,
    })
}

fn argument_record(
    copy: &AssignedAggregateCopy,
    descriptor_offset: u32,
    call_stack_bytes: u32,
    code_offset: usize,
    bytes: &[u8],
) -> InternalUnitCallArgumentRecord {
    InternalUnitCallArgumentRecord {
        place: copy.place,
        access: copy.access,
        path: copy.path.clone(),
        root_structural_type: copy.root_structural_type,
        structural_type: copy.structural_type,
        shape: copy.shape,
        source_byte_offset: copy.source_byte_offset,
        source_home_byte_offset: descriptor_offset,
        call_stack_bytes,
        fixed_array_length: copy.fixed_array_length,
        element_stride: copy.element_stride,
        source: copy.source.clone(),
        destination: copy.destination.clone(),
        code_offset,
        byte_count: bytes.len() - code_offset,
        bytes: bytes[code_offset..].to_vec(),
    }
}

fn target_argument(
    copy: &AssignedAggregateCopy,
) -> omega_target_operations::TargetStructuralArgument {
    omega_target_operations::TargetStructuralArgument {
        place: copy.place,
        access: copy.access,
        path: copy.path.clone(),
        root_structural_type: copy.root_structural_type,
        structural_type: copy.structural_type,
        shape: copy.shape,
        source_byte_offset: copy.source_byte_offset,
        fixed_array_length: copy.fixed_array_length,
        element_stride: copy.element_stride,
        source: copy.source.clone(),
        destination: copy.destination.clone(),
    }
}
