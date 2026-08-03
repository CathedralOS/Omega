use crate::MachineEmissionContext;
use crate::branch_distances::byte_distance_to_next_runtime_write_end;
use crate::layout::LaidOutMachineInstruction;
use omega_assigned_target_operations::{
    RuntimeValueOperand, RuntimeValueOperandHandle, StateGuardOperator,
};
use omega_instruction_selection as architecture;
use psi_diagnostics::Diagnostic;

fn validate_runtime_value_home(
    input: MachineEmissionContext<'_>,
    operand: RuntimeValueOperandHandle,
) -> Result<(), Diagnostic> {
    let Some(home) = input.assigned_target_operations.runtime_value_home(operand) else {
        return Err(Diagnostic::error(format!(
            "missing assigned runtime value home for operand #{} during machine emission",
            operand.arena_index()
        )));
    };
    let runtime_value = input
        .assigned_target_operations
        .runtime_value_operand(operand)
        .expect("assigned runtime value operand should exist after home validation");
    match &runtime_value.kind {
        RuntimeValueOperand::Immediate(_) => {
            if !matches!(
                home,
                omega_assigned_target_operations::AssignedValueHomeKind::Immediate
            ) {
                return Err(Diagnostic::error(
                    "immediate runtime value must keep an immediate assigned home",
                ));
            }
        }
        RuntimeValueOperand::Storage {
            region,
            byte_offset,
            byte_size,
        } => match region {
            omega_target_operations::RuntimeStorageRegion::Machine => {
                if !matches!(
                    home,
                    omega_assigned_target_operations::AssignedValueHomeKind::RuntimeStorage {
                        region: omega_target_operations::RuntimeStorageRegion::Machine,
                        byte_offset: home_offset,
                        byte_size: home_size,
                    } if home_offset == *byte_offset && home_size == *byte_size
                ) {
                    return Err(Diagnostic::error(
                        "machine runtime storage value must keep a matching runtime-storage home",
                    ));
                }
            }
            omega_target_operations::RuntimeStorageRegion::RuntimeFrame => {
                if !matches!(
                    home,
                    omega_assigned_target_operations::AssignedValueHomeKind::StackSlot {
                        byte_offset: home_offset,
                        byte_size: home_size,
                    } if home_offset == *byte_offset && home_size == *byte_size
                ) {
                    return Err(Diagnostic::error(
                        "runtime-frame value must lower through a matching stack-slot home",
                    ));
                }
            }
        },
        RuntimeValueOperand::Pointee {
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
        } => {
            if !matches!(
                home,
                omega_assigned_target_operations::AssignedValueHomeKind::RuntimePointee {
                    pointer_byte_offset: home_pointer,
                    field_byte_offset: home_field,
                    byte_size: home_size,
                } if home_pointer == *pointer_byte_offset
                    && home_field == *field_byte_offset
                    && home_size == *byte_size
            ) {
                return Err(Diagnostic::error(
                    "runtime pointee value must keep a matching pointee home",
                ));
            }
        }
        RuntimeValueOperand::FrameIndexed {
            descriptor_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => {
            if !matches!(
                home,
                omega_assigned_target_operations::AssignedValueHomeKind::RuntimeFrameIndexed {
                    descriptor_offset: home_descriptor,
                    index_region: home_index_region,
                    index_offset: home_index,
                    index_byte_size: home_index_size,
                    element_byte_size: home_element_size,
                    field_byte_offset: home_field,
                    byte_size: home_size,
                } if home_descriptor == *descriptor_offset
                    && home_index_region == *index_region
                    && home_index == *index_offset
                    && home_index_size == *index_byte_size
                    && home_element_size == *element_byte_size
                    && home_field == *field_byte_offset
                    && home_size == *byte_size
            ) {
                return Err(Diagnostic::error(
                    "runtime frame-indexed value must keep a matching frame-indexed home",
                ));
            }
        }
        RuntimeValueOperand::FrameBaseIndexed {
            base_byte_offset,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => {
            if !matches!(
                home,
                omega_assigned_target_operations::AssignedValueHomeKind::RuntimeFrameBaseIndexed {
                    base_byte_offset: home_base,
                    index_offset: home_index,
                    index_byte_size: home_index_size,
                    element_byte_size: home_element_size,
                    field_byte_offset: home_field,
                    byte_size: home_size,
                } if home_base == *base_byte_offset
                    && home_index == *index_offset
                    && home_index_size == *index_byte_size
                    && home_element_size == *element_byte_size
                    && home_field == *field_byte_offset
                    && home_size == *byte_size
            ) {
                return Err(Diagnostic::error(
                    "runtime frame-base-indexed value must keep a matching frame-base-indexed home",
                ));
            }
        }
        RuntimeValueOperand::FrameFixedIndexed {
            descriptor_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => {
            if !matches!(
                home,
                omega_assigned_target_operations::AssignedValueHomeKind::RuntimeFrameFixedIndexed {
                    descriptor_offset: home_descriptor,
                    element_index: home_index,
                    element_byte_size: home_element_size,
                    field_byte_offset: home_field,
                    byte_size: home_size,
                } if home_descriptor == *descriptor_offset
                    && home_index == *element_index
                    && home_element_size == *element_byte_size
                    && home_field == *field_byte_offset
                    && home_size == *byte_size
            ) {
                return Err(Diagnostic::error(
                    "runtime fixed frame-indexed value must keep a matching frame-indexed home",
                ));
            }
        }
        RuntimeValueOperand::BitField { .. }
        | RuntimeValueOperand::Binary { .. }
        | RuntimeValueOperand::Convert { .. }
        | RuntimeValueOperand::TextEquals { .. }
        | RuntimeValueOperand::TextEqualsLiteral { .. }
        | RuntimeValueOperand::MachineIndexed { .. } => {
            if !matches!(
                home,
                omega_assigned_target_operations::AssignedValueHomeKind::ScratchRegister { .. }
                    | omega_assigned_target_operations::AssignedValueHomeKind::StackSlot { .. }
            ) {
                return Err(Diagnostic::error(
                    "computed runtime value must lower through a scratch-register or stack-slot home",
                ));
            }
        }
    }

    Ok(())
}

pub(super) fn encode_runtime_value_compare(
    input: MachineEmissionContext<'_>,
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
    byte_size: usize,
    operator: StateGuardOperator,
) -> Result<Vec<u8>, Diagnostic> {
    validate_runtime_value_home(input, left)?;
    validate_runtime_value_home(input, right)?;
    architecture::encode_runtime_value_compare(
        input.target.architecture,
        input.assigned_target_operations,
        left,
        right,
        byte_size,
        byte_distance_to_next_runtime_write_end(
            input,
            machine_instructions,
            machine_instruction_index,
        )?,
        operator,
    )
}

#[allow(clippy::too_many_arguments)]
/// Binary rung 2a: the place-shaped binary write's per-arch dispatcher.
#[allow(clippy::too_many_arguments)]
pub(super) fn encode_write_place_binary(
    input: MachineEmissionContext<'_>,
    target: &omega_target_operations::Place,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
    is_float: bool,
    domain: psi_numerics::arithmetic::ArithmeticDomain,
    target_signed: bool,
) -> Result<Vec<u8>, Diagnostic> {
    validate_runtime_value_home(input, left)?;
    validate_runtime_value_home(input, right)?;
    architecture::encode_write_place_binary(
        input.target.architecture,
        input.assigned_target_operations,
        target,
        byte_size,
        left,
        operator,
        right,
        is_float,
        domain,
        target_signed,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn encode_runtime_storage_convert(
    input: MachineEmissionContext<'_>,
    target_offset: usize,
    target_byte_size: usize,
    source: RuntimeValueOperandHandle,
    source_byte_size: usize,
    source_is_float: bool,
    target_is_float: bool,
    source_signed: bool,
    target_signed: bool,
    trapping: bool,
    saturating: bool,
) -> Result<Vec<u8>, Diagnostic> {
    validate_runtime_value_home(input, source)?;
    architecture::encode_runtime_storage_convert(
        input.target.architecture,
        input.assigned_target_operations,
        target_offset,
        target_byte_size,
        source,
        source_byte_size,
        source_is_float,
        target_is_float,
        source_signed,
        target_signed,
        trapping,
        saturating,
    )
}

pub(super) fn encode_atomic_load_to_storage(
    input: MachineEmissionContext<'_>,
    source_offset: usize,
    byte_size: usize,
    result_offset: usize,
    ordering: psi_language_core::AtomicOrderingPlan,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_atomic_load_to_storage(
        input.target.architecture,
        source_offset,
        byte_size,
        result_offset,
        ordering,
    )
}

pub(super) fn encode_atomic_store_from_operand(
    input: MachineEmissionContext<'_>,
    target_offset: usize,
    byte_size: usize,
    value: RuntimeValueOperandHandle,
    ordering: psi_language_core::AtomicOrderingPlan,
) -> Result<Vec<u8>, Diagnostic> {
    validate_runtime_value_home(input, value)?;
    architecture::encode_atomic_store_from_operand(
        input.target.architecture,
        input.assigned_target_operations,
        target_offset,
        byte_size,
        value,
        ordering,
    )
}

pub(super) fn encode_atomic_fetch_add(
    input: MachineEmissionContext<'_>,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    delta: RuntimeValueOperandHandle,
    ordering: psi_language_core::AtomicOrderingPlan,
) -> Result<Vec<u8>, Diagnostic> {
    validate_runtime_value_home(input, delta)?;
    architecture::encode_atomic_fetch_add(
        input.target.architecture,
        input.assigned_target_operations,
        target_offset,
        byte_size,
        result_offset,
        delta,
        ordering,
    )
}

pub(super) fn encode_atomic_fetch_sub(
    input: MachineEmissionContext<'_>,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    delta: RuntimeValueOperandHandle,
    ordering: psi_language_core::AtomicOrderingPlan,
) -> Result<Vec<u8>, Diagnostic> {
    validate_runtime_value_home(input, delta)?;
    architecture::encode_atomic_fetch_sub(
        input.target.architecture,
        input.assigned_target_operations,
        target_offset,
        byte_size,
        result_offset,
        delta,
        ordering,
    )
}

pub(super) fn encode_atomic_fetch_xor(
    input: MachineEmissionContext<'_>,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    value: RuntimeValueOperandHandle,
    ordering: psi_language_core::AtomicOrderingPlan,
) -> Result<Vec<u8>, Diagnostic> {
    validate_runtime_value_home(input, value)?;
    architecture::encode_atomic_fetch_xor(
        input.target.architecture,
        input.assigned_target_operations,
        target_offset,
        byte_size,
        result_offset,
        value,
        ordering,
    )
}

pub(super) fn encode_atomic_fetch_or(
    input: MachineEmissionContext<'_>,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    value: RuntimeValueOperandHandle,
    ordering: psi_language_core::AtomicOrderingPlan,
) -> Result<Vec<u8>, Diagnostic> {
    validate_runtime_value_home(input, value)?;
    architecture::encode_atomic_fetch_or(
        input.target.architecture,
        input.assigned_target_operations,
        target_offset,
        byte_size,
        result_offset,
        value,
        ordering,
    )
}

pub(super) fn encode_atomic_fetch_and(
    input: MachineEmissionContext<'_>,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    value: RuntimeValueOperandHandle,
    ordering: psi_language_core::AtomicOrderingPlan,
) -> Result<Vec<u8>, Diagnostic> {
    validate_runtime_value_home(input, value)?;
    architecture::encode_atomic_fetch_and(
        input.target.architecture,
        input.assigned_target_operations,
        target_offset,
        byte_size,
        result_offset,
        value,
        ordering,
    )
}

pub(super) fn encode_atomic_swap(
    input: MachineEmissionContext<'_>,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    new_value: RuntimeValueOperandHandle,
    ordering: psi_language_core::AtomicOrderingPlan,
) -> Result<Vec<u8>, Diagnostic> {
    validate_runtime_value_home(input, new_value)?;
    architecture::encode_atomic_swap(
        input.target.architecture,
        input.assigned_target_operations,
        target_offset,
        byte_size,
        result_offset,
        new_value,
        ordering,
    )
}

pub(super) fn encode_atomic_compare_exchange(
    input: MachineEmissionContext<'_>,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    expected: RuntimeValueOperandHandle,
    new_value: RuntimeValueOperandHandle,
    ordering: psi_language_core::AtomicOrderingPlan,
) -> Result<Vec<u8>, Diagnostic> {
    validate_runtime_value_home(input, expected)?;
    validate_runtime_value_home(input, new_value)?;
    architecture::encode_atomic_compare_exchange(
        input.target.architecture,
        input.assigned_target_operations,
        target_offset,
        byte_size,
        result_offset,
        expected,
        new_value,
        ordering,
    )
}
