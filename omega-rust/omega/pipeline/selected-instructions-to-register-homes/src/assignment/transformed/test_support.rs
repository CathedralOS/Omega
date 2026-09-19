use super::model::{
    StagedOptimizedRegisterHomesAfterLiteralFolds,
    StagedOptimizedRegisterHomesAfterSelectedLowering,
};

/// One substitutable field of [`StagedOptimizedPostLiteralFoldHomeCustodyReceipt`](super::StagedOptimizedPostLiteralFoldHomeCustodyReceipt). `Source` takes the
/// donor's authentic foreign literal-fold custody receipt; the remaining flat
/// fields take fixed alternates. Every field is representable in memory; the
/// receipt has no wire form, so no field is canonical-encoding-closed. The
/// independent checker is
/// `validate_optimized_register_home_after_literal_fold_custody`, and joined
/// `replay_allocation` surfaces its rejection as
/// `AllocationReplayError::LiteralFolds`.
#[derive(Debug, Clone, Copy)]
pub enum OptimizedPostLiteralFoldHomeCustodyFieldForTest {
    Source,
    Homes,
    PostAllocationManifest,
    FunctionCount,
    AssignmentCount,
}

impl StagedOptimizedRegisterHomesAfterLiteralFolds {
    /// Mutate only retained receipt facts; the donor's nested source receipt
    /// is authentic foreign evidence and grants no new authority here.
    pub fn corrupt_custody_for_test(
        &mut self,
        field: OptimizedPostLiteralFoldHomeCustodyFieldForTest,
        donor: &Self,
    ) {
        match field {
            OptimizedPostLiteralFoldHomeCustodyFieldForTest::Source => {
                self.custody.source = donor.custody.source.clone();
            }
            OptimizedPostLiteralFoldHomeCustodyFieldForTest::Homes => {
                self.custody.homes = crate::RegisterHomeIdentity::from_bytes([0xb2; 32]);
            }
            OptimizedPostLiteralFoldHomeCustodyFieldForTest::PostAllocationManifest => {
                self.custody.post_allocation_manifest =
                    optimization_core::PostAllocationOptimizationManifestIdentity::from_bytes(
                        [0xb3; 32],
                    );
            }
            OptimizedPostLiteralFoldHomeCustodyFieldForTest::FunctionCount => {
                self.custody.function_count += 1;
            }
            OptimizedPostLiteralFoldHomeCustodyFieldForTest::AssignmentCount => {
                self.custody.assignment_count += 1;
            }
        }
    }
}

/// One substitutable field of [`StagedOptimizedPostSelectedLoweringHomeCustodyReceipt`](super::StagedOptimizedPostSelectedLoweringHomeCustodyReceipt). `Source` takes the
/// donor's authentic foreign selected-lowering custody receipt; the remaining
/// flat fields take fixed alternates. Every field is representable in memory;
/// the receipt has no wire form, so no field is canonical-encoding-closed.
/// The independent checker is
/// `validate_optimized_register_home_after_selected_lowering_custody`, and
/// joined `replay_allocation` surfaces its rejection as
/// `AllocationReplayError::SelectedLowering`.
#[derive(Debug, Clone, Copy)]
pub enum OptimizedPostSelectedLoweringHomeCustodyFieldForTest {
    Source,
    Homes,
    PostAllocationManifest,
    FunctionCount,
    AssignmentCount,
}

impl StagedOptimizedRegisterHomesAfterSelectedLowering {
    /// Mutate only retained receipt facts; the donor's nested source receipt
    /// is authentic foreign evidence and grants no new authority here.
    pub fn corrupt_custody_for_test(
        &mut self,
        field: OptimizedPostSelectedLoweringHomeCustodyFieldForTest,
        donor: &Self,
    ) {
        match field {
            OptimizedPostSelectedLoweringHomeCustodyFieldForTest::Source => {
                self.custody.source = donor.custody.source.clone();
            }
            OptimizedPostSelectedLoweringHomeCustodyFieldForTest::Homes => {
                self.custody.homes = crate::RegisterHomeIdentity::from_bytes([0xb2; 32]);
            }
            OptimizedPostSelectedLoweringHomeCustodyFieldForTest::PostAllocationManifest => {
                self.custody.post_allocation_manifest =
                    optimization_core::PostAllocationOptimizationManifestIdentity::from_bytes(
                        [0xb3; 32],
                    );
            }
            OptimizedPostSelectedLoweringHomeCustodyFieldForTest::FunctionCount => {
                self.custody.function_count += 1;
            }
            OptimizedPostSelectedLoweringHomeCustodyFieldForTest::AssignmentCount => {
                self.custody.assignment_count += 1;
            }
        }
    }
}
