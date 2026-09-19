use super::model::ValidatedPostAllocationMachinePlan;

/// One substitutable field of [`PostAllocationMachineReceipt`](super::PostAllocationMachineReceipt). The custody matrix substitutes
/// exactly one field per leg so a rejection attributes to that claim alone.
/// Every field is representable in memory; the receipt has no wire form, so
/// no field is canonical-encoding-closed. The independent checker is
/// `validate_optimized_post_allocation_machine_plan_custody`: it rebuilds the
/// receipt from the retained source stage and rejects the substitution.
#[derive(Debug, Clone, Copy)]
pub enum PostAllocationMachinePlanReceiptFieldForTest {
    Identity,
    Selected,
    Effects,
    Homes,
    PostAllocationManifest,
    RegisterEnvironment,
    FunctionCount,
    BlockCount,
    InstructionCount,
    OperandCount,
    UnitActionCount,
}

impl ValidatedPostAllocationMachinePlan {
    /// Mutate only retained receipt facts; no donor is needed because every
    /// field substitutes a fixed alternate that cannot equal any honest value.
    pub fn corrupt_receipt_for_test(
        &mut self,
        field: PostAllocationMachinePlanReceiptFieldForTest,
    ) {
        match field {
            PostAllocationMachinePlanReceiptFieldForTest::Identity => {
                self.receipt.identity =
                    physical_instructions::PostAllocationMachineIdentity::from_bytes([0xc0; 32]);
            }
            PostAllocationMachinePlanReceiptFieldForTest::Selected => {
                self.receipt.selected =
                    selected_instructions::SelectedInstructionPlanIdentity::from_bytes([0xa9; 32]);
            }
            PostAllocationMachinePlanReceiptFieldForTest::Effects => {
                self.receipt.effects =
                    selected_instructions::PreAllocationMachineEffectIdentity::from_bytes(
                        [0xc1; 32],
                    );
            }
            PostAllocationMachinePlanReceiptFieldForTest::Homes => {
                self.receipt.homes =
                    selected_instructions_to_register_homes::RegisterHomeIdentity::from_bytes(
                        [0xb2; 32],
                    );
            }
            PostAllocationMachinePlanReceiptFieldForTest::PostAllocationManifest => {
                self.receipt.post_allocation_manifest =
                    optimization_core::PostAllocationOptimizationManifestIdentity::from_bytes(
                        [0xb3; 32],
                    );
            }
            PostAllocationMachinePlanReceiptFieldForTest::RegisterEnvironment => {
                self.receipt.register_environment =
                    register_model::TargetRegisterEnvironmentIdentity::from_bytes([0xa6; 32]);
            }
            PostAllocationMachinePlanReceiptFieldForTest::FunctionCount => {
                self.receipt.function_count += 1;
            }
            PostAllocationMachinePlanReceiptFieldForTest::BlockCount => {
                self.receipt.block_count += 1;
            }
            PostAllocationMachinePlanReceiptFieldForTest::InstructionCount => {
                self.receipt.instruction_count += 1;
            }
            PostAllocationMachinePlanReceiptFieldForTest::OperandCount => {
                self.receipt.operand_count += 1;
            }
            PostAllocationMachinePlanReceiptFieldForTest::UnitActionCount => {
                self.receipt.unit_action_count += 1;
            }
        }
    }
}
