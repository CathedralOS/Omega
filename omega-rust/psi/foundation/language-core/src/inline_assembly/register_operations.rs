//! Scalar register events shared by checked planning and target realization.
//!
//! The register catalog supplies identity and operand contracts. These events
//! retain an actual read or write, independently of the source place receiving
//! a read or the expression supplying a write. Neither grants machine authority.

use super::{AsmSystemRegister, AsmTargetApplicability};

/// Exact unsigned scalar carriers of physical register operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmRegisterCarrier {
    U8,
    U16,
    U32,
    U64,
}

impl AsmRegisterCarrier {
    pub const fn bits(self) -> u16 {
        match self {
            Self::U8 => 8,
            Self::U16 => 16,
            Self::U32 => 32,
            Self::U64 => 64,
        }
    }
    pub const fn name(self) -> &'static str {
        match self {
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AsmRegisterOperation {
    SystemRead(AsmSystemRegister),
    SystemWrite(AsmSystemRegister),
    PortRead,
    PortWrite,
    ModelSpecificRead,
    ModelSpecificWrite,
}

impl AsmRegisterOperation {
    /// Stable event identity: the operation family and exact register. These
    /// tags carry no ISA encoding, operand values or authority disposition.
    pub const fn catalog_tag(self) -> u16 {
        match self {
            Self::SystemRead(register) => register.catalog_id() as u16,
            Self::SystemWrite(register) => 0x100 | register.catalog_id() as u16,
            Self::PortRead => 0x200,
            Self::PortWrite => 0x201,
            Self::ModelSpecificRead => 0x300,
            Self::ModelSpecificWrite => 0x301,
        }
    }

    pub const fn from_catalog_tag(tag: u16) -> Option<Self> {
        match tag {
            0x200 => return Some(Self::PortRead),
            0x201 => return Some(Self::PortWrite),
            0x300 => return Some(Self::ModelSpecificRead),
            0x301 => return Some(Self::ModelSpecificWrite),
            _ => {}
        }
        let Some(register) = AsmSystemRegister::from_catalog_id((tag & 0xff) as u8) else {
            return None;
        };
        match tag >> 8 {
            0 => Some(Self::SystemRead(register)),
            1 if register.write_intrinsic_name().is_some() => Some(Self::SystemWrite(register)),
            _ => None,
        }
    }

    /// Decode the canonical name of an already resolved compiler builtin.
    pub fn from_intrinsic_name(name: &str) -> Option<Self> {
        match name {
            "asm#port_in" => return Some(Self::PortRead),
            "asm#port_out" => return Some(Self::PortWrite),
            "asm#rdmsr" => return Some(Self::ModelSpecificRead),
            "asm#wrmsr" => return Some(Self::ModelSpecificWrite),
            _ => {}
        }
        AsmSystemRegister::from_read_intrinsic_name(name)
            .map(Self::SystemRead)
            .or_else(|| AsmSystemRegister::from_write_intrinsic_name(name).map(Self::SystemWrite))
    }

    pub const fn produces_value(self) -> bool {
        self.result_carrier().is_some()
    }

    /// Explicit source input carriers. A read's destination is its result,
    /// rather than another value argument or an untyped destination address.
    pub const fn input_carriers(self) -> &'static [AsmRegisterCarrier] {
        use AsmRegisterCarrier::*;
        match self {
            Self::SystemRead(_) => &[],
            Self::SystemWrite(_) => &[U64],
            Self::PortRead => &[U16],
            Self::PortWrite => &[U16, U8],
            Self::ModelSpecificRead => &[U32],
            Self::ModelSpecificWrite => &[U32, U64],
        }
    }

    pub const fn result_carrier(self) -> Option<AsmRegisterCarrier> {
        match self {
            Self::SystemRead(_) | Self::ModelSpecificRead => Some(AsmRegisterCarrier::U64),
            Self::PortRead => Some(AsmRegisterCarrier::U8),
            _ => None,
        }
    }

    pub const fn target_applicability(self) -> AsmTargetApplicability {
        match self {
            Self::SystemRead(_) | Self::SystemWrite(_) => AsmTargetApplicability::Aarch64,
            _ => AsmTargetApplicability::X86_64,
        }
    }

    /// Registers without a source-catalog write accessor cannot acquire one
    /// through decoding; hardware support alone does not add a source contract.
    pub const fn is_admitted(self) -> bool {
        match self {
            Self::SystemRead(_)
            | Self::PortRead
            | Self::PortWrite
            | Self::ModelSpecificRead
            | Self::ModelSpecificWrite => true,
            Self::SystemWrite(register) => register.write_intrinsic_name().is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn x86_register_signatures_retain_exact_carriers_and_catalog_identity() {
        use AsmRegisterCarrier::*;
        for (name, operation, inputs, result) in [
            (
                "asm#port_in",
                AsmRegisterOperation::PortRead,
                &[U16][..],
                Some(U8),
            ),
            (
                "asm#port_out",
                AsmRegisterOperation::PortWrite,
                &[U16, U8][..],
                None,
            ),
            (
                "asm#rdmsr",
                AsmRegisterOperation::ModelSpecificRead,
                &[U32][..],
                Some(U64),
            ),
            (
                "asm#wrmsr",
                AsmRegisterOperation::ModelSpecificWrite,
                &[U32, U64][..],
                None,
            ),
        ] {
            assert_eq!(
                AsmRegisterOperation::from_intrinsic_name(name),
                Some(operation)
            );
            assert_eq!(
                AsmRegisterOperation::from_catalog_tag(operation.catalog_tag()),
                Some(operation)
            );
            assert_eq!(operation.input_carriers(), inputs);
            assert_eq!(operation.result_carrier(), result);
            assert_eq!(
                operation.target_applicability(),
                AsmTargetApplicability::X86_64
            );
        }
        for tag in [0x202, 0x302, 0x400, u16::MAX] {
            assert_eq!(AsmRegisterOperation::from_catalog_tag(tag), None);
        }
    }
}
