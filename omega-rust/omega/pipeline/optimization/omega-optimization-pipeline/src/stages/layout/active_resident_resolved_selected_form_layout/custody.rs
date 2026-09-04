use crate::{
    StagedOptimizedActiveResidentRematerializationSelectedFormEncodingCustodyReceipt,
    StagedOptimizedResolvedSelectedFormLayout,
};

use super::StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayoutCustodyReceipt;

pub(super) fn project_active_resident_resolved_layout_custody(
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
