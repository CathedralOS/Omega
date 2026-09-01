use super::setup::PreparedScalarLowering;
use super::*;

pub(super) fn lower_conditional(
    function: &AbstractFunction,
    function_result: AbstractResult,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    prepared: &PreparedScalarLowering,
) -> Result<Option<TargetFunction>, LoweringError> {
    let values = prepared.values.clone();
    let call_plan = prepared.call_plan.clone();
    let target_structural_parameters = prepared.target_structural_parameters.clone();
    if function
        .operations
        .iter()
        .any(|operation| matches!(operation, AbstractOperation::Conditional { .. }))
    {
        if function.structural_parameters.is_empty() {
            if function.operations.iter().any(|operation| {
                matches!(operation,
                    AbstractOperation::Return { cleanup_actions, .. }
                        if !cleanup_actions.is_empty())
            }) {
                return Err(LoweringError::UnsupportedOperationInScalarFunction(
                    function.machine,
                ));
            }
            return match function_result.scalar_type {
                ScalarType::Integer(_) => {
                    lower_integer_conditional(function, &values, target, functions)
                }
                ScalarType::Boolean => {
                    lower_boolean_conditional(function, &values, target, functions)
                }
                ScalarType::IeeeFloat(_) => {
                    return Err(LoweringError::UnsupportedOperationInScalarFunction(
                        function.machine,
                    ));
                }
            }
            .map(Some);
        }
        if function_result.scalar_type != ScalarType::Boolean {
            return Err(LoweringError::UnsupportedOperationInScalarFunction(
                function.machine,
            ));
        }
        let lowered = lower_boolean_block(
            function,
            values,
            function.entry,
            BTreeSet::new(),
            target,
            functions,
            &target_structural_parameters,
            structural_types,
        )?;
        if let Some(return_edges) = shared_boolean_cleanup_return_edges(&lowered.control) {
            let cleanup_actions = uniform_conditional_cleanup(
                function,
                &return_edges,
                &target_structural_parameters,
                functions,
                structural_types,
            )?;
            let cleanup_edge = return_edges[0];
            return Ok(Some(TargetFunction {
                machine: function.machine,
                attachment: function.attachment,
                fixed_integer_scalar_abi: None,
                provenance: conditional_provenance(function, lowered.operations, lowered.edges),
                operation: TargetOperation::ScalarReturnWithCleanup {
                    scalar: Box::new(TargetOperation::ReturnBooleanSharedConvergence {
                        return_edges,
                        psi_edge: cleanup_edge,
                        control: lowered.control,
                    }),
                    structural_types: structural_types
                        .values()
                        .map(|declaration| (*declaration).clone())
                        .collect(),
                    call_plan,
                    structural_parameters: target_structural_parameters,
                    cleanup_actions,
                    psi_edge: cleanup_edge,
                },
            }));
        }
        let return_edges = finite_boolean_cleanup_return_edges(&lowered.control).ok_or(
            LoweringError::UnsupportedOperationInScalarFunction(function.machine),
        )?;
        let cleanup_actions = uniform_conditional_cleanup(
            function,
            &return_edges,
            &target_structural_parameters,
            functions,
            structural_types,
        )?;
        return Ok(Some(TargetFunction {
            machine: function.machine,
            attachment: function.attachment,
            fixed_integer_scalar_abi: None,
            provenance: conditional_provenance(function, lowered.operations, lowered.edges),
            operation: TargetOperation::BooleanControlWithCleanup {
                control: lowered.control,
                structural_types: structural_types
                    .values()
                    .map(|declaration| (*declaration).clone())
                    .collect(),
                call_plan,
                structural_parameters: target_structural_parameters,
                cleanup_actions,
            },
        }));
    }

    Ok(None)
}
