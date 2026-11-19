//! Encode an already selected callable bridge and preserve its exact entry contract.
use crate::native_entry_bridge::encode_native_entry_bridge;
use function_identity::MachineFunctionIdentity;
use physical_recipes::{
    callable_entry::CallableContext,
    native_entry_bridge::{NativeEntryBridgeCode, NativeEntryBridgeError},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallableEntryTransport {
    context: CallableContext,
    bridge: Option<NativeEntryBridgeCode>,
}
impl CallableEntryTransport {
    pub fn context(&self) -> &CallableContext {
        &self.context
    }
    pub fn bridge(&self) -> Option<&NativeEntryBridgeCode> {
        self.bridge.as_ref()
    }
    pub fn into_parts(self) -> (CallableContext, Option<NativeEntryBridgeCode>) {
        (self.context, self.bridge)
    }
}

pub fn encode_callable_entry_transport(
    plan: installation_binding::entries::BoundCallableEntry,
) -> Result<CallableEntryTransport, NativeEntryBridgeError> {
    let (context, bridge) = plan.into_parts();
    let bridge = bridge
        .map(|plan| encode_native_entry_bridge(plan).map(|bridge| bridge.into_code()))
        .transpose()?;
    Ok(CallableEntryTransport { context, bridge })
}

pub struct EncodedPrivateEntry {
    identity: MachineFunctionIdentity,
    transport: CallableEntryTransport,
}
impl EncodedPrivateEntry {
    pub fn into_parts(self) -> (MachineFunctionIdentity, CallableEntryTransport) {
        (self.identity, self.transport)
    }
}
pub fn encode_private_function_entry(
    plan: installation_binding::entries::BoundPrivateEntry,
) -> Result<EncodedPrivateEntry, NativeEntryBridgeError> {
    let (identity, transport) = plan.into_parts();
    Ok(EncodedPrivateEntry {
        identity,
        transport: encode_callable_entry_transport(transport)?,
    })
}
