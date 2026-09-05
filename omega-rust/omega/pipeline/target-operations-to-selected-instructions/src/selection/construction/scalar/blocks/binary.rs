//! Exact add/subtract leaf block construction.

use crate::selection::constraints::instruction;
use crate::selection::shared::*;

use super::super::model::ScalarConstructionContext;

pub(in crate::selection::construction::scalar) fn exact_binary_return(
    context: &ScalarConstructionContext<'_>,
    id: SelectedBlockId,
    source_block: semantic_vocabulary::BlockId,
    instruction_ids: [u32; 4],
    registers: [VirtualRegisterId; 3],
    source: &SourceLeaf,
) -> Result<SelectedBlock, SelectedInstructionError> {
    let keys = context.constraints.keys;
    let (obligation, operations, values, operation_fuel, left, right, kind, key) =
        match &source.value {
            SourceLeafValue::ExactAdd {
                obligation,
                accepted_fact,
                add_operation,
                add_fuel,
                left,
                right,
                ..
            } => (
                obligation,
                vec![*add_operation],
                vec![left.source_value, right.source_value, source.source_value],
                add_fuel.clone(),
                left,
                right,
                SelectedInstructionKind::ExactAddI64 {
                    obligation: *obligation,
                    accepted_fact: *accepted_fact,
                },
                keys.add_i64,
            ),
            SourceLeafValue::WidenedExactAdd {
                obligation,
                accepted_fact,
                add_operation,
                narrow_result,
                add_fuel,
                widen_operation,
                widen_fuel,
                left,
                right,
                ..
            } => (
                obligation,
                vec![*add_operation, *widen_operation],
                vec![
                    left.source_value,
                    right.source_value,
                    *narrow_result,
                    source.source_value,
                ],
                add_fuel.iter().chain(widen_fuel).copied().collect(),
                left,
                right,
                SelectedInstructionKind::ExactAddI64 {
                    obligation: *obligation,
                    accepted_fact: *accepted_fact,
                },
                keys.add_i64,
            ),
            SourceLeafValue::ExactSubtract {
                obligation,
                accepted_fact,
                subtract_operation,
                subtract_fuel,
                left,
                right,
                ..
            } => (
                obligation,
                vec![*subtract_operation],
                vec![left.source_value, right.source_value, source.source_value],
                subtract_fuel.clone(),
                left,
                right,
                SelectedInstructionKind::ExactSubtractI64 {
                    obligation: *obligation,
                    accepted_fact: *accepted_fact,
                },
                keys.subtract_i64,
            ),
            SourceLeafValue::WidenedExactSubtract {
                obligation,
                accepted_fact,
                subtract_operation,
                narrow_result,
                subtract_fuel,
                widen_operation,
                widen_fuel,
                left,
                right,
                ..
            } => (
                obligation,
                vec![*subtract_operation, *widen_operation],
                vec![
                    left.source_value,
                    right.source_value,
                    *narrow_result,
                    source.source_value,
                ],
                subtract_fuel.iter().chain(widen_fuel).copied().collect(),
                left,
                right,
                SelectedInstructionKind::ExactSubtractI64 {
                    obligation: *obligation,
                    accepted_fact: *accepted_fact,
                },
                keys.subtract_i64,
            ),
            _ => {
                return Err(SelectedInstructionError::UnsupportedSourceShape {
                    function: context.function,
                });
            }
        };
    let materialize = |id, register, immediate: &SourceImmediate| {
        instruction(
            SelectedInstructionId(id),
            SelectedInstructionKind::MaterializeI64 {
                value: immediate.value,
            },
            keys.materialize_i64,
            &[register],
            SelectedInstructionProvenance {
                operations: vec![immediate.constant_operation],
                values: vec![immediate.source_value],
                fuel: immediate.fuel.clone(),
                ..Default::default()
            },
            context.catalog,
        )
    };
    Ok(SelectedBlock {
        id,
        source_block,
        instructions: vec![
            materialize(instruction_ids[0], registers[0], left)?,
            materialize(instruction_ids[1], registers[1], right)?,
            instruction(
                SelectedInstructionId(instruction_ids[2]),
                kind,
                key,
                &registers,
                SelectedInstructionProvenance {
                    operations,
                    values,
                    obligations: vec![*obligation],
                    fuel: operation_fuel,
                    ..Default::default()
                },
                context.catalog,
            )?,
        ],
        terminator: SelectedTerminator::Return {
            instruction: instruction(
                SelectedInstructionId(instruction_ids[3]),
                SelectedInstructionKind::ReturnI64,
                keys.return_i64,
                &[registers[2]],
                SelectedInstructionProvenance {
                    values: vec![source.source_value],
                    edges: vec![source.return_edge],
                    fuel: source.return_fuel.clone(),
                    ..Default::default()
                },
                context.catalog,
            )?,
            psi_return_edge: source.return_edge,
        },
    })
}
