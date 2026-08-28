#![forbid(unsafe_code)]

//! Transitional baseline publication assignment for clean Terminal Psi.
//!
//! This is not a second compiler pipeline. Native realization can reach it
//! only through `omega-optimization-pipeline`'s single continuation API, and
//! only for the empty identity selection while selected-instruction coverage
//! is incomplete. Delete this crate once the selected/physical route covers
//! the full target-operation vocabulary and rejoins image publication.

use std::collections::BTreeMap;

use omega_assigned_target_operations::{
    AssignedAggregateCopy, AssignedBooleanControl, AssignedBooleanExpression, AssignedCallArgument,
    AssignedCallDestination, AssignedConditionalBooleanArm, AssignedConditionalIntegerArm,
    AssignedFunction, AssignedIntegerControl, AssignedIntegerExpression, AssignedOperation,
    AssignedOperationPlan, AssignedScalarExpression, AssignedScalarLocation, AssignedUnitBody,
    AssignedUnitOperation, EntryRegisterSpill, ExpressionFrame,
};
use omega_calling_conventions::{
    CallSignature, CallingPolicy, ValueClass, ValueLocation, evaluate_call_plan,
};
use omega_target::{Architecture, NativeTarget};
use omega_target_operations::{
    MachineRegister, ScalarParameterLocation, TargetBooleanControl, TargetBooleanExpression,
    TargetCallArgument, TargetFunction, TargetIntegerControl, TargetIntegerExpression,
    TargetOperation, TargetOperationPlan, TargetScalarExpression, TargetUnitOperation,
};
use psi_core::{EdgeId, MachineId, OperationId, ValueId};

mod structural_result;
mod structural_scalar;

pub fn assign_registers(
    plan: &TargetOperationPlan,
) -> Result<AssignedOperationPlan, AssignmentError> {
    if !plan
        .functions
        .iter()
        .any(|function| function.machine == plan.entry)
    {
        return Err(AssignmentError::EntryFunctionMissing(plan.entry));
    }
    Ok(AssignedOperationPlan {
        psi: plan.psi,
        target: plan.target,
        entry: plan.entry,
        functions: plan
            .functions
            .iter()
            .map(|function| assign_function(function, plan.target))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn assign_function(
    function: &TargetFunction,
    target: NativeTarget,
) -> Result<AssignedFunction, AssignmentError> {
    let architecture = target.architecture;
    let operation = match &function.operation {
        operation @ TargetOperation::ReturnStructuralScalarCall { .. } => {
            structural_scalar::assign(function.machine, operation, target)?
        }
        operation @ TargetOperation::ReturnStructuralCall { .. } => {
            structural_result::assign(function.machine, operation, target)?
        }
        TargetOperation::ScalarReturnWithCleanup {
            scalar,
            structural_types,
            call_plan,
            structural_parameters,
            cleanup_actions,
            psi_edge,
        } => {
            if matches!(
                scalar.as_ref(),
                TargetOperation::ScalarReturnWithCleanup { .. }
                    | TargetOperation::BooleanControlWithCleanup { .. }
                    | TargetOperation::ReturnBoundaryPortReadU8 { .. }
            ) {
                return Err(AssignmentError::UnsupportedScalarCleanup(function.machine));
            }
            validate_scalar_cleanup_signature(
                function.machine,
                target,
                call_plan,
                structural_parameters,
                cleanup_actions,
            )?;
            let assigned_scalar = assign_function(
                &TargetFunction {
                    machine: function.machine,
                    attachment: function.attachment,
                    provenance: function.provenance.clone(),
                    operation: scalar.as_ref().clone(),
                },
                target,
            )?
            .operation;
            AssignedOperation::ScalarReturnWithCleanup {
                scalar: Box::new(assigned_scalar),
                structural_types: structural_types.clone(),
                call_plan: call_plan.clone(),
                structural_parameters: structural_parameters.clone(),
                cleanup_actions: cleanup_actions.clone(),
                psi_edge: *psi_edge,
            }
        }
        TargetOperation::ReturnBoundaryPortReadU8 {
            psi_edge,
            psi_operation,
            source_value,
            boundary,
            provider_execution,
            realization,
            arguments,
            completion_claim_sources,
            completion_receipts,
            call_plan,
            structural_parameters,
        } => {
            if architecture != Architecture::X86_64 {
                return Err(AssignmentError::BoundaryPortReadUnsupported {
                    machine: function.machine,
                    architecture,
                });
            }
            AssignedOperation::ReturnBoundaryPortReadU8 {
                psi_edge: *psi_edge,
                psi_operation: *psi_operation,
                source_value: *source_value,
                boundary: *boundary,
                provider_execution: *provider_execution,
                realization: *realization,
                arguments: arguments.clone(),
                completion_claim_sources: completion_claim_sources.clone(),
                completion_receipts: completion_receipts.clone(),
                call_plan: call_plan.clone(),
                structural_parameters: structural_parameters.clone(),
            }
        }
        TargetOperation::ExitProcessI32 {
            constant_operation,
            psi_operation,
            nominal_return_edge,
            boundary,
            provider_execution,
            realization,
            argument,
            completion_claim_sources,
            completion_receipts,
        } => {
            let expected_destination = match (target.object_format, architecture) {
                (omega_target::ObjectFormat::Elf, Architecture::X86_64) => MachineRegister::X86Rdi,
                (omega_target::ObjectFormat::Elf, Architecture::Aarch64) => {
                    MachineRegister::Aarch64X(0)
                }
                _ => {
                    return Err(AssignmentError::LinuxExitGroupUnsupported {
                        machine: function.machine,
                        target,
                    });
                }
            };
            if argument.destination != expected_destination {
                return Err(AssignmentError::LinuxExitGroupArgumentMismatch(
                    function.machine,
                ));
            }
            AssignedOperation::ExitProcessI32 {
                constant_operation: *constant_operation,
                psi_operation: *psi_operation,
                nominal_return_edge: *nominal_return_edge,
                boundary: *boundary,
                provider_execution: *provider_execution,
                realization: *realization,
                argument: *argument,
                completion_claim_sources: completion_claim_sources.clone(),
                completion_receipts: completion_receipts.clone(),
            }
        }
        TargetOperation::BooleanControlWithCleanup {
            control,
            structural_types,
            call_plan,
            structural_parameters,
            cleanup_actions,
        } => {
            validate_scalar_cleanup_signature(
                function.machine,
                target,
                call_plan,
                structural_parameters,
                cleanup_actions,
            )?;
            finite_boolean_cleanup_edges(control)
                .ok_or(AssignmentError::UnsupportedScalarCleanup(function.machine))?;
            AssignedOperation::BooleanControlWithCleanup {
                control: assign_boolean_control(control, architecture)?,
                structural_types: structural_types.clone(),
                call_plan: call_plan.clone(),
                structural_parameters: structural_parameters.clone(),
                cleanup_actions: cleanup_actions.clone(),
            }
        }
        TargetOperation::UnitBody(body) => {
            let operations = body
                .operations
                .iter()
                .map(|operation| {
                    Ok(match operation {
                        TargetUnitOperation::EstablishByteSequenceLiteral {
                            psi_operation,
                            place,
                            structural_type,
                            bytes,
                        } => AssignedUnitOperation::EstablishByteSequenceLiteral {
                            psi_operation: *psi_operation,
                            place: place.clone(),
                            structural_type: structural_type.clone(),
                            bytes: bytes.clone(),
                        },
                        TargetUnitOperation::IntegerConstant {
                            psi_operation,
                            result,
                            scalar_type,
                            value,
                        } => AssignedUnitOperation::IntegerConstant {
                            psi_operation: *psi_operation,
                            result: *result,
                            scalar_type: *scalar_type,
                            value: *value,
                        },
                        TargetUnitOperation::EstablishTrivialAffineLocal {
                            psi_operation,
                            place,
                            structural_type,
                        } => AssignedUnitOperation::EstablishTrivialAffineLocal {
                            psi_operation: *psi_operation,
                            place: place.clone(),
                            structural_type: structural_type.clone(),
                        },
                        TargetUnitOperation::Call {
                            psi_operation,
                            callee,
                            arguments,
                            claim_transfers,
                        } => AssignedUnitOperation::Call {
                            psi_operation: *psi_operation,
                            callee: *callee,
                            result: None,
                            copies: arguments
                                .iter()
                                .map(|argument| AssignedAggregateCopy {
                                    place: argument.place,
                                    access: argument.access,
                                    path: argument.path.clone(),
                                    root_structural_type: argument.root_structural_type,
                                    structural_type: argument.structural_type,
                                    shape: argument.shape,
                                    source_byte_offset: argument.source_byte_offset,
                                    fixed_array_length: argument.fixed_array_length,
                                    element_stride: argument.element_stride,
                                    source: argument.source.clone(),
                                    destination: argument.destination.clone(),
                                })
                                .collect(),
                            claim_transfers: claim_transfers.clone(),
                        },
                        TargetUnitOperation::InstalledProviderCall {
                            psi_operation,
                            boundary,
                            ..
                        } => {
                            return Err(
                                AssignmentError::InstalledProviderCallRequiresOptimizedLane {
                                    machine: function.machine,
                                    operation: *psi_operation,
                                    boundary: *boundary,
                                },
                            );
                        }
                        TargetUnitOperation::PortWrite {
                            psi_operation,
                            service,
                            port,
                            value,
                        } => AssignedUnitOperation::PortWrite {
                            psi_operation: *psi_operation,
                            service: *service,
                            port: *port,
                            value: *value,
                        },
                        TargetUnitOperation::BoundarySettlement {
                            psi_operation,
                            boundary,
                            provider_execution,
                            realization,
                            scalar_arguments,
                            arguments,
                            byte_sequence_arguments,
                            completion_claim_sources,
                            completion_receipts,
                        } => AssignedUnitOperation::BoundarySettlement {
                            psi_operation: *psi_operation,
                            boundary: *boundary,
                            provider_execution: *provider_execution,
                            realization: *realization,
                            scalar_arguments: scalar_arguments.clone(),
                            arguments: arguments.clone(),
                            byte_sequence_arguments: byte_sequence_arguments.clone(),
                            completion_claim_sources: completion_claim_sources.clone(),
                            completion_receipts: completion_receipts.clone(),
                        },
                        TargetUnitOperation::Return {
                            psi_edge,
                            cleanup_actions,
                        } => AssignedUnitOperation::Return {
                            psi_edge: *psi_edge,
                            cleanup_actions: cleanup_actions.clone(),
                        },
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            AssignedOperation::UnitBody(AssignedUnitBody {
                structural_types: body.structural_types.clone(),
                call_plan: body.call_plan.clone(),
                parameters: body.parameters.clone(),
                operations,
            })
        }
        TargetOperation::ReturnStructuralParameter {
            call_plan,
            parameters,
            source,
            result,
            shape,
            source_placement,
            result_placement,
            psi_edge,
            returned_claims,
            trivial_affine_locals,
            trivial_affine_discards,
        } => {
            let source_index = 0;
            if parameters.first() != Some(source) {
                return Err(AssignmentError::UnsupportedStructuralPlacement(
                    source.place,
                ));
            }
            let expected_call_plan = evaluate_call_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature {
                    parameters: call_plan
                        .parameters
                        .iter()
                        .map(|placement| placement.shape)
                        .collect(),
                    result: Some(*shape),
                },
            )
            .map_err(|_| AssignmentError::UnsupportedStructuralPlacement(source.place))?;
            if *call_plan != expected_call_plan
                || call_plan.parameters.len() != parameters.len()
                || call_plan.parameters.get(source_index) != Some(source_placement)
                || call_plan.result.as_ref() != Some(result_placement)
                || source.place == result.place
                || source.multiplicity != psi_terminal::StructuralMultiplicity::Linear
                || result.multiplicity != psi_terminal::StructuralMultiplicity::Linear
                || source.structural_type != result.structural_type
                || source.qualifications != result.qualifications
                || trivial_affine_discards.len() + 1
                    != parameters.len() + trivial_affine_locals.len()
                || parameters.iter().enumerate().any(|(index, parameter)| {
                    usize::try_from(parameter.position) != Ok(index) || parameter.is_self
                })
                || parameters.iter().skip(1).any(|parameter| {
                    parameter.place == source.place
                        || parameter.place == result.place
                        || !parameter.qualifications.is_empty()
                })
                || parameters
                    .iter()
                    .map(|parameter| parameter.place)
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    != parameters.len()
                || trivial_affine_locals
                    .iter()
                    .enumerate()
                    .any(|(index, (_, local, local_type))| {
                    !matches!(
                        local.kind,
                        psi_core::StructuralPlaceKind::TrivialAffineLocal {
                            declaration_ordinal,
                            structural_type
                        } if usize::try_from(declaration_ordinal) == Ok(index)
                            && structural_type == local_type.id
                    ) || local.id == source.place
                        || local.id == result.place
                        || parameters.iter().any(|parameter| parameter.place == local.id)
                        || local_type.identity.is_empty()
                        || !matches!(
                            local_type.shape,
                            psi_terminal::StructuralTypeShape::Record { ref fields } if fields.is_empty()
                        )
                })
                || trivial_affine_locals
                    .iter()
                    .map(|(_, local, _)| local.id)
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    != trivial_affine_locals.len()
                || trivial_affine_discards
                    != &trivial_affine_locals
                        .iter()
                        .rev()
                        .map(|(_, local, _)| local.id)
                        .chain(parameters.iter().skip(1).rev().map(|parameter| parameter.place))
                        .collect::<Vec<_>>()
                || parameters
                    .iter()
                    .skip(1)
                    .any(|parameter| parameter.multiplicity != psi_terminal::StructuralMultiplicity::Affine)
            {
                return Err(AssignmentError::UnsupportedStructuralPlacement(
                    source.place,
                ));
            }
            for (parameter, placement) in parameters.iter().zip(&call_plan.parameters) {
                if parameter.place == source.place {
                    validate_direct_structural_return_placement(
                        parameter.place,
                        placement,
                        architecture,
                    )?;
                } else {
                    validate_structural_placement(parameter.place, placement, architecture)?;
                }
            }
            validate_direct_structural_return_placement(
                result.place,
                result_placement,
                architecture,
            )?;
            AssignedOperation::ReturnStructuralParameter {
                call_plan: call_plan.clone(),
                parameters: parameters.clone(),
                source: source.clone(),
                result: result.clone(),
                shape: *shape,
                source_placement: source_placement.clone(),
                result_placement: result_placement.clone(),
                psi_edge: *psi_edge,
                returned_claims: returned_claims.clone(),
                trivial_affine_locals: trivial_affine_locals.clone(),
                trivial_affine_discards: trivial_affine_discards.clone(),
            }
        }
        TargetOperation::Crash {
            psi_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        } => AssignedOperation::Crash {
            psi_edge: *psi_edge,
            cause: *cause,
            site_guard: site_guard.clone(),
            frontier_lower_bound: frontier_lower_bound.clone(),
        },
        TargetOperation::ReturnIntegerImmediate {
            psi_edge,
            source_value,
            scalar_type,
            value,
        } => AssignedOperation::ReturnIntegerImmediate {
            psi_edge: *psi_edge,
            source_value: *source_value,
            scalar_type: *scalar_type,
            value: *value,
        },
        TargetOperation::ReturnBooleanImmediate {
            psi_edge,
            source_value,
            value,
        } => AssignedOperation::ReturnBooleanImmediate {
            psi_edge: *psi_edge,
            source_value: *source_value,
            value: *value,
        },
        TargetOperation::ReturnIntegerParameter {
            psi_edge,
            source_value,
            scalar_type,
            parameter_index,
            location,
        } => AssignedOperation::ReturnIntegerParameter {
            psi_edge: *psi_edge,
            source_value: *source_value,
            scalar_type: *scalar_type,
            parameter_index: *parameter_index,
            location: assign_direct_location(*source_value, *location, architecture)?,
        },
        TargetOperation::ReturnBooleanParameter {
            psi_edge,
            source_value,
            parameter_index,
            location,
        } => AssignedOperation::ReturnBooleanParameter {
            psi_edge: *psi_edge,
            source_value: *source_value,
            parameter_index: *parameter_index,
            location: assign_direct_location(*source_value, *location, architecture)?,
        },
        TargetOperation::ReturnBooleanNotParameter {
            psi_edge,
            source_value,
            parameter_index,
            location,
        } => AssignedOperation::ReturnBooleanNotParameter {
            psi_edge: *psi_edge,
            source_value: *source_value,
            parameter_index: *parameter_index,
            location: assign_direct_location(*source_value, *location, architecture)?,
        },
        TargetOperation::ReturnBooleanSharedConvergence { psi_edge, control } => {
            AssignedOperation::ReturnBooleanSharedConvergence {
                psi_edge: *psi_edge,
                control: assign_boolean_control(control, architecture)?,
            }
        }
        TargetOperation::ReturnBooleanExpression {
            psi_edge,
            source_value,
            expression,
        } => {
            let (frame, expression) = assign_boolean_expression_frame(expression, architecture)?;
            AssignedOperation::ReturnBooleanExpression {
                psi_edge: *psi_edge,
                source_value: *source_value,
                frame,
                expression,
            }
        }
        TargetOperation::ReturnIntegerExpression {
            psi_edge,
            source_value,
            scalar_type,
            expression,
        } => {
            let (frame, expression) = assign_integer_expression_frame(expression, architecture)?;
            AssignedOperation::ReturnIntegerExpression {
                psi_edge: *psi_edge,
                source_value: *source_value,
                scalar_type: *scalar_type,
                frame,
                expression,
            }
        }
        TargetOperation::ReturnIntegerConditionalControl {
            condition_source,
            condition_parameter_index,
            condition_location,
            scalar_type,
            when_true,
            when_false,
        } => AssignedOperation::ReturnIntegerConditionalControl {
            condition_source: *condition_source,
            condition_parameter_index: *condition_parameter_index,
            condition_location: assign_direct_location(
                *condition_source,
                *condition_location,
                architecture,
            )?,
            scalar_type: *scalar_type,
            when_true: assign_control_arm(when_true, architecture)?,
            when_false: assign_control_arm(when_false, architecture)?,
        },
        TargetOperation::ReturnIntegerExpressionConditionalControl {
            condition_source,
            condition,
            scalar_type,
            when_true,
            when_false,
        } => {
            let preserved = integer_control_arms_parameter_locations(when_true, when_false)?;
            let (condition_frame, condition) =
                assign_boolean_expression_frame_preserving(condition, architecture, preserved)?;
            AssignedOperation::ReturnIntegerExpressionConditionalControl {
                condition_source: *condition_source,
                condition_frame,
                condition,
                scalar_type: *scalar_type,
                when_true: assign_control_arm(when_true, architecture)?,
                when_false: assign_control_arm(when_false, architecture)?,
            }
        }
        TargetOperation::ReturnBooleanConditionalControl {
            condition_source,
            condition_parameter_index,
            condition_location,
            when_true,
            when_false,
        } => AssignedOperation::ReturnBooleanConditionalControl {
            condition_source: *condition_source,
            condition_parameter_index: *condition_parameter_index,
            condition_location: assign_direct_location(
                *condition_source,
                *condition_location,
                architecture,
            )?,
            when_true: assign_boolean_control_arm(when_true, architecture)?,
            when_false: assign_boolean_control_arm(when_false, architecture)?,
        },
        TargetOperation::ReturnBooleanExpressionConditionalControl {
            condition_source,
            condition,
            when_true,
            when_false,
        } => {
            let preserved = boolean_control_arms_parameter_locations(when_true, when_false)?;
            let (condition_frame, condition) =
                assign_boolean_expression_frame_preserving(condition, architecture, preserved)?;
            AssignedOperation::ReturnBooleanExpressionConditionalControl {
                condition_source: *condition_source,
                condition_frame,
                condition,
                when_true: assign_boolean_control_arm(when_true, architecture)?,
                when_false: assign_boolean_control_arm(when_false, architecture)?,
            }
        }
    };
    Ok(AssignedFunction {
        machine: function.machine,
        attachment: function.attachment,
        provenance: function.provenance.clone(),
        operation,
    })
}

fn validate_scalar_cleanup_signature(
    machine: MachineId,
    target: NativeTarget,
    call_plan: &omega_calling_conventions::CallPlan,
    structural_parameters: &[omega_target_operations::TargetStructuralParameter],
    cleanup_actions: &[psi_terminal::TerminalAffineCleanupAction],
) -> Result<(), AssignmentError> {
    let expected_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: call_plan
                .parameters
                .iter()
                .map(|placement| placement.shape)
                .collect(),
            result: call_plan.result.as_ref().map(|placement| placement.shape),
        },
    )
    .map_err(|_| AssignmentError::UnsupportedScalarCleanup(machine))?;
    if cleanup_actions.is_empty()
        || expected_call_plan != *call_plan
        || call_plan.result.is_none()
        || call_plan.parameters.len() < structural_parameters.len()
        || call_plan.parameters[call_plan.parameters.len() - structural_parameters.len()..]
            .iter()
            .zip(structural_parameters)
            .any(|(placement, parameter)| placement != &parameter.placement)
        || cleanup_actions.len() != structural_parameters.len()
        || structural_parameters
            .iter()
            .rev()
            .zip(cleanup_actions)
            .any(|(parameter, action)| match action {
                psi_terminal::TerminalAffineCleanupAction::DiscardRoot(place) => {
                    *place != parameter.place
                }
                psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                    cleanup.place != parameter.place
                        || cleanup.structural_type != parameter.structural_type
                }
                psi_terminal::TerminalAffineCleanupAction::DiscardResidual(_) => true,
            })
    {
        return Err(AssignmentError::UnsupportedScalarCleanup(machine));
    }
    for parameter in structural_parameters {
        validate_structural_placement(parameter.place, &parameter.placement, target.architecture)?;
    }
    Ok(())
}

/// Return every terminal value edge in canonical true-before-false DFS order.
/// Cleanup control is finite and branch-only: crashes are a distinct terminal
/// carrier, and replaying one return edge on multiple leaves is forbidden.
fn finite_boolean_cleanup_edges(control: &TargetBooleanControl) -> Option<Vec<EdgeId>> {
    fn collect(control: &TargetBooleanControl, edges: &mut Vec<EdgeId>) -> Option<()> {
        match control {
            TargetBooleanControl::ReturnImmediate {
                psi_return_edge, ..
            }
            | TargetBooleanControl::ReturnParameter {
                psi_return_edge, ..
            }
            | TargetBooleanControl::ReturnNotParameter {
                psi_return_edge, ..
            }
            | TargetBooleanControl::ReturnExpression {
                psi_return_edge, ..
            } => edges.push(*psi_return_edge),
            TargetBooleanControl::Conditional {
                when_true,
                when_false,
                ..
            }
            | TargetBooleanControl::ConditionalExpression {
                when_true,
                when_false,
                ..
            } => {
                collect(&when_true.control, edges)?;
                collect(&when_false.control, edges)?;
            }
            TargetBooleanControl::Crash { .. } => return None,
        }
        Some(())
    }

    let mut edges = Vec::new();
    collect(control, &mut edges)?;
    (edges.len() >= 2
        && edges
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == edges.len())
    .then_some(edges)
}

fn validate_structural_placement(
    place: psi_core::PlaceId,
    placement: &omega_calling_conventions::ValuePlacement,
    architecture: Architecture,
) -> Result<(), AssignmentError> {
    let [location] = placement.locations.as_slice() else {
        return Err(AssignmentError::UnsupportedStructuralPlacement(place));
    };
    let ValueLocation::Register { register, .. } = location else {
        return match location {
            ValueLocation::Stack {
                value_byte_offset: 0,
                byte_size,
                alignment,
                ..
            } if u16::try_from(placement.shape.byte_size) == Ok(*byte_size)
                && u16::try_from(placement.shape.alignment) == Ok(*alignment) =>
            {
                Ok(())
            }
            _ => Err(AssignmentError::UnsupportedStructuralPlacement(place)),
        };
    };
    validate_structural_register(place, *register, architecture)
}

fn validate_direct_structural_return_placement(
    place: psi_core::PlaceId,
    placement: &omega_calling_conventions::ValuePlacement,
    architecture: Architecture,
) -> Result<(), AssignmentError> {
    if placement.shape.class != ValueClass::Integer
        || !((placement.shape.byte_size == 8 && placement.shape.alignment == 8)
            || (9..=16).contains(&placement.shape.byte_size))
    {
        return Err(AssignmentError::UnsupportedStructuralPlacement(place));
    }
    if placement.locations.len() == 1 {
        return validate_structural_placement(place, placement, architecture);
    }
    if placement.locations.len() != 2 || !(9..=16).contains(&placement.shape.byte_size) {
        return Err(AssignmentError::UnsupportedStructuralPlacement(place));
    }
    let mut expected_offset = 0_u16;
    for location in &placement.locations {
        let ValueLocation::Register {
            register,
            value_byte_offset,
            byte_size,
        } = *location
        else {
            return Err(AssignmentError::UnsupportedStructuralPlacement(place));
        };
        let expected_size = (placement.shape.byte_size - expected_offset).min(8);
        if value_byte_offset != expected_offset || byte_size != expected_size {
            return Err(AssignmentError::UnsupportedStructuralPlacement(place));
        }
        validate_structural_register(place, register, architecture)?;
        expected_offset = expected_offset
            .checked_add(byte_size)
            .ok_or(AssignmentError::UnsupportedStructuralPlacement(place))?;
    }
    if expected_offset != placement.shape.byte_size {
        return Err(AssignmentError::UnsupportedStructuralPlacement(place));
    }
    Ok(())
}

fn validate_structural_register(
    place: psi_core::PlaceId,
    register: MachineRegister,
    architecture: Architecture,
) -> Result<(), AssignmentError> {
    let matches_architecture = match (architecture, register) {
        (Architecture::X86_64, omega_target_operations::MachineRegister::X86Rax)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86Rcx)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86Rdx)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86Rbx)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86Rsp)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86Rbp)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86Rsi)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86Rdi)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86R8)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86R9)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86R10)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86R11)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86R12)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86R13)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86R14)
        | (Architecture::X86_64, omega_target_operations::MachineRegister::X86R15)
        | (Architecture::Aarch64, omega_target_operations::MachineRegister::Aarch64X(0..=30)) => {
            true
        }
        _ => false,
    };
    if !matches_architecture {
        return Err(AssignmentError::StructuralRegisterArchitectureMismatch {
            place,
            register,
            architecture,
        });
    }
    Ok(())
}

fn assign_boolean_control_arm(
    arm: &omega_target_operations::TargetConditionalBooleanArm,
    architecture: Architecture,
) -> Result<AssignedConditionalBooleanArm, AssignmentError> {
    Ok(AssignedConditionalBooleanArm {
        psi_edge: arm.psi_edge,
        control: Box::new(assign_boolean_control(&arm.control, architecture)?),
    })
}

fn assign_boolean_control(
    control: &TargetBooleanControl,
    architecture: Architecture,
) -> Result<AssignedBooleanControl, AssignmentError> {
    Ok(match control {
        TargetBooleanControl::Crash {
            psi_crash_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        } => AssignedBooleanControl::Crash {
            psi_crash_edge: *psi_crash_edge,
            cause: *cause,
            site_guard: site_guard.clone(),
            frontier_lower_bound: frontier_lower_bound.clone(),
        },
        TargetBooleanControl::ReturnImmediate {
            psi_return_edge,
            source_value,
            value,
        } => AssignedBooleanControl::ReturnImmediate {
            psi_return_edge: *psi_return_edge,
            source_value: *source_value,
            value: *value,
        },
        TargetBooleanControl::ReturnParameter {
            psi_return_edge,
            source_value,
            parameter_index,
            location,
        } => AssignedBooleanControl::ReturnParameter {
            psi_return_edge: *psi_return_edge,
            source_value: *source_value,
            parameter_index: *parameter_index,
            location: assign_direct_location(*source_value, *location, architecture)?,
        },
        TargetBooleanControl::ReturnNotParameter {
            psi_return_edge,
            source_value,
            parameter_index,
            location,
        } => AssignedBooleanControl::ReturnNotParameter {
            psi_return_edge: *psi_return_edge,
            source_value: *source_value,
            parameter_index: *parameter_index,
            location: assign_direct_location(*source_value, *location, architecture)?,
        },
        TargetBooleanControl::ReturnExpression {
            psi_return_edge,
            source_value,
            expression,
        } => {
            let (frame, expression) = assign_boolean_expression_frame(expression, architecture)?;
            AssignedBooleanControl::ReturnExpression {
                psi_return_edge: *psi_return_edge,
                source_value: *source_value,
                frame,
                expression,
            }
        }
        TargetBooleanControl::Conditional {
            condition_source,
            condition_parameter_index,
            condition_location,
            when_true,
            when_false,
        } => AssignedBooleanControl::Conditional {
            condition_source: *condition_source,
            condition_parameter_index: *condition_parameter_index,
            condition_location: assign_direct_location(
                *condition_source,
                *condition_location,
                architecture,
            )?,
            when_true: assign_boolean_control_arm(when_true, architecture)?,
            when_false: assign_boolean_control_arm(when_false, architecture)?,
        },
        TargetBooleanControl::ConditionalExpression {
            condition_source,
            condition,
            when_true,
            when_false,
        } => {
            let preserved = boolean_control_arms_parameter_locations(when_true, when_false)?;
            let (condition_frame, condition) =
                assign_boolean_expression_frame_preserving(condition, architecture, preserved)?;
            AssignedBooleanControl::ConditionalExpression {
                condition_source: *condition_source,
                condition_frame,
                condition,
                when_true: assign_boolean_control_arm(when_true, architecture)?,
                when_false: assign_boolean_control_arm(when_false, architecture)?,
            }
        }
    })
}

fn assign_control_arm(
    arm: &omega_target_operations::TargetConditionalIntegerArm,
    architecture: Architecture,
) -> Result<AssignedConditionalIntegerArm, AssignmentError> {
    Ok(AssignedConditionalIntegerArm {
        psi_edge: arm.psi_edge,
        control: Box::new(assign_integer_control(&arm.control, architecture)?),
    })
}

fn assign_integer_control(
    control: &TargetIntegerControl,
    architecture: Architecture,
) -> Result<AssignedIntegerControl, AssignmentError> {
    Ok(match control {
        TargetIntegerControl::Crash {
            psi_crash_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        } => AssignedIntegerControl::Crash {
            psi_crash_edge: *psi_crash_edge,
            cause: *cause,
            site_guard: site_guard.clone(),
            frontier_lower_bound: frontier_lower_bound.clone(),
        },
        TargetIntegerControl::Return {
            psi_return_edge,
            source_value,
            expression,
        } => {
            let (frame, expression) = assign_integer_expression_frame(expression, architecture)?;
            AssignedIntegerControl::Return {
                psi_return_edge: *psi_return_edge,
                source_value: *source_value,
                frame,
                expression,
            }
        }
        TargetIntegerControl::Conditional {
            condition_source,
            condition_parameter_index,
            condition_location,
            when_true,
            when_false,
        } => AssignedIntegerControl::Conditional {
            condition_source: *condition_source,
            condition_parameter_index: *condition_parameter_index,
            condition_location: assign_direct_location(
                *condition_source,
                *condition_location,
                architecture,
            )?,
            when_true: assign_control_arm(when_true, architecture)?,
            when_false: assign_control_arm(when_false, architecture)?,
        },
        TargetIntegerControl::ConditionalExpression {
            condition_source,
            condition,
            when_true,
            when_false,
        } => {
            let preserved = integer_control_arms_parameter_locations(when_true, when_false)?;
            let (condition_frame, condition) =
                assign_boolean_expression_frame_preserving(condition, architecture, preserved)?;
            AssignedIntegerControl::ConditionalExpression {
                condition_source: *condition_source,
                condition_frame,
                condition,
                when_true: assign_control_arm(when_true, architecture)?,
                when_false: assign_control_arm(when_false, architecture)?,
            }
        }
    })
}

fn assign_direct_location(
    source_value: ValueId,
    location: ScalarParameterLocation,
    architecture: Architecture,
) -> Result<AssignedScalarLocation, AssignmentError> {
    Ok(match location {
        ScalarParameterLocation::Register(register) => {
            require_register_architecture(source_value, register, architecture)?;
            AssignedScalarLocation::Register(register)
        }
        ScalarParameterLocation::IncomingStack { byte_offset } => {
            AssignedScalarLocation::IncomingStack { byte_offset }
        }
    })
}

fn assign_expression_locations(
    architecture: Architecture,
    locations: &BTreeMap<usize, (ValueId, ScalarParameterLocation)>,
    force_register_spills: bool,
) -> Result<(ExpressionFrame, BTreeMap<usize, AssignedScalarLocation>), AssignmentError> {
    let mut register_spills = Vec::new();
    let mut assigned = BTreeMap::new();
    for (&parameter_index, &(source_value, location)) in locations {
        match location {
            ScalarParameterLocation::Register(register) => {
                require_register_architecture(source_value, register, architecture)?;
                if architecture == Architecture::X86_64 && register == MachineRegister::X86Rsp {
                    return Err(AssignmentError::ExpressionRegisterCannotHoldParameter {
                        value: source_value,
                        register,
                    });
                }
                if force_register_spills
                    || architecture == Architecture::Aarch64
                    || x86_expression_scratch_conflict(register)
                {
                    let byte_offset = u32::try_from(register_spills.len())
                        .ok()
                        .and_then(|count| count.checked_mul(8))
                        .ok_or(AssignmentError::ExpressionStackFrameNotEncodable)?;
                    register_spills.push(EntryRegisterSpill {
                        source_value,
                        parameter_index,
                        register,
                        byte_offset,
                    });
                    assigned.insert(
                        parameter_index,
                        AssignedScalarLocation::FrameSpill { byte_offset },
                    );
                } else {
                    assigned.insert(parameter_index, AssignedScalarLocation::Register(register));
                }
            }
            ScalarParameterLocation::IncomingStack { byte_offset } => {
                assigned.insert(
                    parameter_index,
                    AssignedScalarLocation::IncomingStack { byte_offset },
                );
            }
        }
    }
    let used_bytes = u32::try_from(register_spills.len())
        .ok()
        .and_then(|count| count.checked_mul(8))
        .ok_or(AssignmentError::ExpressionStackFrameNotEncodable)?;
    let byte_size = used_bytes
        .checked_add(15)
        .map(|bytes| bytes & !15)
        .ok_or(AssignmentError::ExpressionStackFrameNotEncodable)?;
    if byte_size > 0xfff {
        return Err(AssignmentError::ExpressionStackFrameNotEncodable);
    }
    Ok((
        ExpressionFrame {
            byte_size,
            register_spills,
        },
        assigned,
    ))
}

fn assign_integer_expression_frame(
    expression: &TargetIntegerExpression,
    architecture: Architecture,
) -> Result<(ExpressionFrame, AssignedIntegerExpression), AssignmentError> {
    let locations = expression_parameter_locations(expression)?;
    let (mut frame, assigned_locations) = assign_expression_locations(
        architecture,
        &locations,
        integer_expression_contains_call(expression),
    )?;
    let mut next_spill = frame.byte_size;
    let expression = assign_expression(
        expression,
        &assigned_locations,
        architecture,
        &mut next_spill,
    )?;
    frame.byte_size = aligned_frame_size(next_spill)?;
    Ok((frame, expression))
}

fn assign_boolean_expression_frame(
    expression: &TargetBooleanExpression,
    architecture: Architecture,
) -> Result<(ExpressionFrame, AssignedBooleanExpression), AssignmentError> {
    assign_boolean_expression_frame_preserving(expression, architecture, BTreeMap::new())
}

fn assign_boolean_expression_frame_preserving(
    expression: &TargetBooleanExpression,
    architecture: Architecture,
    preserved: BTreeMap<usize, (ValueId, ScalarParameterLocation)>,
) -> Result<(ExpressionFrame, AssignedBooleanExpression), AssignmentError> {
    let mut locations = boolean_expression_parameter_locations(expression)?;
    merge_expression_locations(&mut locations, preserved)?;
    let (mut frame, assigned_locations) = assign_expression_locations(
        architecture,
        &locations,
        boolean_expression_contains_call(expression),
    )?;
    let mut next_spill = frame.byte_size;
    let expression = assign_boolean_expression(
        expression,
        &assigned_locations,
        architecture,
        &mut next_spill,
    )?;
    frame.byte_size = aligned_frame_size(next_spill)?;
    Ok((frame, expression))
}

fn aligned_frame_size(used_bytes: u32) -> Result<u32, AssignmentError> {
    let byte_size = used_bytes
        .checked_add(15)
        .map(|bytes| bytes & !15)
        .ok_or(AssignmentError::ExpressionStackFrameNotEncodable)?;
    if byte_size > 0xfff {
        return Err(AssignmentError::ExpressionStackFrameNotEncodable);
    }
    Ok(byte_size)
}

fn integer_expression_contains_call(expression: &TargetIntegerExpression) -> bool {
    match expression {
        TargetIntegerExpression::Call { .. } => true,
        TargetIntegerExpression::Immediate { .. } | TargetIntegerExpression::Parameter { .. } => {
            false
        }
        TargetIntegerExpression::BitwiseNot { operand, .. }
        | TargetIntegerExpression::IntegerWiden { operand, .. }
        | TargetIntegerExpression::IntegerExactCast { operand, .. } => {
            integer_expression_contains_call(operand)
        }
        TargetIntegerExpression::WrappingAdd { left, right, .. }
        | TargetIntegerExpression::ExactAdd { left, right, .. }
        | TargetIntegerExpression::BitwiseAnd { left, right, .. }
        | TargetIntegerExpression::BitwiseOr { left, right, .. }
        | TargetIntegerExpression::BitwiseXor { left, right, .. }
        | TargetIntegerExpression::WrappingShiftLeft {
            value: left,
            count: right,
            ..
        }
        | TargetIntegerExpression::WrappingShiftRight {
            value: left,
            count: right,
            ..
        }
        | TargetIntegerExpression::ExactShiftLeft {
            value: left,
            count: right,
            ..
        }
        | TargetIntegerExpression::ExactShiftRight {
            value: left,
            count: right,
            ..
        }
        | TargetIntegerExpression::SaturatingAdd { left, right, .. }
        | TargetIntegerExpression::WrappingSubtract { left, right, .. }
        | TargetIntegerExpression::ExactSubtract { left, right, .. }
        | TargetIntegerExpression::SaturatingSubtract { left, right, .. }
        | TargetIntegerExpression::WrappingMultiply { left, right, .. }
        | TargetIntegerExpression::ExactMultiply { left, right, .. }
        | TargetIntegerExpression::SaturatingMultiply { left, right, .. }
        | TargetIntegerExpression::ExactDivide { left, right, .. }
        | TargetIntegerExpression::ExactRemainder { left, right, .. }
        | TargetIntegerExpression::WrappingDivide { left, right, .. }
        | TargetIntegerExpression::WrappingRemainder { left, right, .. }
        | TargetIntegerExpression::SaturatingDivide { left, right, .. }
        | TargetIntegerExpression::SaturatingRemainder { left, right, .. } => {
            integer_expression_contains_call(left) || integer_expression_contains_call(right)
        }
    }
}

fn boolean_expression_contains_call(expression: &TargetBooleanExpression) -> bool {
    match expression {
        TargetBooleanExpression::Call { .. } => true,
        TargetBooleanExpression::Immediate { .. }
        | TargetBooleanExpression::Parameter { .. }
        | TargetBooleanExpression::StructuralField { .. } => false,
        TargetBooleanExpression::Not { operand, .. } => boolean_expression_contains_call(operand),
        TargetBooleanExpression::Equal { left, right, .. } => {
            boolean_expression_contains_call(left) || boolean_expression_contains_call(right)
        }
        TargetBooleanExpression::IntegerEqual { left, right, .. }
        | TargetBooleanExpression::IntegerLessThan { left, right, .. }
        | TargetBooleanExpression::IntegerLessOrEqual { left, right, .. } => {
            integer_expression_contains_call(left) || integer_expression_contains_call(right)
        }
    }
}

fn assign_call_arguments(
    arguments: &[TargetCallArgument],
    locations: &BTreeMap<usize, AssignedScalarLocation>,
    architecture: Architecture,
    next_spill: &mut u32,
) -> Result<Vec<AssignedCallArgument>, AssignmentError> {
    arguments
        .iter()
        .map(|argument| {
            let expression = match &argument.expression {
                TargetScalarExpression::Boolean(expression) => AssignedScalarExpression::Boolean(
                    assign_boolean_expression(expression, locations, architecture, next_spill)?,
                ),
                TargetScalarExpression::Integer {
                    scalar_type,
                    expression,
                } => AssignedScalarExpression::Integer {
                    scalar_type: *scalar_type,
                    expression: assign_expression(expression, locations, architecture, next_spill)?,
                },
            };
            let destination = match argument.location {
                ScalarParameterLocation::Register(register) => {
                    let valid = match architecture {
                        Architecture::Aarch64 => {
                            matches!(register, MachineRegister::Aarch64X(0..=30))
                        }
                        Architecture::X86_64 => matches!(
                            register,
                            MachineRegister::X86Rax
                                | MachineRegister::X86Rcx
                                | MachineRegister::X86Rdx
                                | MachineRegister::X86Rbx
                                | MachineRegister::X86Rsp
                                | MachineRegister::X86Rbp
                                | MachineRegister::X86Rsi
                                | MachineRegister::X86Rdi
                                | MachineRegister::X86R8
                                | MachineRegister::X86R9
                                | MachineRegister::X86R10
                                | MachineRegister::X86R11
                                | MachineRegister::X86R12
                                | MachineRegister::X86R13
                                | MachineRegister::X86R14
                                | MachineRegister::X86R15
                        ),
                    };
                    if !valid || register == MachineRegister::X86Rsp {
                        return Err(AssignmentError::UnsupportedCallArgumentRegister(register));
                    }
                    AssignedCallDestination::Register(register)
                }
                ScalarParameterLocation::IncomingStack { byte_offset } => {
                    AssignedCallDestination::OutgoingStack { byte_offset }
                }
            };
            let spill_byte_offset = *next_spill;
            *next_spill = next_spill
                .checked_add(8)
                .ok_or(AssignmentError::ExpressionStackFrameNotEncodable)?;
            Ok(AssignedCallArgument {
                scalar_type: argument.scalar_type,
                destination,
                spill_byte_offset,
                expression,
            })
        })
        .collect()
}

fn assign_expression(
    expression: &TargetIntegerExpression,
    locations: &BTreeMap<usize, AssignedScalarLocation>,
    architecture: Architecture,
    next_spill: &mut u32,
) -> Result<AssignedIntegerExpression, AssignmentError> {
    fn binary(
        psi_operation: OperationId,
        left: &TargetIntegerExpression,
        right: &TargetIntegerExpression,
        locations: &BTreeMap<usize, AssignedScalarLocation>,
        architecture: Architecture,
        next_spill: &mut u32,
        constructor: impl FnOnce(
            OperationId,
            Box<AssignedIntegerExpression>,
            Box<AssignedIntegerExpression>,
        ) -> AssignedIntegerExpression,
    ) -> Result<AssignedIntegerExpression, AssignmentError> {
        Ok(constructor(
            psi_operation,
            Box::new(assign_expression(
                left,
                locations,
                architecture,
                next_spill,
            )?),
            Box::new(assign_expression(
                right,
                locations,
                architecture,
                next_spill,
            )?),
        ))
    }
    match expression {
        TargetIntegerExpression::Call {
            psi_operation,
            source_value,
            callee,
            arguments,
        } => Ok(AssignedIntegerExpression::Call {
            psi_operation: *psi_operation,
            source_value: *source_value,
            callee: *callee,
            arguments: assign_call_arguments(arguments, locations, architecture, next_spill)?,
        }),
        TargetIntegerExpression::Immediate {
            source_value,
            value,
        } => Ok(AssignedIntegerExpression::Immediate {
            source_value: *source_value,
            value: *value,
        }),
        TargetIntegerExpression::Parameter {
            source_value,
            parameter_index,
            ..
        } => Ok(AssignedIntegerExpression::Parameter {
            source_value: *source_value,
            parameter_index: *parameter_index,
            location: *locations.get(parameter_index).ok_or(
                AssignmentError::ExpressionParameterAssignmentMissing {
                    value: *source_value,
                    parameter_index: *parameter_index,
                },
            )?,
        }),
        TargetIntegerExpression::BitwiseNot {
            psi_operation,
            operand,
        } => Ok(AssignedIntegerExpression::BitwiseNot {
            psi_operation: *psi_operation,
            operand: Box::new(assign_expression(
                operand,
                locations,
                architecture,
                next_spill,
            )?),
        }),
        TargetIntegerExpression::IntegerWiden {
            psi_operation,
            source_type,
            operand,
        } => Ok(AssignedIntegerExpression::IntegerWiden {
            psi_operation: *psi_operation,
            source_type: *source_type,
            operand: Box::new(assign_expression(
                operand,
                locations,
                architecture,
                next_spill,
            )?),
        }),
        TargetIntegerExpression::IntegerExactCast {
            psi_operation,
            obligation,
            source_type,
            operand,
        } => Ok(AssignedIntegerExpression::IntegerExactCast {
            psi_operation: *psi_operation,
            obligation: *obligation,
            source_type: *source_type,
            operand: Box::new(assign_expression(
                operand,
                locations,
                architecture,
                next_spill,
            )?),
        }),
        TargetIntegerExpression::BitwiseAnd {
            psi_operation,
            left,
            right,
        } => binary(
            *psi_operation,
            left,
            right,
            locations,
            architecture,
            next_spill,
            |psi_operation, left, right| AssignedIntegerExpression::BitwiseAnd {
                psi_operation,
                left,
                right,
            },
        ),
        TargetIntegerExpression::BitwiseOr {
            psi_operation,
            left,
            right,
        } => binary(
            *psi_operation,
            left,
            right,
            locations,
            architecture,
            next_spill,
            |psi_operation, left, right| AssignedIntegerExpression::BitwiseOr {
                psi_operation,
                left,
                right,
            },
        ),
        TargetIntegerExpression::BitwiseXor {
            psi_operation,
            left,
            right,
        } => binary(
            *psi_operation,
            left,
            right,
            locations,
            architecture,
            next_spill,
            |psi_operation, left, right| AssignedIntegerExpression::BitwiseXor {
                psi_operation,
                left,
                right,
            },
        ),
        TargetIntegerExpression::WrappingShiftLeft {
            psi_operation,
            count_type,
            value,
            count,
        } => Ok(AssignedIntegerExpression::WrappingShiftLeft {
            psi_operation: *psi_operation,
            count_type: *count_type,
            value: Box::new(assign_expression(
                value,
                locations,
                architecture,
                next_spill,
            )?),
            count: Box::new(assign_expression(
                count,
                locations,
                architecture,
                next_spill,
            )?),
        }),
        TargetIntegerExpression::WrappingShiftRight {
            psi_operation,
            count_type,
            value,
            count,
        } => Ok(AssignedIntegerExpression::WrappingShiftRight {
            psi_operation: *psi_operation,
            count_type: *count_type,
            value: Box::new(assign_expression(
                value,
                locations,
                architecture,
                next_spill,
            )?),
            count: Box::new(assign_expression(
                count,
                locations,
                architecture,
                next_spill,
            )?),
        }),
        TargetIntegerExpression::ExactShiftRight {
            psi_operation,
            obligation,
            count_type,
            value,
            count,
        } => Ok(AssignedIntegerExpression::ExactShiftRight {
            psi_operation: *psi_operation,
            obligation: *obligation,
            count_type: *count_type,
            value: Box::new(assign_expression(
                value,
                locations,
                architecture,
                next_spill,
            )?),
            count: Box::new(assign_expression(
                count,
                locations,
                architecture,
                next_spill,
            )?),
        }),
        TargetIntegerExpression::ExactShiftLeft {
            psi_operation,
            obligation,
            count_type,
            value,
            count,
        } => Ok(AssignedIntegerExpression::ExactShiftLeft {
            psi_operation: *psi_operation,
            obligation: *obligation,
            count_type: *count_type,
            value: Box::new(assign_expression(
                value,
                locations,
                architecture,
                next_spill,
            )?),
            count: Box::new(assign_expression(
                count,
                locations,
                architecture,
                next_spill,
            )?),
        }),
        TargetIntegerExpression::WrappingAdd {
            psi_operation,
            left,
            right,
        } => binary(
            *psi_operation,
            left,
            right,
            locations,
            architecture,
            next_spill,
            |psi_operation, left, right| AssignedIntegerExpression::WrappingAdd {
                psi_operation,
                left,
                right,
            },
        ),
        TargetIntegerExpression::ExactAdd {
            psi_operation,
            obligation,
            left,
            right,
        } => {
            let obligation = *obligation;
            binary(
                *psi_operation,
                left,
                right,
                locations,
                architecture,
                next_spill,
                move |psi_operation, left, right| AssignedIntegerExpression::ExactAdd {
                    psi_operation,
                    obligation,
                    left,
                    right,
                },
            )
        }
        TargetIntegerExpression::SaturatingAdd {
            psi_operation,
            left,
            right,
        } => binary(
            *psi_operation,
            left,
            right,
            locations,
            architecture,
            next_spill,
            |psi_operation, left, right| AssignedIntegerExpression::SaturatingAdd {
                psi_operation,
                left,
                right,
            },
        ),
        TargetIntegerExpression::WrappingSubtract {
            psi_operation,
            left,
            right,
        } => binary(
            *psi_operation,
            left,
            right,
            locations,
            architecture,
            next_spill,
            |psi_operation, left, right| AssignedIntegerExpression::WrappingSubtract {
                psi_operation,
                left,
                right,
            },
        ),
        TargetIntegerExpression::ExactSubtract {
            psi_operation,
            obligation,
            left,
            right,
        } => {
            let obligation = *obligation;
            binary(
                *psi_operation,
                left,
                right,
                locations,
                architecture,
                next_spill,
                move |psi_operation, left, right| AssignedIntegerExpression::ExactSubtract {
                    psi_operation,
                    obligation,
                    left,
                    right,
                },
            )
        }
        TargetIntegerExpression::SaturatingSubtract {
            psi_operation,
            left,
            right,
        } => binary(
            *psi_operation,
            left,
            right,
            locations,
            architecture,
            next_spill,
            |psi_operation, left, right| AssignedIntegerExpression::SaturatingSubtract {
                psi_operation,
                left,
                right,
            },
        ),
        TargetIntegerExpression::WrappingMultiply {
            psi_operation,
            left,
            right,
        } => binary(
            *psi_operation,
            left,
            right,
            locations,
            architecture,
            next_spill,
            |psi_operation, left, right| AssignedIntegerExpression::WrappingMultiply {
                psi_operation,
                left,
                right,
            },
        ),
        TargetIntegerExpression::ExactMultiply {
            psi_operation,
            obligation,
            left,
            right,
        } => {
            let obligation = *obligation;
            binary(
                *psi_operation,
                left,
                right,
                locations,
                architecture,
                next_spill,
                move |psi_operation, left, right| AssignedIntegerExpression::ExactMultiply {
                    psi_operation,
                    obligation,
                    left,
                    right,
                },
            )
        }
        TargetIntegerExpression::SaturatingMultiply {
            psi_operation,
            left,
            right,
        } => binary(
            *psi_operation,
            left,
            right,
            locations,
            architecture,
            next_spill,
            |psi_operation, left, right| AssignedIntegerExpression::SaturatingMultiply {
                psi_operation,
                left,
                right,
            },
        ),
        TargetIntegerExpression::ExactDivide {
            psi_operation,
            obligation,
            left,
            right,
        } => {
            let obligation = *obligation;
            binary(
                *psi_operation,
                left,
                right,
                locations,
                architecture,
                next_spill,
                move |psi_operation, left, right| AssignedIntegerExpression::ExactDivide {
                    psi_operation,
                    obligation,
                    left,
                    right,
                },
            )
        }
        TargetIntegerExpression::ExactRemainder {
            psi_operation,
            obligation,
            left,
            right,
        } => {
            let obligation = *obligation;
            binary(
                *psi_operation,
                left,
                right,
                locations,
                architecture,
                next_spill,
                move |psi_operation, left, right| AssignedIntegerExpression::ExactRemainder {
                    psi_operation,
                    obligation,
                    left,
                    right,
                },
            )
        }
        TargetIntegerExpression::WrappingDivide {
            psi_operation,
            obligation,
            left,
            right,
        } => {
            let obligation = *obligation;
            binary(
                *psi_operation,
                left,
                right,
                locations,
                architecture,
                next_spill,
                move |psi_operation, left, right| AssignedIntegerExpression::WrappingDivide {
                    psi_operation,
                    obligation,
                    left,
                    right,
                },
            )
        }
        TargetIntegerExpression::WrappingRemainder {
            psi_operation,
            obligation,
            left,
            right,
        } => {
            let obligation = *obligation;
            binary(
                *psi_operation,
                left,
                right,
                locations,
                architecture,
                next_spill,
                move |psi_operation, left, right| AssignedIntegerExpression::WrappingRemainder {
                    psi_operation,
                    obligation,
                    left,
                    right,
                },
            )
        }
        TargetIntegerExpression::SaturatingDivide {
            psi_operation,
            obligation,
            left,
            right,
        } => {
            let obligation = *obligation;
            binary(
                *psi_operation,
                left,
                right,
                locations,
                architecture,
                next_spill,
                move |psi_operation, left, right| AssignedIntegerExpression::SaturatingDivide {
                    psi_operation,
                    obligation,
                    left,
                    right,
                },
            )
        }
        TargetIntegerExpression::SaturatingRemainder {
            psi_operation,
            obligation,
            left,
            right,
        } => {
            let obligation = *obligation;
            binary(
                *psi_operation,
                left,
                right,
                locations,
                architecture,
                next_spill,
                move |psi_operation, left, right| AssignedIntegerExpression::SaturatingRemainder {
                    psi_operation,
                    obligation,
                    left,
                    right,
                },
            )
        }
    }
}

fn assign_boolean_expression(
    expression: &TargetBooleanExpression,
    locations: &BTreeMap<usize, AssignedScalarLocation>,
    architecture: Architecture,
    next_spill: &mut u32,
) -> Result<AssignedBooleanExpression, AssignmentError> {
    match expression {
        TargetBooleanExpression::Call {
            psi_operation,
            source_value,
            callee,
            arguments,
        } => Ok(AssignedBooleanExpression::Call {
            psi_operation: *psi_operation,
            source_value: *source_value,
            callee: *callee,
            arguments: assign_call_arguments(arguments, locations, architecture, next_spill)?,
        }),
        TargetBooleanExpression::Immediate {
            source_value,
            value,
        } => Ok(AssignedBooleanExpression::Immediate {
            source_value: *source_value,
            value: *value,
        }),
        TargetBooleanExpression::Parameter {
            source_value,
            parameter_index,
            ..
        } => Ok(AssignedBooleanExpression::Parameter {
            source_value: *source_value,
            parameter_index: *parameter_index,
            location: *locations.get(parameter_index).ok_or(
                AssignmentError::ExpressionParameterAssignmentMissing {
                    value: *source_value,
                    parameter_index: *parameter_index,
                },
            )?,
        }),
        TargetBooleanExpression::StructuralField {
            psi_operation,
            source_value,
            source,
            field,
            source_placement,
            field_byte_offset,
        } => Ok(AssignedBooleanExpression::StructuralField {
            psi_operation: *psi_operation,
            source_value: *source_value,
            source: *source,
            field: *field,
            source_placement: source_placement.clone(),
            field_byte_offset: *field_byte_offset,
        }),
        TargetBooleanExpression::Not {
            psi_operation,
            operand,
        } => Ok(AssignedBooleanExpression::Not {
            psi_operation: *psi_operation,
            operand: Box::new(assign_boolean_expression(
                operand,
                locations,
                architecture,
                next_spill,
            )?),
        }),
        TargetBooleanExpression::Equal {
            psi_operation,
            left,
            right,
        } => Ok(AssignedBooleanExpression::Equal {
            psi_operation: *psi_operation,
            left: Box::new(assign_boolean_expression(
                left,
                locations,
                architecture,
                next_spill,
            )?),
            right: Box::new(assign_boolean_expression(
                right,
                locations,
                architecture,
                next_spill,
            )?),
        }),
        TargetBooleanExpression::IntegerEqual {
            psi_operation,
            scalar_type,
            left,
            right,
        } => Ok(AssignedBooleanExpression::IntegerEqual {
            psi_operation: *psi_operation,
            scalar_type: *scalar_type,
            left: Box::new(assign_expression(
                left,
                locations,
                architecture,
                next_spill,
            )?),
            right: Box::new(assign_expression(
                right,
                locations,
                architecture,
                next_spill,
            )?),
        }),
        TargetBooleanExpression::IntegerLessThan {
            psi_operation,
            scalar_type,
            left,
            right,
        } => Ok(AssignedBooleanExpression::IntegerLessThan {
            psi_operation: *psi_operation,
            scalar_type: *scalar_type,
            left: Box::new(assign_expression(
                left,
                locations,
                architecture,
                next_spill,
            )?),
            right: Box::new(assign_expression(
                right,
                locations,
                architecture,
                next_spill,
            )?),
        }),
        TargetBooleanExpression::IntegerLessOrEqual {
            psi_operation,
            scalar_type,
            left,
            right,
        } => Ok(AssignedBooleanExpression::IntegerLessOrEqual {
            psi_operation: *psi_operation,
            scalar_type: *scalar_type,
            left: Box::new(assign_expression(
                left,
                locations,
                architecture,
                next_spill,
            )?),
            right: Box::new(assign_expression(
                right,
                locations,
                architecture,
                next_spill,
            )?),
        }),
    }
}

fn expression_parameter_locations(
    expression: &TargetIntegerExpression,
) -> Result<BTreeMap<usize, (ValueId, ScalarParameterLocation)>, AssignmentError> {
    fn collect(
        expression: &TargetIntegerExpression,
        locations: &mut BTreeMap<usize, (ValueId, ScalarParameterLocation)>,
    ) -> Result<(), AssignmentError> {
        match expression {
            TargetIntegerExpression::Call { arguments, .. } => {
                for argument in arguments {
                    let nested = match &argument.expression {
                        TargetScalarExpression::Boolean(expression) => {
                            boolean_expression_parameter_locations(expression)?
                        }
                        TargetScalarExpression::Integer { expression, .. } => {
                            expression_parameter_locations(expression)?
                        }
                    };
                    merge_expression_locations(locations, nested)?;
                }
            }
            TargetIntegerExpression::Immediate { .. } => {}
            TargetIntegerExpression::Parameter {
                source_value,
                parameter_index,
                location,
            } => {
                if let Some((_, established)) = locations.get(parameter_index) {
                    if established != location {
                        return Err(AssignmentError::ExpressionParameterLocationConflict {
                            value: *source_value,
                            parameter_index: *parameter_index,
                        });
                    }
                } else {
                    locations.insert(*parameter_index, (*source_value, *location));
                }
            }
            TargetIntegerExpression::BitwiseNot { operand, .. } => {
                collect(operand, locations)?;
            }
            TargetIntegerExpression::IntegerWiden { operand, .. } => {
                collect(operand, locations)?;
            }
            TargetIntegerExpression::IntegerExactCast { operand, .. } => {
                collect(operand, locations)?;
            }
            TargetIntegerExpression::WrappingAdd { left, right, .. }
            | TargetIntegerExpression::ExactAdd { left, right, .. }
            | TargetIntegerExpression::BitwiseAnd { left, right, .. }
            | TargetIntegerExpression::BitwiseOr { left, right, .. }
            | TargetIntegerExpression::BitwiseXor { left, right, .. }
            | TargetIntegerExpression::WrappingShiftLeft {
                value: left,
                count: right,
                ..
            }
            | TargetIntegerExpression::WrappingShiftRight {
                value: left,
                count: right,
                ..
            }
            | TargetIntegerExpression::ExactShiftRight {
                value: left,
                count: right,
                ..
            }
            | TargetIntegerExpression::ExactShiftLeft {
                value: left,
                count: right,
                ..
            }
            | TargetIntegerExpression::SaturatingAdd { left, right, .. }
            | TargetIntegerExpression::WrappingSubtract { left, right, .. }
            | TargetIntegerExpression::ExactSubtract { left, right, .. }
            | TargetIntegerExpression::SaturatingSubtract { left, right, .. }
            | TargetIntegerExpression::WrappingMultiply { left, right, .. }
            | TargetIntegerExpression::ExactMultiply { left, right, .. }
            | TargetIntegerExpression::SaturatingMultiply { left, right, .. } => {
                collect(left, locations)?;
                collect(right, locations)?;
            }
            TargetIntegerExpression::ExactDivide { left, right, .. } => {
                collect(left, locations)?;
                collect(right, locations)?;
            }
            TargetIntegerExpression::ExactRemainder { left, right, .. } => {
                collect(left, locations)?;
                collect(right, locations)?;
            }
            TargetIntegerExpression::WrappingDivide { left, right, .. } => {
                collect(left, locations)?;
                collect(right, locations)?;
            }
            TargetIntegerExpression::WrappingRemainder { left, right, .. } => {
                collect(left, locations)?;
                collect(right, locations)?;
            }
            TargetIntegerExpression::SaturatingDivide { left, right, .. } => {
                collect(left, locations)?;
                collect(right, locations)?;
            }
            TargetIntegerExpression::SaturatingRemainder { left, right, .. } => {
                collect(left, locations)?;
                collect(right, locations)?;
            }
        }
        Ok(())
    }
    let mut locations = BTreeMap::new();
    collect(expression, &mut locations)?;
    Ok(locations)
}

fn boolean_expression_parameter_locations(
    expression: &TargetBooleanExpression,
) -> Result<BTreeMap<usize, (ValueId, ScalarParameterLocation)>, AssignmentError> {
    fn collect(
        expression: &TargetBooleanExpression,
        locations: &mut BTreeMap<usize, (ValueId, ScalarParameterLocation)>,
    ) -> Result<(), AssignmentError> {
        match expression {
            TargetBooleanExpression::Call { arguments, .. } => {
                for argument in arguments {
                    let nested = match &argument.expression {
                        TargetScalarExpression::Boolean(expression) => {
                            boolean_expression_parameter_locations(expression)?
                        }
                        TargetScalarExpression::Integer { expression, .. } => {
                            expression_parameter_locations(expression)?
                        }
                    };
                    merge_expression_locations(locations, nested)?;
                }
            }
            TargetBooleanExpression::Immediate { .. } => {}
            TargetBooleanExpression::StructuralField { .. } => {}
            TargetBooleanExpression::Parameter {
                source_value,
                parameter_index,
                location,
            } => {
                if let Some((_, established)) = locations.get(parameter_index) {
                    if established != location {
                        return Err(AssignmentError::ExpressionParameterLocationConflict {
                            value: *source_value,
                            parameter_index: *parameter_index,
                        });
                    }
                } else {
                    locations.insert(*parameter_index, (*source_value, *location));
                }
            }
            TargetBooleanExpression::Not { operand, .. } => {
                collect(operand, locations)?;
            }
            TargetBooleanExpression::Equal { left, right, .. } => {
                collect(left, locations)?;
                collect(right, locations)?;
            }
            TargetBooleanExpression::IntegerEqual { left, right, .. }
            | TargetBooleanExpression::IntegerLessThan { left, right, .. }
            | TargetBooleanExpression::IntegerLessOrEqual { left, right, .. } => {
                merge_expression_locations(locations, expression_parameter_locations(left)?)?;
                merge_expression_locations(locations, expression_parameter_locations(right)?)?;
            }
        }
        Ok(())
    }

    let mut locations = BTreeMap::new();
    collect(expression, &mut locations)?;
    Ok(locations)
}

fn integer_control_arms_parameter_locations(
    when_true: &omega_target_operations::TargetConditionalIntegerArm,
    when_false: &omega_target_operations::TargetConditionalIntegerArm,
) -> Result<BTreeMap<usize, (ValueId, ScalarParameterLocation)>, AssignmentError> {
    let mut locations = integer_control_parameter_locations(&when_true.control)?;
    merge_expression_locations(
        &mut locations,
        integer_control_parameter_locations(&when_false.control)?,
    )?;
    Ok(locations)
}

fn integer_control_parameter_locations(
    control: &TargetIntegerControl,
) -> Result<BTreeMap<usize, (ValueId, ScalarParameterLocation)>, AssignmentError> {
    let mut locations = BTreeMap::new();
    match control {
        TargetIntegerControl::Crash { .. } => {}
        TargetIntegerControl::Return { expression, .. } => {
            locations = expression_parameter_locations(expression)?;
        }
        TargetIntegerControl::Conditional {
            condition_source,
            condition_parameter_index,
            condition_location,
            when_true,
            when_false,
        } => {
            locations.insert(
                *condition_parameter_index,
                (*condition_source, *condition_location),
            );
            merge_expression_locations(
                &mut locations,
                integer_control_arms_parameter_locations(when_true, when_false)?,
            )?;
        }
        TargetIntegerControl::ConditionalExpression {
            condition,
            when_true,
            when_false,
            ..
        } => {
            locations = boolean_expression_parameter_locations(condition)?;
            merge_expression_locations(
                &mut locations,
                integer_control_arms_parameter_locations(when_true, when_false)?,
            )?;
        }
    }
    Ok(locations)
}

fn boolean_control_arms_parameter_locations(
    when_true: &omega_target_operations::TargetConditionalBooleanArm,
    when_false: &omega_target_operations::TargetConditionalBooleanArm,
) -> Result<BTreeMap<usize, (ValueId, ScalarParameterLocation)>, AssignmentError> {
    let mut locations = boolean_control_parameter_locations(&when_true.control)?;
    merge_expression_locations(
        &mut locations,
        boolean_control_parameter_locations(&when_false.control)?,
    )?;
    Ok(locations)
}

fn boolean_control_parameter_locations(
    control: &TargetBooleanControl,
) -> Result<BTreeMap<usize, (ValueId, ScalarParameterLocation)>, AssignmentError> {
    let mut locations = BTreeMap::new();
    match control {
        TargetBooleanControl::Crash { .. } | TargetBooleanControl::ReturnImmediate { .. } => {}
        TargetBooleanControl::ReturnParameter {
            source_value,
            parameter_index,
            location,
            ..
        }
        | TargetBooleanControl::ReturnNotParameter {
            source_value,
            parameter_index,
            location,
            ..
        } => {
            locations.insert(*parameter_index, (*source_value, *location));
        }
        TargetBooleanControl::ReturnExpression { expression, .. } => {
            locations = boolean_expression_parameter_locations(expression)?;
        }
        TargetBooleanControl::Conditional {
            condition_source,
            condition_parameter_index,
            condition_location,
            when_true,
            when_false,
        } => {
            locations.insert(
                *condition_parameter_index,
                (*condition_source, *condition_location),
            );
            merge_expression_locations(
                &mut locations,
                boolean_control_arms_parameter_locations(when_true, when_false)?,
            )?;
        }
        TargetBooleanControl::ConditionalExpression {
            condition,
            when_true,
            when_false,
            ..
        } => {
            locations = boolean_expression_parameter_locations(condition)?;
            merge_expression_locations(
                &mut locations,
                boolean_control_arms_parameter_locations(when_true, when_false)?,
            )?;
        }
    }
    Ok(locations)
}

fn merge_expression_locations(
    locations: &mut BTreeMap<usize, (ValueId, ScalarParameterLocation)>,
    nested: BTreeMap<usize, (ValueId, ScalarParameterLocation)>,
) -> Result<(), AssignmentError> {
    for (parameter_index, (source_value, location)) in nested {
        if let Some((_, established)) = locations.get(&parameter_index) {
            if established != &location {
                return Err(AssignmentError::ExpressionParameterLocationConflict {
                    value: source_value,
                    parameter_index,
                });
            }
        } else {
            locations.insert(parameter_index, (source_value, location));
        }
    }
    Ok(())
}

fn require_register_architecture(
    value: ValueId,
    register: MachineRegister,
    architecture: Architecture,
) -> Result<(), AssignmentError> {
    let matches = match architecture {
        Architecture::Aarch64 => matches!(register, MachineRegister::Aarch64X(0..=30)),
        Architecture::X86_64 => matches!(
            register,
            MachineRegister::X86Rax
                | MachineRegister::X86Rcx
                | MachineRegister::X86Rdx
                | MachineRegister::X86Rbx
                | MachineRegister::X86Rsp
                | MachineRegister::X86Rbp
                | MachineRegister::X86Rsi
                | MachineRegister::X86Rdi
                | MachineRegister::X86R8
                | MachineRegister::X86R9
                | MachineRegister::X86R10
                | MachineRegister::X86R11
                | MachineRegister::X86R12
                | MachineRegister::X86R13
                | MachineRegister::X86R14
                | MachineRegister::X86R15
        ),
    };
    if matches {
        Ok(())
    } else {
        Err(AssignmentError::ParameterRegisterArchitectureMismatch {
            value,
            register,
            architecture,
        })
    }
}

fn x86_expression_scratch_conflict(register: MachineRegister) -> bool {
    matches!(
        register,
        MachineRegister::X86Rax | MachineRegister::X86R10 | MachineRegister::X86R11
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignmentError {
    EntryFunctionMissing(MachineId),
    UnsupportedScalarCleanup(MachineId),
    InstalledProviderCallRequiresOptimizedLane {
        machine: MachineId,
        operation: OperationId,
        boundary: psi_core::BoundaryMachineId,
    },
    BoundaryPortReadUnsupported {
        machine: MachineId,
        architecture: Architecture,
    },
    LinuxExitGroupUnsupported {
        machine: MachineId,
        target: NativeTarget,
    },
    LinuxExitGroupArgumentMismatch(MachineId),
    UnsupportedStructuralPlacement(psi_core::PlaceId),
    StructuralRegisterArchitectureMismatch {
        place: psi_core::PlaceId,
        register: MachineRegister,
        architecture: Architecture,
    },
    ParameterRegisterArchitectureMismatch {
        value: ValueId,
        register: MachineRegister,
        architecture: Architecture,
    },
    ExpressionParameterLocationConflict {
        value: ValueId,
        parameter_index: usize,
    },
    ExpressionParameterAssignmentMissing {
        value: ValueId,
        parameter_index: usize,
    },
    ExpressionStackFrameNotEncodable,
    ExpressionRegisterCannotHoldParameter {
        value: ValueId,
        register: MachineRegister,
    },
    UnsupportedCallArgumentRegister(MachineRegister),
}

impl std::fmt::Display for AssignmentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for AssignmentError {}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_assigned_target_operations::{
        AssignedBooleanControl, AssignedIntegerExpression, AssignedOperation,
        AssignedScalarExpression, AssignedScalarLocation,
    };
    use omega_target::NativeTarget;
    use omega_target_operations::{
        TargetBooleanControl, TargetCallArgument, TargetConditionalBooleanArm, TargetFunction,
        TargetIntegerExpression, TargetOperation, TargetScalarExpression,
        TargetStructuralParameter, TerminalPsiProvenance,
    };
    use psi_core::{
        EdgeId, IntegerSign, IntegerType, ObligationId, OperationId, PlaceId, ScalarType,
        StructuralTypeId,
    };
    use psi_terminal::{
        SemanticFingerprint, StructuralMultiplicity, StructuralPathSegment,
        TerminalAffineCleanupAction, TerminalPsiIdentity, VocabularyMarker,
    };

    #[test]
    fn three_leaf_boolean_cleanup_assignment_retains_exact_edges() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let plan = boolean_cleanup_plan(target);
            let assigned = assign_registers(&plan).expect("assign bounded Boolean cleanup");
            let AssignedOperation::BooleanControlWithCleanup {
                control,
                structural_parameters,
                cleanup_actions,
                ..
            } = &assigned.functions[0].operation
            else {
                panic!("fixture must retain its Boolean cleanup carrier")
            };
            assert_eq!(structural_parameters.len(), 1);
            assert_eq!(cleanup_actions.len(), 1);
            let AssignedBooleanControl::Conditional {
                when_true,
                when_false,
                ..
            } = control
            else {
                panic!("root decision must survive assignment")
            };
            let AssignedBooleanControl::Conditional {
                when_true: nested_true,
                when_false: nested_false,
                ..
            } = when_true.control.as_ref()
            else {
                panic!("true arm must retain the nested decision")
            };
            assert!(matches!(
                nested_true.control.as_ref(),
                AssignedBooleanControl::ReturnImmediate {
                    psi_return_edge,
                    ..
                } if *psi_return_edge == EdgeId::new(10).unwrap()
            ));
            assert!(matches!(
                nested_false.control.as_ref(),
                AssignedBooleanControl::ReturnParameter {
                    psi_return_edge,
                    ..
                } if *psi_return_edge == EdgeId::new(11).unwrap()
            ));
            assert!(matches!(
                when_false.control.as_ref(),
                AssignedBooleanControl::ReturnNotParameter {
                    psi_return_edge,
                    ..
                } if *psi_return_edge == EdgeId::new(12).unwrap()
            ));
        }
    }

    #[test]
    fn finite_boolean_cleanup_accepts_two_leaf_and_wider_trees() {
        let mut two_leaf = boolean_cleanup_plan(NativeTarget::linux_x64());
        let TargetOperation::BooleanControlWithCleanup { control, .. } =
            &mut two_leaf.functions[0].operation
        else {
            unreachable!()
        };
        let TargetBooleanControl::Conditional { when_true, .. } = control else {
            unreachable!()
        };
        when_true.control = Box::new(boolean_immediate_return(13));
        assign_registers(&two_leaf).expect("assign two-leaf Boolean cleanup");

        let mut wider = boolean_cleanup_plan(NativeTarget::linux_x64());
        let location = boolean_cleanup_condition_location(&wider);
        let TargetOperation::BooleanControlWithCleanup { control, .. } =
            &mut wider.functions[0].operation
        else {
            unreachable!()
        };
        let TargetBooleanControl::Conditional { when_true, .. } = control else {
            unreachable!()
        };
        let TargetBooleanControl::Conditional {
            when_true: nested_true,
            ..
        } = when_true.control.as_mut()
        else {
            unreachable!()
        };
        nested_true.control = Box::new(TargetBooleanControl::Conditional {
            condition_source: ValueId::new(1).unwrap(),
            condition_parameter_index: 0,
            condition_location: location,
            when_true: boolean_arm(20, boolean_immediate_return(20)),
            when_false: boolean_arm(21, boolean_immediate_return(21)),
        });
        assign_registers(&wider).expect("assign wider Boolean cleanup");
    }

    #[test]
    fn finite_boolean_cleanup_requires_distinct_return_edges() {
        let mut plan = boolean_cleanup_plan(NativeTarget::linux_x64());
        let TargetOperation::BooleanControlWithCleanup { control, .. } =
            &mut plan.functions[0].operation
        else {
            unreachable!()
        };
        let TargetBooleanControl::Conditional { when_true, .. } = control else {
            unreachable!()
        };
        let TargetBooleanControl::Conditional { when_false, .. } = when_true.control.as_mut()
        else {
            unreachable!()
        };
        when_false.control = Box::new(boolean_immediate_return(10));
        assert!(matches!(
            assign_registers(&plan),
            Err(AssignmentError::UnsupportedScalarCleanup(_))
        ));
    }

    #[test]
    fn finite_boolean_cleanup_rejects_misaligned_cleanup_signature() {
        let mut plan = boolean_cleanup_plan(NativeTarget::linux_x64());
        let TargetOperation::BooleanControlWithCleanup {
            cleanup_actions, ..
        } = &mut plan.functions[0].operation
        else {
            unreachable!()
        };
        cleanup_actions.clear();
        assert!(matches!(
            assign_registers(&plan),
            Err(AssignmentError::UnsupportedScalarCleanup(_))
        ));
    }

    #[test]
    fn aarch64_expression_registers_receive_stable_frame_spills() {
        let plan = expression_plan(
            NativeTarget::linux_arm64(),
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
        );
        let assigned = assign_registers(&plan).expect("assign AArch64 homes");
        let AssignedOperation::ReturnIntegerExpression {
            frame, expression, ..
        } = &assigned.functions[0].operation
        else {
            panic!("fixture must remain an expression")
        };
        assert_eq!(frame.byte_size, 16);
        assert_eq!(frame.register_spills.len(), 2);
        assert_eq!(frame.register_spills[0].byte_offset, 0);
        assert_eq!(frame.register_spills[1].byte_offset, 8);
        let AssignedIntegerExpression::WrappingAdd { left, right, .. } = expression else {
            panic!("fixture must remain wrapping addition")
        };
        assert!(matches!(
            left.as_ref(),
            AssignedIntegerExpression::Parameter {
                location: AssignedScalarLocation::FrameSpill { byte_offset: 0 },
                ..
            }
        ));
        assert!(matches!(
            right.as_ref(),
            AssignedIntegerExpression::Parameter {
                location: AssignedScalarLocation::FrameSpill { byte_offset: 8 },
                ..
            }
        ));
    }

    #[test]
    fn x86_expression_registers_remain_explicit_without_a_frame() {
        let plan = expression_plan(
            NativeTarget::linux_x64(),
            ScalarParameterLocation::Register(MachineRegister::X86Rdi),
            ScalarParameterLocation::IncomingStack { byte_offset: 16 },
        );
        let assigned = assign_registers(&plan).expect("assign x86-64 homes");
        let AssignedOperation::ReturnIntegerExpression {
            frame, expression, ..
        } = &assigned.functions[0].operation
        else {
            panic!("fixture must remain an expression")
        };
        assert_eq!(frame.byte_size, 0);
        assert!(frame.register_spills.is_empty());
        let AssignedIntegerExpression::WrappingAdd { left, right, .. } = expression else {
            panic!("fixture must remain wrapping addition")
        };
        assert!(matches!(
            left.as_ref(),
            AssignedIntegerExpression::Parameter {
                location: AssignedScalarLocation::Register(MachineRegister::X86Rdi),
                ..
            }
        ));
        assert!(matches!(
            right.as_ref(),
            AssignedIntegerExpression::Parameter {
                location: AssignedScalarLocation::IncomingStack { byte_offset: 16 },
                ..
            }
        ));
    }

    #[test]
    fn exact_arithmetic_obligation_survives_register_assignment() {
        let obligation = ObligationId::new(17).expect("obligation");
        let mut plan = expression_plan(
            NativeTarget::linux_x64(),
            ScalarParameterLocation::Register(MachineRegister::X86Rdi),
            ScalarParameterLocation::Register(MachineRegister::X86Rsi),
        );
        let TargetOperation::ReturnIntegerExpression { expression, .. } =
            &mut plan.functions[0].operation
        else {
            unreachable!()
        };
        let TargetIntegerExpression::WrappingAdd {
            psi_operation,
            left,
            right,
        } = std::mem::replace(
            expression,
            TargetIntegerExpression::Immediate {
                source_value: ValueId::new(3).expect("result"),
                value: psi_core::IntegerValue::Unsigned(0),
            },
        )
        else {
            unreachable!()
        };
        *expression = TargetIntegerExpression::ExactAdd {
            psi_operation,
            obligation,
            left,
            right,
        };

        let assigned = assign_registers(&plan).expect("assign exact arithmetic homes");
        let AssignedOperation::ReturnIntegerExpression { expression, .. } =
            &assigned.functions[0].operation
        else {
            panic!("fixture must remain an expression")
        };
        assert!(matches!(
            expression,
            AssignedIntegerExpression::ExactAdd {
                obligation: retained,
                ..
            } if *retained == obligation
        ));
    }

    #[test]
    fn x86_scratch_conflicting_parameter_receives_a_frame_spill() {
        let plan = expression_plan(
            NativeTarget::linux_x64(),
            ScalarParameterLocation::Register(MachineRegister::X86R10),
            ScalarParameterLocation::Register(MachineRegister::X86Rdi),
        );
        let assigned = assign_registers(&plan).expect("assign x86-64 scratch conflict");
        let AssignedOperation::ReturnIntegerExpression {
            frame, expression, ..
        } = &assigned.functions[0].operation
        else {
            panic!("fixture must remain an expression")
        };
        assert_eq!(frame.byte_size, 16);
        assert_eq!(frame.register_spills.len(), 1);
        assert_eq!(frame.register_spills[0].register, MachineRegister::X86R10);
        let AssignedIntegerExpression::WrappingAdd { left, .. } = expression else {
            panic!("fixture must remain wrapping addition")
        };
        assert!(matches!(
            left.as_ref(),
            AssignedIntegerExpression::Parameter {
                location: AssignedScalarLocation::FrameSpill { byte_offset: 0 },
                ..
            }
        ));
    }

    #[test]
    fn x86_calling_expression_spills_live_caller_registers() {
        let scalar_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let mut plan = expression_plan(
            NativeTarget::linux_x64(),
            ScalarParameterLocation::Register(MachineRegister::X86Rdi),
            ScalarParameterLocation::Register(MachineRegister::X86Rsi),
        );
        let TargetOperation::ReturnIntegerExpression { expression, .. } =
            &mut plan.functions[0].operation
        else {
            unreachable!()
        };
        *expression = TargetIntegerExpression::WrappingAdd {
            psi_operation: OperationId::new(8).unwrap(),
            left: Box::new(TargetIntegerExpression::Call {
                psi_operation: OperationId::new(7).unwrap(),
                source_value: ValueId::new(4).unwrap(),
                callee: MachineId::new(2).unwrap(),
                arguments: vec![TargetCallArgument {
                    scalar_type: ScalarType::Integer(scalar_type),
                    location: ScalarParameterLocation::Register(MachineRegister::X86Rdi),
                    expression: TargetScalarExpression::Integer {
                        scalar_type,
                        expression: TargetIntegerExpression::Parameter {
                            source_value: ValueId::new(1).unwrap(),
                            parameter_index: 0,
                            location: ScalarParameterLocation::Register(MachineRegister::X86Rdi),
                        },
                    },
                }],
            }),
            right: Box::new(TargetIntegerExpression::Parameter {
                source_value: ValueId::new(1).unwrap(),
                parameter_index: 0,
                location: ScalarParameterLocation::Register(MachineRegister::X86Rdi),
            }),
        };

        let assigned = assign_registers(&plan).expect("assign call-preserved parameter");
        let AssignedOperation::ReturnIntegerExpression {
            frame, expression, ..
        } = &assigned.functions[0].operation
        else {
            unreachable!()
        };
        assert_eq!(frame.byte_size, 32);
        assert_eq!(frame.register_spills.len(), 1);
        let AssignedIntegerExpression::WrappingAdd { left, right, .. } = expression else {
            unreachable!()
        };
        let AssignedIntegerExpression::Call { arguments, .. } = left.as_ref() else {
            unreachable!()
        };
        assert!(matches!(
            &arguments[0].expression,
            AssignedScalarExpression::Integer {
                expression: AssignedIntegerExpression::Parameter {
                    location: AssignedScalarLocation::FrameSpill { byte_offset: 0 },
                    ..
                },
                ..
            }
        ));
        assert!(matches!(
            right.as_ref(),
            AssignedIntegerExpression::Parameter {
                location: AssignedScalarLocation::FrameSpill { byte_offset: 0 },
                ..
            }
        ));
    }

    #[test]
    fn call_stack_arguments_receive_concrete_outgoing_homes() {
        let scalar_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let mut plan = expression_plan(
            NativeTarget::linux_x64(),
            ScalarParameterLocation::Register(MachineRegister::X86Rdi),
            ScalarParameterLocation::Register(MachineRegister::X86Rsi),
        );
        let TargetOperation::ReturnIntegerExpression { expression, .. } =
            &mut plan.functions[0].operation
        else {
            unreachable!()
        };
        *expression = TargetIntegerExpression::Call {
            psi_operation: OperationId::new(7).unwrap(),
            source_value: ValueId::new(4).unwrap(),
            callee: MachineId::new(2).unwrap(),
            arguments: vec![TargetCallArgument {
                scalar_type: ScalarType::Integer(scalar_type),
                location: ScalarParameterLocation::IncomingStack { byte_offset: 8 },
                expression: TargetScalarExpression::Integer {
                    scalar_type,
                    expression: TargetIntegerExpression::Immediate {
                        source_value: ValueId::new(5).unwrap(),
                        value: psi_core::IntegerValue::Unsigned(9),
                    },
                },
            }],
        };

        let assigned = assign_registers(&plan).expect("assign outgoing stack argument");
        let AssignedOperation::ReturnIntegerExpression {
            frame, expression, ..
        } = &assigned.functions[0].operation
        else {
            unreachable!()
        };
        assert_eq!(frame.byte_size, 16);
        let AssignedIntegerExpression::Call { arguments, .. } = expression else {
            unreachable!()
        };
        assert_eq!(arguments[0].spill_byte_offset, 0);
        assert_eq!(
            arguments[0].destination,
            AssignedCallDestination::OutgoingStack { byte_offset: 8 }
        );
    }

    #[test]
    fn x86_stack_pointer_cannot_be_an_expression_parameter_home() {
        let plan = expression_plan(
            NativeTarget::linux_x64(),
            ScalarParameterLocation::Register(MachineRegister::X86Rsp),
            ScalarParameterLocation::Register(MachineRegister::X86Rdi),
        );
        assert!(matches!(
            assign_registers(&plan),
            Err(AssignmentError::ExpressionRegisterCannotHoldParameter {
                register: MachineRegister::X86Rsp,
                ..
            })
        ));
    }

    #[test]
    fn repeated_parameter_location_drift_rejects_before_emission() {
        let mut plan = expression_plan(
            NativeTarget::linux_x64(),
            ScalarParameterLocation::Register(MachineRegister::X86Rdi),
            ScalarParameterLocation::Register(MachineRegister::X86Rsi),
        );
        let TargetOperation::ReturnIntegerExpression { expression, .. } =
            &mut plan.functions[0].operation
        else {
            panic!("fixture must contain an expression")
        };
        let TargetIntegerExpression::WrappingAdd { right, .. } = expression else {
            panic!("fixture must contain wrapping addition")
        };
        let TargetIntegerExpression::Parameter {
            parameter_index, ..
        } = right.as_mut()
        else {
            panic!("right operand must be a parameter")
        };
        *parameter_index = 0;
        assert!(matches!(
            assign_registers(&plan),
            Err(AssignmentError::ExpressionParameterLocationConflict {
                parameter_index: 0,
                ..
            })
        ));
    }

    #[test]
    fn cross_architecture_register_rejects_during_assignment() {
        let plan = expression_plan(
            NativeTarget::linux_arm64(),
            ScalarParameterLocation::Register(MachineRegister::X86Rdi),
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
        );
        assert!(matches!(
            assign_registers(&plan),
            Err(AssignmentError::ParameterRegisterArchitectureMismatch {
                architecture: Architecture::Aarch64,
                ..
            })
        ));
    }

    #[test]
    fn unit_assignment_retains_typed_structural_argument_paths() {
        let target = NativeTarget::linux_x64();
        let shape = omega_calling_conventions::ValueShape::integer(8, 8);
        let call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![shape],
                result: None,
            },
        )
        .unwrap();
        let place = PlaceId::new(1).unwrap();
        let structural_type = StructuralTypeId::new(1).unwrap();
        let path = vec![StructuralPathSegment::FixedIndex(1)];
        let plan = TargetOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([3; 32]),
            },
            target,
            entry: MachineId::new(1).unwrap(),
            functions: vec![TargetFunction {
                machine: MachineId::new(1).unwrap(),
                attachment: None,
                provenance: TerminalPsiProvenance::default(),
                operation: TargetOperation::UnitBody(omega_target_operations::TargetUnitBody {
                    structural_types: Vec::new(),
                    call_plan: call_plan.clone(),
                    parameters: Vec::new(),
                    operations: vec![TargetUnitOperation::Call {
                        psi_operation: OperationId::new(1).unwrap(),
                        callee: MachineId::new(2).unwrap(),
                        arguments: vec![omega_target_operations::TargetStructuralArgument {
                            place,
                            access: psi_terminal::StructuralAccess::Owned,
                            path: path.clone(),
                            root_structural_type: structural_type,
                            structural_type,
                            shape,
                            source_byte_offset: 0,
                            fixed_array_length: None,
                            element_stride: None,
                            source: call_plan.parameters[0].clone(),
                            destination: call_plan.parameters[0].clone(),
                        }],
                        claim_transfers: Vec::new(),
                    }],
                }),
            }],
        };

        let assigned = assign_registers(&plan).unwrap();
        let AssignedOperation::UnitBody(body) = &assigned.functions[0].operation else {
            panic!("Unit body")
        };
        let AssignedUnitOperation::Call { copies, .. } = &body.operations[0] else {
            panic!("Unit call")
        };
        assert_eq!(copies[0].path, path);
    }

    fn expression_plan(
        target: NativeTarget,
        left_location: ScalarParameterLocation,
        right_location: ScalarParameterLocation,
    ) -> TargetOperationPlan {
        TargetOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([3; 32]),
            },
            target,
            entry: MachineId::new(1).expect("machine"),
            functions: vec![TargetFunction {
                machine: MachineId::new(1).expect("machine"),
                attachment: None,
                provenance: TerminalPsiProvenance::default(),
                operation: TargetOperation::ReturnIntegerExpression {
                    psi_edge: EdgeId::new(1).expect("edge"),
                    source_value: ValueId::new(3).expect("result"),
                    scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
                    expression: TargetIntegerExpression::WrappingAdd {
                        psi_operation: OperationId::new(1).expect("operation"),
                        left: Box::new(TargetIntegerExpression::Parameter {
                            source_value: ValueId::new(1).expect("left"),
                            parameter_index: 0,
                            location: left_location,
                        }),
                        right: Box::new(TargetIntegerExpression::Parameter {
                            source_value: ValueId::new(2).expect("right"),
                            parameter_index: 1,
                            location: right_location,
                        }),
                    },
                },
            }],
        }
    }

    fn boolean_cleanup_plan(target: NativeTarget) -> TargetOperationPlan {
        let scalar_shape = omega_calling_conventions::ValueShape::integer(1, 1);
        let structural_shape = omega_calling_conventions::ValueShape::integer(8, 8);
        let call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![scalar_shape, structural_shape],
                result: Some(scalar_shape),
            },
        )
        .expect("bounded Boolean cleanup ABI");
        let [ValueLocation::Register { register, .. }] =
            call_plan.parameters[0].locations.as_slice()
        else {
            panic!("first Boolean input must have one direct register home")
        };
        let condition_location = ScalarParameterLocation::Register(*register);
        let nested = TargetBooleanControl::Conditional {
            condition_source: ValueId::new(1).unwrap(),
            condition_parameter_index: 0,
            condition_location,
            when_true: boolean_arm(4, boolean_immediate_return(10)),
            when_false: boolean_arm(
                5,
                TargetBooleanControl::ReturnParameter {
                    psi_return_edge: EdgeId::new(11).unwrap(),
                    source_value: ValueId::new(1).unwrap(),
                    parameter_index: 0,
                    location: condition_location,
                },
            ),
        };
        let place = PlaceId::new(1).unwrap();
        TargetOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
            },
            target,
            entry: MachineId::new(1).unwrap(),
            functions: vec![TargetFunction {
                machine: MachineId::new(1).unwrap(),
                attachment: None,
                provenance: TerminalPsiProvenance {
                    operations: Vec::new(),
                    edges: (1..=5)
                        .chain(10..=12)
                        .map(|edge| EdgeId::new(edge).unwrap())
                        .collect(),
                },
                operation: TargetOperation::BooleanControlWithCleanup {
                    control: TargetBooleanControl::Conditional {
                        condition_source: ValueId::new(1).unwrap(),
                        condition_parameter_index: 0,
                        condition_location,
                        when_true: boolean_arm(2, nested),
                        when_false: boolean_arm(
                            3,
                            TargetBooleanControl::ReturnNotParameter {
                                psi_return_edge: EdgeId::new(12).unwrap(),
                                source_value: ValueId::new(1).unwrap(),
                                parameter_index: 0,
                                location: condition_location,
                            },
                        ),
                    },
                    structural_types: Vec::new(),
                    call_plan: call_plan.clone(),
                    structural_parameters: vec![TargetStructuralParameter {
                        place,
                        structural_type: StructuralTypeId::new(1).unwrap(),
                        multiplicity: StructuralMultiplicity::Affine,
                        access: psi_terminal::StructuralAccess::Owned,
                        shape: structural_shape,
                        placement: call_plan.parameters[1].clone(),
                    }],
                    cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(place)],
                },
            }],
        }
    }

    fn boolean_arm(edge: u64, control: TargetBooleanControl) -> TargetConditionalBooleanArm {
        TargetConditionalBooleanArm {
            psi_edge: EdgeId::new(edge).unwrap(),
            control: Box::new(control),
        }
    }

    fn boolean_immediate_return(edge: u64) -> TargetBooleanControl {
        TargetBooleanControl::ReturnImmediate {
            psi_return_edge: EdgeId::new(edge).unwrap(),
            source_value: ValueId::new(edge).unwrap(),
            value: edge % 2 == 0,
        }
    }

    fn boolean_cleanup_condition_location(plan: &TargetOperationPlan) -> ScalarParameterLocation {
        let TargetOperation::BooleanControlWithCleanup { control, .. } =
            &plan.functions[0].operation
        else {
            unreachable!()
        };
        let TargetBooleanControl::Conditional {
            condition_location, ..
        } = control
        else {
            unreachable!()
        };
        *condition_location
    }
}
