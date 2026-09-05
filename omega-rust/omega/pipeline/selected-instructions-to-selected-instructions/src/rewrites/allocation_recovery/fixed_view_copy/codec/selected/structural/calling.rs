use calling_conventions::{
    CallPlan, CallbackMaterialization, CallingPolicy, EntryControl, IndirectPointerLocation,
    LayoutPlanId, LayoutSlotId, MachineRegister, NativeParameterId, NativePlace, RegisterSet,
    StaticMachineBinderId, SystemVEightbyteClass, ValueClass, ValueLocation, ValuePlacement,
    ValueShape,
};
use selected_instructions::{
    SelectedMicrosoftX64OwnedIndirectPairLayout, SelectedStructuralUnitAbi,
    SelectedStructuralUnitAbiRecipe, SelectedStructuralUnitIndirectBinding,
};

use crate::FixedViewCopyDecodeError;

use super::declarations::{decode_parameter, encode_parameter};
use crate::rewrites::allocation_recovery::fixed_view_copy::codec::primitives::{Cursor, length};

pub(super) fn encode_abi(
    bytes: &mut Vec<u8>,
    abi: &SelectedStructuralUnitAbi,
    retain_projected_qualifications: bool,
) {
    bytes.push(match abi.recipe {
        SelectedStructuralUnitAbiRecipe::MicrosoftX64OwnedIndirectPairV1 => 1,
    });
    encode_call_plan(bytes, &abi.call_plan);
    length(bytes, abi.parameters.len());
    for parameter in &abi.parameters {
        encode_parameter(bytes, parameter, retain_projected_qualifications);
    }
    encode_layout(bytes, abi.layout);
}

pub(super) fn decode_abi(
    cursor: &mut Cursor<'_>,
    retain_projected_qualifications: bool,
) -> Result<SelectedStructuralUnitAbi, FixedViewCopyDecodeError> {
    let recipe = match cursor.byte()? {
        1 => SelectedStructuralUnitAbiRecipe::MicrosoftX64OwnedIndirectPairV1,
        tag => return Err(FixedViewCopyDecodeError::UnknownStructuralAbiRecipe(tag)),
    };
    let call_plan = decode_call_plan(cursor)?;
    let parameter_count = cursor.length()?;
    let mut parameters = Vec::with_capacity(parameter_count.min(cursor.remaining()));
    for _ in 0..parameter_count {
        parameters.push(decode_parameter(cursor, retain_projected_qualifications)?);
    }
    Ok(SelectedStructuralUnitAbi {
        recipe,
        call_plan,
        parameters,
        layout: decode_layout(cursor)?,
    })
}

pub(super) fn encode_layout(
    bytes: &mut Vec<u8>,
    layout: SelectedMicrosoftX64OwnedIndirectPairLayout,
) {
    bytes.extend_from_slice(&layout.shadow_byte_count.to_le_bytes());
    bytes.extend_from_slice(&layout.outgoing_frame_byte_count.to_le_bytes());
    bytes.extend_from_slice(&layout.pre_call_stack_alignment.to_le_bytes());
    for binding in layout.bindings {
        length(bytes, binding.parameter_index);
        encode_machine_register(bytes, binding.pointer);
        bytes.extend_from_slice(&binding.copy_stack_byte_offset.to_le_bytes());
        bytes.extend_from_slice(&binding.byte_count.to_le_bytes());
        bytes.extend_from_slice(&binding.alignment.to_le_bytes());
    }
}

pub(super) fn decode_layout(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedMicrosoftX64OwnedIndirectPairLayout, FixedViewCopyDecodeError> {
    Ok(SelectedMicrosoftX64OwnedIndirectPairLayout {
        shadow_byte_count: cursor.u32()?,
        outgoing_frame_byte_count: cursor.u32()?,
        pre_call_stack_alignment: cursor.u16()?,
        bindings: [decode_binding(cursor)?, decode_binding(cursor)?],
    })
}

fn decode_binding(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedStructuralUnitIndirectBinding, FixedViewCopyDecodeError> {
    Ok(SelectedStructuralUnitIndirectBinding {
        parameter_index: cursor.length()?,
        pointer: decode_machine_register(cursor)?,
        copy_stack_byte_offset: cursor.u32()?,
        byte_count: cursor.u16()?,
        alignment: cursor.u16()?,
    })
}

pub(super) fn encode_call_plan(bytes: &mut Vec<u8>, plan: &CallPlan) {
    bytes.push(match plan.policy {
        CallingPolicy::MicrosoftX64 => 1,
        CallingPolicy::SystemVAMD64 => 2,
        CallingPolicy::Aapcs64 => 3,
        CallingPolicy::LinuxSyscallX86_64 => 4,
        CallingPolicy::LinuxSyscallAarch64 => 5,
    });
    length(bytes, plan.parameters.len());
    for placement in &plan.parameters {
        encode_placement(bytes, placement);
    }
    match &plan.result {
        None => bytes.push(0),
        Some(result) => {
            bytes.push(1);
            encode_placement(bytes, result);
        }
    }
    length(bytes, plan.callback_materializations.len());
    for row in &plan.callback_materializations {
        encode_callback(bytes, row);
    }
    length(bytes, plan.ordinary_clobbers.as_slice().len());
    for register in plan.ordinary_clobbers.as_slice() {
        encode_machine_register(bytes, *register);
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
            encode_machine_register(bytes, number_register);
            bytes.extend_from_slice(&immediate.to_le_bytes());
        }
        EntryControl::InterruptReturn => bytes.push(3),
    }
}

pub(super) fn decode_call_plan(
    cursor: &mut Cursor<'_>,
) -> Result<CallPlan, FixedViewCopyDecodeError> {
    let policy = match cursor.byte()? {
        1 => CallingPolicy::MicrosoftX64,
        2 => CallingPolicy::SystemVAMD64,
        3 => CallingPolicy::Aapcs64,
        4 => CallingPolicy::LinuxSyscallX86_64,
        5 => CallingPolicy::LinuxSyscallAarch64,
        tag => return Err(FixedViewCopyDecodeError::UnknownCallingPolicy(tag)),
    };
    let parameter_count = cursor.length()?;
    let mut parameters = Vec::with_capacity(parameter_count.min(cursor.remaining()));
    for _ in 0..parameter_count {
        parameters.push(decode_placement(cursor)?);
    }
    let result = match cursor.byte()? {
        0 => None,
        1 => Some(decode_placement(cursor)?),
        tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    };
    let callback_count = cursor.length()?;
    let mut callback_materializations = Vec::with_capacity(callback_count.min(cursor.remaining()));
    for _ in 0..callback_count {
        callback_materializations.push(decode_callback(cursor)?);
    }
    let clobber_count = cursor.length()?;
    let mut ordinary_clobbers = Vec::with_capacity(clobber_count.min(cursor.remaining()));
    for _ in 0..clobber_count {
        ordinary_clobbers.push(decode_machine_register(cursor)?);
    }
    let stack_alignment = cursor.u16()?;
    let shadow_bytes = cursor.u16()?;
    let entry_control = match cursor.byte()? {
        1 => EntryControl::CallReturn,
        2 => EntryControl::SupervisorCall {
            number_register: decode_machine_register(cursor)?,
            immediate: cursor.u16()?,
        },
        3 => EntryControl::InterruptReturn,
        tag => return Err(FixedViewCopyDecodeError::UnknownEntryControl(tag)),
    };
    Ok(CallPlan {
        policy,
        parameters,
        result,
        callback_materializations,
        ordinary_clobbers: RegisterSet::new(ordinary_clobbers),
        stack_alignment,
        shadow_bytes,
        entry_control,
    })
}

fn encode_callback(bytes: &mut Vec<u8>, row: &CallbackMaterialization) {
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
            length(bytes, field_path.len());
            for slot in field_path {
                bytes.extend_from_slice(&slot.get().to_le_bytes());
            }
        }
    }
}

fn decode_callback(
    cursor: &mut Cursor<'_>,
) -> Result<CallbackMaterialization, FixedViewCopyDecodeError> {
    let binder = decode_nominal(cursor, StaticMachineBinderId::new)?;
    let destination = match cursor.byte()? {
        1 => NativePlace::Parameter(decode_nominal(cursor, NativeParameterId::new)?),
        2 => {
            let parameter = decode_nominal(cursor, NativeParameterId::new)?;
            let layout = decode_nominal(cursor, LayoutPlanId::new)?;
            let count = cursor.length()?;
            let mut field_path = Vec::with_capacity(count.min(cursor.remaining()));
            for _ in 0..count {
                field_path.push(decode_nominal(cursor, LayoutSlotId::new)?);
            }
            NativePlace::Field {
                parameter,
                layout,
                field_path,
            }
        }
        tag => return Err(FixedViewCopyDecodeError::UnknownNativePlace(tag)),
    };
    Ok(CallbackMaterialization {
        binder,
        destination,
    })
}

pub(super) fn encode_placement(bytes: &mut Vec<u8>, placement: &ValuePlacement) {
    encode_shape(bytes, placement.shape);
    length(bytes, placement.locations.len());
    for location in &placement.locations {
        match *location {
            ValueLocation::Register {
                register,
                value_byte_offset,
                byte_size,
            } => {
                bytes.push(1);
                encode_machine_register(bytes, register);
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
                    None => bytes.push(0),
                    Some(offset) => {
                        bytes.push(1);
                        bytes.extend_from_slice(&offset.to_le_bytes());
                    }
                }
                bytes.extend_from_slice(&byte_size.to_le_bytes());
                bytes.extend_from_slice(&alignment.to_le_bytes());
            }
        }
    }
}

pub(super) fn decode_placement(
    cursor: &mut Cursor<'_>,
) -> Result<ValuePlacement, FixedViewCopyDecodeError> {
    let shape = decode_shape(cursor)?;
    let count = cursor.length()?;
    let mut locations = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        locations.push(match cursor.byte()? {
            1 => ValueLocation::Register {
                register: decode_machine_register(cursor)?,
                value_byte_offset: cursor.u16()?,
                byte_size: cursor.u16()?,
            },
            2 => ValueLocation::Stack {
                stack_byte_offset: cursor.u32()?,
                value_byte_offset: cursor.u16()?,
                byte_size: cursor.u16()?,
                alignment: cursor.u16()?,
            },
            3 => ValueLocation::Indirect {
                pointer: decode_indirect_pointer(cursor)?,
                copy_stack_byte_offset: match cursor.byte()? {
                    0 => None,
                    1 => Some(cursor.u32()?),
                    tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
                },
                byte_size: cursor.u16()?,
                alignment: cursor.u16()?,
            },
            tag => return Err(FixedViewCopyDecodeError::UnknownValueLocation(tag)),
        });
    }
    Ok(ValuePlacement { shape, locations })
}

fn encode_indirect_pointer(bytes: &mut Vec<u8>, pointer: IndirectPointerLocation) {
    match pointer {
        IndirectPointerLocation::Register(register) => {
            bytes.push(1);
            encode_machine_register(bytes, register);
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

fn decode_indirect_pointer(
    cursor: &mut Cursor<'_>,
) -> Result<IndirectPointerLocation, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        1 => Ok(IndirectPointerLocation::Register(decode_machine_register(
            cursor,
        )?)),
        2 => Ok(IndirectPointerLocation::Stack {
            stack_byte_offset: cursor.u32()?,
            alignment: cursor.u16()?,
        }),
        tag => Err(FixedViewCopyDecodeError::UnknownIndirectPointer(tag)),
    }
}

pub(super) fn encode_shape(bytes: &mut Vec<u8>, shape: ValueShape) {
    match shape.class {
        ValueClass::Integer => bytes.push(1),
        ValueClass::Float => bytes.push(2),
        ValueClass::BorrowedReference => bytes.push(5),
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

pub(super) fn decode_shape(
    cursor: &mut Cursor<'_>,
) -> Result<ValueShape, FixedViewCopyDecodeError> {
    let class = match cursor.byte()? {
        1 => ValueClass::Integer,
        2 => ValueClass::Float,
        5 => ValueClass::BorrowedReference,
        3 => ValueClass::HomogeneousFloatAggregate {
            members: cursor.byte()?,
        },
        4 => ValueClass::SystemVAggregate {
            first: decode_eightbyte_class(cursor)?,
            second: decode_eightbyte_class(cursor)?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownValueClass(tag)),
    };
    Ok(ValueShape {
        class,
        byte_size: cursor.u16()?,
        alignment: cursor.u16()?,
    })
}

fn encode_eightbyte_class(bytes: &mut Vec<u8>, class: SystemVEightbyteClass) {
    bytes.push(match class {
        SystemVEightbyteClass::Integer => 1,
        SystemVEightbyteClass::Sse => 2,
    });
}

fn decode_eightbyte_class(
    cursor: &mut Cursor<'_>,
) -> Result<SystemVEightbyteClass, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        1 => Ok(SystemVEightbyteClass::Integer),
        2 => Ok(SystemVEightbyteClass::Sse),
        tag => Err(FixedViewCopyDecodeError::UnknownSystemVEightbyteClass(tag)),
    }
}

fn encode_machine_register(bytes: &mut Vec<u8>, register: MachineRegister) {
    let (tag, payload) = match register {
        MachineRegister::X86Rax => (0, 0),
        MachineRegister::X86Rcx => (1, 0),
        MachineRegister::X86Rdx => (2, 0),
        MachineRegister::X86Rbx => (3, 0),
        MachineRegister::X86Rsp => (4, 0),
        MachineRegister::X86Rbp => (5, 0),
        MachineRegister::X86Rsi => (6, 0),
        MachineRegister::X86Rdi => (7, 0),
        MachineRegister::X86R8 => (8, 0),
        MachineRegister::X86R9 => (9, 0),
        MachineRegister::X86R10 => (10, 0),
        MachineRegister::X86R11 => (11, 0),
        MachineRegister::X86R12 => (12, 0),
        MachineRegister::X86R13 => (13, 0),
        MachineRegister::X86R14 => (14, 0),
        MachineRegister::X86R15 => (15, 0),
        MachineRegister::X86Xmm(index) => (16, index),
        MachineRegister::Aarch64X(index) => (17, index),
        MachineRegister::Aarch64V(index) => (18, index),
    };
    bytes.push(tag);
    bytes.push(payload);
}

fn decode_machine_register(
    cursor: &mut Cursor<'_>,
) -> Result<MachineRegister, FixedViewCopyDecodeError> {
    let tag = cursor.byte()?;
    let payload = cursor.byte()?;
    if tag <= 15 && payload != 0 {
        return Err(FixedViewCopyDecodeError::InvalidMachineRegisterPayload(
            payload,
        ));
    }
    Ok(match tag {
        0 => MachineRegister::X86Rax,
        1 => MachineRegister::X86Rcx,
        2 => MachineRegister::X86Rdx,
        3 => MachineRegister::X86Rbx,
        4 => MachineRegister::X86Rsp,
        5 => MachineRegister::X86Rbp,
        6 => MachineRegister::X86Rsi,
        7 => MachineRegister::X86Rdi,
        8 => MachineRegister::X86R8,
        9 => MachineRegister::X86R9,
        10 => MachineRegister::X86R10,
        11 => MachineRegister::X86R11,
        12 => MachineRegister::X86R12,
        13 => MachineRegister::X86R13,
        14 => MachineRegister::X86R14,
        15 => MachineRegister::X86R15,
        16 => MachineRegister::X86Xmm(payload),
        17 => MachineRegister::Aarch64X(payload),
        18 => MachineRegister::Aarch64V(payload),
        tag => return Err(FixedViewCopyDecodeError::UnknownMachineRegister(tag)),
    })
}

fn decode_nominal<T>(
    cursor: &mut Cursor<'_>,
    constructor: fn(u64) -> Option<T>,
) -> Result<T, FixedViewCopyDecodeError> {
    let raw = cursor.u64()?;
    constructor(raw).ok_or(FixedViewCopyDecodeError::InvalidNominalId(raw))
}
