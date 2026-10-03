//! Replay one legalized normalized foreign call against the abstract boundary
//! call it legalizes and the exact target row it must carry.
//!
//! The admitted provider execution, typed locator binding, and evaluated plan
//! are custody evidence: replay rejoins the unique target row and requires the
//! legalized payload to carry it exactly, while every derivable coordinate —
//! scalar argument order, placement, source identity, structural projection,
//! and the completion result home — is reconstructed independently from
//! the boundary declaration and the claimed plan. A substituted binding,
//! provider, argument, or result home fails closed.
use super::super::{Error, PsiOptimizationUnit, TargetOperationPlan};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input;
use abstract_operations::AbstractOperation;
use abstract_operations::AbstractOperationPlan;
use calling_conventions::{CallSignature, ValueLocation};
use legalized_operations::LegalizedScalarInstruction;
use semantic_vocabulary::OperationId;
use target_operations::{TargetBoundaryResult, TargetUnitOperation};

#[allow(clippy::too_many_arguments)]
pub(super) fn validate(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    operation: OperationId,
) -> Result<(), LegalizationError> {
    let invalid = Error::NonCanonicalLegalizedPlan;
    let (
        legalized_operations::LegalizedScalarInstructionKind::NormalizedForeignCall(call),
        AbstractOperation::BoundaryCall {
            boundary,
            result,
            arguments,
            structural_arguments,
            completion_claim_sources,
            completion_receipts,
            ..
        },
    ) = (&actual.kind, &node.operation)
    else {
        unreachable!("dispatched normalized foreign replay")
    };
    // The exact lowered row is the sole custody carrier of the admitted
    // execution and evaluated binding; the legalized payload must equal them,
    // never a different realization.
    let Some(TargetUnitOperation::NormalizedForeignCall {
        psi_operation,
        boundary: row_boundary,
        provider_execution,
        binding,
        scalar_arguments,
        structural_arguments: row_structural,
        result: row_result,
    }) = scalar_graph_input::normalized_foreign::row(native, optimized.machine, operation)?
    else {
        return Err(invalid);
    };
    let function = native
        .functions
        .iter()
        .find(|function| function.machine == optimized.machine)
        .ok_or(invalid.clone())?;
    let mut declarations = plan
        .boundary_machines
        .iter()
        .filter(|row| row.id == call.boundary);
    let declaration = declarations.next().ok_or(invalid.clone())?;
    if unit
        .boundary_machines
        .iter()
        .find(|row| row.id == call.boundary)
        != Some(declaration)
    {
        return Err(invalid);
    }
    let scalar_shapes = declaration
        .scalar_parameters
        .iter()
        .map(|parameter| scalar_graph_input::scalar_shape(*parameter).ok_or(invalid.clone()))
        .collect::<Result<Vec<_>, LegalizationError>>()?;
    // Lane-local custody rejoins the declaration's retained authored order.
    if !declaration.has_valid_parameter_order()
        || structural_arguments.len() != declaration.structural_parameters.len()
        || arguments.len() != declaration.scalar_parameters.len()
    {
        return Err(invalid);
    }
    // Each structural argument is re-derived from the boundary declaration and
    // the claimed plan position; the legalized row cannot substitute a place,
    // path, projected type, offset, or destination.
    let callbacks = scalar_graph_input::normalized_foreign::native_callbacks_at(
        native,
        operation,
        &call.binding.boundary_entry_plan,
    )?;
    // Only parameter-destination callbacks occupy private plan slots and
    // shift authored positions; field destinations name slots inside a
    // record an authored argument already carries.
    let mut private_ordinals = Vec::new();
    for callback in &callbacks {
        if matches!(
            callback.destination,
            calling_conventions::NativePlace::Parameter(_)
        ) {
            private_ordinals.push(
                usize::try_from(callback.application.native_ordinal)
                    .map_err(|_| invalid.clone())?,
            );
        }
    }
    private_ordinals.sort_unstable();
    let derived_structural = structural_arguments
        .iter()
        .zip(&declaration.structural_parameters)
        .enumerate()
        .zip(declaration.parameter_positions(terminal_psi::BoundaryParameterKind::Structural))
        .map(
            |((index, (semantic, declaration_parameter)), semantic_position)| {
                let native_position = semantic_position
                    + private_ordinals
                        .iter()
                        .filter(|ordinal| **ordinal <= semantic_position)
                        .count();
                scalar_graph_input::normalized_foreign::structural_argument_at(
                    semantic,
                    index,
                    declaration_parameter,
                    call.binding
                        .boundary_entry_plan
                        .call
                        .parameters
                        .get(native_position),
                    &function.graph.parameters,
                    optimized,
                    native.target,
                    call.binding.boundary_entry_plan.call.policy,
                    plan,
                    operation,
                )
            },
        )
        .collect::<Result<Vec<_>, LegalizationError>>()?;
    let (expected_result, result_shape) = scalar_graph_input::normalized_foreign::result_at(
        result,
        &declaration.result,
        operation,
        optimized,
        plan,
        call.binding.boundary_entry_plan.call.policy,
    )
    .map_err(|_| invalid.clone())?;
    // A retained callback occupies one native-only parameter slot in the
    // registrar plan; the validated signature then spells every authored and
    // private placement while the declaration still counts only semantic
    // formals. The plan's retained roster supplies the binder/demand context
    // the materialized signature replays against.
    let mut scalars = scalar_shapes.iter().copied();
    let mut structures = derived_structural
        .iter()
        .map(|argument| argument.destination.shape);
    let mut parameters = declaration
        .parameter_order
        .iter()
        .map(|kind| {
            match kind {
                terminal_psi::BoundaryParameterKind::Scalar => scalars.next(),
                terminal_psi::BoundaryParameterKind::Structural => structures.next(),
            }
            .ok_or(invalid.clone())
        })
        .collect::<Result<Vec<_>, _>>()?;
    for (callback, ordinal) in callbacks
        .iter()
        .filter(|callback| {
            matches!(
                callback.destination,
                calling_conventions::NativePlace::Parameter(_)
            )
        })
        .zip(&private_ordinals)
    {
        if *ordinal > parameters.len() {
            return Err(invalid);
        }
        parameters.insert(*ordinal, callback.application.shape);
    }
    let signature = CallSignature {
        parameters,
        result: result_shape,
    };
    let validated = match callbacks.first() {
        Some(callback) => {
            calling_conventions::validate_boundary_entry_plan_with_callback_materializations(
                call.binding.boundary_entry_plan.clone(),
                &signature,
                &callback.registrar_context,
            )
        }
        None => calling_conventions::validate_boundary_entry_plan(
            call.binding.boundary_entry_plan.clone(),
            &signature,
        ),
    }
    .map_err(|_| invalid.clone())?;
    if declarations.next().is_some()
        || *psi_operation != operation
        || *row_boundary != call.boundary
        || call.boundary != *boundary
        || *provider_execution != call.provider_execution
        || *binding != call.binding
        || scalar_arguments.as_slice() != call.scalar_arguments.as_slice()
        || row_structural.as_slice() != call.structural_arguments.as_slice()
        || *row_result != call.result
        || callbacks != call.callbacks.iter().collect::<Vec<_>>()
        || !completion_claim_sources.is_empty()
        || !completion_receipts.is_empty()
        || call.binding.native_target() != Some(native.target)
        || call.binding.provider_plan_report_identity()
            != call
                .provider_execution
                .provider_plan_report_identity()
                .get()
        || call.binding.requirement_identity() != declaration.identity
        || !call.binding.has_valid_entry_mechanism()
        || validated.plan() != &call.binding.boundary_entry_plan
        || call.binding.boundary_entry_plan.call.parameters.len()
            != scalar_shapes.len() + derived_structural.len() + private_ordinals.len()
        || call.structural_arguments.as_slice() != derived_structural.as_slice()
        || call.scalar_arguments.len() != arguments.len()
        || call
            .scalar_arguments
            .iter()
            .zip(arguments)
            .zip(&declaration.scalar_parameters)
            .zip(&scalar_shapes)
            .zip(declaration.parameter_positions(terminal_psi::BoundaryParameterKind::Scalar))
            .any(|((((argument, value), parameter), shape), index)| {
                let placed_byte_size = match argument.placement.locations.as_slice() {
                    [
                        ValueLocation::Register {
                            value_byte_offset: 0,
                            byte_size,
                            ..
                        },
                    ]
                    | [
                        ValueLocation::Stack {
                            value_byte_offset: 0,
                            byte_size,
                            ..
                        },
                    ] => *byte_size,
                    _ => return true,
                };
                let expected_index = index
                    + private_ordinals
                        .iter()
                        .filter(|ordinal| **ordinal <= index)
                        .count();
                argument.parameter_index != expected_index as u32
                    || call
                        .binding
                        .boundary_entry_plan
                        .call
                        .parameters
                        .get(expected_index)
                        != Some(&argument.placement)
                    || argument.placement.shape != *shape
                    || shape.byte_size != placed_byte_size
                    || argument.source.scalar_type() != *parameter
                    || argument.source.source_value() != *value
                    || match &argument.source {
                        target_operations::TargetUnitScalarArgumentSource::IntegerImmediate {
                            scalar_type,
                            value,
                            ..
                        } => {
                            semantic_vocabulary::ScalarTerm::integer(*scalar_type, *value).is_err()
                        }
                        target_operations::TargetUnitScalarArgumentSource::Home(home) => {
                            home.shape != *shape
                        }
                        _ => false,
                    }
            })
        || call.result != expected_result
        || match (&call.binding.boundary_entry_plan.call.result, result_shape) {
            (None, None) => false,
            (Some(placement), Some(shape)) => {
                placement.shape != shape
                    || matches!(expected_result, TargetBoundaryResult::Scalar(_))
                        && !matches!(placement.locations.as_slice(),
                        [ValueLocation::Register { value_byte_offset: 0, byte_size, .. }]
                            if *byte_size == shape.byte_size)
            }
            _ => true,
        }
    {
        return Err(invalid);
    }
    Ok(())
}
