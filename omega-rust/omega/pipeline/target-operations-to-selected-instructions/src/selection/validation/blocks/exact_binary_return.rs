use crate::selection::shared::*;

use super::instruction_projection;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate(
    function_index: usize,
    block: &SelectedBlock,
    instruction_ids: [u32; 4],
    registers: [VirtualRegisterId; 3],
    source: &SourceLeaf,
    keys: &SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
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
                    function: function_index,
                });
            }
        };
    if block.instructions.len() != 3 {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function: function_index,
            block: block.id.0,
        });
    }
    for (position, immediate) in [left, right].into_iter().enumerate() {
        instruction_projection::validate(
            function_index,
            &block.instructions[position],
            SelectedInstructionId(instruction_ids[position]),
            SelectedInstructionKind::MaterializeI64 {
                value: immediate.value,
            },
            keys.materialize_i64,
            &[registers[position]],
            &SelectedInstructionProvenance {
                operations: vec![immediate.constant_operation],
                values: vec![immediate.source_value],
                fuel: immediate.fuel.clone(),
                ..Default::default()
            },
            catalog,
        )?;
    }
    instruction_projection::validate(
        function_index,
        &block.instructions[2],
        SelectedInstructionId(instruction_ids[2]),
        kind,
        key,
        &registers,
        &SelectedInstructionProvenance {
            operations,
            values,
            obligations: vec![*obligation],
            fuel: operation_fuel,
            ..Default::default()
        },
        catalog,
    )?;
    let SelectedTerminator::Return {
        instruction,
        psi_return_edge,
    } = &block.terminator
    else {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function: function_index,
            block: block.id.0,
        });
    };
    if *psi_return_edge != source.return_edge {
        return Err(SelectedInstructionError::SuccessorProjectionMismatch {
            function: function_index,
            block: block.id.0,
        });
    }
    instruction_projection::validate(
        function_index,
        instruction,
        SelectedInstructionId(instruction_ids[3]),
        SelectedInstructionKind::ReturnI64,
        keys.return_i64,
        &[registers[2]],
        &SelectedInstructionProvenance {
            values: vec![source.source_value],
            edges: vec![source.return_edge],
            fuel: source.return_fuel.clone(),
            ..Default::default()
        },
        catalog,
    )
}
