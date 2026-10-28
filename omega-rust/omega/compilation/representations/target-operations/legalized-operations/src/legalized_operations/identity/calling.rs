use super::encoding::{encode_len, encode_option};
use super::scalar::encode_register;
use calling_conventions::{
    CallPlan, CallbackMaterialization, CallingPolicy, EntryControl, IndirectPointerLocation,
    NativePlace, SystemVEightbyteClass, ValueClass, ValueLocation, ValuePlacement, ValueShape,
};

pub fn encode_call_plan(bytes: &mut Vec<u8>, plan: &CallPlan) {
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
    encode_option(bytes, plan.result.as_ref(), encode_placement);
    encode_len(bytes, plan.callback_materializations.len());
    for materialization in &plan.callback_materializations {
        encode_callback_materialization(bytes, materialization);
    }
    encode_len(bytes, plan.ordinary_clobbers.as_slice().len());
    for register in plan.ordinary_clobbers.as_slice() {
        encode_register(bytes, *register);
    }
    crate::codec_bytes::push_encoded(bytes, plan.stack_alignment.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, plan.shadow_bytes.to_le_bytes());
    match plan.entry_control {
        EntryControl::CallReturn => bytes.push(1),
        EntryControl::SupervisorCall {
            number_register,
            immediate,
        } => {
            bytes.push(2);
            encode_register(bytes, number_register);
            crate::codec_bytes::push_encoded(bytes, immediate.to_le_bytes());
        }
        EntryControl::InterruptReturn => bytes.push(3),
    }
}

pub(crate) fn encode_callback_materialization(bytes: &mut Vec<u8>, row: &CallbackMaterialization) {
    crate::codec_bytes::push_encoded(bytes, row.binder.get().to_le_bytes());
    encode_native_place(bytes, &row.destination);
}

pub(crate) fn encode_native_place(bytes: &mut Vec<u8>, destination: &NativePlace) {
    match destination {
        NativePlace::Parameter(parameter) => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, parameter.get().to_le_bytes());
        }
        NativePlace::Field {
            parameter,
            layout,
            field_path,
        } => {
            bytes.push(2);
            crate::codec_bytes::push_encoded(bytes, parameter.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, layout.get().to_le_bytes());
            encode_len(bytes, field_path.len());
            for slot in field_path {
                crate::codec_bytes::push_encoded(bytes, slot.get().to_le_bytes());
            }
        }
    }
}

pub fn encode_placement(bytes: &mut Vec<u8>, placement: &ValuePlacement) {
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
                crate::codec_bytes::push_encoded(bytes, value_byte_offset.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, byte_size.to_le_bytes());
            }
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset,
                byte_size,
                alignment,
            } => {
                bytes.push(2);
                crate::codec_bytes::push_encoded(bytes, stack_byte_offset.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, value_byte_offset.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, byte_size.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, alignment.to_le_bytes());
            }
            ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset,
                byte_size,
                alignment,
            } => {
                bytes.push(3);
                encode_indirect_pointer(bytes, pointer);
                encode_option(bytes, copy_stack_byte_offset, |bytes, offset| {
                    crate::codec_bytes::push_encoded(bytes, offset.to_le_bytes())
                });
                crate::codec_bytes::push_encoded(bytes, byte_size.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, alignment.to_le_bytes());
            }
        }
    }
}

pub(crate) fn encode_indirect_pointer(bytes: &mut Vec<u8>, pointer: IndirectPointerLocation) {
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
            crate::codec_bytes::push_encoded(bytes, stack_byte_offset.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, alignment.to_le_bytes());
        }
    }
}

pub fn encode_shape(bytes: &mut Vec<u8>, shape: ValueShape) {
    match shape.class {
        ValueClass::Integer => bytes.push(1),
        ValueClass::Float => bytes.push(2),
        ValueClass::BorrowedReference => bytes.push(5),
        ValueClass::BorrowedScalarReference => bytes.push(6),
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
    crate::codec_bytes::push_encoded(bytes, shape.byte_size.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, shape.alignment.to_le_bytes());
}

pub(crate) fn encode_eightbyte_class(bytes: &mut Vec<u8>, class: SystemVEightbyteClass) {
    bytes.push(match class {
        SystemVEightbyteClass::Integer => 1,
        SystemVEightbyteClass::Sse => 2,
    });
}
