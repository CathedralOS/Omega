use super::model::StagedAllocationRecoveryFunctionRelativeRealization;

pub(crate) fn replace_allocation_recovery_realization_exit_for_test(
    staged: &mut StagedAllocationRecoveryFunctionRelativeRealization,
    foreign: &StagedAllocationRecoveryFunctionRelativeRealization,
) {
    staged.exit_contract = foreign.exit_contract.clone();
}

pub(crate) fn swap_allocation_recovery_realization_source_for_test(
    staged: &mut StagedAllocationRecoveryFunctionRelativeRealization,
    foreign: &mut StagedAllocationRecoveryFunctionRelativeRealization,
) {
    std::mem::swap(&mut staged.allocation, &mut foreign.allocation);
}

pub(crate) fn corrupt_allocation_recovery_realization_encoding_for_test(
    staged: &mut StagedAllocationRecoveryFunctionRelativeRealization,
) {
    let row = staged
        .encoding
        .rows_mut()
        .iter_mut()
        .find(|row| matches!(row.state, crate::SelectedFormEncodingState::Encoded { .. }))
        .expect("recovery fixture has encoded selected forms");
    let crate::SelectedFormEncodingState::Encoded { bytes, .. } = &mut row.state else {
        unreachable!()
    };
    bytes[0] ^= 1;
}

pub(crate) fn corrupt_allocation_recovery_realization_layout_for_test(
    staged: &mut StagedAllocationRecoveryFunctionRelativeRealization,
) {
    staged.layout.functions_mut()[0].blocks[0].instructions[0].bytes[0] ^= 1;
}

pub(crate) fn corrupt_allocation_recovery_realization_exit_for_test(
    staged: &mut StagedAllocationRecoveryFunctionRelativeRealization,
) {
    staged.exit_contract.contract_mut().result_view =
        omega_register_model::RegisterViewId(u16::MAX);
}

pub(crate) fn corrupt_allocation_recovery_realization_manifest_for_test(
    staged: &mut StagedAllocationRecoveryFunctionRelativeRealization,
) {
    staged.manifest.record_mut().allocation_recovery_selections =
        omega_optimization_core::OptimizationSelections::default().identity();
}

pub(crate) fn corrupt_allocation_recovery_realization_custody_for_test(
    staged: &mut StagedAllocationRecoveryFunctionRelativeRealization,
) {
    staged.custody.realization = omega_optimization_core::FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(b"corrupt");
}
