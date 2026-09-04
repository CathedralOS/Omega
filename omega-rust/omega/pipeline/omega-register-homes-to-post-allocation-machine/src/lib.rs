#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Post-allocation machine analysis components.
//!
//! Route adapters project each supported selected-instruction lineage into the
//! common construction entrance. Shared receipt sealing remains beside the
//! construction, model, and validation components at this nearest ancestor.

mod construction;
mod model;
mod validation;

pub use construction::*;
pub use model::*;
pub use validation::*;

use omega_machine_optimizer::ValidatedPostAllocationMachinePlan;

use omega_machine_optimizer::ValidatedPreAllocationMachineEffects;

fn seal_staged_post_allocation_machine(
    source: StagedOptimizedPostAllocationMachineSourceCustodyReceipt,
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
