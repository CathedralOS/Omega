use super::{Error, Reader};
use crate::record::{
    PackageReviewBoundaryValueClass as Class, PackageReviewBoundaryValueLocation as Location,
    PackageReviewBoundaryValuePlacement, PackageReviewBoundaryValueShape,
    PackageReviewIndirectPointerLocation as Pointer, PackageReviewMachineRegister as Register,
    PackageReviewSystemVEightbyteClass,
};

pub(in crate::encoding::recovery::policy) fn value_placement(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewBoundaryValuePlacement, Error> {
    let class = match reader.byte()? {
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
        _ => return Err(Error::InvalidTag),
    };
    Ok(PackageReviewBoundaryValuePlacement {
        shape: PackageReviewBoundaryValueShape {
            class,
            byte_size: reader.u16()?,
            alignment: reader.u16()?,
        },
        locations: reader.sequence(6, location)?,
    })
}

fn eightbyte(reader: &mut Reader<'_>) -> Result<PackageReviewSystemVEightbyteClass, Error> {
    Ok(match reader.byte()? {
        0 => PackageReviewSystemVEightbyteClass::Integer,
        1 => PackageReviewSystemVEightbyteClass::Sse,
        _ => return Err(Error::InvalidTag),
    })
}

fn location(reader: &mut Reader<'_>) -> Result<Location, Error> {
    Ok(match reader.byte()? {
        0 => Location::Register {
            register: register(reader)?,
            value_byte_offset: reader.u16()?,
            byte_size: reader.u16()?,
        },
        1 => Location::Stack {
            stack_byte_offset: reader.u32()?,
            value_byte_offset: reader.u16()?,
            byte_size: reader.u16()?,
            alignment: reader.u16()?,
        },
        2 => Location::Indirect {
            pointer: match reader.byte()? {
                0 => Pointer::Register(register(reader)?),
                1 => Pointer::Stack {
                    stack_byte_offset: reader.u32()?,
                    alignment: reader.u16()?,
                },
                _ => return Err(Error::InvalidTag),
            },
            copy_stack_byte_offset: reader.option(Reader::u32)?,
            byte_size: reader.u16()?,
            alignment: reader.u16()?,
        },
        _ => return Err(Error::InvalidTag),
    })
}

pub(super) fn register(reader: &mut Reader<'_>) -> Result<Register, Error> {
    Ok(match reader.byte()? {
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
        _ => return Err(Error::InvalidTag),
    })
}
