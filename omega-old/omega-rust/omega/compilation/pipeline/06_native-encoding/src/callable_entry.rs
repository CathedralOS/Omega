//! Encode an already selected callable bridge and preserve its exact entry contract.
use crate::native_entry_bridge::{NativeEntryBridge, encode_native_entry_bridge};
use function_identity::MachineFunctionIdentity;
use physical_recipes::{
    callable_entry::{
        CallableContext, CallableEntry, CallableInstructions, PrivateEntry, PrivateEntryParts,
    },
    native_entry_bridge::NativeEntryBridgeError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallableEntryTransport<Origin> {
    context: CallableContext,
    bridge: Option<NativeEntryBridge<Origin>>,
}
impl<Origin> CallableEntryTransport<Origin> {
    pub fn context(&self) -> &CallableContext {
        &self.context
    }
    pub fn bridge(&self) -> Option<&NativeEntryBridge<Origin>> {
        self.bridge.as_ref()
    }
    pub fn into_parts(self) -> (CallableContext, Option<NativeEntryBridge<Origin>>) {
        (self.context, self.bridge)
    }
}

pub fn encode_callable_entry_transport<Origin, Cpu>(
    plan: CallableEntry<Origin, Cpu>,
) -> Result<CallableEntryTransport<Origin>, NativeEntryBridgeError> {
    let CallableInstructions {
        context, bridge, ..
    } = plan.into_parts();
    let bridge = bridge.map(encode_native_entry_bridge).transpose()?;
    Ok(CallableEntryTransport { context, bridge })
}

pub struct EncodedPrivateEntry<Origin> {
    identity: MachineFunctionIdentity,
    transport: CallableEntryTransport<Origin>,
}
impl<Origin> EncodedPrivateEntry<Origin> {
    pub fn into_parts(self) -> (MachineFunctionIdentity, CallableEntryTransport<Origin>) {
        (self.identity, self.transport)
    }
}
pub fn encode_private_function_entry<Origin, Cpu>(
    plan: PrivateEntry<Origin, Cpu>,
) -> Result<EncodedPrivateEntry<Origin>, NativeEntryBridgeError> {
    let PrivateEntryParts {
        identity,
        transport,
    } = plan.into_parts();
    Ok(EncodedPrivateEntry {
        identity,
        transport: encode_callable_entry_transport(transport)?,
    })
}
