//! Replay one legalized normalized foreign call against the abstract boundary
//! call it legalizes and the exact target row it must carry.
//!
//! The admitted provider execution, typed locator binding, and evaluated plan
//! are custody evidence: replay rejoins the unique target row and requires the
//! legalized payload to carry it exactly, while every derivable coordinate —
//! scalar argument order, placement, source identity, structural projection,
//! and the optional scalar result home — is reconstructed independently from
//! the boundary declaration and the claimed plan. A substituted binding,
//! provider, argument, or result home fails closed.
use super::super::{AbstractOperation, Error, PsiOptimizationUnit, TargetOperationPlan};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input;
use abstract_operations::AbstractOperationPlan;
use calling_conventions::{CallSignature, CallingPolicy, EntryControl, ValueLocation, ValueShape};
use legalized_operations::LegalizedScalarInstruction;
use semantic_vocabulary::{OperationId, ScalarType};
use target_operations::{TargetUnitOperation, TargetUnitScalarHomeRequirement};

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
        result_home,
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
    if structural_arguments.len() != declaration.structural_parameters.len()
        || (!structural_arguments.is_empty() && !declaration.scalar_parameters.is_empty())
        || arguments.len() != declaration.scalar_parameters.len()
    {
        return Err(invalid);
    }
    // Each structural argument is re-derived from the boundary declaration and
    // the claimed plan position; the legalized row cannot substitute a place,
    // path, projected type, offset, or destination.
    let derived_structural = structural_arguments
        .iter()
        .zip(&declaration.structural_parameters)
        .enumerate()
        .map(|(index, (semantic, declaration_parameter))| {
            scalar_graph_input::normalized_foreign::structural_argument_at(
                semantic,
                index,
                declaration_parameter,
                call.binding.boundary_entry_plan.call.parameters.get(index),
                &function.graph.parameters,
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
                || scalar_graph_input::value_type(optimized, result.value)
                    != Some(result.scalar_type)
                || function.attachment.is_none()
            {
                return Err(invalid);
            }
            let shape =
                scalar_graph_input::scalar_shape(result.scalar_type).ok_or(invalid.clone())?;
            Some((
                TargetUnitScalarHomeRequirement {
                    defining_operation: operation,
                    source_value: result.value,
                    scalar_type: result.scalar_type,
                    shape,
                },
                shape,
            ))
        }
        _ => return Err(invalid),
    };
    let signature = CallSignature {
        parameters: if derived_structural.is_empty() {
            scalar_shapes.clone()
        } else {
            derived_structural
                .iter()
                .map(|argument| argument.destination.shape)
                .collect()
        },
        result: expected_result.map(|(_, shape)| shape),
    };
    let Ok(validated) = calling_conventions::validate_boundary_entry_plan(
        call.binding.boundary_entry_plan.clone(),
        &signature,
    ) else {
        return Err(invalid);
    };
    if declarations.next().is_some()
        || *psi_operation != operation
        || *row_boundary != call.boundary
        || call.boundary != *boundary
        || *provider_execution != call.provider_execution
        || *binding != call.binding
        || scalar_arguments.as_slice() != call.scalar_arguments.as_slice()
        || row_structural.as_slice() != call.structural_arguments.as_slice()
        || *result_home != call.result_home
        || !completion_claim_sources.is_empty()
        || !completion_receipts.is_empty()
        || call.binding.locator.target().native_target() != native.target
        || call
            .binding
            .same_stack_contribution
            .provider_plan_report_identity()
            != call
                .provider_execution
                .provider_plan_report_identity()
                .get()
        || call.binding.same_stack_contribution.requirement_identity() != declaration.identity
        || call.binding.boundary_entry_plan.call.policy
            != CallingPolicy::native_for_target(native.target)
        || call.binding.boundary_entry_plan.call.entry_control != EntryControl::CallReturn
        || !call
            .binding
            .boundary_entry_plan
            .call
            .callback_materializations
            .is_empty()
        || validated.plan() != &call.binding.boundary_entry_plan
        || call.binding.boundary_entry_plan.call.parameters.len()
            != scalar_shapes.len() + derived_structural.len()
        || call.structural_arguments.as_slice() != derived_structural.as_slice()
        || call.scalar_arguments.len() != arguments.len()
        || call
            .scalar_arguments
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
                argument.parameter_index != index as u32
                    || call.binding.boundary_entry_plan.call.parameters.get(index)
                        != Some(&argument.placement)
                    || argument.placement.shape != *shape
                    || shape.byte_size != placed_byte_size
                    || argument.source.scalar_type() != ScalarType::Integer(*integer_type)
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
        || call.result_home != expected_result.map(|(home, _)| home)
        || match (
            &call.binding.boundary_entry_plan.call.result,
            expected_result,
        ) {
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
    Ok(())
}
