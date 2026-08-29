//! Terminal-edge lowering for straight-line scalar functions.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_exit(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    function_result: AbstractResult,
    values: &BTreeMap<ValueId, KnownScalar>,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    call_plan: &CallPlan,
    target_structural_parameters: &[TargetStructuralParameter],
    provenance: &mut TerminalPsiProvenance,
    returned: &mut Option<TargetOperation>,
) -> Result<(), LoweringError> {
    match operation {
        AbstractOperation::Crash {
            psi_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        } => {
            provenance.edges.push(*psi_edge);
            *returned = Some(TargetOperation::Crash {
                psi_edge: *psi_edge,
                cause: *cause,
                site_guard: site_guard.clone(),
                frontier_lower_bound: frontier_lower_bound.clone(),
            });
        }
        AbstractOperation::Return {
            psi_edge,
            result,
            value,
            scalar_type,
            cleanup_actions,
        } => {
            if *result != function_result.value || *scalar_type != function_result.scalar_type {
                return Err(LoweringError::FunctionResultMismatch(function.machine));
            }
            let returned_value = values
                .get(value)
                .cloned()
                .ok_or(LoweringError::UnknownValue(*value))?;
            if *scalar_type != returned_value.scalar_type() {
                return Err(LoweringError::ValueTypeMismatch(*result));
            }
            provenance.edges.push(*psi_edge);
            let scalar = lower_scalar_return(*psi_edge, *value, returned_value);
            if cleanup_actions.is_empty() {
                *returned = Some(scalar);
            } else {
                validate_scalar_cleanup_frontier(
                    function.machine,
                    cleanup_actions,
                    target_structural_parameters,
                    functions,
                    structural_types,
                )?;
                *returned = Some(TargetOperation::ScalarReturnWithCleanup {
                    scalar: Box::new(scalar),
                    structural_types: structural_types
                        .values()
                        .map(|declaration| (*declaration).clone())
                        .collect(),
                    call_plan: call_plan.clone(),
                    structural_parameters: target_structural_parameters.to_vec(),
                    cleanup_actions: cleanup_actions.clone(),
                    psi_edge: *psi_edge,
                });
            }
        }
        AbstractOperation::ReturnUnit { .. } | AbstractOperation::ReturnStructural { .. } => {
            return Err(LoweringError::FunctionResultKindMismatch(function.machine));
        }
        _ => unreachable!("exit routing admits only terminal operations"),
    }
    Ok(())
}

fn lower_scalar_return(
    psi_edge: EdgeId,
    source_value: ValueId,
    returned_value: KnownScalar,
) -> TargetOperation {
    match returned_value {
        KnownScalar::Boolean(value) => TargetOperation::ReturnBooleanImmediate {
            psi_edge,
            source_value,
            value,
        },
        KnownScalar::Integer {
            scalar_type,
            value: KnownInteger::Immediate(value),
        } => TargetOperation::ReturnIntegerImmediate {
            psi_edge,
            source_value,
            scalar_type,
            value,
        },
        KnownScalar::Integer {
            scalar_type,
            value:
                KnownInteger::Runtime(TargetIntegerExpression::Parameter {
                    parameter_index,
                    location,
                    ..
                }),
        } => TargetOperation::ReturnIntegerParameter {
            psi_edge,
            source_value,
            scalar_type,
            parameter_index,
            location,
        },
        KnownScalar::Integer {
            scalar_type,
            value: KnownInteger::Runtime(expression),
        } => TargetOperation::ReturnIntegerExpression {
            psi_edge,
            source_value,
            scalar_type,
            expression,
        },
        KnownScalar::BooleanRuntime(TargetBooleanExpression::Parameter {
            parameter_index,
            location,
            ..
        }) => TargetOperation::ReturnBooleanParameter {
            psi_edge,
            source_value,
            parameter_index,
            location,
        },
        KnownScalar::BooleanRuntime(TargetBooleanExpression::Not { operand, .. })
            if matches!(*operand, TargetBooleanExpression::Parameter { .. }) =>
        {
            let TargetBooleanExpression::Parameter {
                parameter_index,
                location,
                ..
            } = *operand
            else {
                unreachable!("guard requires a parameter operand")
            };
            TargetOperation::ReturnBooleanNotParameter {
                psi_edge,
                source_value,
                parameter_index,
                location,
            }
        }
        KnownScalar::BooleanRuntime(expression) => TargetOperation::ReturnBooleanExpression {
            psi_edge,
            source_value,
            expression,
        },
    }
}
