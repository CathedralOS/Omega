//! Function-relative realization: the first machine-emission stage.
//!
//! [`stage_fixed_frame_function_relative_realization`] (`fixed_frame`)
//! replays the retained allocation against the post-allocation machine plan,
//! stages the function frame (`frame`: callee-save requirements, their
//! storage, the frame layout and its byte protocol), encodes and lays out the
//! selected forms, runs the phase-local layout optimization, stages the
//! whole-function exit contract, and seals the manifest defined here with its
//! custody receipt (`assembly`, `carriers`). Its validator replays each of
//! those joins against the same immutable inputs. `codec` encodes, decodes
//! and renders the manifest.

mod assembly;
mod carriers;
mod codec;
mod error;
mod fixed_frame;
#[cfg(any(test, feature = "test-support"))]
mod fixed_frame_test_support;
mod frame;

pub use carriers::{
    StagedFixedFrameFunctionRelativeRealization,
    StagedFixedFrameFunctionRelativeRealizationCustodyReceipt,
};
pub use codec::FunctionRelativeOptimizationRealizationManifestDecodeError;
pub use error::FunctionRelativeOptimizationRealizationError;
pub use fixed_frame::{
    stage_fixed_frame_function_relative_realization,
    validate_fixed_frame_function_relative_realization,
};
#[cfg(any(test, feature = "test-support"))]
pub use fixed_frame_test_support::{
    FixedFramePublicationCustodyFieldForTest, corrupt_fixed_frame_realization_custody_for_test,
    corrupt_fixed_frame_realization_exit_for_test,
    corrupt_fixed_frame_realization_manifest_for_test,
    replace_fixed_frame_realization_exit_for_test, swap_fixed_frame_realization_source_for_test,
};
#[cfg(feature = "test-support")]
pub use fixed_frame_test_support::{
    corrupt_fixed_frame_realization_encoding_for_test,
    corrupt_fixed_frame_realization_layout_for_test,
};
pub use frame::FunctionRelativeFrame;

use machine_code::{
    ResolvedSelectedFormLayoutIdentity, SelectedFormEncodingIdentity, SelectedFunctionLayoutPolicy,
    TargetFrameLayoutIdentity, TargetFrameProtocolEncodingIdentity,
    WholeFunctionExitContractIdentity, X86BranchRelaxationIdentity,
};
use optimization_core::{
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationSelectionIdentity,
    PostAllocationOptimizationManifestIdentity, PrePhysicalOptimizationManifestIdentity,
    SelectedLoweringOptimizationCompletionIdentity,
};
use selected_instructions::SelectedInstructionPlanIdentity;
use target::NativeTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionRelativeOptimizationRealizationStage {
    ValidatedFunctionRelativeSelectedFormsAndWholeFunctionExitV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionRelativeOptimizationRealizationScope {
    FunctionRelativeFragmentsWithValidatedWholeFunctionExitV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionRelativeOptimizationUnavailableData {
    Unavailable,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FunctionRelativeOptimizationRealizationStatistics {
    pub functions: u64,
    pub blocks: u64,
    pub instructions: u64,
    pub bytes: u64,
    pub resolved_conditional_branches: u64,
    pub unresolved_internal_machine_fixups: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionRelativeFrameDisposition {
    Unavailable,
    CanonicalFixedFrameV1 {
        layout: TargetFrameLayoutIdentity,
        protocol: TargetFrameProtocolEncodingIdentity,
    },
}

/// Structured report at the function-relative selected-form boundary after
/// validating the admitted whole-function exit discipline. A role-tagged
/// frame disposition retains fixed-frame planning when that route was
/// selected; this boundary owns no emitted frame bytes, section, symbol,
/// relocation, executable image, installation, or publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionRelativeOptimizationRealizationManifest {
    pub identity: FunctionRelativeOptimizationRealizationManifestIdentity,
    pub stage: FunctionRelativeOptimizationRealizationStage,
    pub selections: OptimizationSelectionIdentity,
    pub selected_lowering_selections: OptimizationSelectionIdentity,
    pub selected_lowering_completion: Option<SelectedLoweringOptimizationCompletionIdentity>,
    pub allocation_recovery_selections: OptimizationSelectionIdentity,
    pub post_allocation_machine_selections: OptimizationSelectionIdentity,
    pub function_relative_layout_selections: OptimizationSelectionIdentity,
    pub pre_physical_manifest: PrePhysicalOptimizationManifestIdentity,
    pub post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub pre_allocation_machine_effects: selected_instructions::PreAllocationMachineEffectIdentity,
    pub post_allocation_machine: physical_instructions::PostAllocationMachineIdentity,
    pub baseline_pre_layout: SelectedFormEncodingIdentity,
    pub pre_layout: SelectedFormEncodingIdentity,
    pub baseline_resolved_layout: ResolvedSelectedFormLayoutIdentity,
    pub resolved_layout: ResolvedSelectedFormLayoutIdentity,
    pub x86_branch_relaxation: Option<X86BranchRelaxationIdentity>,
    pub post_allocation_machine_optimization:
        Option<physical_instructions::PostAllocationMachineOptimizationCustody>,
    pub whole_function_exit_contract: WholeFunctionExitContractIdentity,
    pub target: NativeTarget,
    pub layout_policy: SelectedFunctionLayoutPolicy,
    pub scope: FunctionRelativeOptimizationRealizationScope,
    pub statistics: FunctionRelativeOptimizationRealizationStatistics,
    pub frame: FunctionRelativeFrameDisposition,
    pub machine_emission: FunctionRelativeOptimizationUnavailableData,
    pub section_placement: FunctionRelativeOptimizationUnavailableData,
    pub symbols: FunctionRelativeOptimizationUnavailableData,
    pub object_relocations: FunctionRelativeOptimizationUnavailableData,
    pub executable_image: FunctionRelativeOptimizationUnavailableData,
    pub installation: FunctionRelativeOptimizationUnavailableData,
    pub publication: FunctionRelativeOptimizationUnavailableData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFunctionRelativeOptimizationRealizationManifest {
    record: FunctionRelativeOptimizationRealizationManifest,
}

impl ValidatedFunctionRelativeOptimizationRealizationManifest {
    pub const fn record(&self) -> &FunctionRelativeOptimizationRealizationManifest {
        &self.record
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn record_mut(&mut self) -> &mut FunctionRelativeOptimizationRealizationManifest {
        &mut self.record
    }
}
