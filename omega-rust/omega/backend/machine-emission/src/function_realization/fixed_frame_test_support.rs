use super::carriers::StagedFixedFrameFunctionRelativeRealization;

pub fn replace_fixed_frame_realization_exit_for_test(
    staged: &mut StagedFixedFrameFunctionRelativeRealization,
    foreign: &StagedFixedFrameFunctionRelativeRealization,
) {
    staged.exit_contract = foreign.exit_contract.clone();
}

pub fn swap_fixed_frame_realization_source_for_test(
    staged: &mut StagedFixedFrameFunctionRelativeRealization,
    foreign: &mut StagedFixedFrameFunctionRelativeRealization,
) {
    std::mem::swap(&mut staged.allocation, &mut foreign.allocation);
}

pub fn corrupt_fixed_frame_realization_encoding_for_test(
    staged: &mut StagedFixedFrameFunctionRelativeRealization,
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
        .expect("fixed-frame fixture has encoded selected forms");
    let machine_code::SelectedFormEncodingState::Encoded { bytes, .. } = &mut row.state else {
        unreachable!()
    };
    bytes[0] ^= 1;
}

pub fn corrupt_fixed_frame_realization_layout_for_test(
    staged: &mut StagedFixedFrameFunctionRelativeRealization,
) {
    staged.layout.functions_mut()[0].blocks[0].instructions[0].bytes[0] ^= 1;
}

pub fn corrupt_fixed_frame_realization_exit_for_test(
    staged: &mut StagedFixedFrameFunctionRelativeRealization,
) {
    staged.exit_contract.contract_mut().result_view = register_model::RegisterViewId(u16::MAX);
}

pub fn corrupt_fixed_frame_realization_manifest_for_test(
    staged: &mut StagedFixedFrameFunctionRelativeRealization,
) {
    staged.manifest.record_mut().allocation_recovery_selections =
        optimization_core::OptimizationSelections::default().identity();
}

pub fn corrupt_fixed_frame_realization_custody_for_test(
    staged: &mut StagedFixedFrameFunctionRelativeRealization,
) {
    staged.custody.realization = optimization_core::FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(b"corrupt");
}
