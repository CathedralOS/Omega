//! Architectural state affected by a selected model-specific register.
use calling_conventions::{MachineState, MachineStateSet, RegisterSet, StateEffectsEvidence};
use selected_instructions::RegisterOperation;

pub fn x86_64_model_specific_register_state(
    operation: RegisterOperation,
    selector: Option<u32>,
) -> Result<StateEffectsEvidence, &'static str> {
    match operation {
        RegisterOperation::ModelSpecificRead => match selector {
            Some(0xc000_0080 | 0x277) => Ok(StateEffectsEvidence::new(
                RegisterSet::default(),
                RegisterSet::default(),
                MachineStateSet::new([MachineState::ControlState]),
                MachineStateSet::empty(),
            )),
            Some(_) => Err("MSR read selector has no target-defined architectural footprint"),
            None => Err("MSR read selector is not an exact retained u32 constant"),
        },
        RegisterOperation::ModelSpecificWrite => {
            Err("MSR write has no target-defined architectural footprint")
        }
        _ => Err("MSR architectural footprint requested for another operation"),
    }
}
