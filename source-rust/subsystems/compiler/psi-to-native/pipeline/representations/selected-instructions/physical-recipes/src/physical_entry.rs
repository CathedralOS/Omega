//! Checked stack-switch instructions and their physical-entry requirements.
use crate::{
    PhysicalRecipe,
    physical_stack_switch::{PhysicalIncomingStackRequirement, StackSwitchPlan},
};

pub type PhysicalEntryProgram<Origin, Context> =
    PhysicalRecipe<Origin, PhysicalEntryParts<Origin, Context>>;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalEntryParts<Origin, Context> {
    pub context: Context,
    pub switch: StackSwitchPlan<Origin>,
}
impl<Origin, Context> PhysicalEntryProgram<Origin, Context> {
    pub fn into_parts(self) -> PhysicalEntryParts<Origin, Context> {
        self.instructions
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstalledPhysicalCpuRequirement {
    X86LongModeCpl0WithoutCet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalEntryStackRequirement {
    CheckedPrivateImage {
        incoming: PhysicalIncomingStackRequirement,
        minimum_private_stack_bytes: u64,
    },
}
