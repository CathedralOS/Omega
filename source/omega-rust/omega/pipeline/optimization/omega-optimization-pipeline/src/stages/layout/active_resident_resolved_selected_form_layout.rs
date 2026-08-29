use crate::{
    OptimizedActiveResidentRematerializationSelectedFormEncodingError,
    OptimizedResolvedSelectedFormLayoutError, ResolvedSelectedFormLayoutIdentity,
    SelectedFormEncodingIdentity, SelectedFunctionLayoutPolicy,
    StagedOptimizedActiveResidentRematerializationSelectedFormEncoding,
    StagedOptimizedActiveResidentRematerializationSelectedFormEncodingCustodyReceipt,
    StagedOptimizedResolvedSelectedFormLayout, stage_optimized_resolved_selected_form_layout,
    validate_optimized_active_resident_rematerialization_selected_form_encoding,
    validate_optimized_resolved_selected_form_layout,
};

/// Owning resolved-layout custody for the active-resident rematerialization
/// vertical. This retains the complete source-specific pre-layout carrier and
/// grants no relaxation, exit-contract, frame, emission, section, object, or
/// publication authority.
#[derive(Debug)]
pub struct StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout {
    pre_layout: StagedOptimizedActiveResidentRematerializationSelectedFormEncoding,
    layout: StagedOptimizedResolvedSelectedFormLayout,
    custody: StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayoutCustodyReceipt,
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
    pre_layout_custody:
        StagedOptimizedActiveResidentRematerializationSelectedFormEncodingCustodyReceipt,
    selected: omega_selected_instructions::SelectedInstructionPlanIdentity,
    machine: omega_machine_optimizer::PostAllocationMachineIdentity,
    pre_layout: SelectedFormEncodingIdentity,
    physical: omega_register_model::PhysicalRegisterModelIdentity,
    layout: ResolvedSelectedFormLayoutIdentity,
    target: omega_target::NativeTarget,
    policy: SelectedFunctionLayoutPolicy,
    function_count: usize,
    block_count: usize,
    instruction_count: usize,
    byte_count: u64,
    resolved_branch_count: usize,
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

pub fn stage_optimized_active_resident_rematerialization_resolved_selected_form_layout(
    pre_layout: StagedOptimizedActiveResidentRematerializationSelectedFormEncoding,
) -> Result<
    StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout,
    OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError,
> {
    let pre_layout_custody =
        validate_optimized_active_resident_rematerialization_selected_form_encoding(&pre_layout)
            .map_err(
                OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError::PreLayout,
            )?;
    let selected = pre_layout.source().rematerialization();
    let machine = pre_layout.machine();
    let physical = pre_layout
        .source()
        .source()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment()
        .physical();
    let layout = stage_optimized_resolved_selected_form_layout(
        selected,
        machine,
        physical,
        pre_layout.encoding(),
    )
    .map_err(OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError::Layout)?;
    if layout.policy() != SelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1 {
        return Err(
            OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError::ReceiptMismatch,
        );
    }
    let custody = custody_receipt(pre_layout_custody, physical.identity(), &layout);
    let staged = StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout {
        pre_layout,
        layout,
        custody,
    };
    validate_optimized_active_resident_rematerialization_resolved_selected_form_layout(&staged)?;
    Ok(staged)
}

pub fn validate_optimized_active_resident_rematerialization_resolved_selected_form_layout(
    staged: &StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout,
) -> Result<
    StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayoutCustodyReceipt,
    OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError,
> {
    let pre_layout_custody =
        validate_optimized_active_resident_rematerialization_selected_form_encoding(
            &staged.pre_layout,
        )
        .map_err(
            OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError::PreLayout,
        )?;
    let selected = staged.pre_layout.source().rematerialization();
    let machine = staged.pre_layout.machine();
    let physical = staged
        .pre_layout
        .source()
        .source()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment()
        .physical();
    validate_optimized_resolved_selected_form_layout(
        selected,
        machine,
        physical,
        staged.pre_layout.encoding(),
        &staged.layout,
    )
    .map_err(OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError::Layout)?;
    if staged.layout.policy() != SelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1
    {
        return Err(
            OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError::ReceiptMismatch,
        );
    }
    let custody = custody_receipt(pre_layout_custody, physical.identity(), &staged.layout);
    if custody != staged.custody {
        return Err(
            OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError::ReceiptMismatch,
        );
    }
    Ok(custody)
}

fn custody_receipt(
    pre_layout: StagedOptimizedActiveResidentRematerializationSelectedFormEncodingCustodyReceipt,
    physical: omega_register_model::PhysicalRegisterModelIdentity,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
) -> StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayoutCustodyReceipt {
    let function_count = layout.functions().len();
    let block_count = layout
        .functions()
        .iter()
        .map(|function| function.blocks.len())
        .sum();
    let instruction_count = layout
        .functions()
        .iter()
        .flat_map(|function| &function.blocks)
        .map(|block| block.instructions.len())
        .sum();
    let byte_count = layout
        .functions()
        .iter()
        .map(|function| function.byte_count)
        .sum();
    let resolved_branch_count = layout
        .functions()
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instructions)
        .filter(|instruction| instruction.branch.is_some())
        .count();
    StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayoutCustodyReceipt {
        pre_layout_custody: pre_layout,
        selected: layout.selected(),
        machine: layout.machine(),
        pre_layout: layout.pre_layout(),
        physical,
        layout: layout.identity(),
        target: layout.target(),
        policy: layout.policy(),
        function_count,
        block_count,
        instruction_count,
        byte_count,
        resolved_branch_count,
    }
}

#[cfg(test)]
pub(crate) fn corrupt_active_resident_resolved_layout_pre_layout_byte_for_test(
    staged: &mut StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout,
) {
    crate::stages::encoding::active_resident_selected_form_encoding::corrupt_active_resident_selected_form_encoding_byte_for_test(
        &mut staged.pre_layout,
    );
}

#[cfg(test)]
pub(crate) fn corrupt_active_resident_resolved_layout_byte_for_test(
    staged: &mut StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout,
) {
    let byte = staged
        .layout
        .functions_mut()
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| instruction.bytes.first_mut())
        .expect("active-resident resolved-layout fixture must contain encoded bytes");
    *byte ^= 1;
}

#[cfg(test)]
pub(crate) fn corrupt_active_resident_resolved_layout_receipt_for_test(
    staged: &mut StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout,
) {
    staged.custody.resolved_branch_count ^= 1;
}
