//! Native-callback argument admission beside the lowering coordinator: bind
//! each admitted registrar row to its exact boundary call and require the
//! lowered target roster to retain it.

use crate::LoweringError;
use abstract_operations::{AbstractOperation, AbstractOperationPlan};
use calling_conventions::{CallSignature, NativePlace, ValueShape};
use semantic_vocabulary::OperationId;
use std::collections::BTreeMap;
use target::NativeTarget;
use target_operations::{TargetOperationPlan, TargetUnitOperation};

pub(crate) fn bind_native_callback_arguments(
    plan: &AbstractOperationPlan,
    target: NativeTarget,
    admitted_rows: &[crate::AdmittedNativeCallbackArgument],
) -> Result<
    BTreeMap<OperationId, Vec<target_operations::TargetNativeCallbackArgument>>,
    LoweringError,
> {
    let mut native_callbacks_by_operation: BTreeMap<
        OperationId,
        Vec<target_operations::TargetNativeCallbackArgument>,
    > = BTreeMap::new();
    for admitted in admitted_rows {
        // One operation may register several private callbacks; the placement
        // index — the exact retained roster row — is the custody key.
        if admitted_rows
            .iter()
            .filter(|row| {
                row.terminal_operation == admitted.terminal_operation
                    && row.placement_index == admitted.placement_index
            })
            .count()
            != 1
        {
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
        native_callbacks_by_operation
            .entry(admitted.terminal_operation)
            .or_default()
            .push(target_operations::TargetNativeCallbackArgument {
                terminal_operation: admitted.terminal_operation,
                placement_index: admitted.placement_index,
                callback_function: admitted.callback_function,
                destination: admitted.destination.clone(),
                field_byte_offset: admitted.field_byte_offset,
                application: admitted.application.clone(),
                registrar_boundary_entry_plan: admitted.registrar_boundary_entry_plan.clone(),
                registrar_context: admitted.registrar_context.clone(),
                registrar_application_commitment: admitted.registrar_application_commitment,
            });
    }
    for callbacks in native_callbacks_by_operation.values_mut() {
        callbacks.sort_by_key(|callback| callback.placement_index);
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
    if validated.plan() != &admitted.registrar_boundary_entry_plan
        || application.shape != application.placement.shape
        || Some(application.shape) != expected_shape
        || admitted
            .registrar_boundary_entry_plan
            .call
            .parameters
            .get(ordinal)
            != Some(&application.placement)
    {
        return false;
    }
    // The admitted destination joins exactly one plan materialization, one
    // binder, and one demand; any other pairing is substituted custody.
    let materializations = admitted
        .registrar_boundary_entry_plan
        .call
        .callback_materializations
        .iter()
        .filter(|row| row.destination == admitted.destination)
        .collect::<Vec<_>>();
    let [materialization] = materializations.as_slice() else {
        return false;
    };
    let binders = admitted
        .registrar_context
        .binders
        .iter()
        .filter(|binder| binder.binder == materialization.binder)
        .collect::<Vec<_>>();
    let [binder] = binders.as_slice() else {
        return false;
    };
    let demands = admitted
        .registrar_context
        .demands
        .iter()
        .filter(|demand| {
            demand.destination == materialization.destination
                && demand.requirement == binder.requirement
        })
        .collect::<Vec<_>>();
    if demands.len() != 1 {
        return false;
    }
    match &admitted.destination {
        NativePlace::Parameter(parameter) => {
            admitted.field_byte_offset.is_none() && *parameter == application.parameter
        }
        NativePlace::Field { parameter, .. } => {
            // A field destination's application is its root record
            // parameter's — the write flows through that parameter's
            // placement to the slot `field_byte_offset` names.
            admitted.field_byte_offset.is_some() && *parameter == application.parameter
        }
    }
}

pub(crate) fn validate_native_callback_target_rows(
    plan: &TargetOperationPlan,
    expected: &BTreeMap<OperationId, Vec<target_operations::TargetNativeCallbackArgument>>,
) -> Result<(), LoweringError> {
    for (operation, callbacks) in expected {
        let matches = plan
            .functions
            .iter()
            .flat_map(|function| &function.graph.blocks)
            .flat_map(|block| &block.operations)
            .filter(|candidate| {
                matches!(candidate,
                TargetUnitOperation::NormalizedForeignCall { psi_operation, binding, .. }
                    if psi_operation == operation
                        && callbacks.iter().all(|callback| {
                            binding.boundary_entry_plan
                                == callback.registrar_boundary_entry_plan
                        }))
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
