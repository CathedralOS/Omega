use crate::{
    OptimizedActiveResidentRematerializationSelectedFormEncodingError,
    OptimizedResolvedSelectedFormLayoutError, ResolvedSelectedFormLayoutIdentity,
    SelectedFormEncodingIdentity, SelectedFunctionLayoutPolicy,
    StagedOptimizedActiveResidentRematerializationSelectedFormEncoding,
    StagedOptimizedActiveResidentRematerializationSelectedFormEncodingCustodyReceipt,
    StagedOptimizedResolvedSelectedFormLayout,
};

/// Owning resolved-layout custody for the active-resident rematerialization
/// vertical. This retains the complete source-specific pre-layout carrier and
/// grants no relaxation, exit-contract, frame, emission, section, object, or
/// publication authority.
#[derive(Debug)]
pub struct StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout {
    pub(super) pre_layout: StagedOptimizedActiveResidentRematerializationSelectedFormEncoding,
    pub(super) layout: StagedOptimizedResolvedSelectedFormLayout,
    pub(super) custody:
        StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayoutCustodyReceipt,
}

impl StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout {
    pub const fn pre_layout(
        &self,
    ) -> &StagedOptimizedActiveResidentRematerializationSelectedFormEncoding {
        &self.pre_layout
    }

    pub const fn layout(&self) -> &StagedOptimizedResolvedSelectedFormLayout {
        &self.layout
    }

    pub const fn custody(
        &self,
    ) -> &StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayoutCustodyReceipt
    {
        &self.custody
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayoutCustodyReceipt {
    pub(super) pre_layout_custody:
        StagedOptimizedActiveResidentRematerializationSelectedFormEncodingCustodyReceipt,
    pub(super) selected: omega_selected_instructions::SelectedInstructionPlanIdentity,
    pub(super) machine: omega_machine_optimizer::PostAllocationMachineIdentity,
    pub(super) pre_layout: SelectedFormEncodingIdentity,
    pub(super) physical: omega_register_model::PhysicalRegisterModelIdentity,
    pub(super) layout: ResolvedSelectedFormLayoutIdentity,
    pub(super) target: omega_target::NativeTarget,
    pub(super) policy: SelectedFunctionLayoutPolicy,
    pub(super) function_count: usize,
    pub(super) block_count: usize,
    pub(super) instruction_count: usize,
    pub(super) byte_count: u64,
    pub(super) resolved_branch_count: usize,
}

impl StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayoutCustodyReceipt {
    pub const fn pre_layout_custody(
        &self,
    ) -> &StagedOptimizedActiveResidentRematerializationSelectedFormEncodingCustodyReceipt {
        &self.pre_layout_custody
    }

    pub const fn selected(&self) -> omega_selected_instructions::SelectedInstructionPlanIdentity {
        self.selected
    }

    pub const fn machine(&self) -> omega_machine_optimizer::PostAllocationMachineIdentity {
        self.machine
    }

    pub const fn pre_layout(&self) -> SelectedFormEncodingIdentity {
        self.pre_layout
    }

    pub const fn physical(&self) -> omega_register_model::PhysicalRegisterModelIdentity {
        self.physical
    }

    pub const fn layout(&self) -> ResolvedSelectedFormLayoutIdentity {
        self.layout
    }

    pub const fn target(&self) -> omega_target::NativeTarget {
        self.target
    }

    pub const fn policy(&self) -> SelectedFunctionLayoutPolicy {
        self.policy
    }

    pub const fn function_count(&self) -> usize {
        self.function_count
    }

    pub const fn block_count(&self) -> usize {
        self.block_count
    }

    pub const fn instruction_count(&self) -> usize {
        self.instruction_count
    }

    pub const fn byte_count(&self) -> u64 {
        self.byte_count
    }

    pub const fn resolved_branch_count(&self) -> usize {
        self.resolved_branch_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError {
    PreLayout(OptimizedActiveResidentRematerializationSelectedFormEncodingError),
    Layout(OptimizedResolvedSelectedFormLayoutError),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized active-resident rematerialization resolved selected-form layout failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError {}
