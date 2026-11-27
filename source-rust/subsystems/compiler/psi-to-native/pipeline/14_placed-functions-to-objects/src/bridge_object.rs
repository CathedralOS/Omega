//! An adapter fragment and its unresolved object-relative call relocation.
use crate::RelocationKind;
use placement::entry_layout::{ResolvedBridge, ResolvedCallableEntry};
use machine_code::layout::AdapterCallField;
use physical_recipes::{
    native_entry_bridge::NativeEntryBridgeCode, native_foreign_bridge::NativeForeignBridgeCode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BridgeCallRelocation {
    pub offset: usize,
    pub kind: RelocationKind,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBridgeObject<Code> {
    code: Code,
    relocation: BridgeCallRelocation,
}

impl<Code> NativeBridgeObject<Code> {
    pub(crate) fn from_call_field(placed: ResolvedBridge<Code>) -> Self {
        let (code, call) = placed.into_parts();
        let relocation = match call {
            AdapterCallField::X86Relative32 { field_offset } => BridgeCallRelocation {
                offset: field_offset,
                kind: RelocationKind::X86_64Relative32,
            },
            AdapterCallField::Aarch64Branch26 { instruction_offset } => BridgeCallRelocation {
                offset: instruction_offset,
                kind: RelocationKind::Aarch64Branch26,
            },
        };
        Self { code, relocation }
    }

    pub fn call_relocation(&self) -> BridgeCallRelocation {
        self.relocation
    }
}
impl<Code> std::ops::Deref for NativeBridgeObject<Code> {
    type Target = Code;
    fn deref(&self) -> &Code {
        &self.code
    }
}

pub type NativeEntryBridge = NativeBridgeObject<NativeEntryBridgeCode>;
pub type NativeForeignBridge = NativeBridgeObject<NativeForeignBridgeCode>;

use machine_code::import_code::EncodedImportProgram;
use physical_recipes::native_foreign_bridge::ForeignAdapterParts;

/// The object owns the exported function, its normalized import, and the
/// relocation from the placed call field to that import.
pub struct ForeignAdapterObject(ForeignAdapterObjectParts);

pub struct ForeignAdapterObjectParts {
    pub bridge: NativeForeignBridge,
    pub locator: target::NormalizedForeignLocator,
    pub application: [u8; 32],
    pub imports: EncodedImportProgram,
    pub object: crate::ObjectPlan,
    pub relocations: crate::RelocationPlan,
    pub export: String,
    pub export_commitment: [u8; 32],
}

impl ForeignAdapterObject {
    pub fn from_layout(
        placed: placement::entry_layout::ResolvedForeignAdapter,
    ) -> Result<Self, String> {
        let ForeignAdapterParts {
            bridge,
            imports,
            locator,
            application,
        } = placed.into_parts();
        let bridge = NativeBridgeObject::from_call_field(bridge);
        let target = bridge.request().target;
        let policy = crate::object_target_policy(target);
        let mut identity = ::wire::digest_encoder::DomainDigestEncoder::new(
            b"omega.native-foreign-frame-export.v1",
        );
        identity.raw(&application);
        identity.raw(&locator.identity_digest().as_bytes());
        identity.bytes(bridge.bytes());
        let export_commitment = identity.finish();
        use std::fmt::Write;
        let mut export = String::from("__omega_foreign_frame_");
        for byte in export_commitment {
            write!(&mut export, "{byte:02x}").expect("writing into String");
        }

        let mut object = crate::ObjectPlan::with_capacity(target, 1, 2);
        object.layout.sections.insert(crate::SectionPlan {
            kind: crate::SectionKind::Text,
            size: bridge.bytes().len(),
            alignment: usize::try_from(policy.text_section_alignment)
                .map_err(|_| "foreign adapter text alignment exceeds the host size")?,
        });
        let entry = object.layout.symbols.insert(crate::SymbolPlan {
            name: if target.object_format == target::ObjectFormat::MachO {
                format!("_{export}")
            } else {
                export.clone()
            },
            section: crate::SymbolSection::Section(crate::SectionKind::Text),
            offset: 0,
            size: bridge.bytes().len(),
            kind: crate::SymbolKind::Function,
        });
        object.layout.entry_symbol = entry;
        let imported = object
            .layout
            .insert_import("foreign-adapter-import".into(), &locator);
        let call = bridge.call_relocation();
        let mut relocations = crate::RelocationPlan::with_record_capacity(target, 1);
        relocations.push_record(crate::RelocationRecord {
            origin: crate::RelocationOrigin::Materialization {
                object_symbol_handle: entry,
            },
            section: crate::SectionKind::Text,
            offset: call.offset,
            byte_width: 4,
            symbol_handle: imported,
            addend: 0,
            kind: call.kind,
        });
        Ok(Self(ForeignAdapterObjectParts {
            bridge,
            imports,
            locator,
            application,
            object,
            relocations,
            export,
            export_commitment,
        }))
    }

    pub fn into_parts(self) -> ForeignAdapterObjectParts {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallableEntryTransport {
    context: physical_recipes::callable_entry::CallableContext,
    bridge: Option<NativeEntryBridge>,
}
impl CallableEntryTransport {
    pub fn from_layout(layout: ResolvedCallableEntry) -> Self {
        let (context, bridge) = layout.into_parts();
        Self {
            context,
            bridge: bridge.map(NativeBridgeObject::from_call_field),
        }
    }
    pub fn context(&self) -> &physical_recipes::callable_entry::CallableContext {
        &self.context
    }
    pub fn bridge(&self) -> Option<&NativeEntryBridge> {
        self.bridge.as_ref()
    }
    pub fn into_parts(
        self,
    ) -> (
        physical_recipes::callable_entry::CallableContext,
        Option<NativeEntryBridge>,
    ) {
        (self.context, self.bridge)
    }
}

pub struct PrivateEntryObject {
    identity: function_identity::MachineFunctionIdentity,
    transport: CallableEntryTransport,
}
impl PrivateEntryObject {
    /// A private entry must have passed call-field layout before object construction.
    ///
    /// ```compile_fail,E0308
    /// fn skip_layout(encoded_bytes: Vec<u8>) {
    ///     let _ = object_file::PrivateEntryObject::from_layout(encoded_bytes);
    /// }
    /// ```
    pub fn from_layout(placed: placement::entry_layout::ResolvedPrivateEntry) -> Self {
        let (identity, transport) = placed.into_parts();
        Self {
            identity,
            transport: CallableEntryTransport::from_layout(transport),
        }
    }
    pub fn into_parts(
        self,
    ) -> (
        function_identity::MachineFunctionIdentity,
        CallableEntryTransport,
    ) {
        (self.identity, self.transport)
    }
}
