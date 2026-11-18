//! Selected physical stack transfers for a single entry bridge.
use crate::native_entry_bridge::NativeEntryBridgePlan;
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
