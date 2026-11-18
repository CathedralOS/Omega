//! Branches and effects for the selected Windows helper exports.
use calling_conventions::StateEffectsEvidence;
use post_allocation_machine::adapters::AdapterOperation;
pub type WindowsInternetProgram<Origin> = crate::PhysicalRecipe<Origin, WindowsInternetExports>;
#[derive(Debug)]
pub struct WindowsInternetExports {
    pub make_address: PlannedAuxiliaryExport,
    pub local_address: PlannedAuxiliaryExport,
}

#[derive(Debug)]
pub enum Step {
    Instruction(AdapterOperation),
    Label(usize),
    Branch {
        condition: Option<Condition>,
        target: usize,
    },
}
#[derive(Debug, Clone, Copy)]
pub enum Condition {
    UnsignedGreaterOrEqual,
    NotEqual,
}

#[derive(Debug)]
pub struct PlannedAuxiliaryExport {
    pub steps: Vec<Step>,
    pub effects: StateEffectsEvidence,
}

impl<Origin> WindowsInternetProgram<Origin> {
    pub fn into_exports(self) -> (PlannedAuxiliaryExport, PlannedAuxiliaryExport) {
        (
            self.instructions.make_address,
            self.instructions.local_address,
        )
    }
}
impl PlannedAuxiliaryExport {
    pub fn into_parts(self) -> (Vec<Step>, StateEffectsEvidence) {
        (self.steps, self.effects)
    }
}
