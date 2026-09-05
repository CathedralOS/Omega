//! Symbolic materialization of native callback addresses.

/// Physical destination retained without inventing a semantic callback value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallbackAddressDestination {
    Register(omega_target_operations::MachineRegister),
    OutgoingStack { byte_offset: u32 },
}

/// Architecture-native symbolic address encoding. Offsets are relative to the
/// containing machine-code function and identify only mutable relocation
/// fields; every surrounding instruction bit remains final-byte checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallbackAddressEncoding {
    X86_64Relative32 {
        relocation_offset: usize,
    },
    Aarch64PageAddress {
        page_relocation_offset: usize,
        page_offset_relocation_offset: usize,
    },
}

/// Source-free custody for one callback function address loaded immediately
/// before the exact normalized registrar call that consumes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackAddressMaterialization {
    pub target: omega_target_operations::TargetNativeCallbackArgument,
    pub destination: CallbackAddressDestination,
    pub code_offset: usize,
    pub byte_count: usize,
    pub encoding: CallbackAddressEncoding,
}
