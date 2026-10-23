//! Optimizer module role: executable entrance.
mod compute;
mod error;
#[cfg(test)]
mod order_suite;
mod ordinary;
mod validation;

pub use error::OptimizedResolvedSelectedFormLayoutError;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use machine_code::ResolvedMachineLayout;
use machine_code::SelectedFormEncodingIdentity;
pub use machine_code::{
    ResolvedBranchEvidence, ResolvedConditionalBranchEvidence, ResolvedConditionalBranchPredicate,
    ResolvedJumpEvidence, ResolvedSelectedBlockLayout, ResolvedSelectedFormLayoutIdentity,
    ResolvedSelectedFormRow, ResolvedSelectedFunctionLayout, SelectedFunctionLayoutPolicy,
};
use physical_instructions::PostAllocationMachineOptimizationCustody;
use post_allocation_machine_to_selected_form_encoding::StagedOptimizedSelectedFormEncoding;
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;
use std::sync::Arc;
use target::NativeTarget;

/// Resolve the current encoded program and independently admit its layout.
pub fn stage_optimized_resolved_selected_form_layout<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
) -> Result<StagedOptimizedResolvedSelectedFormLayout, OptimizedResolvedSelectedFormLayoutError> {
    let artifact = compute::compute(selected, machine, physical, pre_layout)?;
    admit_resolved_machine_layout(selected, machine, physical, pre_layout, artifact.program)
}

/// Replay the exact encoding roots, byte accounting, and all resolved rows.
pub fn validate_optimized_resolved_selected_form_layout<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    artifact: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    validation::validate(selected, machine, physical, pre_layout, artifact)
}

/// Independently admit retained current data, without a producer-stage history.
/// Content identity alone is insufficient: replay checks the selected program,
/// machine, target decoding, offsets, fixups, and exact optimization records.
pub(crate) fn admit_resolved_machine_layout<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    program: std::sync::Arc<machine_code::ResolvedMachineLayout>,
) -> Result<StagedOptimizedResolvedSelectedFormLayout, OptimizedResolvedSelectedFormLayoutError> {
    let artifact = StagedOptimizedResolvedSelectedFormLayout { program };
    validation::validate(selected, machine, physical, pre_layout, &artifact)?;
    Ok(artifact)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedResolvedSelectedFormLayout {
    program: Arc<ResolvedMachineLayout>,
}

impl StagedOptimizedResolvedSelectedFormLayout {
    pub fn selected(&self) -> selected_instructions::SelectedInstructionPlanIdentity {
        self.program.selected
    }

    pub fn machine(&self) -> physical_instructions::PostAllocationMachineIdentity {
        self.program.machine
    }

    pub fn pre_layout(&self) -> SelectedFormEncodingIdentity {
        self.program.pre_layout
    }

    pub fn post_allocation_machine_optimization(
        &self,
    ) -> Option<PostAllocationMachineOptimizationCustody> {
        self.program.post_allocation_machine_optimization
    }

    pub fn target(&self) -> NativeTarget {
        self.program.target
    }

    pub fn policy(&self) -> SelectedFunctionLayoutPolicy {
        self.program.policy
    }

    pub fn identity(&self) -> ResolvedSelectedFormLayoutIdentity {
        self.program.identity
    }

    pub fn functions(&self) -> &[ResolvedSelectedFunctionLayout] {
        &self.program.functions
    }

    pub fn program(&self) -> &ResolvedMachineLayout {
        &self.program
    }

    pub fn shared_program(&self) -> Arc<ResolvedMachineLayout> {
        Arc::clone(&self.program)
    }

    /// Reconstruct a candidate only within the owning layout stage. Public
    /// admission independently checks the raw program before returning it.
    fn from_program(program: ResolvedMachineLayout) -> Self {
        Self {
            program: Arc::new(program),
        }
    }
}

fn declared_size_matches(
    knowledge: selected_instructions::MachineSizeKnowledge,
    actual: u64,
) -> bool {
    match knowledge {
        selected_instructions::MachineSizeKnowledge::ExactBytes(expected) => {
            u64::from(expected) == actual
        }
        selected_instructions::MachineSizeKnowledge::EncoderResolved {
            minimum_bytes,
            maximum_bytes,
        } => {
            actual >= u64::from(minimum_bytes)
                && maximum_bytes.is_none_or(|maximum| actual <= u64::from(maximum))
        }
    }
}

fn is_admitted_canonical_shape(function: &selected_instructions::SelectedFunction) -> bool {
    !function.blocks.is_empty() && function.block(function.entry_block).is_some()
}

fn is_single_entry(function: &selected_instructions::SelectedFunction) -> bool {
    let Some(block) = function
        .blocks
        .first()
        .filter(|_| function.blocks.len() == 1)
    else {
        return false;
    };
    function.entry_block == block.id
        && matches!(
            block.terminator,
            selected_instructions::SelectedTerminator::Return { .. }
        )
}
