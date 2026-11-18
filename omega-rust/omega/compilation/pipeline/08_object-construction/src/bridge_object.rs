//! An adapter fragment and its unresolved object-relative call relocation.
use crate::RelocationKind;
use function_layout::entry_placement::{AdapterCallField, PlacedBridge};
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
    pub fn from_placement(placed: PlacedBridge<Code>) -> Self {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallableEntryTransport {
    context: physical_recipes::callable_entry::CallableContext,
    bridge: Option<NativeEntryBridge>,
}
impl CallableEntryTransport {
    pub fn from_placement(placed: function_layout::entry_placement::PlacedCallableEntry) -> Self {
        let (context, bridge) = placed.into_parts();
        Self {
            context,
            bridge: bridge.map(NativeBridgeObject::from_placement),
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
    pub fn from_placement(placed: function_layout::entry_placement::PlacedPrivateEntry) -> Self {
        let (identity, transport) = placed.into_parts();
        Self {
            identity,
            transport: CallableEntryTransport::from_placement(transport),
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
