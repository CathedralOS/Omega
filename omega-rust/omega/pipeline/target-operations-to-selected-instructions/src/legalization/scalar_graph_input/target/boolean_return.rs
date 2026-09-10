//! Exact Boolean return expressions reuse the ordinary source value checker.
use super::*;
pub(super) fn uses(operation: &TargetOperation) -> bool {
    matches!(
        operation,
        TargetOperation::ReturnBooleanExpression { .. }
            | TargetOperation::ReturnBooleanImmediate { .. }
            | TargetOperation::ReturnBooleanParameter { .. }
            | TargetOperation::ReturnBooleanNotParameter { .. }
    )
}
pub(super) fn validate(
    target: &TargetFunction,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let Some(declaration) = abstracted.result.scalar() else {
        return Err(invalid);
    };
    if declaration.scalar_type != ScalarType::Boolean {
        return Err(invalid);
    }
    let checker = Checker {
        function: target,
        available: None,
        optimized,
        native,
        plan,
        unit,
    };
    let (edge, value) = match &target.operation {
        TargetOperation::ReturnBooleanExpression {
            psi_edge,
            source_value,
            ..
        }
        | TargetOperation::ReturnBooleanImmediate {
            psi_edge,
            source_value,
            ..
        }
        | TargetOperation::ReturnBooleanParameter {
            psi_edge,
            source_value,
            ..
        }
        | TargetOperation::ReturnBooleanNotParameter {
            psi_edge,
            source_value,
            ..
        } => (*psi_edge, *source_value),
        _ => return Err(invalid),
    };
    // Follow source entry and exact Jump bindings independently of block storage order.
    let mut block = optimized.entry;
    let mut path = Vec::new();
    let mut aliases = Vec::new();
    loop {
        if path.contains(&block) {
            return Err(invalid);
        }
        path.push(block);
        let source = optimized
            .blocks
            .iter()
            .find(|candidate| candidate.id == block)
            .ok_or(invalid.clone())?;
        let node = source.nodes.last().ok_or(invalid.clone())?;
        match &node.operation {
            AbstractOperation::Jump {
                target, bindings, ..
            } => {
                aliases = bind(&aliases, bindings);
                block = *target;
            }
            AbstractOperation::Return {
                psi_edge,
                result,
                value: source,
                scalar_type,
                cleanup_actions,
            } if *psi_edge == edge
                && *result == declaration.value
                && *source == value
                && *scalar_type == ScalarType::Boolean
                && cleanup_actions.is_empty() =>
            {
                let matches = match &target.operation {
                    TargetOperation::ReturnBooleanExpression { expression, .. } => {
                        checker.boolean(expression, value, &aliases)
                    }
                    TargetOperation::ReturnBooleanImmediate { value: literal, .. } => checker
                        .boolean(
                            &Boolean::Immediate {
                                source_value: value,
                                value: *literal,
                            },
                            value,
                            &aliases,
                        ),
                    TargetOperation::ReturnBooleanParameter {
                        parameter_index,
                        location,
                        ..
                    }
                    | TargetOperation::ReturnBooleanNotParameter {
                        parameter_index,
                        location,
                        ..
                    } => {
                        let parameter = checker
                            .scalar_parameters()
                            .get(*parameter_index)
                            .ok_or(invalid.clone())?;
                        checker
                            .parameter_predicate(value, &aliases, &mut Vec::new())
                            .is_some_and(|(base, inverted)| {
                                parameter.value == base
                                    && parameter.scalar_type == ScalarType::Boolean
                                    && location_matches(*location, &parameter.placement)
                                    && inverted
                                        == matches!(
                                            target.operation,
                                            TargetOperation::ReturnBooleanNotParameter { .. }
                                        )
                            })
                    }
                    _ => false,
                };
                return if matches { Ok(()) } else { Err(invalid) };
            }
            _ => return Err(invalid),
        }
    }
}
