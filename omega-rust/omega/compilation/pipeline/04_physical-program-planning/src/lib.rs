//! Complete physical instructions and frame recipes before resource binding.
#![forbid(unsafe_code)]

#[macro_use]
extern crate selected_instruction_analyses;
mod instruction_selection;
mod selected_rewrites;
pub use instruction_selection::{
    OptimizedSelectionPipelineError, SelectionInput as PhysicalBodyInput,
};
pub use selected_rewrites::{SelectedInstructionOptimizationError, SelectedOptimizationReplay};
#[cfg(feature = "test-support")]
pub mod test_support {
    pub use crate::instruction_selection::{
        BoundInstallations, InstallationBindingError, OptimizedSelectionPipelineError,
        select_instructions_for_plan,
    };
}
mod allocation;
mod machine;
pub use allocation::{RegisterAllocationError, RetainedAllocation};
pub use machine::{
    OptimizedPostAllocationMachinePipelineError, StagedOptimizedPostAllocationMachinePlan,
};
mod frame_layout;
pub mod frame_protocol;
use frame_admission::frame_layout::{
    NonAuthoritativeCalleeSaveStorageError, NonAuthoritativeCalleeSaveStoragePolicy,
    TargetFrameLayoutError, TargetFrameLayoutPolicy, ValidatedTargetFrameLayout,
};
use frame_admission::frame_protocol::TargetFrameProtocolEncodingError;
use frame_layout::{stage_non_authoritative_callee_save_storage, stage_target_frame_layout};
use frame_protocol::{PlannedTargetFrameProtocols, plan_target_frame_protocols};
use machine_code::TargetFrameProtocolEncodingPolicy;
use post_allocation_machine::PostAllocationMachineProgram;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalFrameError {
    CalleeSaveStorage(NonAuthoritativeCalleeSaveStorageError),
    FrameLayout(TargetFrameLayoutError),
    FrameProtocol(TargetFrameProtocolEncodingError),
}
diagnostics::failed_display!(PhysicalFrameError, "physical frame planning failed");
impl std::error::Error for PhysicalFrameError {}
/// Complete physical instructions and stack geometry before any frame bytes
/// are encoded. Resource binding can inspect this form without emitting code.
#[derive(Debug)]
pub struct PhysicalFrameProgram {
    pub instructions: PostAllocationMachineProgram,
    pub storage: Arc<machine_code::NonAuthoritativeCalleeSaveStoragePlan>,
    pub frame: ValidatedTargetFrameLayout,
    pub protocols: PlannedTargetFrameProtocols,
}

fn plan_function_frame(
    program: PostAllocationMachineProgram,
    policy: TargetFrameLayoutPolicy,
) -> Result<PhysicalFrameProgram, PhysicalFrameError> {
    let environment = &program.environment;
    let requirements = program.preservation.clone();
    let budget = requirements.plan().budget;
    let storage = stage_non_authoritative_callee_save_storage(
        &requirements,
        environment,
        NonAuthoritativeCalleeSaveStoragePolicy::CanonicalTargetPreservationGroupsV1,
        budget,
    )
    .map_err(PhysicalFrameError::CalleeSaveStorage)?;
    let layout = stage_target_frame_layout(
        &program.machine,
        &requirements,
        &storage,
        environment,
        policy,
    )
    .map_err(PhysicalFrameError::FrameLayout)?;
    let protocols = plan_target_frame_protocols(
        &layout,
        environment,
        TargetFrameProtocolEncodingPolicy::CanonicalFixedFrameV1,
    )
    .map_err(PhysicalFrameError::FrameProtocol)?;
    Ok(PhysicalFrameProgram {
        instructions: program,
        storage: Arc::new(storage.into_plan()),
        frame: layout,
        protocols,
    })
}

/// Counts entered private operations without lending a compiler callback.
/// These counters are observations only and never participate in admission.
pub struct PhysicalPlanningObservation<'a> {
    pub instruction_selection: &'a std::sync::atomic::AtomicU64,
    pub selected_optimization: &'a std::sync::atomic::AtomicU64,
    pub register_allocation: &'a std::sync::atomic::AtomicU64,
    pub post_allocation_machine: &'a std::sync::atomic::AtomicU64,
}

/// Issued originals exist only when an explicit audit requested retention.
#[derive(Debug)]
pub struct PhysicalPlanningOriginals {
    pub selected_optimization: Option<SelectedOptimizationReplay>,
    pub allocation: RetainedAllocation,
    pub machine: StagedOptimizedPostAllocationMachinePlan,
}

pub struct PlannedPhysicalProgram {
    pub frame: PhysicalFrameProgram,
    pub originals: Option<PhysicalPlanningOriginals>,
}

#[derive(Debug)]
pub enum PhysicalPlanningError {
    RegisterEnvironment(register_environment::TargetRegisterEnvironmentValidationError),
    Selection(OptimizedSelectionPipelineError),
    SelectedOptimization(SelectedInstructionOptimizationError),
    Allocation(RegisterAllocationError),
    Machine(OptimizedPostAllocationMachinePipelineError),
    Frame(PhysicalFrameError),
}
diagnostics::failed_display!(PhysicalPlanningError, "physical program planning failed");
impl std::error::Error for PhysicalPlanningError {}

/// Allocate, choose physical alternatives, and settle frame recipes as one
/// operation. Intermediate graphs and solver entrances stay private here.
pub fn plan_physical_program(
    input: PhysicalBodyInput,
    observation: PhysicalPlanningObservation<'_>,
    retain_originals: bool,
) -> Result<PlannedPhysicalProgram, PhysicalPlanningError> {
    use std::sync::atomic::Ordering;
    let environment =
        register_environment::baseline_target_register_environment(input.program.plan.plan().target)
            .map_err(PhysicalPlanningError::RegisterEnvironment)?;
    observation
        .instruction_selection
        .fetch_add(1, Ordering::Relaxed);
    let selected = instruction_selection::stage_optimized_instruction_selection(input, environment)
        .map_err(PhysicalPlanningError::Selection)?;
    observation
        .selected_optimization
        .fetch_add(1, Ordering::Relaxed);
    let optimized = selected_rewrites::optimize_selected_instructions(
        selected_instruction_analyses::SelectedOptimizationInput {
            program: selected.selected_program(),
            custody: selected.custody().core(),
            register_environment: selected.register_environment,
            selections: selected.selections,
            budget_per_pass: selected.budget_per_pass,
            record: selected_instruction_analyses::SelectedOptimizationRecord::default(),
        },
    )
    .map_err(PhysicalPlanningError::SelectedOptimization)?;
    let (input, selected_optimization) = if retain_originals {
        optimized.retain_replay_for_validation()
    } else {
        (optimized.into_allocation_input(), None)
    };
    observation
        .register_allocation
        .fetch_add(1, Ordering::Relaxed);
    let allocation =
        allocation::stage_register_allocation(input).map_err(PhysicalPlanningError::Allocation)?;
    observation
        .post_allocation_machine
        .fetch_add(1, Ordering::Relaxed);
    let machine = machine::stage_optimized_post_allocation_machine_plan(allocation.facts())
        .map_err(PhysicalPlanningError::Machine)?;
    let frame = plan_function_frame(
        machine.current().clone(),
        TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
    )
    .map_err(PhysicalPlanningError::Frame)?;
    let originals = retain_originals.then_some(PhysicalPlanningOriginals {
        selected_optimization,
        allocation,
        machine,
    });
    Ok(PlannedPhysicalProgram { frame, originals })
}

mod bridge_values;
pub mod hosted_entry;
pub mod imports;
pub mod handler_order;
pub mod installation_resources;
pub mod installed_arrival;
pub mod native_entry_bridge;
pub mod native_foreign_bridge;
pub mod physical_stack_switch;
pub mod windows_internet;

pub mod boundary_settlements;
pub mod call_origins;
pub mod foreign_call_plans;
pub mod image_origins;
pub mod internal_call_plans;
pub mod physical_facts;
pub mod stack_demand;
