//! Symbolic materialization of native callback addresses.

/// Physical destination retained without inventing a semantic callback value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallbackAddressDestination {
    Register(target_operations::MachineRegister),
    OutgoingStack {
        byte_offset: u32,
    },
    /// A private slot inside the record the register-resident root parameter
    /// names: the materialization writes the callback address at
    /// `[root_register + byte_offset]` through an architecture-fixed scratch.
    RecordField {
        root_register: target_operations::MachineRegister,
        byte_offset: u32,
    },
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
    pub target: target_operations::TargetNativeCallbackArgument,
    pub destination: CallbackAddressDestination,
    pub code_offset: usize,
    pub byte_count: usize,
    pub encoding: CallbackAddressEncoding,
}

/// Canonical identity encoding of one materialization destination, shared by
/// every identity builder that hashes a fixup's callback roster.
pub(crate) fn encode_callback_destination_identity(
    hasher: &mut sha2::Sha256,
    destination: CallbackAddressDestination,
) {
    use sha2::Digest;
    match destination {
        CallbackAddressDestination::Register(register) => {
            hasher.update([0]);
            hasher.update([register.codec_index()]);
        }
        CallbackAddressDestination::OutgoingStack { byte_offset } => {
            hasher.update([1]);
            hasher.update(byte_offset.to_le_bytes());
        }
        CallbackAddressDestination::RecordField {
            root_register,
            byte_offset,
        } => {
            hasher.update([2]);
            hasher.update([root_register.codec_index()]);
            hasher.update(byte_offset.to_le_bytes());
        }
    }
}
