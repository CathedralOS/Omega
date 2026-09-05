use crate::ValidatedX86MovR32Imm32Materialization;
use physical_instructions::X86MovR32Imm32MaterializationCustodyReceipt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedX86MovR32Imm32Materialization {
    pub(super) materialization: ValidatedX86MovR32Imm32Materialization,
    pub(super) custody: X86MovR32Imm32MaterializationCustodyReceipt,
}

impl StagedOptimizedX86MovR32Imm32Materialization {
    pub const fn materialization(&self) -> &ValidatedX86MovR32Imm32Materialization {
        &self.materialization
    }

    pub const fn custody(&self) -> X86MovR32Imm32MaterializationCustodyReceipt {
        self.custody
    }
}
