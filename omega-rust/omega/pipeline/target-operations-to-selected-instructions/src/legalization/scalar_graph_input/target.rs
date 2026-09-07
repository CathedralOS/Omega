use super::*;
use semantic_vocabulary::OperationId;
use target_operations::{
    ScalarParameterLocation, TargetIntegerExpression as Expression, TargetScalarExpression,
    TargetUnitOperation,
};
mod unit;
pub(super) fn validate_target(
    target: &TargetFunction,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let block = &optimized.blocks[0];
    let (returned, body) = block.nodes.split_last().ok_or(invalid.clone())?;
    let operations = body
        .iter()
        .map(|node| instruction(node).map(|value| value.0))
        .collect::<Option<Vec<_>>>()
        .ok_or(invalid.clone())?;
    let edge = match returned.operation {
        AbstractOperation::Return { psi_edge, .. }
        | AbstractOperation::ReturnUnit { psi_edge, .. } => psi_edge,
        _ => return Err(invalid),
    };
    if target.provenance.operations != operations || target.provenance.edges != [edge] {
        return Err(invalid);
    }
    if let TargetOperation::UnitBody(target_body) = &target.operation {
        return unit::validate(target_body, abstracted, optimized, native, plan, unit);
    }
    let AbstractOperation::Return { value, .. } = returned.operation else {
        return Err(invalid);
    };
    let mut seen_calls = Vec::new();
    let matches = match &target.operation {
        TargetOperation::ReturnIntegerImmediate {
            psi_edge,
            source_value,
            scalar_type,
            value: literal,
        } => {
            *psi_edge == edge
                && *source_value == value
                && *scalar_type == u64_type()
                && expression(
                    &Expression::Immediate {
                        source_value: *source_value,
                        value: *literal,
                    },
                    value,
                    target,
                    body,
                    native,
                    plan,
                    unit,
                    &mut seen_calls,
                )
        }
        TargetOperation::ReturnIntegerParameter {
            psi_edge,
            source_value,
            scalar_type,
            parameter_index,
            location,
        } => {
            *psi_edge == edge
                && *source_value == value
                && *scalar_type == u64_type()
                && expression(
                    &Expression::Parameter {
                        source_value: *source_value,
                        parameter_index: *parameter_index,
                        location: *location,
                    },
                    value,
                    target,
                    body,
                    native,
                    plan,
                    unit,
                    &mut seen_calls,
                )
        }
        TargetOperation::ReturnIntegerExpression {
            psi_edge,
            source_value,
            scalar_type,
            expression: expr,
        } => {
            *psi_edge == edge
                && *source_value == value
                && *scalar_type == u64_type()
                && expression(
                    expr,
                    value,
                    target,
                    body,
                    native,
                    plan,
                    unit,
                    &mut seen_calls,
                )
        }
        _ => false,
    };
    if !matches {
        return Err(invalid);
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn expression(
    expression: &Expression,
    value: ValueId,
    function: &TargetFunction,
    body: &[OptimizationNode],
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    seen: &mut Vec<OperationId>,
) -> bool {
    match expression {
        Expression::Immediate {source_value,value:literal} => *source_value == value && body.iter().any(|node|
            matches!(&node.operation,AbstractOperation::IntegerConstant {result,value:actual,..} if *result==value && actual==literal)),
        Expression::Parameter {source_value,parameter_index,location} => {
            let Some(parameter) = function.fixed_integer_scalar_abi.as_ref().and_then(|abi|abi.parameters.get(*parameter_index)) else {return false;};
            *source_value==value && parameter.value==value && location_matches(*location,&parameter.placement)
        },
        Expression::Call {psi_operation,source_value,callee,arguments,requirement_obligations,crash_continuations} => {
            let Some(node)=body.iter().find(|node|matches!(&node.operation,AbstractOperation::Call {psi_operation:actual,..} if actual==psi_operation)) else {return false;};
            let AbstractOperation::Call {result,callee:actual,arguments:sources,requirement_obligations:requirements,crash_continuations:crashes,..}=&node.operation else {return false;};
            let Ok(call_plan)=callee_plan(*callee,native,plan,unit) else {return false;};
            if *source_value!=value || *result!=value || actual!=callee || requirement_obligations!=requirements || crash_continuations!=crashes
                || arguments.len()!=sources.len() || arguments.len()!=call_plan.parameters.len() {return false;}
            if !seen.contains(psi_operation) {seen.push(*psi_operation);}
            arguments.iter().zip(sources).zip(&call_plan.parameters).all(|((argument,source),placement)| {
                let TargetScalarExpression::Integer {scalar_type,expression:child}=&argument.expression else {return false;};
                argument.scalar_type==ScalarType::Integer(u64_type()) && *scalar_type==u64_type()
                    && location_matches(argument.location,placement)
                    && self::expression(child,*source,function,body,native,plan,unit,seen)
            })
        },
        _=>false,
    }
}
fn location_matches(location: ScalarParameterLocation, placement: &ValuePlacement) -> bool {
    matches!(placement.locations.as_slice(),[ValueLocation::Register {register,value_byte_offset:0,byte_size:8}]
        if location==ScalarParameterLocation::Register(*register))
}
