//! Borrowed access to the same allocated program, not another representation.

use crate::register_homes::{AllocatedProgram, RegisterHomePlan};
use target_operations_to_selected_instructions::SelectedInstructionPlan;

#[derive(Debug, Clone, Copy)]
pub struct AllocatedProgramRef<'program> {
    pub selected: &'program SelectedInstructionPlan,
    pub homes: &'program RegisterHomePlan,
}

impl AllocatedProgram {
    pub fn as_ref(&self) -> AllocatedProgramRef<'_> {
        AllocatedProgramRef {
            selected: &self.selected,
            homes: &self.homes,
        }
    }
}
