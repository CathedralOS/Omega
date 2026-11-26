// Inert physical calling-contract recovery without validator or native custody.

use super::Error;
use super::reader::Reader;
use crate::record::{
    PackagePolicyEntryControl, PackagePolicyEntryStack, PackagePolicyMachineRegime,
    PackagePolicyMachineState, PackagePolicyMachineStateSet, PackagePolicyPhysicalCallingContract,
    PackagePolicyPreemption, PackagePolicyStatePlan, PackageReviewBoundaryCallingPolicy,
    PackageReviewBoundaryValueClass as Class, PackageReviewBoundaryValueLocation as Location,
    PackageReviewBoundaryValuePlacement, PackageReviewBoundaryValueShape,
    PackageReviewIndirectPointerLocation as Pointer, PackageReviewMachineRegister as Register,
    PackageReviewSystemVEightbyteClass,
};

pub(in crate::encoding::recovery) fn physical(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyPhysicalCallingContract, Error> {
    Ok(PackagePolicyPhysicalCallingContract {
        policy: policy(reader)?,
        parameters: reader.sequence(13, value_placement)?,
        result: reader.option(value_placement)?,
        ordinary_clobbers: reader.sequence(1, register)?,
        stack_alignment: reader.u16()?,
        shadow_bytes: reader.u16()?,
        entry_control: entry_control(reader)?,
        state: state_plan(reader)?,
    })
}

pub(in crate::encoding::recovery) fn policy(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewBoundaryCallingPolicy, Error> {
    Ok(super::tag!(reader => {
        0 => PackageReviewBoundaryCallingPolicy::MicrosoftX64,
        1 => PackageReviewBoundaryCallingPolicy::SystemVAMD64,
        2 => PackageReviewBoundaryCallingPolicy::Aapcs64,
        3 => PackageReviewBoundaryCallingPolicy::LinuxSyscallX86_64,
        4 => PackageReviewBoundaryCallingPolicy::LinuxSyscallAarch64,
        5 => PackageReviewBoundaryCallingPolicy::DarwinAapcs64}))
}

// Bounded recovery for physical entry control, machine regimes, state sets, stacks, and preemption plans.

pub(in crate::encoding::recovery) fn entry_control(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyEntryControl, Error> {
    Ok(super::tag!(reader => {
        0 => PackagePolicyEntryControl::CallReturn,
        1 => PackagePolicyEntryControl::SupervisorCall {
            number_register: register(reader)?,
            immediate: reader.u16()?,
        },
        2 => PackagePolicyEntryControl::InterruptReturn,
    }))
}

pub(in crate::encoding::recovery) fn state_plan(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyStatePlan, Error> {
    Ok(PackagePolicyStatePlan {
        initial_regime: super::tag!(reader => {
            0 => PackagePolicyMachineRegime::X86Long64,
            1 => PackagePolicyMachineRegime::Aarch64A64 {
                exception_level: reader.byte()?,
            },
        }),
        interrupted_state: state_set(reader)?,
        saved_state: state_set(reader)?,
        restored_state: state_set(reader)?,
        permitted_transitive_use: state_set(reader)?,
        stack: super::tag!(reader => {
            0 => PackagePolicyEntryStack::Interrupted,
            1 => PackagePolicyEntryStack::Dedicated {
                class: reader.u16()?,
            },
            2 => PackagePolicyEntryStack::ProviderSelected,
        }),
        preemption: super::tag!(reader => {
            0 => PackagePolicyPreemption::NotApplicable,
            1 => PackagePolicyPreemption::Masked,
            2 => PackagePolicyPreemption::Nestable {
                maximum_depth: reader.u16()?,
            },
            3 => PackagePolicyPreemption::ProviderDefined,
        }),
    })
}

pub(in crate::encoding::recovery) fn state_set(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyMachineStateSet, Error> {
    let states = reader.sequence(1, |reader| {
        Ok(super::tag!(reader => {
            0 => PackagePolicyMachineState::GeneralRegisters,
            1 => PackagePolicyMachineState::VectorRegisters,
            2 => PackagePolicyMachineState::Flags,
            3 => PackagePolicyMachineState::InstructionPointer,
            4 => PackagePolicyMachineState::StackPointer,
            5 => PackagePolicyMachineState::SegmentState,
            6 => PackagePolicyMachineState::ControlState,
            7 => PackagePolicyMachineState::DebugState,
            8 => PackagePolicyMachineState::ExtendedState,
        }))
    })?;
    PackagePolicyMachineStateSet::from_canonical(states).ok_or(Error::NonCanonicalEncoding)
}

// Bounded recovery for boundary value placements, System V eightbyte classes, and machine registers.

pub(in crate::encoding::recovery) fn value_placement(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewBoundaryValuePlacement, Error> {
    let class = super::tag!(reader => {
        0 => Class::Integer,
        6 => Class::IntegerAggregate,
        1 => Class::Float,
        2 => Class::HomogeneousFloatAggregate {
            members: reader.byte()?,
        },
        3 => Class::SystemVAggregate {
            first: eightbyte(reader)?,
            second: eightbyte(reader)?,
        },
        4 => Class::BorrowedReference,
        5 => Class::BorrowedScalarReference,
    });
    Ok(PackageReviewBoundaryValuePlacement {
        shape: PackageReviewBoundaryValueShape {
            class,
            byte_size: reader.u32()?,
            alignment: reader.u16()?,
        },
        locations: reader.sequence(6, location)?,
    })
}

pub(in crate::encoding::recovery) fn eightbyte(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewSystemVEightbyteClass, Error> {
    Ok(super::tag!(reader => {
        0 => PackageReviewSystemVEightbyteClass::Integer,
        1 => PackageReviewSystemVEightbyteClass::Sse,
    }))
}

pub(in crate::encoding::recovery) fn location(reader: &mut Reader<'_>) -> Result<Location, Error> {
    Ok(super::tag!(reader => {
        0 => Location::Register {
            register: register(reader)?,
            value_byte_offset: reader.u32()?,
            byte_size: reader.u32()?,
        },
        1 => Location::Stack {
            stack_byte_offset: reader.u32()?,
            value_byte_offset: reader.u32()?,
            byte_size: reader.u32()?,
            alignment: reader.u16()?,
        },
        2 => Location::Indirect {
            pointer: super::tag!(reader => {
                0 => Pointer::Register(register(reader)?),
                1 => Pointer::Stack {
                    stack_byte_offset: reader.u32()?,
                    alignment: reader.u16()?,
                },
            }),
            copy_stack_byte_offset: reader.option(Reader::u32)?,
            byte_size: reader.u32()?,
            alignment: reader.u16()?,
        },
    }))
}

pub(in crate::encoding::recovery) fn register(reader: &mut Reader<'_>) -> Result<Register, Error> {
    Ok(super::tag!(reader => {
        0 => Register::X86Rax,
        1 => Register::X86Rcx,
        2 => Register::X86Rdx,
        3 => Register::X86Rbx,
        4 => Register::X86Rsp,
        5 => Register::X86Rbp,
        6 => Register::X86Rsi,
        7 => Register::X86Rdi,
        8 => Register::X86R8,
        9 => Register::X86R9,
        10 => Register::X86R10,
        11 => Register::X86R11,
        12 => Register::X86R12,
        13 => Register::X86R13,
        14 => Register::X86R14,
        15 => Register::X86R15,
        16 => Register::X86Xmm(reader.byte()?),
        17 => Register::Aarch64X(reader.byte()?),
        18 => Register::Aarch64V(reader.byte()?),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::{
        PackagePolicyRecoveryLimits,
        encode::{declarations::encode_value_placement, encoder::Encoder},
    };

    #[test]
    fn darwin_integer_aggregate_shape_round_trip_preserves_composite_identity() {
        let mut encoded = Vec::new();
        for class in [Class::Integer, Class::IntegerAggregate] {
            let placement = PackageReviewBoundaryValuePlacement {
                shape: PackageReviewBoundaryValueShape {
                    class,
                    byte_size: 4,
                    alignment: 2,
                },
                locations: vec![Location::Stack {
                    stack_byte_offset: 8,
                    value_byte_offset: 0,
                    byte_size: 4,
                    alignment: 8,
                }],
            };
            let mut encoder = Encoder::bounded(512);
            encode_value_placement(&mut encoder, &placement).unwrap();
            let bytes = encoder.finish().unwrap();
            let mut reader = Reader::new(&bytes, PackagePolicyRecoveryLimits::default()).unwrap();
            assert_eq!(value_placement(&mut reader).unwrap(), placement);
            reader.finish().unwrap();
            encoded.push(bytes);
        }
        assert_ne!(encoded[0], encoded[1]);
    }
}
