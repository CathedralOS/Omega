use calling_conventions::{
    CallPlan, CallbackMaterialization, CallingPolicy, EntryControl, IndirectPointerLocation,
    LayoutPlanId, LayoutSlotId, MachineRegister, NativeParameterId, NativePlace, RegisterSet,
    StaticMachineBinderId, SystemVEightbyteClass, ValueClass, ValueLocation, ValuePlacement,
    ValueShape,
};

use crate::FixedViewCopyDecodeError;

use crate::register_homes::recovery::fixed_view_copy::codec::primitives::{Cursor, decode_list};

#[cfg(test)]
#[test]
fn wide_shape_and_fragment_offsets_round_trip_without_truncation() {
    for placement in [
        ValuePlacement {
            shape: ValueShape::borrowed_reference(u32::MAX, 8),
            locations: vec![ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(MachineRegister::Aarch64X(0)),
                copy_stack_byte_offset: None,
                byte_size: u32::MAX,
                alignment: 8,
            }],
        },
        ValuePlacement {
            shape: ValueShape::integer(131_072, 8),
            locations: vec![
                ValueLocation::Stack {
                    stack_byte_offset: 0,
                    value_byte_offset: 0,
                    byte_size: 65_536,
                    alignment: 8,
                },
                ValueLocation::Stack {
                    stack_byte_offset: 65_536,
                    value_byte_offset: 65_536,
                    byte_size: 65_536,
                    alignment: 8,
                },
            ],
        },
    ] {
        let mut encoded = Vec::new();
        legalized_operations::encode_placement(&mut encoded, &placement);
        let mut cursor = Cursor::new(&encoded);
        assert_eq!(decode_placement(&mut cursor).unwrap(), placement);
        assert_eq!(cursor.remaining(), 0);
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
    let parameters = decode_list(cursor, decode_placement)?;
    let result = crate::cursor::optional(
        cursor,
        |c| Ok(decode_placement(c)?),
        FixedViewCopyDecodeError::UnknownOption,
    )?;
    let callback_materializations = decode_list(cursor, decode_callback)?;
    let ordinary_clobbers = decode_list(cursor, decode_machine_register)?;
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

fn decode_callback(
    cursor: &mut Cursor<'_>,
) -> Result<CallbackMaterialization, FixedViewCopyDecodeError> {
    let binder = decode_nominal(cursor, StaticMachineBinderId::new)?;
    Ok(CallbackMaterialization {
        binder,
        destination: decode_native_place(cursor)?,
    })
}

pub(super) fn decode_native_place(
    cursor: &mut Cursor<'_>,
) -> Result<NativePlace, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        1 => Ok(NativePlace::Parameter(decode_nominal(
            cursor,
            NativeParameterId::new,
        )?)),
        2 => {
            let parameter = decode_nominal(cursor, NativeParameterId::new)?;
            let layout = decode_nominal(cursor, LayoutPlanId::new)?;
            let field_path =
                decode_list(cursor, |cursor| decode_nominal(cursor, LayoutSlotId::new))?;
            Ok(NativePlace::Field {
                parameter,
                layout,
                field_path,
            })
        }
        tag => Err(FixedViewCopyDecodeError::UnknownNativePlace(tag)),
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
                value_byte_offset: cursor.u32()?,
                byte_size: cursor.u32()?,
            },
            2 => ValueLocation::Stack {
                stack_byte_offset: cursor.u32()?,
                value_byte_offset: cursor.u32()?,
                byte_size: cursor.u32()?,
                alignment: cursor.u16()?,
            },
            3 => ValueLocation::Indirect {
                pointer: decode_indirect_pointer(cursor)?,
                copy_stack_byte_offset: match cursor.byte()? {
                    0 => None,
                    1 => Some(cursor.u32()?),
                    tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
                },
                byte_size: cursor.u32()?,
                alignment: cursor.u16()?,
            },
            tag => return Err(FixedViewCopyDecodeError::UnknownValueLocation(tag)),
        });
    }
    Ok(ValuePlacement { shape, locations })
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

pub(super) fn decode_shape(
    cursor: &mut Cursor<'_>,
) -> Result<ValueShape, FixedViewCopyDecodeError> {
    let class = match cursor.byte()? {
        1 => ValueClass::Integer,
        2 => ValueClass::Float,
        5 => ValueClass::BorrowedReference,
        6 => ValueClass::BorrowedScalarReference,
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
        byte_size: cursor.u32()?,
        alignment: cursor.u16()?,
    })
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
