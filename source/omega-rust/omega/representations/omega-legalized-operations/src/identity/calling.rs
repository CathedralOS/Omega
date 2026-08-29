use super::scalar::encode_register;
use super::shared::*;

pub(super) fn encode_call_plan(bytes: &mut Vec<u8>, plan: &CallPlan) {
    bytes.push(match plan.policy {
        CallingPolicy::MicrosoftX64 => 1,
        CallingPolicy::SystemVAMD64 => 2,
        CallingPolicy::Aapcs64 => 3,
        CallingPolicy::LinuxSyscallX86_64 => 4,
        CallingPolicy::LinuxSyscallAarch64 => 5,
    });
    encode_len(bytes, plan.parameters.len());
    for placement in &plan.parameters {
        encode_placement(bytes, placement);
    }
    match &plan.result {
        Some(result) => {
            bytes.push(1);
            encode_placement(bytes, result);
        }
        None => bytes.push(0),
    }
    encode_len(bytes, plan.callback_materializations.len());
    for materialization in &plan.callback_materializations {
        encode_callback_materialization(bytes, materialization);
    }
    encode_len(bytes, plan.ordinary_clobbers.as_slice().len());
    for register in plan.ordinary_clobbers.as_slice() {
        encode_register(bytes, *register);
    }
    bytes.extend_from_slice(&plan.stack_alignment.to_le_bytes());
    bytes.extend_from_slice(&plan.shadow_bytes.to_le_bytes());
    match plan.entry_control {
        EntryControl::CallReturn => bytes.push(1),
        EntryControl::SupervisorCall {
            number_register,
            immediate,
        } => {
            bytes.push(2);
            encode_register(bytes, number_register);
            bytes.extend_from_slice(&immediate.to_le_bytes());
        }
        EntryControl::InterruptReturn => bytes.push(3),
    }
}

pub(super) fn encode_callback_materialization(bytes: &mut Vec<u8>, row: &CallbackMaterialization) {
    bytes.extend_from_slice(&row.binder.get().to_le_bytes());
    match &row.destination {
        NativePlace::Parameter(parameter) => {
            bytes.push(1);
            bytes.extend_from_slice(&parameter.get().to_le_bytes());
        }
        NativePlace::Field {
            parameter,
            layout,
            field_path,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&parameter.get().to_le_bytes());
            bytes.extend_from_slice(&layout.get().to_le_bytes());
            encode_len(bytes, field_path.len());
            for slot in field_path {
                bytes.extend_from_slice(&slot.get().to_le_bytes());
            }
        }
    }
}

pub(super) fn encode_placement(bytes: &mut Vec<u8>, placement: &ValuePlacement) {
    encode_shape(bytes, placement.shape);
    encode_len(bytes, placement.locations.len());
    for location in &placement.locations {
        match *location {
            ValueLocation::Register {
                register,
                value_byte_offset,
                byte_size,
            } => {
                bytes.push(1);
                encode_register(bytes, register);
                bytes.extend_from_slice(&value_byte_offset.to_le_bytes());
                bytes.extend_from_slice(&byte_size.to_le_bytes());
            }
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset,
                byte_size,
                alignment,
            } => {
                bytes.push(2);
                bytes.extend_from_slice(&stack_byte_offset.to_le_bytes());
                bytes.extend_from_slice(&value_byte_offset.to_le_bytes());
                bytes.extend_from_slice(&byte_size.to_le_bytes());
                bytes.extend_from_slice(&alignment.to_le_bytes());
            }
            ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset,
                byte_size,
                alignment,
            } => {
                bytes.push(3);
                encode_indirect_pointer(bytes, pointer);
                match copy_stack_byte_offset {
                    Some(offset) => {
                        bytes.push(1);
                        bytes.extend_from_slice(&offset.to_le_bytes());
                    }
                    None => bytes.push(0),
                }
                bytes.extend_from_slice(&byte_size.to_le_bytes());
                bytes.extend_from_slice(&alignment.to_le_bytes());
            }
        }
    }
}

pub(super) fn encode_indirect_pointer(bytes: &mut Vec<u8>, pointer: IndirectPointerLocation) {
    match pointer {
        IndirectPointerLocation::Register(register) => {
            bytes.push(1);
            encode_register(bytes, register);
        }
        IndirectPointerLocation::Stack {
            stack_byte_offset,
            alignment,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&stack_byte_offset.to_le_bytes());
            bytes.extend_from_slice(&alignment.to_le_bytes());
        }
    }
}

pub(super) fn encode_shape(bytes: &mut Vec<u8>, shape: ValueShape) {
    match shape.class {
        ValueClass::Integer => bytes.push(1),
        ValueClass::Float => bytes.push(2),
        ValueClass::HomogeneousFloatAggregate { members } => {
            bytes.push(3);
            bytes.push(members);
        }
        ValueClass::SystemVAggregate { first, second } => {
            bytes.push(4);
            encode_eightbyte_class(bytes, first);
            encode_eightbyte_class(bytes, second);
        }
    }
    bytes.extend_from_slice(&shape.byte_size.to_le_bytes());
    bytes.extend_from_slice(&shape.alignment.to_le_bytes());
}

pub(super) fn encode_eightbyte_class(bytes: &mut Vec<u8>, class: SystemVEightbyteClass) {
    bytes.push(match class {
        SystemVEightbyteClass::Integer => 1,
        SystemVEightbyteClass::Sse => 2,
    });
}
