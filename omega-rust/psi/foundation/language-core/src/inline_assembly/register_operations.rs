//! Scalar register events shared by checked planning and target realization.
//!
//! The register catalog supplies identity and operand contracts. These events
//! retain an actual read or write, independently of the source place receiving
//! a read or the expression supplying a write. Neither grants machine authority.

use super::AsmSystemRegister;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AsmRegisterOperation {
    SystemRead(AsmSystemRegister),
    SystemWrite(AsmSystemRegister),
}

impl AsmRegisterOperation {
    /// Stable event identity: the operation family and exact register. These
    /// tags carry no ISA encoding, operand values or authority disposition.
    pub const fn catalog_tag(self) -> u16 {
        match self {
            Self::SystemRead(register) => register.catalog_id() as u16,
            Self::SystemWrite(register) => 0x100 | register.catalog_id() as u16,
        }
    }

    pub const fn from_catalog_tag(tag: u16) -> Option<Self> {
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
        AsmSystemRegister::from_read_intrinsic_name(name)
            .map(Self::SystemRead)
            .or_else(|| AsmSystemRegister::from_write_intrinsic_name(name).map(Self::SystemWrite))
    }

    pub const fn produces_value(self) -> bool {
        matches!(self, Self::SystemRead(_))
    }

    /// Explicit source input carriers. A read's destination is its result,
    /// rather than another value argument or an untyped destination address.
    pub const fn input_type_names(self) -> &'static [&'static str] {
        match self {
            Self::SystemRead(_) => &[],
            Self::SystemWrite(_) => &["u64"],
        }
    }

    /// Registers without a source-catalog write accessor cannot acquire one
    /// through decoding; hardware support alone does not add a source contract.
    pub const fn is_admitted(self) -> bool {
        match self {
            Self::SystemRead(_) => true,
            Self::SystemWrite(register) => register.write_intrinsic_name().is_some(),
        }
    }
}
