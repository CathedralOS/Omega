//! Source-side result and cleanup projections: reconstruct the scalar result
//! each operation claims and the tails a hosted-exit or affine cleanup must
//! carry.

use abstract_operations::{
    AbstractBoundaryResult, AbstractFunction, AbstractOperation, AbstractResult,
    AbstractStructuralCallResult,
};
use semantic_vocabulary::{IntegerType, OperationId, ScalarType, ValueId};
use terminal_psi::TerminalAffineCleanupAction;

pub(super) fn no_code_cleanup(actions: &[TerminalAffineCleanupAction]) -> bool {
    actions.iter().all(|action| {
        matches!(
            action,
            TerminalAffineCleanupAction::DiscardRoot(_)
                | TerminalAffineCleanupAction::DiscardResidual(_)
        )
    })
}

/// A `HostedExitProcessI32` settlement never returns: the source boundary
/// call is the last non-terminator operation of its block and the block
/// closes on a `ReturnUnit` with only no-code discards. Independent graph
/// replay checks their exact source equality, owned homes and residual paths.
pub(super) fn hosted_exit_source_tail(source: &AbstractFunction, position: usize) -> bool {
    let mut block_end = None;
    for (index, entry) in source.block_entries.iter().enumerate() {
        let end = source
            .block_entries
            .get(index + 1)
            .map_or(source.operations.len(), |next| next.operation_offset);
        if entry.operation_offset <= position && position < end {
            block_end = Some(end);
            break;
        }
    }
    let Some(end) = block_end else {
        return false;
    };
    position + 2 == end
        && matches!(
            source.operations.get(end - 1),
            Some(AbstractOperation::ReturnUnit {
                cleanup_actions,
                ..
            }) if no_code_cleanup(cleanup_actions)
        )
}

/// The integer-result operations whose durable scalar home enters the
/// producer's known-integer map: scalar definitions, structural scalar
/// reads, and the scalar-result call family. Constants bind `Immediate`
/// sources and parameters bind parameter sources, so a `Home` citation can
/// only name one of these producing operations.
pub(super) fn integer_home_result(
    operation: &AbstractOperation,
) -> Option<(OperationId, AbstractResult)> {
    let integer = |operation: OperationId, result: ValueId, scalar_type: IntegerType| {
        (
            operation,
            AbstractResult {
                value: result,
                scalar_type: ScalarType::Integer(scalar_type),
            },
        )
    };
    Some(match operation {
        AbstractOperation::PrimitiveScalarRead {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::StructuralScalarFieldRead {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::StructuralByteSequenceFieldLength {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::StructuralByteSequenceFieldRead {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::StructuralCaseMembership {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::ByteSequenceLength {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::ByteSequenceRead {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::ElementViewLength {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::ElementViewRead {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::CallStructural {
            psi_operation,
            result: AbstractStructuralCallResult::Scalar(result),
            ..
        } => (*psi_operation, *result),
        AbstractOperation::Call {
            psi_operation,
            result,
            scalar_type,
            ..
        } => (
            *psi_operation,
            AbstractResult {
                value: *result,
                scalar_type: *scalar_type,
            },
        ),
        AbstractOperation::BoundaryCall {
            psi_operation,
            result: AbstractBoundaryResult::Scalar(result),
            ..
        } => (*psi_operation, *result),
        AbstractOperation::IntegerWiden {
            psi_operation,
            result,
            target_type,
            ..
        }
        | AbstractOperation::IntegerExactCast {
            psi_operation,
            result,
            target_type,
            ..
        } => integer(*psi_operation, *result, *target_type),
        AbstractOperation::IntegerShift {
            psi_operation,
            result,
            value_type,
            ..
        } => integer(*psi_operation, *result, *value_type),
        AbstractOperation::IntegerArithmetic {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | AbstractOperation::TrappingInteger {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | AbstractOperation::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | AbstractOperation::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | AbstractOperation::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | AbstractOperation::IntegerBitwiseNot {
            psi_operation,
            result,
            scalar_type,
            ..
        } => integer(*psi_operation, *result, *scalar_type),
        _ => return None,
    })
}

/// Non-integer scalar producers: boolean and IEEE results that
/// `integer_home_result` does not project. Integer-producing operations
/// delegate there first so one projector owns each operation kind.
pub(super) fn scalar_home_result(
    operation: &AbstractOperation,
) -> Option<(OperationId, AbstractResult)> {
    if let Some(result) = integer_home_result(operation) {
        return Some(result);
    }
    let boolean = |operation: OperationId, result: ValueId| {
        (
            operation,
            AbstractResult {
                value: result,
                scalar_type: ScalarType::Boolean,
            },
        )
    };
    Some(match operation {
        AbstractOperation::ScalarRepresentationView {
            psi_operation,
            result,
            target_type,
            ..
        } => (
            *psi_operation,
            AbstractResult {
                value: *result,
                scalar_type: *target_type,
            },
        ),
        AbstractOperation::AtomicEvent {
            psi_operation,
            event,
            ..
        } => {
            let result = match event {
                abstract_operations::AbstractAtomicEvent::Load { result, .. } => result,
                abstract_operations::AbstractAtomicEvent::ReadModifyWrite { prior, .. }
                | abstract_operations::AbstractAtomicEvent::Swap { prior, .. } => prior,
                abstract_operations::AbstractAtomicEvent::CompareExchange { observed, .. } => {
                    observed
                }
                _ => return None,
            };
            (*psi_operation, *result)
        }
        AbstractOperation::BooleanNot {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::BooleanEqual {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::IntegerEqual {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::IntegerLessThan {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::IntegerLessOrEqual {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::IeeeFloatCompare {
            psi_operation,
            result,
            ..
        } => boolean(*psi_operation, *result),
        AbstractOperation::NearestIeeeFloatFusedMultiplyAdd {
            psi_operation,
            result,
            format,
            ..
        }
        | AbstractOperation::FloatResultPolicy {
            psi_operation,
            result,
            format,
            ..
        } => (
            *psi_operation,
            AbstractResult {
                value: *result,
                scalar_type: ScalarType::IeeeFloat(*format),
            },
        ),
        _ => return None,
    })
}
