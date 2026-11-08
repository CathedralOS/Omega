/*
We lower a register-allocated selected instruction program into a sealed
post-allocation physical machine plan and run the post-allocation optimization
phase over that plan before sealing custody.

In Stage 05 (`register-homes`), virtual registers received concrete physical
register views (`rax`, `x0`, `xmm1`), and pre-allocation machine effects were
verified against the target register environment. However, each selected
instruction still listed a family of candidate machine alternatives rather than
one chosen physical instruction form, and its operand footprints still needed to
be projected onto concrete physical storage, read, and write units.

Our stage (`stage_optimized_post_allocation_machine_plan`) executes in three
deterministic steps:

    AllocatedProgramFacts + ValidatedTargetRegisterEnvironment
      |
      |  1. plan::compute_post_allocation_machine_plan (:UniqueApplicableChoice)
      v
    Baseline PostAllocationMachinePlan
      |     (every operand bound to its home view's physical units;
      |      each instruction bound to its unique applicable baseline alternative)
      |
      |  2. optimization::execute_post_allocation_machine_optimizations (:DeclaredFormLiveness)
      v
    Rewritten PostAllocationMachinePlan + PostAllocationMachineOptimizationRun
      |     (selected post-allocation rules rewrite instructions onto ISA-declared
      |      OptimizationSelected forms when extra clobbered units are proved dead)
      |
      |  3. plan::seal_post_allocation_machine_plan (:PostAllocationCustody)
      v
    StagedOptimizedPostAllocationMachinePlan

We never inspect prior rewrite history to choose a different construction
route, and we never synthesize machine encodings outside the target ISA's
declared alternative catalog.
*/

/*
Why we separate baseline alternative choice from post-allocation optimization:

During baseline construction (`:UniqueApplicableChoice`), target applicability
must identify a single legal alternative from the operand aliasing pattern
alone (for example, whether an x86-64 two-address destination aliases the left
source, the right source, both, or neither). Alternatives that clobber extra
physical units not written by the baseline form — such as replacing a 10-byte
`mov r64, 0` with a 3-byte `xor r32, r32` that clobbers `rflags` — cannot be
chosen by local operand aliasing alone because they are only sound when those
extra units are dead after the instruction.

By constructing the complete physical unit footprint of every instruction first,
our post-allocation optimization phase (`:DeclaredFormLiveness`) can compute
exact physical-unit liveness directly over `PostAllocationMachinePlan` without
reaching back to virtual-register live ranges. Both baseline construction and
optimization-selected forms draw from the same ISA alternative table, keeping
target instruction definitions in one place.
*/

/*
Provenance and custody boundaries:

We consume `AllocatedProgramFacts` from `register-homes` (produced by Stage 05)
and `ValidatedTargetRegisterEnvironment` from `register-environment`. We
produce `StagedOptimizedPostAllocationMachinePlan`, which pairs the sealed
`ValidatedPostAllocationMachinePlan` (`physical-instructions`) with the shared
`PreAllocationMachineEffectPlan`, the `PostAllocationMachineOptimizationRun`
evidence, and a `StagedOptimizedPostAllocationMachineCustodyReceipt`
(`:PostAllocationCustody`). Stage 07 (`post-allocation-machine-to-selected-form-encoding`)
and backend machine emission consume this staged plan to encode layout-independent
instruction bytes and frame-slot accesses.
*/

#![forbid(unsafe_code)]

macro_rules! stage_error_display {
    ($error:ty, $stage:literal) => {
        impl std::fmt::Display for $error {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, "{}: {self:?}", $stage)
            }
        }

        impl std::error::Error for $error {}
    };
}

macro_rules! copied_accessors {
    ($($(#[$meta:meta])* $name:ident : $ty:ty),* $(,)?) => {
        $(
            $(#[$meta])*
            pub const fn $name(&self) -> $ty {
                self.$name
            }
        )*
    };
}

macro_rules! borrowed_accessors {
    ($($(#[$meta:meta])* $name:ident : $ty:ty),* $(,)?) => {
        $(
            $(#[$meta])*
            pub const fn $name(&self) -> &$ty {
                &self.$name
            }
        )*
    };
}

mod optimization;
mod plan;

use std::sync::Arc;

use optimization_core::{
    OptimizationSelectionIdentity, OptimizationSelections, OptimizationWorkBudget,
};
use physical_instructions::PostAllocationMachineIdentity;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_homes::AllocatedProgramFacts;
use selected_instructions::PreAllocationMachineEffectPlan;

use optimization::execute_post_allocation_machine_optimizations;
pub use optimization::{
    PostAllocationMachineAction, PostAllocationMachineOptimizationError,
    PostAllocationMachineOptimizationRun, PostAllocationMachineRuleExecution,
};
use plan::{PlanSources, compute_post_allocation_machine_plan, seal_post_allocation_machine_plan};
pub use plan::{
    PostAllocationMachineError, PostAllocationMachineReceipt, ValidatedPostAllocationMachinePlan,
};

pub fn stage_optimized_post_allocation_machine_plan(
    allocation: AllocatedProgramFacts<'_>,
    environment: &ValidatedTargetRegisterEnvironment,
    selections: &OptimizationSelections,
    budget: OptimizationWorkBudget,
) -> Result<StagedOptimizedPostAllocationMachinePlan, OptimizedPostAllocationMachinePipelineError> {
    let mut machine = compute_post_allocation_machine_plan(&PlanSources {
        allocation,
        register_environment: environment.identity(),
        physical: environment.physical(),
        constraints: environment.constraints(),
    })
    .map_err(OptimizedPostAllocationMachinePipelineError::PostAllocation)?;
    let optimization = execute_post_allocation_machine_optimizations(
        &mut machine,
        allocation.selected.plan,
        allocation.effects,
        selections,
        budget,
    )
    .map_err(OptimizedPostAllocationMachinePipelineError::Optimization)?;
    let machine = seal_post_allocation_machine_plan(machine)
        .map_err(OptimizedPostAllocationMachinePipelineError::PostAllocation)?;
    Ok(seal_staged_post_allocation_machine(
        Arc::clone(allocation.effects),
        machine,
        optimization,
    ))
}

fn seal_staged_post_allocation_machine(
    effects: Arc<PreAllocationMachineEffectPlan>,
    machine: ValidatedPostAllocationMachinePlan,
    optimization: PostAllocationMachineOptimizationRun,
) -> StagedOptimizedPostAllocationMachinePlan {
    let custody = post_allocation_machine_custody(&effects, &machine, &optimization);
    StagedOptimizedPostAllocationMachinePlan {
        effects,
        machine,
        optimization,
        custody,
    }
}

fn post_allocation_machine_custody(
    effects: &PreAllocationMachineEffectPlan,
    machine: &ValidatedPostAllocationMachinePlan,
    optimization: &PostAllocationMachineOptimizationRun,
) -> StagedOptimizedPostAllocationMachineCustodyReceipt {
    StagedOptimizedPostAllocationMachineCustodyReceipt {
        effects: effects.identity,
        machine: machine.receipt().identity(),
        phase_selections: optimization.phase_selections(),
        optimization_source: optimization.source(),
        optimization_action_count: optimization.action_count(),
        function_count: machine.plan().functions.len(),
        instruction_count: machine.receipt().instruction_count(),
        operand_count: machine.receipt().operand_count(),
        unit_action_count: machine.receipt().unit_action_count(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedPostAllocationMachinePlan {
    effects: Arc<PreAllocationMachineEffectPlan>, // Retained pre-allocation machine effects paired with the plan.
    machine: ValidatedPostAllocationMachinePlan,  // Sealed post-allocation physical machine plan.
    optimization: PostAllocationMachineOptimizationRun, // Post-allocation optimization phase run evidence.
    custody: StagedOptimizedPostAllocationMachineCustodyReceipt, // Custody receipt binding effects, machine, and phase run.
}

impl StagedOptimizedPostAllocationMachinePlan {
    borrowed_accessors! {
        effects: Arc<PreAllocationMachineEffectPlan>,
        machine: ValidatedPostAllocationMachinePlan,
        optimization: PostAllocationMachineOptimizationRun,
        custody: StagedOptimizedPostAllocationMachineCustodyReceipt,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedPostAllocationMachineCustodyReceipt {
    effects: selected_instructions::PreAllocationMachineEffectIdentity,
    machine: PostAllocationMachineIdentity,
    phase_selections: OptimizationSelectionIdentity, // Post-allocation phase projection identity.
    optimization_source: PostAllocationMachineIdentity, // Baseline plan identity before phase rules ran.
    optimization_action_count: usize,
    function_count: usize,
    instruction_count: usize,
    operand_count: usize,
    unit_action_count: usize,
}

impl StagedOptimizedPostAllocationMachineCustodyReceipt {
    copied_accessors! {
        effects: selected_instructions::PreAllocationMachineEffectIdentity,
        machine: PostAllocationMachineIdentity,
        phase_selections: OptimizationSelectionIdentity,
        optimization_source: PostAllocationMachineIdentity,
        optimization_action_count: usize,
        function_count: usize,
        instruction_count: usize,
        operand_count: usize,
        unit_action_count: usize,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedPostAllocationMachinePipelineError {
    PostAllocation(PostAllocationMachineError),
    Optimization(PostAllocationMachineOptimizationError),
}

stage_error_display!(
    OptimizedPostAllocationMachinePipelineError,
    "optimized post-allocation machine staging failed"
);
