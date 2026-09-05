//! Ordered U64 parameter inequality with the Boolean-not provenance retained by the branch.

use crate::selection::constraints::instruction;
use crate::selection::shared::*;

use super::ScalarConstructionContext;
use super::direct_parameter::{false_successor, true_successor};

pub(super) fn build(
    context: &ScalarConstructionContext<'_>,
) -> Result<SelectedBlock, SelectedInstructionError> {
    let source = context.source;
    let LegalizedCondition::IntegerNotEqualParametersV1 {
        equality_operation,
        equality_result,
        equality_fuel,
        boolean_not_operation,
        boolean_not_result,
        boolean_not_fuel,
        left,
        right,
        ..
    } = &source.condition
    else {
        unreachable!("condition entrance selected integer inequality")
    };
    let keys = context.constraints.keys;
    Ok(SelectedBlock {
        id: SelectedBlockId(0),
        source_block: source.entry_block,
        instructions: vec![instruction(
            SelectedInstructionId(0),
            SelectedInstructionKind::CompareI64,
            keys.compare_i64,
            &[VirtualRegisterId(0), VirtualRegisterId(1)],
            SelectedInstructionProvenance {
                operations: vec![*equality_operation],
                values: vec![left.source_value, right.source_value, *equality_result],
                fuel: equality_fuel.clone(),
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
                    operations: vec![*boolean_not_operation],
                    values: vec![*equality_result, *boolean_not_result],
                    fuel: boolean_not_fuel.clone(),
                    ..Default::default()
                },
                context.catalog,
            )?,
            when_nonzero: true_successor(source),
            when_zero: false_successor(source),
        },
    })
}
