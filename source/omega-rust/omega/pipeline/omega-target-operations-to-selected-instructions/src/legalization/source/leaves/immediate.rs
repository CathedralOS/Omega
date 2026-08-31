use super::super::shared::*;
use super::fuel::exact_operation_fuel;
use super::{DerivedValue, LeafContext};

pub(super) fn derive<'a>(
    context: &LeafContext<'a>,
    expression: &TargetIntegerExpression,
) -> Result<DerivedValue<'a>, LegalizationError> {
    let TargetIntegerExpression::Immediate { source_value, .. } = expression else {
        unreachable!("immediate catalog arm supplied the immediate derivation")
    };
    if context.nodes.len() != 2 || context.source_value != *source_value {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    }
    let immediate = derive_operand(
        context.function,
        context.arm_edge,
        expression,
        &context.nodes[0],
        context.u64_type,
    )?;
    Ok((
        &context.nodes[1],
        SourceLeafValue::Immediate {
            value: immediate.value,
            constant_operation: immediate.constant_operation,
            definition_site: immediate.definition_site,
            constant_fuel: immediate.fuel,
        },
    ))
}

pub(super) fn derive_operand(
    function: usize,
    arm_edge: EdgeId,
    target: &TargetIntegerExpression,
    node: &omega_optimization_unit::OptimizationNode,
    expected_type: ScalarType,
) -> Result<SourceImmediate, LegalizationError> {
    let TargetIntegerExpression::Immediate {
        source_value,
        value: target_value,
    } = target
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let AbstractOperation::IntegerConstant {
        psi_operation,
        result,
        scalar_type,
        value,
    } = &node.operation
    else {
        return Err(Error::MissingConstantDefinition { function, arm_edge });
    };
    if result != source_value
        || value != target_value
        || *scalar_type != expected_type
        || node.definitions.len() != 1
        || node.definitions[0].value != *source_value
        || node.provenance != vec![PsiProvenance::Operation(*psi_operation)]
    {
        return Err(Error::MissingConstantDefinition { function, arm_edge });
    }
    Ok(SourceImmediate {
        source_value: *source_value,
        value: *value,
        constant_operation: *psi_operation,
        definition_site: node.definitions[0].site,
        fuel: exact_operation_fuel(node, *psi_operation, function)?,
    })
}
