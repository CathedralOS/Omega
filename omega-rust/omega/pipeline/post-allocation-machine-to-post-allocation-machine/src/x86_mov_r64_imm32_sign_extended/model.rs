use crate::ValidatedX86MovR64Imm32SignExtendedMaterialization;
use physical_instructions::X86MovR64Imm32SignExtendedMaterializationCustodyReceipt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedX86MovR64Imm32SignExtendedMaterialization {
    pub(super) materialization: ValidatedX86MovR64Imm32SignExtendedMaterialization,
    pub(super) custody: X86MovR64Imm32SignExtendedMaterializationCustodyReceipt,
}

impl StagedOptimizedX86MovR64Imm32SignExtendedMaterialization {
    pub const fn materialization(&self) -> &ValidatedX86MovR64Imm32SignExtendedMaterialization {
        &self.materialization
    }

    pub const fn custody(&self) -> X86MovR64Imm32SignExtendedMaterializationCustodyReceipt {
        self.custody
    }
}
