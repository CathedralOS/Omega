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
    if let SelectedInstructionKind::CallI64 { callee }
    | SelectedInstructionKind::CallUnit { callee } = instruction.kind
    {
        return require(
            matches!(actual, Control::DirectInternalCall { callee: target } if *target == callee),
        );
    }
    let (terminal, predicate, successors) = match &block.terminator {
        SelectedTerminator::Jump {
            instruction: terminal,
            successor: target,
        } => {
            return if terminal.id == instruction.id {
                let Control::Jump { successor: actual } = actual else {
                    return Err(ResolvedFragmentEmissionError::ArtifactMismatch);
                };
                successor(target, actual)
            } else {
                require(matches!(actual, Control::None))
            };
        }
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
        actual.role == source.role
            && actual.psi_edge == source.psi_edge
            && (actual.role
                != selected_instructions::SelectedSuccessorRole::EdgeTransferContinuation
                || actual.fuel.is_empty())
            && actual.block == source.block
            && actual.source_target == source.source_target
            && actual
                .bindings
                .iter()
                .copied()
                .eq(source.bindings.iter().map(|binding| binding.semantic))
            && actual.fuel == source.fuel,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use selected_instructions::{SelectedBlockId, SelectedSuccessorRole};
    use semantic_vocabulary::{BlockId, EdgeId};

    #[test]
    fn continuation_retains_exact_successor_role_and_destination() {
        let source = SelectedSuccessor {
            role: SelectedSuccessorRole::EdgeTransferContinuation,
            structural_bindings: Vec::new(),
            psi_edge: EdgeId::new(1).unwrap(),
            block: SelectedBlockId(2),
            source_target: BlockId::new(3).unwrap(),
            bindings: Vec::new(),
            fuel: Vec::new(),
        };
        let mut actual = FunctionFragmentSuccessorProvenance {
            role: source.role,
            psi_edge: source.psi_edge,
            block: source.block,
            source_target: source.source_target,
            bindings: Vec::new(),
            fuel: Vec::new(),
        };
        successor(&source, &actual).unwrap();
        actual.role = SelectedSuccessorRole::Semantic;
        assert!(successor(&source, &actual).is_err());
        actual.role = source.role;
        actual.block = SelectedBlockId(4);
        assert!(successor(&source, &actual).is_err());
    }
}
