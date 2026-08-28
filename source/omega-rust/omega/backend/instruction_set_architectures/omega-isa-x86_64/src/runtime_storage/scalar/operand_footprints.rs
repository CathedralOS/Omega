//! Register and machine-state ceilings for recursive scalar operands.

use omega_calling_conventions::{MachineRegister, MachineState, MachineStateSet, RegisterSet};
use omega_target_operations::{
    RuntimeValueOperandHandle, RuntimeValueOperandSource, StateGuardOperator,
};

/// Closed may-write ceiling of the recursive runtime-value comparison
/// encoder. Operand shapes select subsets of this encoder-owned bank; keeping
/// the family ceiling beside the evaluator makes the retained evidence sound
/// across nested arithmetic, conversions, indexed loads, and text equality.
pub fn runtime_value_compare_register_write_ceiling() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::X86Rax,
        MachineRegister::X86Rcx,
        MachineRegister::X86Rdx,
        MachineRegister::X86R8,
        MachineRegister::X86R9,
        MachineRegister::X86R10,
        MachineRegister::X86R11,
        MachineRegister::X86R15,
        MachineRegister::X86Xmm(0),
        MachineRegister::X86Xmm(1),
    ])
}

/// Closed may-write ceiling of a place-shaped binary write. Recursive operand
/// evaluation owns the runtime-value bank; r14 additionally preserves the
/// materialized destination while those operands reload r15.
pub fn place_binary_write_register_write_ceiling() -> RegisterSet {
    let mut registers = runtime_value_compare_register_write_ceiling()
        .as_slice()
        .to_vec();
    registers.push(MachineRegister::X86R14);
    RegisterSet::new(registers)
}

/// Closed may-write ceiling of a direct conversion write. It shares the
/// recursive runtime-value evaluator with binary writes and preserves the
/// relocated destination in r14 while conversion policy may use r11/xmm0/xmm1.
pub fn storage_convert_write_register_write_ceiling() -> RegisterSet {
    place_binary_write_register_write_ceiling()
}

fn runtime_value_operand_uses_stack(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    operand: RuntimeValueOperandHandle,
) -> bool {
    if runtime_value_operands.binary(operand).is_some() {
        // Every binary operand preserves its recursively evaluated left value
        // with push r10 / pop r10 around evaluation of the right value.
        true
    } else if let Some((source, ..)) = runtime_value_operands.convert(operand) {
        runtime_value_operand_uses_stack(runtime_value_operands, source)
    } else {
        false
    }
}

fn runtime_value_operand_uses_control_state(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    operand: RuntimeValueOperandHandle,
) -> bool {
    if let Some((left, operator, right)) = runtime_value_operands.binary(operand) {
        matches!(
            operator,
            StateGuardOperator::AddTowardZero
                | StateGuardOperator::AddTowardPositive
                | StateGuardOperator::AddTowardNegative
                | StateGuardOperator::SubtractTowardZero
                | StateGuardOperator::SubtractTowardPositive
                | StateGuardOperator::SubtractTowardNegative
                | StateGuardOperator::MultiplyTowardZero
                | StateGuardOperator::MultiplyTowardPositive
                | StateGuardOperator::MultiplyTowardNegative
                | StateGuardOperator::DivideTowardZero
                | StateGuardOperator::DivideTowardPositive
                | StateGuardOperator::DivideTowardNegative
                | StateGuardOperator::SqrtTowardZero
                | StateGuardOperator::SqrtTowardPositive
                | StateGuardOperator::SqrtTowardNegative
        ) || runtime_value_operand_uses_control_state(runtime_value_operands, left)
            || runtime_value_operand_uses_control_state(runtime_value_operands, right)
    } else if let Some((source, ..)) = runtime_value_operands.convert(operand) {
        runtime_value_operand_uses_control_state(runtime_value_operands, source)
    } else {
        false
    }
}

/// Machine state touched while materializing one runtime value operand, before
/// the enclosing operation applies its own effects. This deliberately keeps
/// operand evaluation separate from comparison/atomic semantics: an immediate
/// atomic store does not write flags merely because a compare operation would.
pub fn runtime_value_operand_additional_machine_state(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    operand: RuntimeValueOperandHandle,
) -> MachineStateSet {
    let mut state = MachineStateSet::empty();
    let writes_flags = runtime_value_operands.bit_field(operand).is_some()
        || runtime_value_operands.frame_indexed(operand).is_some()
        || runtime_value_operands.frame_base_indexed(operand).is_some()
        || runtime_value_operands.machine_indexed(operand).is_some()
        || runtime_value_operands.text_equals(operand).is_some()
        || runtime_value_operands
            .text_equals_literal(operand)
            .is_some()
        || runtime_value_operands.binary(operand).is_some()
        || runtime_value_operands.convert(operand).is_some()
        || runtime_value_operands
            .pointee(operand)
            .is_some_and(|(_, field_byte_offset, _)| field_byte_offset != 0);
    if writes_flags {
        state = state.union(MachineStateSet::new([MachineState::Flags]));
    }
    if runtime_value_operand_uses_stack(runtime_value_operands, operand) {
        state = state.union(MachineStateSet::new([MachineState::StackPointer]));
    }
    if runtime_value_operand_uses_control_state(runtime_value_operands, operand) {
        state = state.union(MachineStateSet::new([MachineState::ControlState]));
    }
    state
}

pub fn runtime_value_compare_additional_machine_state(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
) -> MachineStateSet {
    let mut state = MachineStateSet::new([MachineState::Flags]);
    if runtime_value_operand_uses_stack(runtime_value_operands, left)
        || runtime_value_operand_uses_stack(runtime_value_operands, right)
    {
        state = state.union(MachineStateSet::new([MachineState::StackPointer]));
    }
    if runtime_value_operand_uses_control_state(runtime_value_operands, left)
        || runtime_value_operand_uses_control_state(runtime_value_operands, right)
    {
        state = state.union(MachineStateSet::new([MachineState::ControlState]));
    }
    state
}

pub fn storage_convert_write_additional_machine_state(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    source: RuntimeValueOperandHandle,
) -> MachineStateSet {
    runtime_value_compare_additional_machine_state(runtime_value_operands, source, source)
}

/// Machine state touched by a place-shaped binary write. Its outer evaluator
/// always balances one push/pop pair, integer policy/comparison paths may
/// write flags, and directed floating operations temporarily change the
/// floating control word before restoring it.
pub fn place_binary_write_additional_machine_state(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> MachineStateSet {
    let mut state = MachineStateSet::new([MachineState::Flags, MachineState::StackPointer]);
    let operator_uses_control_state = matches!(
        operator,
        StateGuardOperator::AddTowardZero
            | StateGuardOperator::AddTowardPositive
            | StateGuardOperator::AddTowardNegative
            | StateGuardOperator::SubtractTowardZero
            | StateGuardOperator::SubtractTowardPositive
            | StateGuardOperator::SubtractTowardNegative
            | StateGuardOperator::MultiplyTowardZero
            | StateGuardOperator::MultiplyTowardPositive
            | StateGuardOperator::MultiplyTowardNegative
            | StateGuardOperator::DivideTowardZero
            | StateGuardOperator::DivideTowardPositive
            | StateGuardOperator::DivideTowardNegative
            | StateGuardOperator::SqrtTowardZero
            | StateGuardOperator::SqrtTowardPositive
            | StateGuardOperator::SqrtTowardNegative
    );
    if operator_uses_control_state
        || runtime_value_operand_uses_control_state(runtime_value_operands, left)
        || runtime_value_operand_uses_control_state(runtime_value_operands, right)
    {
        state = state.union(MachineStateSet::new([MachineState::ControlState]));
    }
    state
}
