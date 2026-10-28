//! Closed mechanism tags retained by admitted external provider records.

/// Closed mechanism tag for one irreducible external realization. This is
/// retained by admitted provider records, independently of source syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalBindingMechanism {
    Syscall,
    CompilerIntrinsic,
    VtableSlot,
    VtableField,
    TableFunction,
}

impl ExternalBindingMechanism {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Syscall => "syscall",
            Self::CompilerIntrinsic => "compiler_intrinsic",
            Self::VtableSlot => "vtable_slot",
            Self::VtableField => "vtable_field",
            Self::TableFunction => "table_function",
        }
    }

    /// Tag 1 belonged to the retired string-backed import mechanism and stays
    /// unused. These tags fold into contract identity, so renumbering the
    /// survivors would change every external realization's fingerprint.
    pub const fn identity_tag(self) -> u8 {
        match self {
            Self::Syscall => 2,
            Self::CompilerIntrinsic => 3,
            Self::VtableSlot => 4,
            Self::VtableField => 5,
            Self::TableFunction => 6,
        }
    }
}
