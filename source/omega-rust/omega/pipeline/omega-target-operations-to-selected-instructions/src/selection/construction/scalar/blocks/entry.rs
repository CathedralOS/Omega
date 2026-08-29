//! Shared conditional entry block.

use crate::selection::constraints::instruction;
use crate::selection::shared::*;

use super::super::model::ScalarConstructionContext;

pub(in crate::selection::construction::scalar) fn condition(
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
            when_nonzero: SelectedSuccessor {
                psi_edge: source.branch_true_edge,
                block: SelectedBlockId(1),
                source_target: source.true_block,
                bindings: source.branch_true_bindings.clone(),
                fuel: source.branch_true_fuel.clone(),
            },
            when_zero: SelectedSuccessor {
                psi_edge: source.branch_false_edge,
                block: SelectedBlockId(2),
                source_target: source.false_block,
                bindings: source.branch_false_bindings.clone(),
                fuel: source.branch_false_fuel.clone(),
            },
        },
    })
}
