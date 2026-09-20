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
use super::super::{ScalarType, scalar_shape};
use super::{
    AbstractOperationPlan, PsiOptimizationFunction, PsiOptimizationUnit, TargetFunction,
    TargetUnitOperation, ValueId,
};
use crate::LegalizationError;
use calling_conventions::{CallSignature, CallingPolicy, EntryControl, ValueLocation, ValueShape};
use target_operations::{
    TargetOperationPlan, TargetStructuralParameter, TargetUnitScalarArgumentSource as Source,
    TargetUnitScalarHomeRequirement,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn validate(
    target: &TargetUnitOperation,
    source: &abstract_operations::AbstractOperation,
    function: &TargetFunction,
    parameters: &[TargetStructuralParameter],
    native: &TargetOperationPlan,
    optimized: &PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    values: &mut Vec<(ValueId, Source)>,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let (
        TargetUnitOperation::NormalizedForeignCall {
            psi_operation,
            boundary,
            provider_execution,
            binding,
            scalar_arguments,
            structural_arguments,
            result_home,
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
        return Err(invalid);
    };
    let mut declarations = plan
        .boundary_machines
        .iter()
        .filter(|row| row.id == *boundary);
    let declaration = declarations.next().ok_or(invalid.clone())?;
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
        return Err(invalid);
    }
    let scalar_shapes = declaration
        .scalar_parameters
        .iter()
        .map(|parameter| {
            let ScalarType::Integer(integer_type) = parameter else {
                return Err(invalid.clone());
            };
            if integer_type.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
                || !matches!(integer_type.bits(), 8 | 16 | 32 | 64)
            {
                return Err(invalid.clone());
            }
            let bytes = integer_type.bits().div_ceil(8);
            Ok(ValueShape::integer(bytes, bytes.next_power_of_two().min(8)))
        })
        .collect::<Result<Vec<_>, LegalizationError>>()?;
    // Lane-local custody rejoins the declaration's retained authored order.
    if !declaration.has_valid_parameter_order()
        || expected_structural.len() != declaration.structural_parameters.len()
        || expected_structural.len() != structural_arguments.len()
    {
        return Err(invalid);
    }
    // Structural transport uses the evaluated plan's ordered parameter rows.
    // The signature for plan revalidation is the computed destination shapes,
    // matching the producer's own derivation.
    let callback = super::super::normalized_foreign::native_callback_at(
        native,
        *psi_operation,
        &binding.boundary_entry_plan,
    )?;
    let callback_ordinal = callback
        .map(|callback| usize::try_from(callback.application.native_ordinal))
        .transpose()
        .map_err(|_| invalid.clone())?;
    let structural = expected_structural
        .iter()
        .zip(&declaration.structural_parameters)
        .enumerate()
        .zip(declaration.parameter_positions(terminal_psi::BoundaryParameterKind::Structural))
        .map(
            |((index, (semantic, declaration_parameter)), semantic_position)| {
                let native_position = semantic_position
                    + usize::from(
                        callback_ordinal.is_some_and(|ordinal| semantic_position >= ordinal),
                    );
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
                    plan,
                )
            },
        )
        .collect::<Result<Vec<_>, LegalizationError>>()?;
    let expected_result = match (result, &declaration.result) {
        (
            abstract_operations::AbstractBoundaryResult::Unit,
            terminal_psi::BoundaryMachineResult::Unit,
        ) => None,
        (
            abstract_operations::AbstractBoundaryResult::Scalar(result),
            terminal_psi::BoundaryMachineResult::Scalar(ScalarType::Integer(declared)),
        ) => {
            let ScalarType::Integer(result_type) = result.scalar_type else {
                return Err(invalid);
            };
            if *declared != result_type
                || super::super::value_type(optimized, result.value) != Some(result.scalar_type)
                || function.attachment.is_none()
            {
                return Err(invalid);
            }
            let shape = scalar_shape(result.scalar_type).ok_or(invalid.clone())?;
            Some((
                TargetUnitScalarHomeRequirement {
                    defining_operation: *psi_operation,
                    source_value: result.value,
                    scalar_type: result.scalar_type,
                    shape,
                },
                shape,
            ))
        }
        _ => return Err(invalid),
    };
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
            .ok_or(invalid.clone())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(callback) = callback {
        let ordinal = callback_ordinal.ok_or(invalid.clone())?;
        if ordinal > parameters.len() {
            return Err(invalid);
        }
        parameters.insert(ordinal, callback.application.shape);
    }
    let signature = CallSignature {
        parameters,
        result: expected_result.map(|(_, shape)| shape),
    };
    let validated = match callback {
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
    .map_err(|_| invalid.clone())?;
    if declarations.next().is_some()
        || psi_operation != expected_operation
        || boundary != expected_boundary
        || !completion_claim_sources.is_empty()
        || !completion_receipts.is_empty()
        || binding.locator.target().native_target() != native.target
        || binding
            .same_stack_contribution
            .provider_plan_report_identity()
            != provider_execution.provider_plan_report_identity().get()
        || binding.same_stack_contribution.requirement_identity() != declaration.identity
        || binding.boundary_entry_plan.call.policy
            != CallingPolicy::native_for_target(native.target)
        || binding.boundary_entry_plan.call.entry_control != EntryControl::CallReturn
        || validated.plan() != &binding.boundary_entry_plan
        || binding.boundary_entry_plan.call.parameters.len()
            != scalar_shapes.len() + structural.len() + usize::from(callback.is_some())
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
                let ScalarType::Integer(integer_type) = parameter else {
                    return true;
                };
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
                let native_position =
                    index + usize::from(callback_ordinal.is_some_and(|ordinal| index >= ordinal));
                usize::try_from(argument.parameter_index).ok() != Some(native_position)
                    || binding
                        .boundary_entry_plan
                        .call
                        .parameters
                        .get(native_position)
                        != Some(&argument.placement)
                    || argument.placement.shape != *shape
                    || shape.byte_size != placed_byte_size
                    || argument.source.scalar_type() != ScalarType::Integer(*integer_type)
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
        || expected_result.map(|(home, _)| home) != *result_home
        || match (&binding.boundary_entry_plan.call.result, expected_result) {
            (None, None) => false,
            (Some(placement), Some((_, shape))) => {
                placement.shape != shape
                    || !matches!(placement.locations.as_slice(),
                        [ValueLocation::Register { value_byte_offset: 0, byte_size, .. }]
                            if *byte_size == shape.byte_size)
            }
            _ => true,
        }
    {
        return Err(invalid);
    }
    if let Some((home, _)) = expected_result {
        values.push((home.source_value, Source::Home(home)));
    }
    Ok(())
}
