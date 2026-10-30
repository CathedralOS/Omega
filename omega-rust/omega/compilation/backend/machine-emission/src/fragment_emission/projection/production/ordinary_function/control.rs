use machine_code::{
    FunctionFragmentConditionalBranchPredicate, FunctionFragmentControlProvenance,
    FunctionFragmentSuccessorProvenance,
};
use selected_instructions::{
    SelectedBlock, SelectedInstruction, SelectedInstructionKind, SelectedTerminator,
};

pub(super) fn provenance(
    function: &selected_instructions::SelectedFunction,
    block: &SelectedBlock,
    instruction: &SelectedInstruction,
) -> Result<
    FunctionFragmentControlProvenance,
    crate::fragment_emission::projection::ResolvedFragmentEmissionError,
> {
    if let selected_instructions::selected_call_pattern!(target, ..) = instruction.kind {
        let mut rows = function
            .calls
            .iter()
            .filter(|row| row.instruction == instruction.id);
        let row = rows.next().ok_or(
            crate::fragment_emission::projection::ResolvedFragmentEmissionError::ArtifactMismatch,
        )?;
        if rows.next().is_some()
            || selected_instructions::SelectedCallTarget::from_native(row.call.target) != target
        {
            return Err(crate::fragment_emission::projection::ResolvedFragmentEmissionError::ArtifactMismatch);
        }
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
        return Ok(FunctionFragmentControlProvenance::InternalCall {
            target,
            owner: row.owner,
        });
    }
    if let SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } = instruction.kind {
        let row = function
            .normalized_foreign_calls
            .get(ordinal as usize)
            .ok_or(
            crate::fragment_emission::projection::ResolvedFragmentEmissionError::ArtifactMismatch,
        )?;
        if row.instruction != instruction.id || row.call.boundary != boundary {
            return Err(crate::fragment_emission::projection::ResolvedFragmentEmissionError::ArtifactMismatch);
        }
        return Ok(FunctionFragmentControlProvenance::NormalizedForeignCall {
            invocation_site: row.invocation_site,
            boundary,
            ordinal,
        });
    }
    Ok(match &block.terminator {
        SelectedTerminator::OperationTrap {
            instruction: terminal,
            operation,
        } if terminal.id == instruction.id => FunctionFragmentControlProvenance::OperationTrap {
            operation: *operation,
        },
        SelectedTerminator::Crash {
            instruction: terminal,
            psi_edge,
            cause,
            ..
        } if terminal.id == instruction.id => FunctionFragmentControlProvenance::Crash {
            psi_edge: *psi_edge,
            cause: *cause,
        },
        SelectedTerminator::Jump {
            instruction: jump,
            successor,
        } if jump.id == instruction.id => FunctionFragmentControlProvenance::Jump {
            successor: successor_provenance(successor),
        },
        SelectedTerminator::ConditionalBranch {
            instruction: branch,
            when_nonzero,
            when_zero,
        } if branch.id == instruction.id => FunctionFragmentControlProvenance::ConditionalBranch {
            predicate: FunctionFragmentConditionalBranchPredicate::NonZeroV1,
            when_taken: successor_provenance(when_nonzero),
            when_fallthrough: successor_provenance(when_zero),
        },
        SelectedTerminator::ConditionalBranchU64LessThan {
            instruction: branch,
            when_less,
            when_not_less,
        } if branch.id == instruction.id => FunctionFragmentControlProvenance::ConditionalBranch {
            predicate: FunctionFragmentConditionalBranchPredicate::U64LessThanV1,
            when_taken: successor_provenance(when_less),
            when_fallthrough: successor_provenance(when_not_less),
        },
        SelectedTerminator::ConditionalBranchI64LessThan {
            instruction: branch,
            when_less,
            when_not_less,
        } if branch.id == instruction.id => FunctionFragmentControlProvenance::ConditionalBranch {
            predicate: FunctionFragmentConditionalBranchPredicate::I64LessThanV1,
            when_taken: successor_provenance(when_less),
            when_fallthrough: successor_provenance(when_not_less),
        },
        SelectedTerminator::HostedExitProcess {
            instruction: exited,
            completion_edge,
        } if exited.id == instruction.id => FunctionFragmentControlProvenance::HostedExitProcess {
            completion_edge: *completion_edge,
        },
        SelectedTerminator::Return {
            instruction: returned,
            psi_return_edge,
        } if returned.id == instruction.id => FunctionFragmentControlProvenance::Return {
            psi_return_edge: *psi_return_edge,
        },
        _ => FunctionFragmentControlProvenance::None,
    })
}

fn successor_provenance(
    successor: &selected_instructions::SelectedSuccessor,
) -> FunctionFragmentSuccessorProvenance {
    FunctionFragmentSuccessorProvenance {
        role: successor.role,
        psi_edge: successor.psi_edge,
        block: successor.block,
        source_target: successor.source_target,
        bindings: successor
            .bindings
            .iter()
            .map(|binding| binding.semantic)
            .collect(),
        fuel: successor.fuel.clone(),
    }
}
