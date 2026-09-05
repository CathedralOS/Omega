use crate::{ValidatedX86MovR32Imm32Materialization, X86MovR32Imm32MaterializationIdentity};
use optimization_core::OptimizationSelectionIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedX86MovR32Imm32Materialization {
    pub(super) materialization: ValidatedX86MovR32Imm32Materialization,
    pub(super) custody: StagedOptimizedX86MovR32Imm32MaterializationCustodyReceipt,
}

impl StagedOptimizedX86MovR32Imm32Materialization {
    pub const fn materialization(&self) -> &ValidatedX86MovR32Imm32Materialization {
        &self.materialization
    }

    pub const fn custody(&self) -> StagedOptimizedX86MovR32Imm32MaterializationCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedX86MovR32Imm32MaterializationCustodyReceipt {
    pub(super) selections: OptimizationSelectionIdentity,
    pub(super) post_allocation_machine_selections: OptimizationSelectionIdentity,
    pub(super) source: physical_instructions::PostAllocationMachineIdentity,
    pub(super) materialization: X86MovR32Imm32MaterializationIdentity,
    pub(super) action_count: usize,
    pub(super) baseline_bytes: u64,
    pub(super) selected_bytes: u64,
}

impl StagedOptimizedX86MovR32Imm32MaterializationCustodyReceipt {
    pub const fn selections(self) -> OptimizationSelectionIdentity {
        self.selections
    }
    pub const fn post_allocation_machine_selections(self) -> OptimizationSelectionIdentity {
        self.post_allocation_machine_selections
    }
    pub const fn source(self) -> physical_instructions::PostAllocationMachineIdentity {
        self.source
    }
    pub const fn materialization(self) -> X86MovR32Imm32MaterializationIdentity {
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
