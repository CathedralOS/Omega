use super::super::shared::*;

pub(in crate::legalization::source) fn source_operations(
    value: &SourceLeafValue,
) -> Vec<OperationId> {
    match value {
        SourceLeafValue::Immediate {
            constant_operation, ..
        } => vec![*constant_operation],
        SourceLeafValue::EntryParameter { .. } => Vec::new(),
        SourceLeafValue::ExactAdd {
            add_operation,
            left,
            right,
            ..
        } => {
            vec![
                left.constant_operation,
                right.constant_operation,
                *add_operation,
            ]
        }
        SourceLeafValue::ExactSubtract {
            subtract_operation,
            left,
            right,
            ..
        } => {
            vec![
                left.constant_operation,
                right.constant_operation,
                *subtract_operation,
            ]
        }
        SourceLeafValue::WidenedExactAdd {
            add_operation,
            widen_operation,
            left,
            right,
            ..
        } => vec![
            left.constant_operation,
            right.constant_operation,
            *add_operation,
            *widen_operation,
        ],
        SourceLeafValue::WidenedExactSubtract {
            subtract_operation,
            widen_operation,
            left,
            right,
            ..
        } => vec![
            left.constant_operation,
            right.constant_operation,
            *subtract_operation,
            *widen_operation,
        ],
        SourceLeafValue::ExactIntegerSequence(sequence) => sequence
            .steps
            .iter()
            .map(|step| match step {
                legalized_operations::LegalizedIntegerStep::Immediate(value) => {
                    value.constant_operation
                }
                legalized_operations::LegalizedIntegerStep::ExactBinary(value) => value.operation,
            })
            .collect(),
    }
}
