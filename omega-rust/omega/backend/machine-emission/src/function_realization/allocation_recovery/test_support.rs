use super::model::StagedAllocationRecoveryFunctionRelativeRealization;

pub fn replace_allocation_recovery_realization_exit_for_test(
    staged: &mut StagedAllocationRecoveryFunctionRelativeRealization,
    foreign: &StagedAllocationRecoveryFunctionRelativeRealization,
) {
    staged.exit_contract = foreign.exit_contract.clone();
}

pub fn swap_allocation_recovery_realization_source_for_test(
    staged: &mut StagedAllocationRecoveryFunctionRelativeRealization,
    foreign: &mut StagedAllocationRecoveryFunctionRelativeRealization,
) {
    std::mem::swap(&mut staged.allocation, &mut foreign.allocation);
}

pub fn corrupt_allocation_recovery_realization_encoding_for_test(
    staged: &mut StagedAllocationRecoveryFunctionRelativeRealization,
) {
    let row = staged
        .encoding
        .rows_mut()
        .iter_mut()
        .find(|row| {
            matches!(
                row.state,
                machine_code::SelectedFormEncodingState::Encoded { .. }
            )
        })
        .expect("recovery fixture has encoded selected forms");
    let machine_code::SelectedFormEncodingState::Encoded { bytes, .. } = &mut row.state else {
        unreachable!()
    };
    bytes[0] ^= 1;
}

pub fn corrupt_allocation_recovery_realization_layout_for_test(
    staged: &mut StagedAllocationRecoveryFunctionRelativeRealization,
) {
    staged.layout.functions_mut()[0].blocks[0].instructions[0].bytes[0] ^= 1;
}

pub fn corrupt_allocation_recovery_realization_exit_for_test(
    staged: &mut StagedAllocationRecoveryFunctionRelativeRealization,
) {
    staged.exit_contract.contract_mut().result_view = register_model::RegisterViewId(u16::MAX);
}

pub fn corrupt_allocation_recovery_realization_manifest_for_test(
    staged: &mut StagedAllocationRecoveryFunctionRelativeRealization,
) {
    staged.manifest.record_mut().allocation_recovery_selections =
        optimization_core::OptimizationSelections::default().identity();
}

pub fn corrupt_allocation_recovery_realization_custody_for_test(
    staged: &mut StagedAllocationRecoveryFunctionRelativeRealization,
) {
    staged.custody.realization = optimization_core::FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(b"corrupt");
}
