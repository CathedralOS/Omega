//! Ordered U64 parameter comparison with the less-than arm taken directly.

use crate::selection::constraints::instruction;
use crate::selection::shared::*;

use super::ScalarConstructionContext;
use super::direct_parameter::{false_successor, true_successor};

pub(super) fn build(
    context: &ScalarConstructionContext<'_>,
) -> Result<SelectedBlock, SelectedInstructionError> {
    let source = context.source;
    let LegalizedCondition::IntegerLessThanParametersV1 {
        operation,
        fuel,
        left,
        right,
        ..
    } = &source.condition
    else {
        unreachable!("condition entrance selected integer less-than")
    };
    let keys = &context.constraints.keys;
    Ok(SelectedBlock {
        id: SelectedBlockId(0),
        source_block: source.entry_block,
        instructions: vec![instruction(
            SelectedInstructionId(0),
            SelectedInstructionKind::CompareI64,
            keys.compare_i64,
            &[VirtualRegisterId(0), VirtualRegisterId(1)],
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
        terminator: SelectedTerminator::ConditionalBranchU64LessThan {
            instruction: instruction(
                SelectedInstructionId(1),
                SelectedInstructionKind::ConditionalBranchU64LessThan,
                keys.conditional_branch,
                &[],
                SelectedInstructionProvenance {
                    values: vec![source.condition_source],
                    ..Default::default()
                },
                context.catalog,
            )?,
            when_less: true_successor(source),
            when_not_less: false_successor(source),
        },
    })
}
