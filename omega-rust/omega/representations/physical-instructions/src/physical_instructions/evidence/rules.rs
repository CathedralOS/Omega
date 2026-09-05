//! Exact rule custody records carried independently of optimizer implementations.
//!
//! These immutable records describe the selected rule, its replay inputs, and
//! measured output. Constructing them grants no admission: the optimizer retains
//! the validated result and independently replays it before accepting custody.

use crate::{
    Aarch64CbnzFusionIdentity, Aarch64MovnMaterializationIdentity,
    Aarch64SameViewCopyElisionIdentity, X86MovR32Imm32MaterializationIdentity,
    X86MovR64Imm32SignExtendedMaterializationIdentity, X86XorZeroMaterializationIdentity,
};
use optimization_core::{Optimization, OptimizationSelectionIdentity};

// A fused branch reads a value owned by the original selected operand.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QualifiedPhysicalRead {
    pub source_instruction: selected_instructions::SelectedInstructionId,
    pub operand: u16,
    pub virtual_register: selected_instructions::VirtualRegisterId,
    pub class: register_model::RegisterClassId,
    pub view: register_model::RegisterViewId,
    pub units: Vec<register_model::RegisterUnitId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aarch64CbnzFusionCustodyReceipt {
    selections: OptimizationSelectionIdentity,
    post_allocation_machine_selections: OptimizationSelectionIdentity,
    source: crate::PostAllocationMachineIdentity,
    fusion: Aarch64CbnzFusionIdentity,
    action_count: usize,
}

impl Aarch64CbnzFusionCustodyReceipt {
    pub const fn from_parts(
        selections: OptimizationSelectionIdentity,
        post_allocation_machine_selections: OptimizationSelectionIdentity,
        source: crate::PostAllocationMachineIdentity,
        fusion: Aarch64CbnzFusionIdentity,
        action_count: usize,
    ) -> Self {
        Self {
            selections,
            post_allocation_machine_selections,
            source,
            fusion,
            action_count,
        }
    }

    pub const fn selections(self) -> OptimizationSelectionIdentity {
        self.selections
    }
    pub const fn post_allocation_machine_selections(self) -> OptimizationSelectionIdentity {
        self.post_allocation_machine_selections
    }
    pub const fn source(self) -> crate::PostAllocationMachineIdentity {
        self.source
    }
    pub const fn fusion(self) -> Aarch64CbnzFusionIdentity {
        self.fusion
    }
    pub const fn action_count(self) -> usize {
        self.action_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aarch64MovnMaterializationCustodyReceipt {
    selections: OptimizationSelectionIdentity,
    post_allocation_machine_selections: OptimizationSelectionIdentity,
    source: crate::PostAllocationMachineIdentity,
    materialization: Aarch64MovnMaterializationIdentity,
    action_count: usize,
    baseline_words: u64,
    selected_words: u64,
}

impl Aarch64MovnMaterializationCustodyReceipt {
    pub const fn from_parts(
        selections: OptimizationSelectionIdentity,
        post_allocation_machine_selections: OptimizationSelectionIdentity,
        source: crate::PostAllocationMachineIdentity,
        materialization: Aarch64MovnMaterializationIdentity,
        action_count: usize,
        baseline_words: u64,
        selected_words: u64,
    ) -> Self {
        Self {
            selections,
            post_allocation_machine_selections,
            source,
            materialization,
            action_count,
            baseline_words,
            selected_words,
        }
    }

    pub const fn selections(self) -> OptimizationSelectionIdentity {
        self.selections
    }
    pub const fn post_allocation_machine_selections(self) -> OptimizationSelectionIdentity {
        self.post_allocation_machine_selections
    }
    pub const fn source(self) -> crate::PostAllocationMachineIdentity {
        self.source
    }
    pub const fn materialization(self) -> Aarch64MovnMaterializationIdentity {
        self.materialization
    }
    pub const fn action_count(self) -> usize {
        self.action_count
    }
    pub const fn baseline_words(self) -> u64 {
        self.baseline_words
    }
    pub const fn selected_words(self) -> u64 {
        self.selected_words
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aarch64SameViewCopyElisionCustodyReceipt {
    optimization: Optimization,
    selections: OptimizationSelectionIdentity,
    post_allocation_machine_selections: OptimizationSelectionIdentity,
    source: crate::PostAllocationMachineIdentity,
    elision: Aarch64SameViewCopyElisionIdentity,
    action_count: usize,
}

impl Aarch64SameViewCopyElisionCustodyReceipt {
    #[allow(clippy::too_many_arguments)]
    pub const fn from_parts(
        optimization: Optimization,
        selections: OptimizationSelectionIdentity,
        post_allocation_machine_selections: OptimizationSelectionIdentity,
        source: crate::PostAllocationMachineIdentity,
        elision: Aarch64SameViewCopyElisionIdentity,
        action_count: usize,
    ) -> Self {
        Self {
            optimization,
            selections,
            post_allocation_machine_selections,
            source,
            elision,
            action_count,
        }
    }

    pub const fn optimization(self) -> Optimization {
        self.optimization
    }
    pub const fn selections(self) -> OptimizationSelectionIdentity {
        self.selections
    }
    pub const fn post_allocation_machine_selections(self) -> OptimizationSelectionIdentity {
        self.post_allocation_machine_selections
    }
    pub const fn source(self) -> crate::PostAllocationMachineIdentity {
        self.source
    }
    pub const fn elision(self) -> Aarch64SameViewCopyElisionIdentity {
        self.elision
    }
    pub const fn action_count(self) -> usize {
        self.action_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86XorZeroMaterializationCustodyReceipt {
    selections: OptimizationSelectionIdentity,
    post_allocation_machine_selections: OptimizationSelectionIdentity,
    source: crate::PostAllocationMachineIdentity,
    materialization: X86XorZeroMaterializationIdentity,
    action_count: usize,
    baseline_bytes: u64,
    selected_bytes: u64,
}

impl X86XorZeroMaterializationCustodyReceipt {
    pub const fn from_parts(
        selections: OptimizationSelectionIdentity,
        post_allocation_machine_selections: OptimizationSelectionIdentity,
        source: crate::PostAllocationMachineIdentity,
        materialization: X86XorZeroMaterializationIdentity,
        action_count: usize,
        baseline_bytes: u64,
        selected_bytes: u64,
    ) -> Self {
        Self {
            selections,
            post_allocation_machine_selections,
            source,
            materialization,
            action_count,
            baseline_bytes,
            selected_bytes,
        }
    }

    pub const fn selections(self) -> OptimizationSelectionIdentity {
        self.selections
    }
    pub const fn post_allocation_machine_selections(self) -> OptimizationSelectionIdentity {
        self.post_allocation_machine_selections
    }
    pub const fn source(self) -> crate::PostAllocationMachineIdentity {
        self.source
    }
    pub const fn materialization(self) -> X86XorZeroMaterializationIdentity {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86MovR32Imm32MaterializationCustodyReceipt {
    selections: OptimizationSelectionIdentity,
    post_allocation_machine_selections: OptimizationSelectionIdentity,
    source: crate::PostAllocationMachineIdentity,
    materialization: X86MovR32Imm32MaterializationIdentity,
    action_count: usize,
    baseline_bytes: u64,
    selected_bytes: u64,
}

impl X86MovR32Imm32MaterializationCustodyReceipt {
    pub const fn from_parts(
        selections: OptimizationSelectionIdentity,
        post_allocation_machine_selections: OptimizationSelectionIdentity,
        source: crate::PostAllocationMachineIdentity,
        materialization: X86MovR32Imm32MaterializationIdentity,
        action_count: usize,
        baseline_bytes: u64,
        selected_bytes: u64,
    ) -> Self {
        Self {
            selections,
            post_allocation_machine_selections,
            source,
            materialization,
            action_count,
            baseline_bytes,
            selected_bytes,
        }
    }

    pub const fn selections(self) -> OptimizationSelectionIdentity {
        self.selections
    }
    pub const fn post_allocation_machine_selections(self) -> OptimizationSelectionIdentity {
        self.post_allocation_machine_selections
    }
    pub const fn source(self) -> crate::PostAllocationMachineIdentity {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86MovR64Imm32SignExtendedMaterializationCustodyReceipt {
    selections: OptimizationSelectionIdentity,
    post_allocation_machine_selections: OptimizationSelectionIdentity,
    source: crate::PostAllocationMachineIdentity,
    materialization: X86MovR64Imm32SignExtendedMaterializationIdentity,
    action_count: usize,
    baseline_bytes: u64,
    selected_bytes: u64,
}

impl X86MovR64Imm32SignExtendedMaterializationCustodyReceipt {
    pub const fn from_parts(
        selections: OptimizationSelectionIdentity,
        post_allocation_machine_selections: OptimizationSelectionIdentity,
        source: crate::PostAllocationMachineIdentity,
        materialization: X86MovR64Imm32SignExtendedMaterializationIdentity,
        action_count: usize,
        baseline_bytes: u64,
        selected_bytes: u64,
    ) -> Self {
        Self {
            selections,
            post_allocation_machine_selections,
            source,
            materialization,
            action_count,
            baseline_bytes,
            selected_bytes,
        }
    }

    pub const fn selections(self) -> OptimizationSelectionIdentity {
        self.selections
    }
    pub const fn post_allocation_machine_selections(self) -> OptimizationSelectionIdentity {
        self.post_allocation_machine_selections
    }
    pub const fn source(self) -> crate::PostAllocationMachineIdentity {
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
