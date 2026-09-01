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
        SourceLeafValue::ActiveResidentExactAddChain(chain) => vec![
            chain.resident.constant_operation,
            chain.left.constant_operation,
            chain.right.constant_operation,
            chain.inner.operation,
            chain.middle.operation,
            chain.result.operation,
        ],
        SourceLeafValue::ActiveResidentExactAddBridgeChain(chain) => vec![
            chain.resident.constant_operation,
            chain.left.constant_operation,
            chain.right.constant_operation,
            chain.inner.operation,
            chain.middle.operation,
            chain.bridge.operation,
            chain.result.operation,
        ],
    }
}
