use super::FunctionFragmentEmissionSourceKind;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use crate::{
    StagedAllocationRecoveryFunctionRelativeRealization,
    StagedFixedFrameFunctionRelativeRealization,
    StagedFunctionRelativeLayoutOptimizationRealization,
    StagedOptimizedStructuralUnitFunctionRelativeRealization,
    StagedOptimizedUnitFunctionRelativeRealization,
    StagedPostAllocationMachineFunctionRelativeRealization,
    StagedSelectedLoweringFunctionRelativeRealization,
};

#[derive(Debug)]
/// Retained inputs for independently replaying the completed realization.
/// These roles are replay inputs only. Current program data are retained separately.
pub enum FunctionFragmentReplayInputs {
    X86Rel8Direct(Box<StagedFunctionRelativeLayoutOptimizationRealization>),
    SelectedLowering(Box<StagedSelectedLoweringFunctionRelativeRealization>),
    PostAllocationMachine(Box<StagedPostAllocationMachineFunctionRelativeRealization>),
    AllocationRecovery(Box<StagedAllocationRecoveryFunctionRelativeRealization>),
    UnitBaseline(Box<StagedOptimizedUnitFunctionRelativeRealization>),
    StructuralUnit(Box<StagedOptimizedStructuralUnitFunctionRelativeRealization>),
    FixedFrame(Box<StagedFixedFrameFunctionRelativeRealization>),
}

impl FunctionFragmentReplayInputs {
    fn allocation(&self) -> &selected_instructions_to_register_homes::RetainedAllocation {
        match self {
            Self::X86Rel8Direct(realization) => realization.allocation(),
            Self::SelectedLowering(realization) => realization.allocation(),
            Self::PostAllocationMachine(realization) => realization.allocation(),
            Self::AllocationRecovery(realization) => realization.allocation(),
            Self::UnitBaseline(realization) => realization.allocation(),
            Self::StructuralUnit(realization) => realization.allocation(),
            Self::FixedFrame(realization) => realization.allocation(),
        }
    }

    pub fn source_kind(&self) -> FunctionFragmentEmissionSourceKind {
        match self {
            Self::X86Rel8Direct(_) => FunctionFragmentEmissionSourceKind::X86Rel8V1,
            Self::SelectedLowering(_) => FunctionFragmentEmissionSourceKind::SelectedLoweringV1,
            Self::PostAllocationMachine(realization) => {
                FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 {
                    optimization: realization.optimization().optimization(),
                }
            }
            Self::AllocationRecovery(_) => FunctionFragmentEmissionSourceKind::AllocationRecoveryV1,
            Self::UnitBaseline(_) => FunctionFragmentEmissionSourceKind::UnitBaselineV1,
            Self::StructuralUnit(_) => FunctionFragmentEmissionSourceKind::StructuralUnitV1,
            Self::FixedFrame(_) => FunctionFragmentEmissionSourceKind::CanonicalFixedFrameBodyV1,
        }
    }
}

impl FunctionFragmentReplayInputs {
    pub fn machine(
        &self,
    ) -> &register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan {
        match self {
            Self::UnitBaseline(realization) => realization.machine(),
            Self::StructuralUnit(realization) => realization.machine(),
            Self::FixedFrame(realization) => realization.machine(),
            Self::PostAllocationMachine(realization) => realization.machine(),
            Self::AllocationRecovery(realization) => realization.machine(),
            Self::X86Rel8Direct(realization) => realization.machine(),
            Self::SelectedLowering(realization) => realization.machine(),
        }
    }

    pub fn resolved_layout(
        &self,
    ) -> &selected_form_encoding_to_resolved_layout::StagedOptimizedResolvedSelectedFormLayout {
        match self {
            Self::UnitBaseline(realization) => realization.layout(),
            Self::StructuralUnit(realization) => realization.layout(),
            Self::FixedFrame(realization) => realization.layout(),
            Self::PostAllocationMachine(realization) => realization.layout(),
            Self::AllocationRecovery(realization) => realization.layout(),
            Self::X86Rel8Direct(realization) => realization.layout(),
            Self::SelectedLowering(realization) => realization.layout(),
        }
    }

    pub fn post_allocation_machine_optimization(
        &self,
    ) -> Option<&post_allocation_machine_to_post_allocation_machine::StagedOptimizedPostAllocationMachineOptimization>{
        match self {
            Self::PostAllocationMachine(realization) => Some(realization.optimization()),
            _ => None,
        }
    }

    /// Routes whose realization owns a validated target frame protocol. The
    /// Unit baseline route joins the fixed-frame route here because every
    /// AArch64 Unit function carries a saved return address.
    pub fn frame_protocol(&self) -> Option<&crate::ValidatedTargetFrameProtocolEncoding> {
        match self {
            Self::FixedFrame(realization) => Some(realization.protocol()),
            Self::UnitBaseline(realization) => realization.protocol(),
            _ => None,
        }
    }

    /// The target-owned geometry the frame protocol encodes. Present exactly
    /// where [`Self::frame_protocol`] is.
    pub fn frame_layout(
        &self,
    ) -> Option<&post_allocation_machine_to_frame_layout::ValidatedTargetFrameLayout> {
        match self {
            Self::FixedFrame(realization) => Some(realization.frame()),
            Self::UnitBaseline(realization) => realization.frame().map(|frame| frame.layout()),
            _ => None,
        }
    }

    pub fn register_homes(
        &self,
    ) -> &selected_instructions_to_register_homes::ValidatedRegisterHomes {
        self.allocation().current().homes()
    }

    pub fn register_environment(
        &self,
    ) -> &register_environment::ValidatedTargetRegisterEnvironment {
        self.allocation().current().register_environment()
    }

    pub const fn exit_contract(&self) -> &crate::ValidatedWholeFunctionExitContract {
        match self {
            Self::X86Rel8Direct(realization) => realization.exit_contract(),
            Self::SelectedLowering(realization) => realization.exit_contract(),
            Self::PostAllocationMachine(realization) => realization.exit_contract(),
            Self::AllocationRecovery(realization) => realization.exit_contract(),
            Self::UnitBaseline(realization) => realization.exit_contract(),
            Self::StructuralUnit(realization) => realization.exit_contract(),
            Self::FixedFrame(realization) => realization.exit_contract(),
        }
    }

    pub const fn function_relative_manifest(
        &self,
    ) -> &crate::ValidatedFunctionRelativeOptimizationRealizationManifest {
        match self {
            Self::X86Rel8Direct(realization) => realization.manifest(),
            Self::SelectedLowering(realization) => realization.manifest(),
            Self::PostAllocationMachine(realization) => realization.manifest(),
            Self::AllocationRecovery(realization) => realization.manifest(),
            Self::UnitBaseline(realization) => realization.manifest(),
            Self::StructuralUnit(realization) => realization.manifest(),
            Self::FixedFrame(realization) => realization.manifest(),
        }
    }

    pub fn post_allocation_manifest(
        &self,
    ) -> &selected_instructions_to_register_homes::ValidatedPostAllocationOptimizationManifest {
        self.allocation().current().post_allocation_manifest()
    }
}

impl FunctionFragmentReplayInputs {
    pub fn shared_selected_plan(
        &self,
    ) -> std::sync::Arc<selected_instructions::SelectedInstructionPlan> {
        self.allocation()
            .current()
            .selected()
            .shared_selected_plan()
    }

    pub fn target_input_owner(
        &self,
    ) -> &std::sync::Arc<abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations>
    {
        self.allocation().current().target_input_owner()
    }

    pub fn encoding(
        &self,
    ) -> &post_allocation_machine_to_selected_form_encoding::StagedOptimizedSelectedFormEncoding
    {
        match self {
            Self::X86Rel8Direct(realization) => realization.encoding(),
            Self::SelectedLowering(realization) => realization.encoding(),
            Self::PostAllocationMachine(realization) => realization.encoding(),
            Self::AllocationRecovery(realization) => realization.encoding(),
            Self::UnitBaseline(realization) => realization.encoding(),
            Self::StructuralUnit(realization) => realization.encoding(),
            Self::FixedFrame(realization) => realization.encoding(),
        }
    }
}
