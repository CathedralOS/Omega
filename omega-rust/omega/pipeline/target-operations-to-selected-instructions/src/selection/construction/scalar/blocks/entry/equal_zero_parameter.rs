//! U64 parameter equality with zero, selected without materializing zero.

use crate::selection::constraints::instruction;
use crate::selection::shared::*;

use super::ScalarConstructionContext;
use super::direct_parameter::{false_successor, true_successor};

pub(super) fn build(
    context: &ScalarConstructionContext<'_>,
) -> Result<SelectedBlock, SelectedInstructionError> {
    let source = context.source;
    let LegalizedCondition::U64EqualZeroParameterV1 {
        operation,
        fuel,
        parameter,
        zero,
        ..
    } = &source.condition
    else {
        unreachable!("condition entrance selected U64 equality with zero")
    };
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
                operations: vec![zero.constant_operation, *operation],
                values: vec![
                    parameter.source_value,
                    zero.source_value,
                    source.condition_source,
                ],
                fuel: zero.fuel.iter().chain(fuel).copied().collect(),
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
            when_nonzero: false_successor(source),
            when_zero: true_successor(source),
        },
    })
}
