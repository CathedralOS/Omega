//! Check semantic control provenance without constructing a replacement row.

use super::{ResolvedFragmentEmissionError, require};
use machine_code::{
    FunctionFragmentConditionalBranchPredicate as Predicate,
    FunctionFragmentControlProvenance as Control, FunctionFragmentSuccessorProvenance,
};
use selected_instructions::{
    SelectedBlock, SelectedInstruction, SelectedInstructionKind, SelectedSuccessor,
    SelectedTerminator,
};

pub(super) fn check(
    block: &SelectedBlock,
    instruction: &SelectedInstruction,
    actual: &Control,
) -> Result<(), ResolvedFragmentEmissionError> {
    if let SelectedInstructionKind::CallI64 { callee } = instruction.kind {
        return require(
            matches!(actual, Control::DirectInternalCall { callee: target } if *target == callee),
        );
    }
    let (terminal, predicate, successors) = match &block.terminator {
        SelectedTerminator::ConditionalBranch {
            instruction,
            when_nonzero,
            when_zero,
        } => (instruction, Predicate::NonZeroV1, (when_nonzero, when_zero)),
        SelectedTerminator::ConditionalBranchU64LessThan {
            instruction,
            when_less,
            when_not_less,
        } => (
            instruction,
            Predicate::U64LessThanV1,
            (when_less, when_not_less),
        ),
        SelectedTerminator::ConditionalBranchI64LessThan {
            instruction,
            when_less,
            when_not_less,
        } => (
            instruction,
            Predicate::I64LessThanV1,
            (when_less, when_not_less),
        ),
        SelectedTerminator::Return {
            instruction: terminal,
            psi_return_edge,
        } => {
            return if terminal.id == instruction.id {
                require(
                    matches!(actual, Control::Return { psi_return_edge: edge } if edge == psi_return_edge),
                )
            } else {
                require(matches!(actual, Control::None))
            };
        }
    };
    if terminal.id != instruction.id {
        return require(matches!(actual, Control::None));
    }
    let Control::ConditionalBranch {
        predicate: actual_predicate,
        when_taken,
        when_fallthrough,
    } = actual
    else {
        return Err(ResolvedFragmentEmissionError::ArtifactMismatch);
    };
    let (taken, fallthrough) = successors;
    require(*actual_predicate == predicate)?;
    successor(taken, when_taken)?;
    successor(fallthrough, when_fallthrough)
}

fn successor(
    source: &SelectedSuccessor,
    actual: &FunctionFragmentSuccessorProvenance,
) -> Result<(), ResolvedFragmentEmissionError> {
    require(
        actual.psi_edge == source.psi_edge
            && actual.block == source.block
            && actual.source_target == source.source_target
            && actual.bindings == source.bindings
            && actual.fuel == source.fuel,
    )
}
