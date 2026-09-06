//! Retained ordinary frame evidence, shared by all function realization paths.

use super::FunctionRelativeOptimizationRealizationError as Error;
use super::prelude::*;
use selected_instructions_to_register_homes::AllocationOutput;

#[derive(Debug, Clone)]
pub struct FunctionRelativeFrame {
    pub(in crate::function_realization) requirements: ValidatedAllocatedCalleeSavedRequirements,
    pub(in crate::function_realization) storage: ValidatedNonAuthoritativeCalleeSaveStorage,
    pub(in crate::function_realization) layout: ValidatedTargetFrameLayout,
    pub(in crate::function_realization) protocol: ValidatedTargetFrameProtocolEncoding,
}

impl FunctionRelativeFrame {
    pub const fn requirements(&self) -> &ValidatedAllocatedCalleeSavedRequirements {
        &self.requirements
    }

    pub const fn storage(&self) -> &ValidatedNonAuthoritativeCalleeSaveStorage {
        &self.storage
    }

    pub const fn layout(&self) -> &ValidatedTargetFrameLayout {
        &self.layout
    }

    pub const fn protocol(&self) -> &ValidatedTargetFrameProtocolEncoding {
        &self.protocol
    }
}

pub(super) fn stage_frame(
    current: &AllocationOutput<'_>,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    policy: TargetFrameLayoutPolicy,
    budget: OptimizationWorkBudget,
) -> Result<FunctionRelativeFrame, Error> {
    let environment = current.register_environment();
    let requirements = stage_allocated_callee_saved_requirements(
        current,
        AllocatedCalleeSavedRequirementPolicy::AllocatedSelectedWritesIntersectAbiPreservationV1,
        budget,
    )
    .map_err(Error::CalleeSavedRequirements)?;
    let storage = stage_non_authoritative_callee_save_storage(
        &requirements,
        environment,
        NonAuthoritativeCalleeSaveStoragePolicy::CanonicalTargetPreservationGroupsV1,
        budget,
    )
    .map_err(Error::CalleeSaveStorage)?;
    let layout = stage_target_frame_layout(machine, &requirements, &storage, environment, policy)
        .map_err(Error::FrameLayout)?;
    let protocol = stage_target_frame_protocol_encoding(
        &layout,
        environment,
        TargetFrameProtocolEncodingPolicy::CanonicalFixedFrameV1,
    )
    .map_err(Error::FrameProtocol)?;
    Ok(FunctionRelativeFrame {
        requirements,
        storage,
        layout,
        protocol,
    })
}

pub(super) fn validate_frame(
    current: &AllocationOutput<'_>,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    frame: &FunctionRelativeFrame,
    policy: TargetFrameLayoutPolicy,
) -> Result<(), Error> {
    if frame.layout.plan().policy != policy {
        return Err(Error::RootMismatch);
    }
    let environment = current.register_environment();
    let requirements =
        validate_allocated_callee_saved_requirements(current, frame.requirements.plan().clone())
            .map_err(Error::CalleeSavedRequirements)?;
    let storage = validate_non_authoritative_callee_save_storage(
        &requirements,
        environment,
        frame.storage.plan().clone(),
    )
    .map_err(Error::CalleeSaveStorage)?;
    let layout = validate_target_frame_layout(
        machine,
        &requirements,
        &storage,
        environment,
        frame.layout.plan().clone(),
    )
    .map_err(Error::FrameLayout)?;
    validate_target_frame_protocol_encoding(&layout, environment, frame.protocol.plan().clone())
        .map_err(Error::FrameProtocol)?;
    Ok(())
}

/// An input predicate, not frame construction: allocated physical writes and
/// ordinary calls determine whether the current body needs ABI frame support.
pub(super) fn ordinary_frame_required(
    current: &AllocationOutput<'_>,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<bool, Error> {
    let preservation =
        register_environment::selected_abi_preservation(current.register_environment())
            .map_err(|_| Error::RootMismatch)?;
    let saved = &preservation.convention.callee_saved;
    Ok(machine.machine().plan().functions.iter().any(|function| {
        function.blocks.iter().any(|block| {
            block.instructions.iter().any(|instruction| {
                instruction
                    .unit_defs
                    .iter()
                    .chain(&instruction.unit_clobbers)
                    .any(|unit| saved.contains(unit))
            })
        })
    }) || current.selected_plan().functions.iter().any(|function| {
        function.blocks.iter().any(|block| {
            block.instructions.iter().any(|instruction| {
                matches!(
                    instruction.kind,
                    selected_instructions::SelectedInstructionKind::CallI64 { .. }
                )
            })
        })
    }))
}
