use super::{Error, Reader};
use crate::record::{
    PackageReviewBoundaryValueClass as Class, PackageReviewBoundaryValueLocation as Location,
    PackageReviewBoundaryValuePlacement, PackageReviewBoundaryValueShape,
    PackageReviewIndirectPointerLocation as Pointer, PackageReviewMachineRegister as Register,
    PackageReviewSystemVEightbyteClass,
};

pub(in crate::encoding::recovery) fn value_placement(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewBoundaryValuePlacement, Error> {
    let class = super::super::tag!(reader => {
        0 => Class::Integer,
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
    Ok(super::super::tag!(reader => {
        0 => PackageReviewSystemVEightbyteClass::Integer,
        1 => PackageReviewSystemVEightbyteClass::Sse,
    }))
}

pub(in crate::encoding::recovery) fn location(reader: &mut Reader<'_>) -> Result<Location, Error> {
    Ok(super::super::tag!(reader => {
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
            pointer: super::super::tag!(reader => {
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
    Ok(super::super::tag!(reader => {
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
