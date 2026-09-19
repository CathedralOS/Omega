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
    // The Terminal declaration splits scalar and structural formals into two
    // lane-local lists and erases their authored interleave, so a structural
    // argument rejoins its exact plan position only while the scalar lane is
    // empty; a mixed signature fails closed rather than guessing an ordinal
    // the artifact cannot prove.
    if expected_structural.len() != declaration.structural_parameters.len()
        || expected_structural.len() != structural_arguments.len()
        || (!expected_structural.is_empty() && !declaration.scalar_parameters.is_empty())
    {
        return Err(invalid);
    }
    // Structural transport uses the evaluated plan's ordered parameter rows.
    // The signature for plan revalidation is the computed destination shapes,
    // matching the producer's own derivation.
    let structural = expected_structural
        .iter()
        .zip(&declaration.structural_parameters)
        .enumerate()
        .map(|(index, (semantic, declaration_parameter))| {
            super::super::normalized_foreign::structural_argument_at(
                semantic,
                index,
                declaration_parameter,
                binding.boundary_entry_plan.call.parameters.get(index),
                parameters,
                optimized,
                native.target,
                plan,
            )
        })
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
    let callback = super::super::normalized_foreign::native_callback_at(
        native,
        *psi_operation,
        &binding.boundary_entry_plan,
    )?;
    let signature = CallSignature {
        parameters: if callback.is_some() {
            binding
                .boundary_entry_plan
                .call
                .parameters
                .iter()
                .map(|placement| placement.shape)
                .collect()
        } else if structural.is_empty() {
            scalar_shapes.clone()
        } else {
            structural
                .iter()
                .map(|argument| argument.destination.shape)
                .collect()
        },
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
    let callback_ordinal = callback
        .map(|callback| usize::try_from(callback.application.native_ordinal))
        .transpose()
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
            .enumerate()
            .any(|(index, (((argument, value), parameter), shape))| {
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
                argument.parameter_index
                    != (index
                        + usize::from(callback_ordinal.is_some_and(|ordinal| index >= ordinal)))
                        as u32
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
