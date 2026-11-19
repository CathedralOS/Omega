//! Plan callee-save storage, stack layout and entry/exit protocols for allocated machine instructions.
#![forbid(unsafe_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameRecipeOrigin(());

mod frame_layout;
pub mod frame_protocol;
use frame_admission::frame_layout::{
    NonAuthoritativeCalleeSaveStorageError, NonAuthoritativeCalleeSaveStoragePolicy,
    TargetFrameLayoutError, TargetFrameLayoutPolicy,
};
use frame_admission::frame_protocol::TargetFrameProtocolEncodingError;
use frame_layout::{stage_non_authoritative_callee_save_storage, stage_target_frame_layout};
use frame_protocol::plan_target_frame_protocols;
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
pub type PhysicalFrameProgram =
    physical_recipes::frame_protocol::PhysicalFrameProgram<FrameRecipeOrigin>;

pub struct FramedNativeBody {
    frame: PhysicalFrameProgram,
    facts: std::sync::Arc<physical_program::NativeBodyFacts>,
}

impl FramedNativeBody {
    pub fn into_frame_parts(
        self,
    ) -> (
        PhysicalFrameProgram,
        std::sync::Arc<physical_program::NativeBodyFacts>,
    ) {
        (self.frame, self.facts)
    }
}

/// Frame planning takes the machine program issued by post-allocation lowering.
///
/// ```compile_fail,E0308
/// fn bypass(bare: post_allocation_machine::PostAllocationMachineProgram) {
///     let _ = post_allocation_machine_to_framed_machine::plan_function_frame(bare);
/// }
/// ```
pub fn plan_function_frame(
    machine: machine::StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    (
        FramedNativeBody,
        Option<machine::StagedOptimizedPostAllocationMachinePlan>,
    ),
    PhysicalFrameError,
> {
    let body = std::sync::Arc::clone(machine.body_facts());
    if body.retains_stage_inputs() {
        let frame = frame_instructions(machine.current().clone())?;
        Ok((FramedNativeBody { frame, facts: body }, Some(machine)))
    } else {
        Ok((
            FramedNativeBody {
                frame: frame_instructions(machine.into_current())?,
                facts: body,
            },
            None,
        ))
    }
}

fn frame_instructions(
    program: PostAllocationMachineProgram,
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
        TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
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

pub mod boundary_settlements;
pub mod call_origins;
pub mod foreign_call_plans;
pub mod image_origins;
pub mod internal_call_plans;
pub mod physical_facts;
pub mod stack_demand;
