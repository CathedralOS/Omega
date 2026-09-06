//! Shared validation vocabulary and canonical collection helpers.

pub(super) use std::collections::{BTreeMap, BTreeSet};

pub(super) use crate::analyses::liveness::identity::liveness_identity;
pub(super) use crate::analyses::liveness::model::{
    BlockLiveness, EntryDefinition, FunctionLiveness, InstructionLiveness, LivenessError,
    LivenessPlan, LivenessPosition, LivenessValidationReceipt, OperandPosition, SuccessorLiveness,
    ValidatedLiveness,
};
pub(super) use register_model::{RegisterOperandAccess, RegisterUnitId};
pub(super) use selected_instructions::{
    SelectedBlock, SelectedFunction, SelectedInstruction, SelectedStructuralUnitFunction,
    SelectedTerminator, VirtualRegisterId, VirtualRegisterOrigin,
};

pub(super) fn ordered_instructions(block: &SelectedBlock) -> Vec<&SelectedInstruction> {
    block
        .instructions
        .iter()
        .chain(std::iter::once(match &block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
            | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
            | SelectedTerminator::Jump { instruction, .. }
            | SelectedTerminator::Return { instruction, .. } => instruction,
        }))
        .collect()
}

pub(super) fn require_canonical<T: Ord>(
    function: usize,
    instruction: Option<u32>,
    set: &[T],
) -> Result<(), LivenessError> {
    if set.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(LivenessError::NonCanonicalSet {
            function,
            instruction,
        });
    }
    Ok(())
}

pub(super) fn collect<T: Copy + Ord>(set: &BTreeSet<T>) -> Vec<T> {
    set.iter().copied().collect()
}
