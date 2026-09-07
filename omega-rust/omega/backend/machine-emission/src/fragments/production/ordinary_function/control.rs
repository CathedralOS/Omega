use machine_code::{
    FunctionFragmentConditionalBranchPredicate, FunctionFragmentControlProvenance,
    FunctionFragmentSuccessorProvenance,
};
use selected_instructions::{
    SelectedBlock, SelectedInstruction, SelectedInstructionKind, SelectedTerminator,
};

pub(super) fn provenance(
    block: &SelectedBlock,
    instruction: &SelectedInstruction,
) -> FunctionFragmentControlProvenance {
    if let SelectedInstructionKind::CallI64 { callee } = instruction.kind {
        return FunctionFragmentControlProvenance::DirectInternalCall { callee };
    }
    match &block.terminator {
        SelectedTerminator::Jump {
            instruction: jump,
            successor,
        } if jump.id == instruction.id => FunctionFragmentControlProvenance::Jump {
            successor: FunctionFragmentSuccessorProvenance {
                psi_edge: successor.psi_edge,
                block: successor.block,
                source_target: successor.source_target,
                bindings: successor
                    .bindings
                    .iter()
                    .map(|binding| binding.semantic)
                    .collect(),
                fuel: successor.fuel.clone(),
            },
        },
        SelectedTerminator::ConditionalBranch {
            instruction: branch,
            when_nonzero,
            when_zero,
        } if branch.id == instruction.id => FunctionFragmentControlProvenance::ConditionalBranch {
            predicate: FunctionFragmentConditionalBranchPredicate::NonZeroV1,
            when_taken: FunctionFragmentSuccessorProvenance {
                psi_edge: when_nonzero.psi_edge,
                block: when_nonzero.block,
                source_target: when_nonzero.source_target,
                bindings: when_nonzero
                    .bindings
                    .iter()
                    .map(|binding| binding.semantic)
                    .collect(),
                fuel: when_nonzero.fuel.clone(),
            },
            when_fallthrough: FunctionFragmentSuccessorProvenance {
                psi_edge: when_zero.psi_edge,
                block: when_zero.block,
                source_target: when_zero.source_target,
                bindings: when_zero
                    .bindings
                    .iter()
                    .map(|binding| binding.semantic)
                    .collect(),
                fuel: when_zero.fuel.clone(),
            },
        },
        SelectedTerminator::ConditionalBranchU64LessThan {
            instruction: branch,
            when_less,
            when_not_less,
        } if branch.id == instruction.id => FunctionFragmentControlProvenance::ConditionalBranch {
            predicate: FunctionFragmentConditionalBranchPredicate::U64LessThanV1,
            when_taken: FunctionFragmentSuccessorProvenance {
                psi_edge: when_less.psi_edge,
                block: when_less.block,
                source_target: when_less.source_target,
                bindings: when_less
                    .bindings
                    .iter()
                    .map(|binding| binding.semantic)
                    .collect(),
                fuel: when_less.fuel.clone(),
            },
            when_fallthrough: FunctionFragmentSuccessorProvenance {
                psi_edge: when_not_less.psi_edge,
                block: when_not_less.block,
                source_target: when_not_less.source_target,
                bindings: when_not_less
                    .bindings
                    .iter()
                    .map(|binding| binding.semantic)
                    .collect(),
                fuel: when_not_less.fuel.clone(),
            },
        },
        SelectedTerminator::ConditionalBranchI64LessThan {
            instruction: branch,
            when_less,
            when_not_less,
        } if branch.id == instruction.id => FunctionFragmentControlProvenance::ConditionalBranch {
            predicate: FunctionFragmentConditionalBranchPredicate::I64LessThanV1,
            when_taken: FunctionFragmentSuccessorProvenance {
                psi_edge: when_less.psi_edge,
                block: when_less.block,
                source_target: when_less.source_target,
                bindings: when_less
                    .bindings
                    .iter()
                    .map(|binding| binding.semantic)
                    .collect(),
                fuel: when_less.fuel.clone(),
            },
            when_fallthrough: FunctionFragmentSuccessorProvenance {
                psi_edge: when_not_less.psi_edge,
                block: when_not_less.block,
                source_target: when_not_less.source_target,
                bindings: when_not_less
                    .bindings
                    .iter()
                    .map(|binding| binding.semantic)
                    .collect(),
                fuel: when_not_less.fuel.clone(),
            },
        },
        SelectedTerminator::Return {
            instruction: returned,
            psi_return_edge,
        } if returned.id == instruction.id => FunctionFragmentControlProvenance::Return {
            psi_return_edge: *psi_return_edge,
        },
        _ => FunctionFragmentControlProvenance::None,
    }
}
