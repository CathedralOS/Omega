//! U64 parameter inequality with zero, selected without materializing zero.

use crate::selection::constraints::instruction;
use crate::selection::shared::*;

use super::ScalarConstructionContext;
use super::direct_parameter::{false_successor, true_successor};

pub(super) fn build(
    context: &ScalarConstructionContext<'_>,
) -> Result<SelectedBlock, SelectedInstructionError> {
    let source = context.source;
    let LegalizedCondition::U64NotEqualZeroParameterV1 {
        equality_operation,
        equality_result,
        equality_fuel,
        boolean_not_operation,
        boolean_not_result,
        boolean_not_fuel,
        parameter,
        zero,
        ..
    } = &source.condition
    else {
        unreachable!("condition entrance selected U64 inequality with zero")
    };
    let keys = &context.constraints.keys;
    Ok(SelectedBlock {
        id: SelectedBlockId(0),
        source_block: source.entry_block,
        instructions: vec![instruction(
            SelectedInstructionId(0),
            SelectedInstructionKind::CompareI64Zero,
            keys.compare_i64_zero,
            &[VirtualRegisterId(0)],
            SelectedInstructionProvenance {
                operations: vec![zero.constant_operation, *equality_operation],
                values: vec![parameter.source_value, zero.source_value, *equality_result],
                fuel: zero.fuel.iter().chain(equality_fuel).copied().collect(),
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
