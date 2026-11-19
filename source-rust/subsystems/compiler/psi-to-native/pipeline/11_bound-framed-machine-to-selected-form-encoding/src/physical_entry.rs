//! Encode the checked switch to the physical entry stack.
use crate::physical_stack_switch::{StackSwitch, encode};
use diagnostics::Diagnostic;
use physical_recipes::{
    native_entry_bridge::NativeEntryBridgeCode, physical_entry::PhysicalEntryParts,
};

pub struct EncodedPhysicalEntry<Context> {
    context: Context,
    bridge: NativeEntryBridgeCode,
    switch: StackSwitch,
}
impl<Context> EncodedPhysicalEntry<Context> {
    pub fn into_parts(self) -> (Context, NativeEntryBridgeCode, StackSwitch) {
        (self.context, self.bridge, self.switch)
    }
}

pub fn encode_physical_entry<Context>(
    plan: installation_binding::entries::BoundPhysicalEntry<Context>,
) -> Result<EncodedPhysicalEntry<Context>, Diagnostic> {
    let PhysicalEntryParts { context, switch } = plan.into_parts();
    let (bridge, switch) = encode(switch)?;
    Ok(EncodedPhysicalEntry {
        context,
        bridge: bridge.into_code(),
        switch,
    })
}
