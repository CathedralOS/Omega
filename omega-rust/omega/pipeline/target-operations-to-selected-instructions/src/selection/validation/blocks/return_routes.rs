use crate::selection::shared::*;

use super::{exact_binary_return, immediate_return, instruction_projection, parameter_return};

pub(super) fn validate(
    function_index: usize,
    source: &SourceFunction,
    function: &SelectedFunction,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    match (&source.when_true.value, &source.when_false.value) {
        (SourceLeafValue::ExactIntegerSequence(sequence), SourceLeafValue::Immediate { .. }) => {
            let source_leaf = &source.when_true;
            let block = &function.blocks[1];
            let result = super::super::integer_sequence::validate(
                function_index,
                sequence,
                source_leaf.source_value,
                &[],
                2,
                1,
                &function.virtual_registers[..1 + sequence.steps.len()],
                &block.instructions,
                keys,
                catalog,
            )?;
            let SelectedTerminator::Return {
                instruction,
                psi_return_edge,
            } = &block.terminator
            else {
                return Err(SelectedInstructionError::BlockProjectionMismatch {
                    function: function_index,
                    block: 1,
                });
            };
            if *psi_return_edge != source_leaf.return_edge {
                return Err(SelectedInstructionError::SuccessorProjectionMismatch {
                    function: function_index,
                    block: 1,
                });
            }
            let return_id = 2 + sequence.steps.len() as u32;
            instruction_projection::validate(
                function_index,
                instruction,
                SelectedInstructionId(return_id),
                SelectedInstructionKind::ReturnI64,
                keys.return_i64,
                &[result],
                &SelectedInstructionProvenance {
                    values: vec![source_leaf.source_value],
                    edges: vec![source_leaf.return_edge],
                    fuel: source_leaf.return_fuel.clone(),
                    ..Default::default()
                },
                catalog,
            )?;
            immediate_return::validate(
                function_index,
                &function.blocks[2],
                return_id + 1,
                return_id + 2,
                VirtualRegisterId(1 + sequence.steps.len() as u32),
                &source.when_false,
                keys,
                catalog,
            )
        }
        (SourceLeafValue::Immediate { .. }, SourceLeafValue::Immediate { .. }) => {
            let [true_register, false_register] = if matches!(
                source.condition,
                LegalizedCondition::IntegerEqualParametersV1 { .. }
                    | LegalizedCondition::IntegerLessThanParametersV1 { .. }
                    | LegalizedCondition::IntegerLessOrEqualParametersV1 { .. }
                    | LegalizedCondition::IntegerNotEqualParametersV1 { .. }
                    | LegalizedCondition::I64LessThanParametersV1 { .. }
                    | LegalizedCondition::I64LessOrEqualParametersV1 { .. }
            ) {
                [VirtualRegisterId(2), VirtualRegisterId(3)]
            } else {
                [VirtualRegisterId(1), VirtualRegisterId(2)]
            };
            immediate_return::validate(
                function_index,
                &function.blocks[1],
                2,
                3,
                true_register,
                &source.when_true,
                keys,
                catalog,
            )?;
            immediate_return::validate(
                function_index,
                &function.blocks[2],
                4,
                5,
                false_register,
                &source.when_false,
                keys,
                catalog,
            )
        }
        (SourceLeafValue::EntryParameter { .. }, SourceLeafValue::EntryParameter { .. }) => {
            parameter_return::validate(
                function_index,
                &function.blocks[1],
                2,
                VirtualRegisterId(1),
                &source.when_true,
                keys,
                catalog,
            )?;
            parameter_return::validate(
                function_index,
                &function.blocks[2],
                3,
                VirtualRegisterId(1),
                &source.when_false,
                keys,
                catalog,
            )
        }
        (SourceLeafValue::ExactAdd { .. }, SourceLeafValue::ExactAdd { .. }) => {
            validate_exact_binary_pair(function_index, source, function, keys, catalog)
        }
        (SourceLeafValue::WidenedExactAdd { .. }, SourceLeafValue::WidenedExactAdd { .. }) => {
            validate_exact_binary_pair(function_index, source, function, keys, catalog)
        }
        (
            SourceLeafValue::WidenedExactSubtract { .. },
            SourceLeafValue::WidenedExactSubtract { .. },
        ) => validate_exact_binary_pair(function_index, source, function, keys, catalog),
        (SourceLeafValue::ExactSubtract { .. }, SourceLeafValue::ExactSubtract { .. }) => {
            validate_exact_binary_pair(function_index, source, function, keys, catalog)
        }
        _ => Err(SelectedInstructionError::UnsupportedSourceShape {
            function: function_index,
        }),
    }
}

fn validate_exact_binary_pair(
    function_index: usize,
    source: &SourceFunction,
    function: &SelectedFunction,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    exact_binary_return::validate(
        function_index,
        &function.blocks[1],
        [2, 3, 4, 5],
        [
            VirtualRegisterId(1),
            VirtualRegisterId(2),
            VirtualRegisterId(3),
        ],
        &source.when_true,
        keys,
        catalog,
    )?;
    exact_binary_return::validate(
        function_index,
        &function.blocks[2],
        [6, 7, 8, 9],
        [
            VirtualRegisterId(4),
            VirtualRegisterId(5),
            VirtualRegisterId(6),
        ],
        &source.when_false,
        keys,
        catalog,
    )
}
