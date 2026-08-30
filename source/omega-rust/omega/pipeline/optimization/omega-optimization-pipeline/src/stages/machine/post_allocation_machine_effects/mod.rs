//! Optimizer module role: executable entrance. Post-allocation machine analysis stage.
//!
//! Route adapters project each supported selected-instruction lineage into the
//! common analysis join. This entrance seals the resulting effects and machine
//! plan into one replayable custody receipt.

mod construction;
mod model;
mod validation;

pub use construction::*;
pub use model::*;
pub use validation::*;

use omega_machine_optimizer::ValidatedPostAllocationMachinePlan;

use crate::StagedOptimizedMachineEffects;

fn seal_staged_post_allocation_machine(
    source: StagedOptimizedPostAllocationMachineSourceCustodyReceipt,
    effects: StagedOptimizedMachineEffects,
    machine: ValidatedPostAllocationMachinePlan,
) -> StagedOptimizedPostAllocationMachinePlan {
    let custody = post_allocation_machine_custody(source, effects.effects(), &machine);
    StagedOptimizedPostAllocationMachinePlan {
        effects,
        machine,
        custody,
    }
}

fn post_allocation_machine_custody(
    source: StagedOptimizedPostAllocationMachineSourceCustodyReceipt,
    effects: &omega_machine_optimizer::ValidatedPreAllocationMachineEffects,
    machine: &ValidatedPostAllocationMachinePlan,
) -> StagedOptimizedPostAllocationMachineCustodyReceipt {
    StagedOptimizedPostAllocationMachineCustodyReceipt {
        source,
        effects: effects.receipt().identity(),
        machine: machine.receipt().identity(),
        function_count: machine.plan().functions.len(),
        structural_unit_function_count: machine.plan().structural_unit_functions.len(),
        instruction_count: machine.receipt().instruction_count(),
        operand_count: machine.receipt().operand_count(),
        unit_action_count: machine.receipt().unit_action_count(),
    }
}
