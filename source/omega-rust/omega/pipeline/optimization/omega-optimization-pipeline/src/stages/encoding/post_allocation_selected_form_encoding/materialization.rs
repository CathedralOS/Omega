//! Closed row-level view over post-allocation materialization rules.
//!
//! The encoding join sees exactly one catalog-selected machine rule. This
//! adapter prevents that invariant from degrading into one optional tuple
//! element per materialization family.

use omega_machine_optimizer::{
    Aarch64MovnInstructionDisposition, X86MovR32Imm32InstructionDisposition,
    X86MovR64Imm32SignExtendedInstructionDisposition, X86XorZeroInstructionDisposition,
};
use omega_selected_instructions::{SelectedBlockId, SelectedInstructionId};
use psi_core::MachineId;

use crate::StagedOptimizedPostAllocationMachineOptimization;

use super::OptimizedSelectedFormEncodingError;

#[derive(Debug, Clone, Copy)]
pub(super) enum MaterializationDisposition<'a> {
    Aarch64Movn(&'a Aarch64MovnInstructionDisposition),
    X86MovR32Imm32(&'a X86MovR32Imm32InstructionDisposition),
    X86MovR64Imm32SignExtended(&'a X86MovR64Imm32SignExtendedInstructionDisposition),
    X86XorZero(&'a X86XorZeroInstructionDisposition),
}

impl MaterializationDisposition<'_> {
    pub(super) fn is_retained(self) -> bool {
        match self {
            Self::Aarch64Movn(disposition) => {
                matches!(disposition, Aarch64MovnInstructionDisposition::RetainedV1)
            }
            Self::X86MovR32Imm32(disposition) => matches!(
                disposition,
                X86MovR32Imm32InstructionDisposition::RetainedV1
            ),
            Self::X86MovR64Imm32SignExtended(disposition) => matches!(
                disposition,
                X86MovR64Imm32SignExtendedInstructionDisposition::RetainedV1
            ),
            Self::X86XorZero(disposition) => {
                matches!(disposition, X86XorZeroInstructionDisposition::RetainedV1)
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum MaterializationPlan<'a> {
    Aarch64Movn(&'a omega_machine_optimizer::Aarch64MovnMaterializationPlan),
    X86MovR32Imm32(&'a omega_machine_optimizer::X86MovR32Imm32MaterializationPlan),
    X86MovR64Imm32SignExtended(
        &'a omega_machine_optimizer::X86MovR64Imm32SignExtendedMaterializationPlan,
    ),
    X86XorZero(&'a omega_machine_optimizer::X86XorZeroMaterializationPlan),
}

impl<'a> MaterializationPlan<'a> {
    pub(super) fn from_optimization(
        optimization: Option<&'a StagedOptimizedPostAllocationMachineOptimization>,
    ) -> Option<Self> {
        match optimization? {
            StagedOptimizedPostAllocationMachineOptimization::Aarch64Movn(materialization) => {
                Some(Self::Aarch64Movn(materialization.materialization().plan()))
            }
            StagedOptimizedPostAllocationMachineOptimization::X86MovR32Imm32(materialization) => {
                Some(Self::X86MovR32Imm32(
                    materialization.materialization().plan(),
                ))
            }
            StagedOptimizedPostAllocationMachineOptimization::X86MovR64Imm32SignExtended(
                materialization,
            ) => Some(Self::X86MovR64Imm32SignExtended(
                materialization.materialization().plan(),
            )),
            StagedOptimizedPostAllocationMachineOptimization::X86XorZero(materialization) => {
                Some(Self::X86XorZero(materialization.materialization().plan()))
            }
            StagedOptimizedPostAllocationMachineOptimization::Aarch64Cbnz(_) => None,
        }
    }

    pub(super) fn function_count(self) -> usize {
        match self {
            Self::Aarch64Movn(plan) => plan.functions.len(),
            Self::X86MovR32Imm32(plan) => plan.functions.len(),
            Self::X86MovR64Imm32SignExtended(plan) => plan.functions.len(),
            Self::X86XorZero(plan) => plan.functions.len(),
        }
    }

    pub(super) fn function(
        self,
        index: usize,
    ) -> Result<MaterializationFunction<'a>, OptimizedSelectedFormEncodingError> {
        match self {
            Self::Aarch64Movn(plan) => plan
                .functions
                .get(index)
                .map(MaterializationFunction::Aarch64Movn),
            Self::X86MovR32Imm32(plan) => plan
                .functions
                .get(index)
                .map(MaterializationFunction::X86MovR32Imm32),
            Self::X86MovR64Imm32SignExtended(plan) => plan
                .functions
                .get(index)
                .map(MaterializationFunction::X86MovR64Imm32SignExtended),
            Self::X86XorZero(plan) => plan
                .functions
                .get(index)
                .map(MaterializationFunction::X86XorZero),
        }
        .ok_or(OptimizedSelectedFormEncodingError::FunctionRosterMismatch)
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum MaterializationFunction<'a> {
    Aarch64Movn(&'a omega_machine_optimizer::Aarch64MovnMaterializationFunction),
    X86MovR32Imm32(&'a omega_machine_optimizer::X86MovR32Imm32MaterializationFunction),
    X86MovR64Imm32SignExtended(
        &'a omega_machine_optimizer::X86MovR64Imm32SignExtendedMaterializationFunction,
    ),
    X86XorZero(&'a omega_machine_optimizer::X86XorZeroMaterializationFunction),
}

impl<'a> MaterializationFunction<'a> {
    pub(super) fn matches(self, machine: MachineId, block_count: usize) -> bool {
        match self {
            Self::Aarch64Movn(function) => {
                function.machine == machine && function.blocks.len() == block_count
            }
            Self::X86MovR32Imm32(function) => {
                function.machine == machine && function.blocks.len() == block_count
            }
            Self::X86MovR64Imm32SignExtended(function) => {
                function.machine == machine && function.blocks.len() == block_count
            }
            Self::X86XorZero(function) => {
                function.machine == machine && function.blocks.len() == block_count
            }
        }
    }

    pub(super) fn block(
        self,
        index: usize,
    ) -> Result<MaterializationBlock<'a>, OptimizedSelectedFormEncodingError> {
        match self {
            Self::Aarch64Movn(function) => function
                .blocks
                .get(index)
                .map(MaterializationBlock::Aarch64Movn),
            Self::X86MovR32Imm32(function) => function
                .blocks
                .get(index)
                .map(MaterializationBlock::X86MovR32Imm32),
            Self::X86MovR64Imm32SignExtended(function) => function
                .blocks
                .get(index)
                .map(MaterializationBlock::X86MovR64Imm32SignExtended),
            Self::X86XorZero(function) => function
                .blocks
                .get(index)
                .map(MaterializationBlock::X86XorZero),
        }
        .ok_or(OptimizedSelectedFormEncodingError::BlockRosterMismatch)
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum MaterializationBlock<'a> {
    Aarch64Movn(&'a omega_machine_optimizer::Aarch64MovnMaterializationBlock),
    X86MovR32Imm32(&'a omega_machine_optimizer::X86MovR32Imm32MaterializationBlock),
    X86MovR64Imm32SignExtended(
        &'a omega_machine_optimizer::X86MovR64Imm32SignExtendedMaterializationBlock,
    ),
    X86XorZero(&'a omega_machine_optimizer::X86XorZeroMaterializationBlock),
}

impl<'a> MaterializationBlock<'a> {
    pub(super) fn matches(self, block: SelectedBlockId, instruction_count: usize) -> bool {
        match self {
            Self::Aarch64Movn(row) => {
                row.block == block && row.instructions.len() == instruction_count
            }
            Self::X86MovR32Imm32(row) => {
                row.block == block && row.instructions.len() == instruction_count
            }
            Self::X86MovR64Imm32SignExtended(row) => {
                row.block == block && row.instructions.len() == instruction_count
            }
            Self::X86XorZero(row) => {
                row.block == block && row.instructions.len() == instruction_count
            }
        }
    }

    pub(super) fn disposition(
        self,
        index: usize,
        instruction: SelectedInstructionId,
    ) -> Result<MaterializationDisposition<'a>, OptimizedSelectedFormEncodingError> {
        let (actual, disposition) = match self {
            Self::Aarch64Movn(block) => {
                let row = block
                    .instructions
                    .get(index)
                    .ok_or(OptimizedSelectedFormEncodingError::InstructionRosterMismatch)?;
                (
                    row.instruction,
                    MaterializationDisposition::Aarch64Movn(&row.disposition),
                )
            }
            Self::X86MovR32Imm32(block) => {
                let row = block
                    .instructions
                    .get(index)
                    .ok_or(OptimizedSelectedFormEncodingError::InstructionRosterMismatch)?;
                (
                    row.instruction,
                    MaterializationDisposition::X86MovR32Imm32(&row.disposition),
                )
            }
            Self::X86MovR64Imm32SignExtended(block) => {
                let row = block
                    .instructions
                    .get(index)
                    .ok_or(OptimizedSelectedFormEncodingError::InstructionRosterMismatch)?;
                (
                    row.instruction,
                    MaterializationDisposition::X86MovR64Imm32SignExtended(&row.disposition),
                )
            }
            Self::X86XorZero(block) => {
                let row = block
                    .instructions
                    .get(index)
                    .ok_or(OptimizedSelectedFormEncodingError::InstructionRosterMismatch)?;
                (
                    row.instruction,
                    MaterializationDisposition::X86XorZero(&row.disposition),
                )
            }
        };
        if actual != instruction {
            return Err(OptimizedSelectedFormEncodingError::InstructionRosterMismatch);
        }
        Ok(disposition)
    }
}
