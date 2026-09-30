//! Exact target-neutral IEEE scalar operations.

use abstract_operations::AbstractOperation;
use semantic_vocabulary::ScalarType;
use terminal_psi::{Operation, OperationKind};

use crate::lowering::LoweringError;

pub(super) fn lower(
    operation: &Operation,
    value_types: &std::collections::BTreeMap<semantic_vocabulary::ValueId, ScalarType>,
) -> Result<AbstractOperation, LoweringError> {
    let invalid = LoweringError::VerifiedIeeeFloatMalformed(operation.id);
    let result = operation.result.scalar().ok_or(invalid.clone())?;
    Ok(match operation.kind.clone() {
        OperationKind::FloatResultPolicy { policy } => {
            let ScalarType::IeeeFloat(format) = result.scalar_type else {
                return Err(invalid);
            };
            if policy
                .operands()
                .iter()
                .any(|operand| value_types.get(operand) != Some(&ScalarType::IeeeFloat(format)))
            {
                return Err(invalid);
            }
            AbstractOperation::FloatResultPolicy {
                psi_operation: operation.id,
                result: result.id,
                format,
                policy,
            }
        }
        OperationKind::IeeeFloatCompare {
            comparison,
            left,
            right,
        } => {
            let Some(ScalarType::IeeeFloat(format)) = value_types.get(&left).copied() else {
                return Err(invalid);
            };
            if result.scalar_type != ScalarType::Boolean
                || value_types.get(&right) != Some(&ScalarType::IeeeFloat(format))
            {
                return Err(invalid);
            }
            AbstractOperation::IeeeFloatCompare {
                psi_operation: operation.id,
                result: result.id,
                comparison,
                format,
                left,
                right,
            }
        }
        OperationKind::IeeeFloatConstant { value } => {
            if result.scalar_type != ScalarType::IeeeFloat(value.format()) {
                return Err(LoweringError::VerifiedIeeeFloatMalformed(operation.id));
            }
            AbstractOperation::IeeeFloatConstant {
                psi_operation: operation.id,
                result: result.id,
                value,
            }
        }
        OperationKind::NearestIeeeFloatFusedMultiplyAdd {
            left,
            right,
            addend,
        } => {
            let ScalarType::IeeeFloat(format) = result.scalar_type else {
                return Err(LoweringError::VerifiedIeeeFloatMalformed(operation.id));
            };
            AbstractOperation::NearestIeeeFloatFusedMultiplyAdd {
                psi_operation: operation.id,
                result: result.id,
                format,
                left,
                right,
                addend,
            }
        }
        _ => return Err(invalid),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IeeeFloatFormat, OperationId, ValueId};
    use terminal_psi::{FloatResultPolicy, FloatSaturationRule, OperationResult, ValueDeclaration};

    #[test]
    fn float_result_policy_lowering_retains_identity_and_rejects_mixed_carriers() {
        let value = |raw| ValueId::new(raw).expect("value");
        for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
            let scalar_type = ScalarType::IeeeFloat(format);
            for policy in [
                FloatResultPolicy::Trapping {
                    raw_result: value(3),
                },
                FloatResultPolicy::Saturating {
                    left: value(1),
                    right: value(2),
                    raw_result: value(3),
                    rule: FloatSaturationRule::FiniteOperandOverflow,
                },
                FloatResultPolicy::Saturating {
                    left: value(1),
                    right: value(2),
                    raw_result: value(3),
                    rule: FloatSaturationRule::FiniteDivisionOverflow,
                },
            ] {
                let operation = Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id: OperationId::new(8).expect("operation"),
                    result: OperationResult::Scalar(ValueDeclaration {
                        id: value(4),
                        scalar_type,
                        qualifications: Default::default(),
                    }),
                    kind: OperationKind::FloatResultPolicy { policy },
                };
                let value_types = policy
                    .operands()
                    .into_iter()
                    .map(|operand| (operand, scalar_type))
                    .collect();
                assert_eq!(
                    lower(&operation, &value_types).expect("lower policy"),
                    AbstractOperation::FloatResultPolicy {
                        psi_operation: operation.id,
                        result: value(4),
                        format,
                        policy,
                    }
                );
                for operand in policy.operands() {
                    let mut malformed = value_types.clone();
                    malformed.remove(&operand);
                    assert!(lower(&operation, &malformed).is_err());
                    malformed.insert(operand, ScalarType::Boolean);
                    assert!(lower(&operation, &malformed).is_err());
                    let other_format = match format {
                        IeeeFloatFormat::Binary32 => IeeeFloatFormat::Binary64,
                        IeeeFloatFormat::Binary64 => IeeeFloatFormat::Binary32,
                    };
                    malformed.insert(operand, ScalarType::IeeeFloat(other_format));
                    assert!(lower(&operation, &malformed).is_err());
                }
                let mut malformed = operation.clone();
                malformed.result = OperationResult::Unit;
                assert!(lower(&malformed, &value_types).is_err());
            }
        }
    }
}
