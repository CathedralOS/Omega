#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Post-allocation machine analysis components.
//!
//! The allocation phase supplies one replayed current-program view. This stage
//! does not inspect rewrite history or select a different construction route.

mod construction;
mod model;
mod plan;
mod validation;

pub use construction::*;
pub use model::*;
pub use plan::*;
pub use validation::*;

use selected_instructions_to_register_homes::AllocationEvidence;

use selected_instructions_to_register_homes::ValidatedPreAllocationMachineEffects;

fn seal_staged_post_allocation_machine(
    source: AllocationEvidence,
    effects: ValidatedPreAllocationMachineEffects,
    machine: ValidatedPostAllocationMachinePlan,
) -> StagedOptimizedPostAllocationMachinePlan {
    let custody = post_allocation_machine_custody(source, &effects, &machine);
    StagedOptimizedPostAllocationMachinePlan {
        effects,
        machine,
        custody,
    }
}

fn post_allocation_machine_custody(
    source: AllocationEvidence,
    effects: &selected_instructions_to_register_homes::ValidatedPreAllocationMachineEffects,
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
