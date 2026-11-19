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

/// Resolve the call field against the bytes carried by the encoded entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedBridge<Code> {
    code: Code,
    call: AdapterCallField,
}

pub type ResolvedNativeEntryBridge = ResolvedBridge<NativeEntryBridgeCode>;
pub type ResolvedNativeForeignBridge = ResolvedBridge<NativeForeignBridgeCode>;

impl<Code> ResolvedBridge<Code> {
    pub fn into_parts(self) -> (Code, AdapterCallField) {
        (self.code, self.call)
    }
}

pub(crate) fn resolve_native_entry_bridge(
    encoded: NativeEntryBridgeCode,
) -> Result<ResolvedNativeEntryBridge, BridgePlacementError> {
    let call = AdapterCallField::read(
        encoded.request().target.architecture,
        encoded.bytes(),
        encoded.call_field_offset(),
    )
    .ok_or(BridgePlacementError)?;
    Ok(ResolvedBridge {
        code: encoded,
        call,
    })
}

pub(crate) fn resolve_native_foreign_bridge(
    encoded: NativeForeignBridgeCode,
) -> Result<ResolvedNativeForeignBridge, BridgePlacementError> {
    let call = AdapterCallField::read(
        encoded.request().target.architecture,
        encoded.bytes(),
        encoded.call_field_offset(),
    )
    .ok_or(BridgePlacementError)?;
    Ok(ResolvedBridge {
        code: encoded,
        call,
    })
}

use machine_code::import_code::EncodedImportProgram;
use physical_recipes::native_foreign_bridge::ForeignAdapterParts;

pub struct ResolvedForeignAdapter(
    ForeignAdapterParts<ResolvedNativeForeignBridge, EncodedImportProgram>,
);

impl ResolvedForeignAdapter {
    pub fn into_parts(
        self,
    ) -> ForeignAdapterParts<ResolvedNativeForeignBridge, EncodedImportProgram> {
        self.0
    }
}

pub fn resolve_foreign_adapter(
    adapter: native_encoding::native_foreign_bridge::EncodedForeignAdapter,
) -> Result<crate::ResolvedEntry<ResolvedForeignAdapter>, BridgePlacementError> {
    let ForeignAdapterParts {
        bridge,
        imports,
        locator,
        application,
    } = adapter.into_parts();
    Ok(crate::ResolvedEntry(ResolvedForeignAdapter(
        ForeignAdapterParts {
            bridge: resolve_native_foreign_bridge(bridge)?,
            imports,
            locator,
            application,
        },
    )))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCallableEntry {
    context: physical_recipes::callable_entry::CallableContext,
    bridge: Option<ResolvedNativeEntryBridge>,
}
impl ResolvedCallableEntry {
    pub fn into_parts(
        self,
    ) -> (
        physical_recipes::callable_entry::CallableContext,
        Option<ResolvedNativeEntryBridge>,
    ) {
        (self.context, self.bridge)
    }
}

pub fn resolve_callable_entry(
    encoded: native_encoding::callable_entry::CallableEntryTransport,
) -> Result<crate::ResolvedEntry<ResolvedCallableEntry>, BridgePlacementError> {
    let (context, bridge) = encoded.into_parts();
    let bridge = bridge.map(resolve_native_entry_bridge).transpose()?;
    Ok(crate::ResolvedEntry(ResolvedCallableEntry {
        context,
        bridge,
    }))
}

pub struct ResolvedPrivateEntry {
    identity: function_identity::MachineFunctionIdentity,
    transport: ResolvedCallableEntry,
}
impl ResolvedPrivateEntry {
    pub fn into_parts(
        self,
    ) -> (
        function_identity::MachineFunctionIdentity,
        ResolvedCallableEntry,
    ) {
        (self.identity, self.transport)
    }
}

pub fn resolve_private_entry(
    encoded: native_encoding::callable_entry::EncodedPrivateEntry,
) -> Result<crate::ResolvedEntry<ResolvedPrivateEntry>, BridgePlacementError> {
    let (identity, transport) = encoded.into_parts();
    Ok(crate::ResolvedEntry(ResolvedPrivateEntry {
        identity,
        transport: resolve_callable_entry(transport)?.into_layout(),
    }))
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
