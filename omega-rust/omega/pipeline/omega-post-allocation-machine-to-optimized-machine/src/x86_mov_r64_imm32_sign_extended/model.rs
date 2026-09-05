use crate::{
    ValidatedX86MovR64Imm32SignExtendedMaterialization,
    X86MovR64Imm32SignExtendedMaterializationIdentity,
};
use omega_optimization_core::OptimizationSelectionIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedX86MovR64Imm32SignExtendedMaterialization {
    pub(super) materialization: ValidatedX86MovR64Imm32SignExtendedMaterialization,
    pub(super) custody: StagedOptimizedX86MovR64Imm32SignExtendedMaterializationCustodyReceipt,
}

impl StagedOptimizedX86MovR64Imm32SignExtendedMaterialization {
    pub const fn materialization(&self) -> &ValidatedX86MovR64Imm32SignExtendedMaterialization {
        &self.materialization
    }

    pub const fn custody(
        &self,
    ) -> StagedOptimizedX86MovR64Imm32SignExtendedMaterializationCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedX86MovR64Imm32SignExtendedMaterializationCustodyReceipt {
    pub(super) selections: OptimizationSelectionIdentity,
    pub(super) post_allocation_machine_selections: OptimizationSelectionIdentity,
    pub(super) source: omega_physical_instructions::PostAllocationMachineIdentity,
    pub(super) materialization: X86MovR64Imm32SignExtendedMaterializationIdentity,
    pub(super) action_count: usize,
    pub(super) baseline_bytes: u64,
    pub(super) selected_bytes: u64,
}

impl StagedOptimizedX86MovR64Imm32SignExtendedMaterializationCustodyReceipt {
    pub const fn selections(self) -> OptimizationSelectionIdentity {
        self.selections
    }
    pub const fn post_allocation_machine_selections(self) -> OptimizationSelectionIdentity {
        self.post_allocation_machine_selections
    }
    pub const fn source(self) -> omega_physical_instructions::PostAllocationMachineIdentity {
        self.source
    }
    pub const fn materialization(self) -> X86MovR64Imm32SignExtendedMaterializationIdentity {
        self.materialization
    }
    pub const fn action_count(self) -> usize {
        self.action_count
    }
    pub const fn baseline_bytes(self) -> u64 {
        self.baseline_bytes
    }
    pub const fn selected_bytes(self) -> u64 {
        self.selected_bytes
    }
}
