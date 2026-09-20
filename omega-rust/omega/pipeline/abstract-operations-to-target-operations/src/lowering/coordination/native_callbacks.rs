//! Native-callback argument admission beside the lowering coordinator: bind
//! each admitted registrar row to its exact boundary call and require the
//! lowered target roster to retain it.

use super::super::shared::*;

pub(crate) fn bind_native_callback_arguments(
    plan: &AbstractOperationPlan,
    target: NativeTarget,
    admitted_rows: &[crate::AdmittedNativeCallbackArgument],
) -> Result<BTreeMap<OperationId, target_operations::TargetNativeCallbackArgument>, LoweringError> {
    if admitted_rows.len() > 1 {
        for (index, admitted) in admitted_rows.iter().enumerate() {
            if admitted_rows[..index]
                .iter()
                .any(|prior| prior.terminal_operation == admitted.terminal_operation)
            {
                return Err(LoweringError::DuplicateNativeCallbackArgument(
                    admitted.terminal_operation,
                ));
            }
        }
        return Err(LoweringError::MultipleNativeCallbackArguments);
    }
    let mut native_callbacks_by_operation = BTreeMap::new();
    for admitted in admitted_rows {
        if native_callbacks_by_operation.contains_key(&admitted.terminal_operation) {
            return Err(LoweringError::DuplicateNativeCallbackArgument(
                admitted.terminal_operation,
            ));
        }
        let matching_calls = plan
            .functions
            .iter()
            .flat_map(|function| &function.operations)
            .filter(|operation| {
                matches!(operation,
                    AbstractOperation::BoundaryCall { psi_operation, .. }
                        if *psi_operation == admitted.terminal_operation)
            })
            .count();
        if matching_calls != 1 {
            return Err(LoweringError::UnknownNativeCallbackArgument(
                admitted.terminal_operation,
            ));
        }
        if admitted.callback_function.callback_thunk_placement_index()
            != Some(admitted.placement_index)
            || !admitted.callback_function.is_valid()
            || admitted.registrar_application_commitment == [0; 32]
            || !native_callback_application_is_exact(admitted, target)
        {
            return Err(LoweringError::InvalidNativeCallbackArgument(
                admitted.terminal_operation,
            ));
        }
        native_callbacks_by_operation.insert(
            admitted.terminal_operation,
            target_operations::TargetNativeCallbackArgument {
                terminal_operation: admitted.terminal_operation,
                placement_index: admitted.placement_index,
                callback_function: admitted.callback_function,
                application: admitted.application.clone(),
                registrar_boundary_entry_plan: admitted.registrar_boundary_entry_plan.clone(),
                registrar_context: admitted.registrar_context.clone(),
                registrar_application_commitment: admitted.registrar_application_commitment,
            },
        );
    }
    Ok(native_callbacks_by_operation)
}

fn native_callback_application_is_exact(
    admitted: &crate::AdmittedNativeCallbackArgument,
    target: NativeTarget,
) -> bool {
    let application = &admitted.application;
    let Ok(ordinal) = usize::try_from(application.native_ordinal) else {
        return false;
    };
    let expected_shape = u16::try_from(target.pointer_size)
        .ok()
        .zip(u16::try_from(target.pointer_alignment).ok())
        .map(|(size, alignment)| ValueShape::integer(size, alignment));
    let signature = CallSignature {
        parameters: admitted
            .registrar_boundary_entry_plan
            .call
            .parameters
            .iter()
            .map(|placement| placement.shape)
            .collect(),
        result: admitted
            .registrar_boundary_entry_plan
            .call
            .result
            .as_ref()
            .map(|result| result.shape),
    };
    let Ok(validated) =
        calling_conventions::validate_boundary_entry_plan_with_callback_materializations(
            admitted.registrar_boundary_entry_plan.clone(),
            &signature,
            &admitted.registrar_context,
        )
    else {
        return false;
    };
    let ([binder], [demand], [materialization]) = (
        admitted.registrar_context.binders.as_slice(),
        admitted.registrar_context.demands.as_slice(),
        admitted
            .registrar_boundary_entry_plan
            .call
            .callback_materializations
            .as_slice(),
    ) else {
        return false;
    };
    validated.plan() == &admitted.registrar_boundary_entry_plan
        && application.shape == application.placement.shape
        && Some(application.shape) == expected_shape
        && admitted
            .registrar_boundary_entry_plan
            .call
            .parameters
            .get(ordinal)
            == Some(&application.placement)
        && demand.destination == calling_conventions::NativePlace::Parameter(application.parameter)
        && materialization.destination == demand.destination
        && materialization.binder == binder.binder
        && binder.requirement == demand.requirement
}

pub(crate) fn validate_native_callback_target_rows(
    plan: &TargetOperationPlan,
    expected: &BTreeMap<OperationId, target_operations::TargetNativeCallbackArgument>,
) -> Result<(), LoweringError> {
    for (operation, callback) in expected {
        let matches = plan
            .functions
            .iter()
            .flat_map(|function| &function.graph.blocks)
            .flat_map(|block| &block.operations)
            .filter(|candidate| {
                matches!(candidate,
                    TargetUnitOperation::NormalizedForeignCall { psi_operation, binding, .. }
                        if psi_operation == operation
                            && binding.boundary_entry_plan
                                == callback.registrar_boundary_entry_plan)
            })
            .count();
        if matches == 0 {
            return Err(LoweringError::UnusedNativeCallbackArgument(*operation));
        }
        if matches != 1 {
            return Err(LoweringError::InvalidNativeCallbackArgument(*operation));
        }
    }
    Ok(())
}
