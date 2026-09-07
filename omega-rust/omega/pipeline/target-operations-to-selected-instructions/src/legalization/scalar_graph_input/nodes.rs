//! Input row and control contracts. Whole-unit validation owns SSA and effect-chain validity.
use super::*;
use optimization_unit::OptimizationBlock;
use semantic_vocabulary::OperationId;
pub(in crate::legalization) fn instruction(
    node: &OptimizationNode,
) -> Option<(OperationId, ValueId)> {
    match &node.operation {
        AbstractOperation::ByteSequenceRead {
            psi_operation,
            result,
            ..
        } if result.scalar_type == ScalarType::Integer(u8_type()) => {
            Some((*psi_operation, result.value))
        }
        AbstractOperation::ByteSequenceLength {
            psi_operation,
            result,
            ..
        } if result.scalar_type == ScalarType::Integer(u64_type()) => {
            Some((*psi_operation, result.value))
        }
        AbstractOperation::IntegerConstant {
            psi_operation,
            result,
            scalar_type,
            value,
        } if valid_literal(*scalar_type, *value) => Some((*psi_operation, *result)),
        AbstractOperation::Call {
            psi_operation,
            result,
            scalar_type,
            requirement_obligations,
            crash_continuations,
            ..
        } if *scalar_type == ScalarType::Integer(u64_type())
            && requirement_obligations.is_empty()
            && crash_continuations.is_empty() =>
        {
            Some((*psi_operation, *result))
        }
        AbstractOperation::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | AbstractOperation::ExactIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            ..
        } if *scalar_type == u64_type()
            || *scalar_type == u8_type()
            || *scalar_type == u32_type() =>
        {
            Some((*psi_operation, *result))
        }
        AbstractOperation::IntegerEqual {
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
        } => Some((*psi_operation, *result)),
        AbstractOperation::BooleanNot {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::IntegerWiden {
            psi_operation,
            result,
            ..
        } => Some((*psi_operation, *result)),
        _ => None,
    }
}
fn valid_literal(scalar: ScalarType, value: semantic_vocabulary::IntegerValue) -> bool {
    matches!((scalar,value),
        (ScalarType::Integer(integer),semantic_vocabulary::IntegerValue::Unsigned(value))
            if (integer == u64_type() && value <= u128::from(u64::MAX)) || (integer == u8_type() && value <= u128::from(u8::MAX)) || (integer == u32_type() && value <= u128::from(u32::MAX)))
        || matches!((scalar,value),
            (ScalarType::Integer(integer),semantic_vocabulary::IntegerValue::Signed(value))
                if integer == i64_type() && i64::try_from(value).is_ok())
}
pub(super) fn validate(
    block: &OptimizationBlock,
    optimized: &PsiOptimizationFunction,
    ranked: bool,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    super::boolean::validate(block, optimized)?;
    let (terminator, body) = block.nodes.split_last().ok_or(invalid.clone())?;
    for (position, parameter) in block.parameters.iter().enumerate() {
        if integer_type(parameter.scalar_type).is_none()
            || parameter.site
                != (ValueDefinitionSite::BlockParameter {
                    block: block.id,
                    position: position as u32,
                })
        {
            return Err(invalid);
        }
    }
    for (position, node) in body.iter().enumerate() {
        let (operation, result) = instruction(node).ok_or(invalid.clone())?;
        let [definition] = node.definitions.as_slice() else {
            return Err(invalid);
        };
        if definition.value != result
            || definition.site
                != (ValueDefinitionSite::Node {
                    block: block.id,
                    node: position as u32,
                })
            || !node.successors.is_empty()
            || node.provenance != [PsiProvenance::Operation(operation)]
            || node.fuel.is_empty()
            || node
                .fuel
                .iter()
                .any(|fuel| fuel.site != PsiProvenance::Operation(operation))
        {
            return Err(invalid);
        }
        let expected_type = match &node.operation {
            AbstractOperation::ByteSequenceRead {
                source,
                index,
                length,
                ..
            } => {
                if !optimized
                    .structural_parameters
                    .iter()
                    .any(|parameter| parameter.place == *source)
                    || value_type(optimized, *index) != Some(ScalarType::Integer(u64_type()))
                    || value_type(optimized, *length) != Some(ScalarType::Integer(u64_type()))
                {
                    return Err(invalid);
                }
                ScalarType::Integer(u8_type())
            }
            AbstractOperation::ByteSequenceLength { source, .. } => {
                if !optimized
                    .structural_parameters
                    .iter()
                    .any(|parameter| parameter.place == *source)
                {
                    return Err(invalid);
                }
                ScalarType::Integer(u64_type())
            }
            AbstractOperation::IntegerConstant { scalar_type, .. }
            | AbstractOperation::Call { scalar_type, .. } => *scalar_type,
            AbstractOperation::ExactIntegerAdd { scalar_type, .. }
            | AbstractOperation::ExactIntegerSubtract { scalar_type, .. } => {
                ScalarType::Integer(*scalar_type)
            }
            AbstractOperation::IntegerEqual { left, right, .. }
            | AbstractOperation::IntegerLessThan { left, right, .. }
            | AbstractOperation::IntegerLessOrEqual { left, right, .. } => {
                if value_type(optimized, *left)
                    .and_then(integer_type)
                    .is_none()
                    || value_type(optimized, *left) != value_type(optimized, *right)
                {
                    return Err(invalid);
                }
                ScalarType::Boolean
            }
            AbstractOperation::BooleanNot { operand, .. } => {
                if value_type(optimized, *operand) != Some(ScalarType::Boolean) {
                    return Err(invalid);
                }
                ScalarType::Boolean
            }
            AbstractOperation::IntegerWiden {
                operand,
                source_type,
                target_type,
                ..
            } => {
                if *source_type != u8_type()
                    || *target_type != u64_type()
                    || value_type(optimized, *operand) != Some(ScalarType::Integer(*source_type))
                {
                    return Err(invalid);
                }
                ScalarType::Integer(*target_type)
            }
            _ => return Err(invalid),
        };
        if definition.scalar_type != expected_type {
            return Err(invalid);
        }
    }
    super::control::validate(terminator, body, optimized, ranked)
}
