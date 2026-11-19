//! Selected physical stack transfers for a single entry bridge.
use crate::native_entry_bridge::{NativeEntryBridgeCode, NativeEntryBridgePlan};
use arena::Arena;
use diagnostics::Diagnostic;
fn invalid(reason: &str) -> Diagnostic {
    Diagnostic::error(format!("physical image entry: {reason}"))
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalIncomingStackRequirement {
    pub bytes_below_entry_rsp: u64,
    pub readable_bytes_from_entry_rsp: u64,
    pub caller_alignment: u64,
    pub caller_rsp_bias: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackTransfer {
    pub source: u32,
    pub destination: u32,
    pub width: u32,
}

pub type StackSwitchPlan<Origin> = crate::PhysicalRecipe<Origin, StackSwitchInstructions<Origin>>;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackSwitchInstructions<Origin> {
    pub bridge: NativeEntryBridgePlan<Origin>,
    pub transfers: Vec<StackTransfer>,
    pub outgoing_frame_bytes: u32,
    pub incoming: PhysicalIncomingStackRequirement,
    pub effects: calling_conventions::StateEffectsEvidence,
    pub complete_effects: calling_conventions::StateEffectsEvidence,
}

impl<Origin> StackSwitchPlan<Origin> {
    pub fn into_instructions(self) -> StackSwitchInstructions<Origin> {
        self.instructions
    }
    pub fn bridge(&self) -> &NativeEntryBridgePlan<Origin> {
        &self.instructions.bridge
    }
    pub fn incoming(&self) -> PhysicalIncomingStackRequirement {
        self.instructions.incoming
    }
    pub fn stack_overhead_bytes(&self) -> u64 {
        u64::from(self.instructions.outgoing_frame_bytes) + 8
    }
    pub fn effects(&self) -> &calling_conventions::StateEffectsEvidence {
        &self.instructions.effects
    }
    pub fn complete_effects(&self) -> &calling_conventions::StateEffectsEvidence {
        &self.instructions.complete_effects
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Destination {
    #[default]
    SavedStack,
    SavedAccumulator,
    SavedScratch,
    StackTop,
    Body,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Fixup {
    pub field: usize,
    pub destination: Destination,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackSwitch {
    pub bytes: Vec<u8>,
    pub fixups: Arena<Fixup>,
    pub outgoing_frame_bytes: u32,
    pub incoming: PhysicalIncomingStackRequirement,
    pub effects: calling_conventions::StateEffectsEvidence,
    pub body_call: u32,
    pub complete_effects: calling_conventions::StateEffectsEvidence,
}

impl StackSwitch {
    pub fn stack_overhead_bytes(&self) -> u64 {
        u64::from(self.outgoing_frame_bytes) + 8
    }
}

impl StackSwitch {
    /// Verify the encoded inline bridge and the one field patched to the body.
    pub fn validate_bridge(&self, bridge: &NativeEntryBridgeCode) -> Result<(), Diagnostic> {
        let offset = self
            .bytes
            .len()
            .checked_sub(bridge.bytes().len())
            .ok_or_else(|| invalid("stack switch lost its inline entry bridge"))?;
        if self.bytes.get(offset..) != Some(bridge.bytes())
            || offset.checked_add(bridge.call_field_offset()) != Some(self.body_call as usize)
            || self
                .fixups
                .iter()
                .filter(|(_, fixup)| fixup.destination == Destination::Body)
                .map(|(_, fixup)| fixup.field)
                .ne(std::iter::once(self.body_call as usize))
            || bridge
                .request()
                .checked_complete_effects(&self.effects)
                .map_err(|error| invalid(&error.to_string()))?
                != self.complete_effects
        {
            return Err(invalid(
                "stack switch does not contain its exact entry bridge and body call",
            ));
        }
        Ok(())
    }
}
