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
    function: &selected_instructions::SelectedFunction,
    block: &SelectedBlock,
    instruction: &SelectedInstruction,
    actual: &Control,
) -> Result<(), ResolvedFragmentEmissionError> {
    if let selected_instructions::selected_call_pattern!(target, ..) = instruction.kind {
        let mut rows = function
            .calls
            .iter()
            .filter(|row| row.instruction == instruction.id);
        let row = rows
            .next()
            .ok_or(ResolvedFragmentEmissionError::ArtifactMismatch)?;
        require(
            rows.next().is_none()
                && selected_instructions::SelectedCallTarget::from_native(row.call.target)
                    == target,
        )?;
        if matches!(
            instruction.kind,
            SelectedInstructionKind::InvokeNoNormal { .. }
        ) {
            let SelectedTerminator::InvokeNoNormal {
                instruction: terminal,
                psi_edge,
            } = &block.terminator
            else {
                return Err(crate::fragment_emission::projection::ResolvedFragmentEmissionError::ArtifactMismatch);
            };
            if terminal.id != instruction.id
                || row.owner != target_operations::CallSiteOwner::InvokeNoNormal(*psi_edge)
            {
                return Err(crate::fragment_emission::projection::ResolvedFragmentEmissionError::ArtifactMismatch);
            }
        } else if matches!(
            row.owner,
            target_operations::CallSiteOwner::InvokeNoNormal(_)
        ) {
            return Err(crate::fragment_emission::projection::ResolvedFragmentEmissionError::ArtifactMismatch);
        }
        return require(
            matches!(actual, Control::InternalCall { target: actual_target, owner } if *actual_target == target && *owner == row.owner),
        );
    }
    if let SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } = instruction.kind {
        let row = function
            .normalized_foreign_calls
            .get(ordinal as usize)
            .ok_or(ResolvedFragmentEmissionError::ArtifactMismatch)?;
        require(row.instruction == instruction.id && row.call.boundary == boundary)?;
        return require(matches!(
            actual,
            Control::NormalizedForeignCall {
                invocation_site,
                boundary: actual_boundary,
                ordinal: actual_ordinal,
            } if *actual_boundary == boundary && *actual_ordinal == ordinal
                && *invocation_site == row.invocation_site
        ));
    }
    let (terminal, predicate, successors) = match &block.terminator {
        SelectedTerminator::InvokeNoNormal {
            instruction: terminal,
            ..
        } => {
            // The actual invocation was checked above. Other instructions in
            // this block cannot borrow its terminal provenance.
            return require(terminal.id != instruction.id && matches!(actual, Control::None));
        }

        SelectedTerminator::OperationTrap {
            instruction: terminal,
            operation,
        } => {
            return if terminal.id == instruction.id {
                require(
                    matches!(actual, Control::OperationTrap { operation: actual } if actual == operation),
                )
            } else {
                require(matches!(actual, Control::None))
            };
        }
        SelectedTerminator::Crash {
            instruction: terminal,
            psi_edge,
            cause,
            ..
        } => {
            return if terminal.id == instruction.id {
                require(
                    matches!(actual, Control::Crash { psi_edge: edge, cause: actual_cause }
                    if edge == psi_edge && actual_cause == cause),
                )
            } else {
                require(matches!(actual, Control::None))
            };
        }
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
        SelectedTerminator::HostedExitProcess {
            instruction: terminal,
            completion_edge,
        } => {
            return if terminal.id == instruction.id {
                require(
                    matches!(actual, Control::HostedExitProcess { completion_edge: edge } if edge == completion_edge),
                )
            } else {
                require(matches!(actual, Control::None))
            };
        }
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
            && (actual.role == selected_instructions::SelectedSuccessorRole::Semantic
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
    use super::{FunctionFragmentSuccessorProvenance, SelectedSuccessor, successor};
    use selected_instructions::{SelectedBlockId, SelectedSuccessorRole};
    use semantic_vocabulary::{BlockId, EdgeId};

    #[test]
    fn continuation_retains_exact_successor_role_and_destination() {
        let source = SelectedSuccessor {
            structural_case: None,
            role: SelectedSuccessorRole::EdgeTransferContinuation,
            structural_bindings: Vec::new(),
            psi_edge: EdgeId::new(1),
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
