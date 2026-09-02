//! Ordered I64 less-or-equal entry projected through reversed signed less-than.

use crate::selection::constraints::instruction;
use crate::selection::shared::*;

use super::direct_parameter::{false_successor, true_successor};
use super::ScalarConstructionContext;

pub(super) fn build(
    context: &ScalarConstructionContext<'_>,
) -> Result<SelectedBlock, SelectedInstructionError> {
    let source = context.source;
    let LegalizedCondition::I64LessOrEqualParametersV1 {
        operation,
        fuel,
        left,
        right,
        ..
    } = &source.condition
    else {
        unreachable!("condition entrance selected I64 less-or-equal")
    };
    let keys = context.constraints.keys;
    Ok(SelectedBlock {
        id: SelectedBlockId(0),
        source_block: source.entry_block,
        instructions: vec![instruction(
            SelectedInstructionId(0),
            SelectedInstructionKind::CompareI64,
            keys.compare_i64,
            &[VirtualRegisterId(1), VirtualRegisterId(0)],
            SelectedInstructionProvenance {
                operations: vec![*operation],
                values: vec![
                    left.source_value,
                    right.source_value,
                    source.condition_source,
                ],
                fuel: fuel.clone(),
                ..Default::default()
            },
            context.catalog,
        )?],
        terminator: SelectedTerminator::ConditionalBranchI64LessThan {
            instruction: instruction(
                SelectedInstructionId(1),
                SelectedInstructionKind::ConditionalBranchI64LessThan,
                keys.conditional_branch,
                &[],
                SelectedInstructionProvenance {
                    values: vec![source.condition_source],
                    ..Default::default()
                },
                context.catalog,
            )?,
            when_less: false_successor(source),
            when_not_less: true_successor(source),
        },
    })
}
