//! Lay out an encoded adapter's body-call instruction before object relocation.

use physical_recipes::{
    native_entry_bridge::NativeEntryBridgeCode, native_foreign_bridge::NativeForeignBridgeCode,
};
use target::Architecture;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterCallField {
    X86Relative32 { field_offset: usize },
    Aarch64Branch26 { instruction_offset: usize },
}

impl AdapterCallField {
    fn read(architecture: Architecture, bytes: &[u8], field: usize) -> Option<Self> {
        match architecture {
            Architecture::X86_64 => {
                let opcode = field.checked_sub(1)?;
                let end = field.checked_add(4)?;
                (bytes.get(opcode..end)? == target_x86_64::ORDINARY_DIRECT_CALL_BYTES).then_some(
                    Self::X86Relative32 {
                        field_offset: field,
                    },
                )
            }
            Architecture::Aarch64 => {
                let end = field.checked_add(4)?;
                (field.is_multiple_of(4)
                    && bytes.get(field..end)? == target_aarch64::ORDINARY_DIRECT_CALL_BYTES)
                    .then_some(Self::Aarch64Branch26 {
                        instruction_offset: field,
                    })
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BridgePlacementError;
impl std::fmt::Display for BridgePlacementError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("native bridge does not contain its unresolved body call at the declared field")
    }
}
impl std::error::Error for BridgePlacementError {}

/// A call field admitted from the compiler's encoded adapter. The two placement
/// constructors require the physical planner's concrete issuing origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedBridge<Code> {
    code: Code,
    call: AdapterCallField,
}

pub type PlacedNativeEntryBridge = PlacedBridge<NativeEntryBridgeCode>;
pub type PlacedNativeForeignBridge = PlacedBridge<NativeForeignBridgeCode>;

impl<Code> PlacedBridge<Code> {
    pub fn into_parts(self) -> (Code, AdapterCallField) {
        (self.code, self.call)
    }
}

pub fn place_native_entry_bridge(
    encoded: native_verification::bridges::EncodedNativeEntryBridge,
) -> Result<PlacedNativeEntryBridge, BridgePlacementError> {
    let call = AdapterCallField::read(
        encoded.request().target.architecture,
        encoded.bytes(),
        encoded.call_field_offset(),
    )
    .ok_or(BridgePlacementError)?;
    Ok(PlacedBridge {
        code: encoded.into_code(),
        call,
    })
}

pub fn place_native_foreign_bridge(
    encoded: native_verification::bridges::EncodedNativeForeignBridge,
) -> Result<PlacedNativeForeignBridge, BridgePlacementError> {
    let call = AdapterCallField::read(
        encoded.request().target.architecture,
        encoded.bytes(),
        encoded.call_field_offset(),
    )
    .ok_or(BridgePlacementError)?;
    Ok(PlacedBridge {
        code: encoded.into_code(),
        call,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedCallableEntry {
    context: physical_recipes::callable_entry::CallableContext,
    bridge: Option<PlacedNativeEntryBridge>,
}
impl PlacedCallableEntry {
    pub fn into_parts(
        self,
    ) -> (
        physical_recipes::callable_entry::CallableContext,
        Option<PlacedNativeEntryBridge>,
    ) {
        (self.context, self.bridge)
    }
}

pub fn place_callable_entry(
    encoded: native_verification::bridges::EncodedCallableEntry,
) -> Result<PlacedCallableEntry, BridgePlacementError> {
    let (context, bridge) = encoded.into_parts();
    let bridge = bridge.map(place_native_entry_bridge).transpose()?;
    Ok(PlacedCallableEntry { context, bridge })
}

pub struct PlacedPrivateEntry {
    identity: function_identity::MachineFunctionIdentity,
    transport: PlacedCallableEntry,
}
impl PlacedPrivateEntry {
    pub fn into_parts(
        self,
    ) -> (
        function_identity::MachineFunctionIdentity,
        PlacedCallableEntry,
    ) {
        (self.identity, self.transport)
    }
}

pub fn place_private_entry(
    encoded: native_verification::bridges::EncodedPrivateEntry,
) -> Result<PlacedPrivateEntry, BridgePlacementError> {
    let (identity, transport) = encoded.into_parts();
    Ok(PlacedPrivateEntry {
        identity,
        transport: place_callable_entry(transport)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_call_fields_check_offsets_alignment_and_unresolved_operands() {
        for (arch, code, field, expected) in [
            (
                Architecture::X86_64,
                &[0x90, 0xe8, 0, 0, 0, 0, 0xc3][..],
                2,
                AdapterCallField::X86Relative32 { field_offset: 2 },
            ),
            (
                Architecture::Aarch64,
                &[0, 0, 0, 0, 0, 0, 0, 0x94][..],
                4,
                AdapterCallField::Aarch64Branch26 {
                    instruction_offset: 4,
                },
            ),
        ] {
            assert_eq!(AdapterCallField::read(arch, code, field), Some(expected));
            for wrong in [0, field - 1, field + 1, usize::MAX] {
                assert!(AdapterCallField::read(arch, code, wrong).is_none());
            }
            let start = field - usize::from(arch == Architecture::X86_64);
            for index in start..field + 4 {
                let mut changed = code.to_vec();
                changed[index] ^= 1;
                assert!(AdapterCallField::read(arch, &changed, field).is_none());
            }
        }
    }
}
