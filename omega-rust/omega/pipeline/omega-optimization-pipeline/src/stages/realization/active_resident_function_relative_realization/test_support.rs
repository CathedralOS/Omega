use omega_optimization_core::{
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationSelections,
};

use super::model::StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization;

#[cfg(test)]
pub(crate) fn corrupt_active_resident_function_relative_source_for_test(
    staged: &mut StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization,
) {
    crate::stages::layout::active_resident_resolved_selected_form_layout::corrupt_active_resident_resolved_layout_byte_for_test(
        &mut staged.source,
    );
}

#[cfg(test)]
pub(crate) fn corrupt_active_resident_function_relative_exit_for_test(
    staged: &mut StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization,
) {
    staged.exit_contract.contract_mut().result_view =
        omega_register_model::RegisterViewId(u16::MAX);
}

#[cfg(test)]
pub(crate) fn corrupt_active_resident_function_relative_manifest_for_test(
    staged: &mut StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization,
) {
    staged.manifest.record_mut().allocation_recovery_selections =
        OptimizationSelections::default().identity();
}

#[cfg(test)]
pub(crate) fn corrupt_active_resident_function_relative_receipt_for_test(
    staged: &mut StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization,
) {
    staged.custody.realization =
        FunctionRelativeOptimizationRealizationManifestIdentity::from_bytes([0x92; 32]);
}

#[cfg(test)]
pub(crate) fn replace_active_resident_function_relative_exit_for_test(
    staged: &mut StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization,
    foreign: &StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization,
) {
    staged.exit_contract = foreign.exit_contract.clone();
}
