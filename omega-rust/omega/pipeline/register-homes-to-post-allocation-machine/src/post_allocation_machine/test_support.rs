use super::model::StagedOptimizedPostAllocationMachinePlan;
use crate::PostAllocationMachinePlanReceiptFieldForTest;

/// One substitutable field of [`StagedOptimizedPostAllocationMachineCustodyReceipt`](super::StagedOptimizedPostAllocationMachineCustodyReceipt). The custody matrix
/// substitutes exactly one field per leg so a rejection attributes to that
/// claim alone.
#[derive(Debug, Clone, Copy)]
pub enum PostAllocationMachineCustodyFieldForTest {
    Source,
    Effects,
    Machine,
    FunctionCount,
    InstructionCount,
    OperandCount,
    UnitActionCount,
}

impl StagedOptimizedPostAllocationMachinePlan {
    /// Mutate only retained receipt facts; the donor grants no new authority.
    pub fn corrupt_custody_for_test(
        &mut self,
        field: PostAllocationMachineCustodyFieldForTest,
        donor: &Self,
    ) {
        match field {
            PostAllocationMachineCustodyFieldForTest::Source => {
                self.custody.source = donor.custody.source.clone();
            }
            PostAllocationMachineCustodyFieldForTest::Effects => {
                self.custody.effects =
                    selected_instructions::PreAllocationMachineEffectIdentity::from_bytes(
                        [0xb1; 32],
                    );
            }
            PostAllocationMachineCustodyFieldForTest::Machine => {
                self.custody.machine =
                    physical_instructions::PostAllocationMachineIdentity::from_bytes([0xb2; 32]);
            }
            PostAllocationMachineCustodyFieldForTest::FunctionCount => {
                self.custody.function_count += 1;
            }
            PostAllocationMachineCustodyFieldForTest::InstructionCount => {
                self.custody.instruction_count += 1;
            }
            PostAllocationMachineCustodyFieldForTest::OperandCount => {
                self.custody.operand_count += 1;
            }
            PostAllocationMachineCustodyFieldForTest::UnitActionCount => {
                self.custody.unit_action_count += 1;
            }
        }
    }

    /// Mutate exactly one retained field of the sealed machine plan's
    /// [`PostAllocationMachineReceipt`](crate::PostAllocationMachineReceipt);
    /// the mutation grants no new authority.
    pub fn corrupt_machine_receipt_for_test(
        &mut self,
        field: PostAllocationMachinePlanReceiptFieldForTest,
    ) {
        self.machine.corrupt_receipt_for_test(field);
    }
}
