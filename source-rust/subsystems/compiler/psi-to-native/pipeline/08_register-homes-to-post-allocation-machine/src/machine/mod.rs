/*
We lower a register-allocated selected instruction program into a sealed
post-allocation physical machine plan and run the post-allocation optimization
phase over that plan before sealing it.

During register allocation in native stage 07, virtual registers received physical
register views (`rax`, `x0`, `xmm1`), and pre-allocation machine effects were
verified against the target register environment. However, each selected
instruction still listed a family of candidate machine alternatives rather than
one chosen physical instruction form, and its operand footprints still needed to
be projected onto concrete physical storage, read, and write units.

Our stage (`stage_optimized_post_allocation_machine_plan`) executes in three
deterministic steps:

    AllocatedProgramFacts (including its register environment)
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
      |  3. ValidatedPostAllocationMachinePlan::new (:PostAllocationCustody)
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

We consume `AllocatedProgramFacts` from native stage 07,
including its settled preservation requirements and register environment. We
produce `StagedOptimizedPostAllocationMachinePlan`, which pairs the sealed
`ValidatedPostAllocationMachinePlan` (`post-allocation-machine`) with the
`PostAllocationMachineOptimizationRun` evidence (`:PostAllocationCustody`).
The common encoder (`native-encoding`)
and backend machine emission read its current `PostAllocationMachineProgram`.
That form owns physical choices and shares the exact unchanged function
instruction/contract chunks. It retains no selected-program container or
allocation callback.
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

use optimization::execute_post_allocation_machine_optimizations;
pub use optimization::{
    PostAllocationMachineOptimizationError, PostAllocationMachineOptimizationRun,
};
use plan::{PlanSources, compute_post_allocation_machine_plan};
pub use plan::{PostAllocationMachineError, ValidatedPostAllocationMachinePlan};

/// Machine lowering takes the allocation issued by the register allocator.
///
/// ```compile_fail,E0308
/// fn bypass(bare: register_homes::AllocatedProgramFacts<'_>) {
///     let _ = register_homes_to_post_allocation_machine::stage_optimized_post_allocation_machine_plan(bare);
/// }
/// ```
pub fn stage_optimized_post_allocation_machine_plan(
    allocated: &allocation::RetainedAllocation,
) -> Result<StagedOptimizedPostAllocationMachinePlan, OptimizedPostAllocationMachinePipelineError> {
    let allocation = allocated.facts();
    let environment = allocation.environment;
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
        allocation.publication.selections(),
        allocation.publication.budget(),
    )
    .map_err(OptimizedPostAllocationMachinePipelineError::Optimization)?;
    let machine = ValidatedPostAllocationMachinePlan::new(machine);
    let mut current = post_allocation_machine::PostAllocationMachineProgram {
        contracts: Arc::clone(&allocation.selected.plan.contracts),
        psi: allocation.selected.plan.psi,
        work_schedule: allocation.selected.work_schedule,
        entry: allocation.selected.plan.entry,
        machine: machine.shared_plan(),
        environment: environment.clone(),
        preservation: allocation.preservation.clone(),
        publication: Arc::clone(allocation.publication),
        phase: post_allocation_machine::PostAllocationMachinePhaseReceipt::from_parts(
            optimization.complete_selection(),
            optimization.phase_selections(),
        ),
        functions: allocation.selected.plan.functions.clone(),
        identity: [0; 32],
    };
    current.identity = current.recomputed_identity();
    Ok(StagedOptimizedPostAllocationMachinePlan {
        body: std::sync::Arc::clone(allocated.body_facts()),
        current,
        machine,
        optimization,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedPostAllocationMachinePlan {
    body: std::sync::Arc<physical_program::NativeBodyFacts>,
    current: post_allocation_machine::PostAllocationMachineProgram,
    machine: ValidatedPostAllocationMachinePlan, // Sealed post-allocation physical machine plan.
    optimization: PostAllocationMachineOptimizationRun, // Post-allocation optimization phase run evidence.
}

impl StagedOptimizedPostAllocationMachinePlan {
    pub fn body_facts(&self) -> &std::sync::Arc<physical_program::NativeBodyFacts> {
        &self.body
    }

    pub fn into_current(self) -> post_allocation_machine::PostAllocationMachineProgram {
        self.current
    }

    borrowed_accessors! {
        current: post_allocation_machine::PostAllocationMachineProgram,
        machine: ValidatedPostAllocationMachinePlan,
        optimization: PostAllocationMachineOptimizationRun,
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

impl OptimizedPostAllocationMachinePipelineError {
    /// The selected optimization that this target has no implementation of, when that is
    /// why staging failed.
    pub fn unimplemented_optimization(&self) -> Option<optimization_core::Optimization> {
        match self {
            Self::Optimization(
                PostAllocationMachineOptimizationError::NoExecutor { optimization }
                | PostAllocationMachineOptimizationError::TargetMismatch { optimization, .. },
            ) => Some(*optimization),
            _ => None,
        }
    }
}
