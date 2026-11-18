//! Encode the checked switch to the physical entry stack.
use crate::{
    native_entry_bridge::NativeEntryBridge,
    physical_stack_switch::{StackSwitch, encode},
};
use diagnostics::Diagnostic;
use physical_recipes::physical_entry::{PhysicalEntryParts, PhysicalEntryProgram};

pub struct EncodedPhysicalEntry<Origin, Context> {
    context: Context,
    bridge: NativeEntryBridge<Origin>,
    switch: StackSwitch,
}
impl<Origin, Context> EncodedPhysicalEntry<Origin, Context> {
    pub fn into_parts(self) -> (Context, NativeEntryBridge<Origin>, StackSwitch) {
        (self.context, self.bridge, self.switch)
    }
}

pub fn encode_physical_entry<Origin, Context>(
    plan: PhysicalEntryProgram<Origin, Context>,
) -> Result<EncodedPhysicalEntry<Origin, Context>, Diagnostic> {
    let PhysicalEntryParts { context, switch } = plan.into_parts();
    let (bridge, switch) = encode(switch)?;
    Ok(EncodedPhysicalEntry {
        context,
        bridge,
        switch,
    })
}
