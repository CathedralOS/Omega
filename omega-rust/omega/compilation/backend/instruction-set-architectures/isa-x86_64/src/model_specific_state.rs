//! Describe only the architectural MSR reads this target currently models.
//! Register transport, privilege faults and execution authority are independent.

use calling_conventions::{MachineState, MachineStateSet, RegisterSet, StateEffectsEvidence};
use selected_instructions::RegisterOperation;

/// The exact selector and direction determine the architectural state effect.
/// EFER and PAT reads observe control state without writing it. This description
/// excludes the separately replayed GPR transport and grants no access authority.
/// Intel SDM volumes 2 and 4 define RDMSR and these register identities:
/// https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html
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
