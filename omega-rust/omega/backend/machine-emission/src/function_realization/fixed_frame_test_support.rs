use super::carriers::StagedFixedFrameFunctionRelativeRealization;

#[derive(Debug, Clone, Copy)]
pub enum FixedFramePublicationCustodyFieldForTest {
    Source,
    Machine,
    Frame,
    Protocol,
    ExitContract,
    Realization,
}

impl StagedFixedFrameFunctionRelativeRealization {
    /// Mutate only retained receipt facts; the donor grants no new authority.
    pub fn corrupt_publication_custody_for_test(
        &mut self,
        field: FixedFramePublicationCustodyFieldForTest,
        donor: &Self,
    ) {
        match field {
            FixedFramePublicationCustodyFieldForTest::Source => {
                self.custody.source = donor.custody.source.clone();
            }
            FixedFramePublicationCustodyFieldForTest::Machine => {
                self.custody.machine = donor.custody.machine;
            }
            FixedFramePublicationCustodyFieldForTest::Frame => {
                self.custody.frame =
                    machine_code::TargetFrameLayoutIdentity::from_bytes([0xa1; 32]);
            }
            FixedFramePublicationCustodyFieldForTest::Protocol => {
                self.custody.protocol =
                    machine_code::TargetFrameProtocolEncodingIdentity::from_bytes([0xa2; 32]);
            }
            FixedFramePublicationCustodyFieldForTest::ExitContract => {
                self.custody.exit_contract =
                    machine_code::WholeFunctionExitContractIdentity::from_bytes([0xa3; 32]);
            }
            FixedFramePublicationCustodyFieldForTest::Realization => {
                self.custody.realization = optimization_core::FunctionRelativeOptimizationRealizationManifestIdentity::from_bytes([0xa4; 32]);
            }
        }
    }
}

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

#[cfg(feature = "test-support")]
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

#[cfg(feature = "test-support")]
pub fn corrupt_fixed_frame_realization_layout_for_test(
    staged: &mut StagedFixedFrameFunctionRelativeRealization,
) {
    staged.baseline_layout.functions_mut()[0].blocks[0].instructions[0].bytes[0] ^= 1;
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
