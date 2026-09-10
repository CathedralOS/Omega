//! Rejoin aggregate constructors and result calls to their exact source operations.
use super::*;
use crate::legalization::scalar_graph_input;

pub(super) fn validate(
    target: &TargetUnitOperation,
    abstracted: &AbstractOperation,
    sources: &[(ValueId, Source)],
    optimized: &PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    match (target, abstracted) {
        (
            TargetUnitOperation::EstablishScalarArray {
                psi_operation,
                result_home,
                elements,
            },
            AbstractOperation::EstablishScalarArray {
                psi_operation: expected_operation,
                result,
                elements: expected_elements,
            },
        ) => {
            let (scalar_type, count, _) = scalar_graph_input::scalar_arrays::shape(result, plan)?;
            if psi_operation != expected_operation
                || elements != expected_elements
                || u64::try_from(elements.len()).ok() != Some(count)
                || *result_home
                    != scalar_graph_input::aggregate_results::result_home(
                        optimized,
                        result.place,
                        plan,
                    )?
                || elements.iter().any(|element| {
                    !sources.iter().any(|(value, source)| {
                        value == element && source.scalar_type() == scalar_type
                    })
                })
            {
                return Err(invalid);
            }
        }
        (
            TargetUnitOperation::EstablishScalarCase {
                psi_operation,
                result_home,
                result_case,
                fields,
            },
            AbstractOperation::EstablishScalarCase {
                psi_operation: expected_operation,
                result,
                result_case: expected_case,
                fields: expected_fields,
            },
        ) => {
            if psi_operation != expected_operation
                || result_case != expected_case
                || fields != expected_fields
                || *result_home
                    != scalar_graph_input::aggregate_results::result_home(
                        optimized,
                        result.place,
                        plan,
                    )?
            {
                return Err(invalid);
            }
            let declaration = plan
                .structural_types
                .iter()
                .find(|declaration| declaration.id == result.structural_type)
                .ok_or(invalid.clone())?;
            let terminal_psi::StructuralTypeShape::Sum { cases } = &declaration.shape else {
                return Err(invalid);
            };
            let case = cases
                .iter()
                .find(|case| case.id == *result_case)
                .ok_or(invalid.clone())?;
            if fields.len() != case.fields.len() {
                return Err(invalid);
            }
            for (field, declared) in fields.iter().zip(&case.fields) {
                if field.field != declared.id || !sources.iter().any(|(value, source)| *value == field.value && declared.field_type.scalar_type() == Some(source.scalar_type()))
                    || matches!(declared.field_type, terminal_psi::StructuralFieldType::BoundedInteger(_)) != field.range_obligation.is_some()
                    || field.range_obligation.is_some_and(|obligation| !unit.accepted_obligation_facts.iter().any(|fact|
                        fact.machine == optimized.machine && fact.operation == *psi_operation && fact.obligation == obligation)
                        || !optimized.facts.iter().any(|fact| matches!(fact,
                            optimization_unit::OptimizationFact::OperationObligationReference { obligation: retained, support }
                            if *retained == obligation && support == psi_operation)))
                { return Err(invalid); }
            }
        }
        (
            TargetUnitOperation::StructuralResultCall {
                psi_operation,
                result,
                callee,
                callee_result,
                result_home,
                call_plan,
                scalar_arguments,
                arguments,
                claim_transfers,
                returned_claim_transfers,
                requirement_obligations,
                crash_continuations,
            },
            AbstractOperation::CallStructural {
                psi_operation: expected_operation,
                result: expected_result,
                callee: expected_callee,
                arguments: values,
                structural_arguments,
                claim_transfers: expected_claims,
                returned_claim_transfers: expected_returns,
                requirement_obligations: expected_requirements,
                crash_continuations: expected_crashes,
                selected_evidence,
            },
        ) => {
            let called = unit
                .functions
                .iter()
                .find(|function| function.machine == *callee)
                .ok_or(invalid.clone())?;
            let expected_plan = scalar_graph_input::callee_plan(*callee, native, plan, unit)?;
            if psi_operation != expected_operation
                || result != expected_result
                || callee != expected_callee
                || called.result.structural() != Some(callee_result)
                || result.structural_type != callee_result.structural_type
                || result.multiplicity != callee_result.multiplicity
                || result_home.as_ref()
                    != Some(&scalar_graph_input::aggregate_results::result_home(
                        optimized,
                        result.place,
                        plan,
                    )?)
                || *call_plan != expected_plan
                || !claim_transfers.is_empty()
                || !expected_claims.is_empty()
                || !returned_claim_transfers.is_empty()
                || !expected_returns.is_empty()
                || !requirement_obligations.is_empty()
                || !expected_requirements.is_empty()
                || !crash_continuations.is_empty()
                || !expected_crashes.is_empty()
                || !selected_evidence.is_empty()
                || values.len() != called.parameters.len()
                || scalar_arguments.len() != values.len()
                || structural_arguments.len() != called.structural_parameters.len()
                || arguments.len() != structural_arguments.len()
            {
                return Err(invalid);
            }
            for (position, ((value, parameter), argument)) in values
                .iter()
                .zip(&called.parameters)
                .zip(scalar_arguments)
                .enumerate()
            {
                if argument.parameter_index as usize != position
                    || argument.placement != expected_plan.parameters[position]
                    || argument.source.scalar_type() != parameter.scalar_type
                    || !sources
                        .iter()
                        .any(|(identity, source)| identity == value && *source == argument.source)
                {
                    return Err(invalid);
                }
            }
            for (position, (semantic, retained)) in
                structural_arguments.iter().zip(arguments).enumerate()
            {
                if *retained
                    != scalar_graph_input::aggregate_results::call_argument(
                        semantic,
                        position,
                        *psi_operation,
                        optimized,
                        called,
                        &expected_plan,
                        native,
                        plan,
                    )?
                {
                    return Err(invalid);
                }
            }
        }
        _ => return Err(invalid),
    }
    Ok(())
}
