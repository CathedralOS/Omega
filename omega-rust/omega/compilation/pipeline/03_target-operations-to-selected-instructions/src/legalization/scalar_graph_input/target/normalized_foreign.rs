//! Rejoin one evaluated normalized foreign call row to its exact boundary
//! declaration, evaluated entry plan, and caller-side argument custody.
//!
//! The row's typed locator, admitted provider execution, and boundary-entry
//! plan are validated data sealed upstream; this validator independently
//! re-derives every derivable coordinate from the declaration, the caller's
//! retained structural parameters, and the ordered scalar source roster, and
//! re-validates the claimed plan against the reconstructed signature rather
//! than trusting the row's placements. A registrar callback materialization
//! replays against the plan's retained `native_callback_arguments`: the
//! admitted argument must join to this row by Terminal operation and exact
//! registrar entry plan, and its context re-validates the materialized
//! signature in place of the ordinary plan check.
//!
//! Structural formal positions index their own lane, not the native signature.
//! The boundary declaration's parameter order is the authority for interleaving
//! scalar values and referent pointers; callback slots offset that order only
//! at the native boundary. Reconstruct both shapes and placements in that order.
use super::super::scalar_shape;
use super::{
    AbstractOperationPlan, PsiOptimizationFunction, PsiOptimizationUnit, TargetUnitOperation,
    ValueId,
};
use crate::LegalizationError;
use calling_conventions::{CallSignature, ValueLocation};
use target_operations::{
    TargetBoundaryResult, TargetOperationPlan, TargetStructuralParameter,
    TargetUnitScalarArgumentSource as Source,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn validate(
    target: &TargetUnitOperation,
    source: &abstract_operations::AbstractOperation,
    parameters: &[TargetStructuralParameter],
    native: &TargetOperationPlan,
    optimized: &PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    values: &mut Vec<(ValueId, Source)>,
) -> Result<(), LegalizationError> {
    let (
        TargetUnitOperation::NormalizedForeignCall {
            psi_operation,
            boundary,
            provider_execution,
            binding,
            scalar_arguments,
            structural_arguments,
            result: actual_result,
        },
        abstract_operations::AbstractOperation::BoundaryCall {
            psi_operation: expected_operation,
            boundary: expected_boundary,
            result,
            arguments,
            structural_arguments: expected_structural,
            completion_claim_sources,
            completion_receipts,
        },
    ) = (target, source)
    else {
        return Err(LegalizationError::custody());
    };
    let mut declarations = plan
        .boundary_machines
        .iter()
        .filter(|row| row.id == *boundary);
    let declaration = declarations.next().ok_or(LegalizationError::custody())?;
    // Unit custody validates boundary reach against the caller's ceiling.
    // Rejoin that exact declaration; the admitted same-stack claim must name
    // this declaration's identity and the exact selected provider plan that
    // produced this execution binding.
    if unit
        .boundary_machines
        .iter()
        .find(|row| row.id == *boundary)
        != Some(declaration)
    {
        return Err(LegalizationError::custody());
    }
    let scalar_shapes = declaration
        .scalar_parameters
        .iter()
        .map(|parameter| scalar_shape(*parameter).ok_or(LegalizationError::custody()))
        .collect::<Result<Vec<_>, LegalizationError>>()?;
    // Lane-local custody rejoins the declaration's retained authored order.
    if !declaration.has_valid_parameter_order()
        || expected_structural.len() != declaration.structural_parameters.len()
        || expected_structural.len() != structural_arguments.len()
    {
        return Err(LegalizationError::custody());
    }
    // Structural transport uses the evaluated plan's ordered parameter rows.
    // The signature for plan revalidation is the computed destination shapes,
    // matching the producer's own derivation.
    let callbacks = super::super::normalized_foreign::native_callbacks_at(
        native,
        *psi_operation,
        &binding.boundary_entry_plan,
    )?;
    // Only parameter-destination callbacks occupy a private plan slot and
    // shift authored positions; field destinations live inside the record a
    // real argument already names, so they mint no plan operand.
    let mut private_ordinals = Vec::new();
    for callback in &callbacks {
        if matches!(
            callback.destination,
            calling_conventions::NativePlace::Parameter(_)
        ) {
            private_ordinals.push(
                usize::try_from(callback.application.native_ordinal)
                    .map_err(|_| LegalizationError::custody())?,
            );
        }
    }
    private_ordinals.sort_unstable();
    let structural = expected_structural
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
                super::super::normalized_foreign::structural_argument_at(
                    semantic,
                    index,
                    declaration_parameter,
                    binding
                        .boundary_entry_plan
                        .call
                        .parameters
                        .get(native_position),
                    parameters,
                    optimized,
                    native.target,
                    binding.boundary_entry_plan.call.policy,
                    plan,
                    *expected_operation,
                )
            },
        )
        .collect::<Result<Vec<_>, LegalizationError>>()?;
    let (expected_result, result_shape) = super::super::normalized_foreign::result_at(
        result,
        &declaration.result,
        *psi_operation,
        optimized,
        plan,
        binding.boundary_entry_plan.call.policy,
    )?;
    // A retained callback occupies one native-only parameter slot in the
    // registrar plan; the validated signature then spells every authored and
    // private placement while the declaration still counts only semantic
    // formals.
    let mut scalars = scalar_shapes.iter().copied();
    let mut structures = structural.iter().map(|argument| argument.destination.shape);
    let mut parameters = declaration
        .parameter_order
        .iter()
        .map(|kind| {
            match kind {
                terminal_psi::BoundaryParameterKind::Scalar => scalars.next(),
                terminal_psi::BoundaryParameterKind::Structural => structures.next(),
            }
            .ok_or(LegalizationError::custody())
        })
        .collect::<Result<Vec<_>, _>>()?;
    // Each parameter-destination callback reinserts its private slot at the
    // native ordinal it claims; ascending insertion keeps later ordinals
    // addressing the plan's authored coordinates.
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
            return Err(LegalizationError::custody());
        }
        parameters.insert(*ordinal, callback.application.shape);
    }
    // Every retained callback carries the registrar's own binder/demand
    // context; the join tolerates no divergent claim.
    if callbacks
        .iter()
        .any(|callback| callback.registrar_context != callbacks[0].registrar_context)
    {
        return Err(LegalizationError::custody());
    }
    let signature = CallSignature {
        parameters,
        result: result_shape,
    };
    let validated = match callbacks.first() {
        Some(callback) => {
            calling_conventions::validate_boundary_entry_plan_with_callback_materializations(
                binding.boundary_entry_plan.clone(),
                &signature,
                &callback.registrar_context,
            )
        }
        None => calling_conventions::validate_boundary_entry_plan(
            binding.boundary_entry_plan.clone(),
            &signature,
        ),
    }
    .map_err(|_| LegalizationError::custody())?;
    if declarations.next().is_some()
        || psi_operation != expected_operation
        || boundary != expected_boundary
        || !completion_claim_sources.is_empty()
        || !completion_receipts.is_empty()
        || binding.native_target() != Some(native.target)
        || binding.provider_plan_report_identity()
            != provider_execution.provider_plan_report_identity().get()
        || binding.requirement_identity() != declaration.identity
        || !binding.has_valid_entry_mechanism()
        || validated.plan() != &binding.boundary_entry_plan
        || binding.boundary_entry_plan.call.parameters.len()
            != scalar_shapes.len() + structural.len() + private_ordinals.len()
        || scalar_arguments.len() != arguments.len()
        || arguments.len() != declaration.scalar_parameters.len()
        || structural_arguments.as_slice() != structural.as_slice()
        || scalar_arguments
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
                let native_position = index
                    + private_ordinals
                        .iter()
                        .filter(|ordinal| **ordinal <= index)
                        .count();
                usize::try_from(argument.parameter_index).ok() != Some(native_position)
                    || binding
                        .boundary_entry_plan
                        .call
                        .parameters
                        .get(native_position)
                        != Some(&argument.placement)
                    || argument.placement.shape != *shape
                    || shape.byte_size != placed_byte_size
                    || argument.source.scalar_type() != *parameter
                    || match &argument.source {
                        Source::IntegerImmediate {
                            scalar_type, value, ..
                        } => {
                            semantic_vocabulary::ScalarTerm::integer(*scalar_type, *value).is_err()
                        }
                        Source::Home(home) => home.shape != *shape,
                        _ => false,
                    }
                    || !values.iter().any(|(identity, expected)| {
                        identity == value && *expected == argument.source
                    })
            })
        || expected_result != *actual_result
        || match (&binding.boundary_entry_plan.call.result, result_shape) {
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
        return Err(LegalizationError::custody());
    }
    if let Some(home) = expected_result.scalar_home() {
        values.push((home.source_value, Source::Home(*home)));
    }
    Ok(())
}
