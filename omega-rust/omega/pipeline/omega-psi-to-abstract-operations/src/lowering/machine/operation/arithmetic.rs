use omega_abstract_operations::AbstractOperation;
use psi_core::ScalarType;
use psi_terminal::{Operation, OperationKind};

use crate::lowering::LoweringError;

pub(super) fn lower(operation: &Operation) -> Result<AbstractOperation, LoweringError> {
    Ok(match operation.kind.clone() {
        OperationKind::ExactIntegerAdd { left, right, .. }
        | OperationKind::WrappingIntegerAdd { left, right } => {
            let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedWrappingAddMalformed(operation.id));
            };
            match operation.kind.clone() {
                OperationKind::ExactIntegerAdd { obligation, .. } => {
                    AbstractOperation::ExactIntegerAdd {
                        psi_operation: operation.id,
                        obligation,
                        result: operation.result.expect_scalar().id,
                        scalar_type,
                        left,
                        right,
                    }
                }
                OperationKind::WrappingIntegerAdd { .. } => AbstractOperation::WrappingIntegerAdd {
                    psi_operation: operation.id,
                    result: operation.result.expect_scalar().id,
                    scalar_type,
                    left,
                    right,
                },
                _ => unreachable!(),
            }
        }
        OperationKind::ExactIntegerSubtract { left, right, .. }
        | OperationKind::WrappingIntegerSubtract { left, right } => {
            let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedWrappingSubtractMalformed(
                    operation.id,
                ));
            };
            match operation.kind.clone() {
                OperationKind::ExactIntegerSubtract { obligation, .. } => {
                    AbstractOperation::ExactIntegerSubtract {
                        psi_operation: operation.id,
                        obligation,
                        result: operation.result.expect_scalar().id,
                        scalar_type,
                        left,
                        right,
                    }
                }
                OperationKind::WrappingIntegerSubtract { .. } => {
                    AbstractOperation::WrappingIntegerSubtract {
                        psi_operation: operation.id,
                        result: operation.result.expect_scalar().id,
                        scalar_type,
                        left,
                        right,
                    }
                }
                _ => unreachable!(),
            }
        }
        OperationKind::ExactIntegerMultiply { left, right, .. }
        | OperationKind::WrappingIntegerMultiply { left, right } => {
            let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedWrappingMultiplyMalformed(
                    operation.id,
                ));
            };
            match operation.kind.clone() {
                OperationKind::ExactIntegerMultiply { obligation, .. } => {
                    AbstractOperation::ExactIntegerMultiply {
                        psi_operation: operation.id,
                        obligation,
                        result: operation.result.expect_scalar().id,
                        scalar_type,
                        left,
                        right,
                    }
                }
                OperationKind::WrappingIntegerMultiply { .. } => {
                    AbstractOperation::WrappingIntegerMultiply {
                        psi_operation: operation.id,
                        result: operation.result.expect_scalar().id,
                        scalar_type,
                        left,
                        right,
                    }
                }
                _ => unreachable!(),
            }
        }
        OperationKind::ExactIntegerDivide {
            left,
            right,
            obligation,
        } => proof_binary(operation, obligation, left, right, ProofBinary::ExactDivide)?,
        OperationKind::ExactIntegerRemainder {
            left,
            right,
            obligation,
        } => proof_binary(
            operation,
            obligation,
            left,
            right,
            ProofBinary::ExactRemainder,
        )?,
        OperationKind::WrappingIntegerDivide {
            left,
            right,
            obligation,
        } => proof_binary(
            operation,
            obligation,
            left,
            right,
            ProofBinary::WrappingDivide,
        )?,
        OperationKind::WrappingIntegerRemainder {
            left,
            right,
            obligation,
        } => proof_binary(
            operation,
            obligation,
            left,
            right,
            ProofBinary::WrappingRemainder,
        )?,
        OperationKind::SaturatingIntegerDivide {
            left,
            right,
            obligation,
        } => proof_binary(
            operation,
            obligation,
            left,
            right,
            ProofBinary::SaturatingDivide,
        )?,
        OperationKind::SaturatingIntegerRemainder {
            left,
            right,
            obligation,
        } => proof_binary(
            operation,
            obligation,
            left,
            right,
            ProofBinary::SaturatingRemainder,
        )?,
        OperationKind::SaturatingIntegerAdd { left, right } => {
            saturating(operation, left, right, SaturatingBinary::Add)?
        }
        OperationKind::SaturatingIntegerSubtract { left, right } => {
            saturating(operation, left, right, SaturatingBinary::Subtract)?
        }
        OperationKind::SaturatingIntegerMultiply { left, right } => {
            saturating(operation, left, right, SaturatingBinary::Multiply)?
        }
        _ => unreachable!("arithmetic router is exhaustive"),
    })
}

#[derive(Clone, Copy)]
enum ProofBinary {
    ExactDivide,
    ExactRemainder,
    WrappingDivide,
    WrappingRemainder,
    SaturatingDivide,
    SaturatingRemainder,
}

fn proof_binary(
    operation: &Operation,
    obligation: psi_core::ObligationId,
    left: psi_core::ValueId,
    right: psi_core::ValueId,
    kind: ProofBinary,
) -> Result<AbstractOperation, LoweringError> {
    let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type else {
        return Err(match kind {
            ProofBinary::ExactDivide => LoweringError::VerifiedExactDivideMalformed(operation.id),
            ProofBinary::ExactRemainder => {
                LoweringError::VerifiedExactRemainderMalformed(operation.id)
            }
            ProofBinary::WrappingDivide => {
                LoweringError::VerifiedWrappingDivideMalformed(operation.id)
            }
            ProofBinary::WrappingRemainder => {
                LoweringError::VerifiedWrappingRemainderMalformed(operation.id)
            }
            ProofBinary::SaturatingDivide => {
                LoweringError::VerifiedSaturatingDivideMalformed(operation.id)
            }
            ProofBinary::SaturatingRemainder => {
                LoweringError::VerifiedSaturatingRemainderMalformed(operation.id)
            }
        });
    };
    let common = (
        operation.id,
        obligation,
        operation.result.expect_scalar().id,
        scalar_type,
        left,
        right,
    );
    Ok(match kind {
        ProofBinary::ExactDivide => AbstractOperation::ExactIntegerDivide {
            psi_operation: common.0,
            obligation: common.1,
            result: common.2,
            scalar_type: common.3,
            left: common.4,
            right: common.5,
        },
        ProofBinary::ExactRemainder => AbstractOperation::ExactIntegerRemainder {
            psi_operation: common.0,
            obligation: common.1,
            result: common.2,
            scalar_type: common.3,
            left: common.4,
            right: common.5,
        },
        ProofBinary::WrappingDivide => AbstractOperation::WrappingIntegerDivide {
            psi_operation: common.0,
            obligation: common.1,
            result: common.2,
            scalar_type: common.3,
            left: common.4,
            right: common.5,
        },
        ProofBinary::WrappingRemainder => AbstractOperation::WrappingIntegerRemainder {
            psi_operation: common.0,
            obligation: common.1,
            result: common.2,
            scalar_type: common.3,
            left: common.4,
            right: common.5,
        },
        ProofBinary::SaturatingDivide => AbstractOperation::SaturatingIntegerDivide {
            psi_operation: common.0,
            obligation: common.1,
            result: common.2,
            scalar_type: common.3,
            left: common.4,
            right: common.5,
        },
        ProofBinary::SaturatingRemainder => AbstractOperation::SaturatingIntegerRemainder {
            psi_operation: common.0,
            obligation: common.1,
            result: common.2,
            scalar_type: common.3,
            left: common.4,
            right: common.5,
        },
    })
}

#[derive(Clone, Copy)]
enum SaturatingBinary {
    Add,
    Subtract,
    Multiply,
}

fn saturating(
    operation: &Operation,
    left: psi_core::ValueId,
    right: psi_core::ValueId,
    kind: SaturatingBinary,
) -> Result<AbstractOperation, LoweringError> {
    let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type else {
        return Err(match kind {
            SaturatingBinary::Add => LoweringError::VerifiedSaturatingAddMalformed(operation.id),
            SaturatingBinary::Subtract => {
                LoweringError::VerifiedSaturatingSubtractMalformed(operation.id)
            }
            SaturatingBinary::Multiply => {
                LoweringError::VerifiedSaturatingMultiplyMalformed(operation.id)
            }
        });
    };
    let common = (
        operation.id,
        operation.result.expect_scalar().id,
        scalar_type,
        left,
        right,
    );
    Ok(match kind {
        SaturatingBinary::Add => AbstractOperation::SaturatingIntegerAdd {
            psi_operation: common.0,
            result: common.1,
            scalar_type: common.2,
            left: common.3,
            right: common.4,
        },
        SaturatingBinary::Subtract => AbstractOperation::SaturatingIntegerSubtract {
            psi_operation: common.0,
            result: common.1,
            scalar_type: common.2,
            left: common.3,
            right: common.4,
        },
        SaturatingBinary::Multiply => AbstractOperation::SaturatingIntegerMultiply {
            psi_operation: common.0,
            result: common.1,
            scalar_type: common.2,
            left: common.3,
            right: common.4,
        },
    })
}
