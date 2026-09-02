//! Direct Boolean parameter compared with zero before branching.

use crate::selection::constraints::instruction;
use crate::selection::shared::*;

use super::ScalarConstructionContext;

pub(super) fn build(
    context: &ScalarConstructionContext<'_>,
) -> Result<SelectedBlock, SelectedInstructionError> {
    let source = context.source;
    let keys = context.constraints.keys;
    Ok(SelectedBlock {
        id: SelectedBlockId(0),
        source_block: source.entry_block,
        instructions: vec![instruction(
            SelectedInstructionId(0),
            SelectedInstructionKind::CompareI64Zero,
            keys.compare_i64_zero,
            &[VirtualRegisterId(0)],
            SelectedInstructionProvenance {
                values: vec![source.condition_source],
                ..Default::default()
            },
            context.catalog,
        )?],
        terminator: SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                SelectedInstructionId(1),
                SelectedInstructionKind::ConditionalBranchNonZero,
                keys.conditional_branch,
                &[],
                SelectedInstructionProvenance {
                    values: vec![source.condition_source],
                    ..Default::default()
                },
                context.catalog,
            )?,
            when_nonzero: true_successor(source),
            when_zero: false_successor(source),
        },
    })
}

pub(super) fn true_successor(source: &SourceFunction) -> SelectedSuccessor {
    SelectedSuccessor {
        psi_edge: source.branch_true_edge,
        block: SelectedBlockId(1),
        source_target: source.true_block,
        bindings: source.branch_true_bindings.clone(),
        fuel: source.branch_true_fuel.clone(),
    }
}

pub(super) fn false_successor(source: &SourceFunction) -> SelectedSuccessor {
    SelectedSuccessor {
        psi_edge: source.branch_false_edge,
        block: SelectedBlockId(2),
        source_target: source.false_block,
        bindings: source.branch_false_bindings.clone(),
        fuel: source.branch_false_fuel.clone(),
    }
}
