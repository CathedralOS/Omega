//! Function frames and the evidence join for produced function-relative code.
//!
//! [`stage_function_frame`] constructs callee-save requirements, storage, frame
//! geometry and its byte protocol. Native compilation then runs encoding and
//! layout stages. [`assemble_fixed_frame_realization`] consumes their results,
//! constructs the whole-function exit contract, and binds the manifest and
//! custody receipt. The retained-reader validator replays the joins against
//! the same immutable inputs. `codec` encodes and decodes the manifest.

mod assembly;
mod carriers;
mod codec;
mod error;
mod fixed_frame;
mod frame;

pub use carriers::StagedFixedFrameFunctionRelativeRealization;
pub use error::FunctionRelativeOptimizationRealizationError;
pub use fixed_frame::assemble_fixed_frame_realization;
pub(crate) use fixed_frame::validate_fixed_frame_function_relative_realization;
pub use frame::{FunctionRelativeFrame, stage_function_frame};

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
}
