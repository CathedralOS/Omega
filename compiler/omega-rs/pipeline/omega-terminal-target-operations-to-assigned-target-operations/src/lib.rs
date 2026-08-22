#![forbid(unsafe_code)]

//! Assign concrete register and stack homes to clean terminal-Psi target
//! operations before machine emission.

use std::collections::BTreeMap;

use omega_calling_conventions::{CallSignature, CallingPolicy, ValueLocation, evaluate_call_plan};
use omega_target::{Architecture, NativeTarget};
use omega_terminal_assigned_target_operations::{
    TerminalAssignedAggregateCopy, TerminalAssignedBooleanControl,
    TerminalAssignedBooleanExpression, TerminalAssignedCallArgument,
    TerminalAssignedCallDestination, TerminalAssignedConditionalBooleanArm,
    TerminalAssignedConditionalIntegerArm, TerminalAssignedFunction,
    TerminalAssignedIntegerControl, TerminalAssignedIntegerExpression, TerminalAssignedOperation,
    TerminalAssignedOperationPlan, TerminalAssignedScalarExpression,
    TerminalAssignedScalarLocation, TerminalAssignedUnitBody, TerminalAssignedUnitOperation,
    TerminalEntryRegisterSpill, TerminalExpressionFrame,
};
use omega_terminal_target_operations::{
    MachineRegister, TerminalScalarParameterLocation, TerminalTargetBooleanControl,
    TerminalTargetBooleanExpression, TerminalTargetCallArgument, TerminalTargetFunction,
    TerminalTargetIntegerControl, TerminalTargetIntegerExpression, TerminalTargetOperation,
    TerminalTargetOperationPlan, TerminalTargetScalarExpression, TerminalTargetUnitOperation,
};
use psi_core::{EdgeId, MachineId, OperationId, ValueId};

mod structural_scalar;

pub fn assign_registers(
    plan: &TerminalTargetOperationPlan,
) -> Result<TerminalAssignedOperationPlan, AssignmentError> {
    if !plan
        .functions
        .iter()
        .any(|function| function.machine == plan.entry)
    {
        return Err(AssignmentError::EntryFunctionMissing(plan.entry));
    }
    Ok(TerminalAssignedOperationPlan {
        terminal_psi: plan.terminal_psi,
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
    function: &TerminalTargetFunction,
    target: NativeTarget,
) -> Result<TerminalAssignedFunction, AssignmentError> {
    let architecture = target.architecture;
    let operation = match &function.operation {
        operation @ TerminalTargetOperation::ReturnStructuralScalarCall { .. } => {
            structural_scalar::assign(function.machine, operation, target)?
        }
        TerminalTargetOperation::ScalarReturnWithCleanup {
            scalar,
            structural_types,
            call_plan,
            structural_parameters,
            cleanup_actions,
            psi_edge,
        } => {
            if matches!(
                scalar.as_ref(),
                TerminalTargetOperation::ScalarReturnWithCleanup { .. }
                    | TerminalTargetOperation::BooleanControlWithCleanup { .. }
                    | TerminalTargetOperation::ReturnBoundaryPortReadU8 { .. }
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
                &TerminalTargetFunction {
                    machine: function.machine,
                    attachment: function.attachment,
                    provenance: function.provenance.clone(),
                    operation: scalar.as_ref().clone(),
                },
                target,
            )?
            .operation;
            TerminalAssignedOperation::ScalarReturnWithCleanup {
                scalar: Box::new(assigned_scalar),
                structural_types: structural_types.clone(),
                call_plan: call_plan.clone(),
                structural_parameters: structural_parameters.clone(),
                cleanup_actions: cleanup_actions.clone(),
                psi_edge: *psi_edge,
            }
        }
        TerminalTargetOperation::ReturnBoundaryPortReadU8 {
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
            TerminalAssignedOperation::ReturnBoundaryPortReadU8 {
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
        TerminalTargetOperation::BooleanControlWithCleanup {
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
            TerminalAssignedOperation::BooleanControlWithCleanup {
                control: assign_boolean_control(control, architecture)?,
                structural_types: structural_types.clone(),
                call_plan: call_plan.clone(),
                structural_parameters: structural_parameters.clone(),
                cleanup_actions: cleanup_actions.clone(),
            }
        }
        TerminalTargetOperation::UnitBody(body) => {
            let operations = body
                .operations
                .iter()
                .map(|operation| match operation {
                    TerminalTargetUnitOperation::EstablishTrivialAffineLocal {
                        psi_operation,
                        place,
                        structural_type,
                    } => TerminalAssignedUnitOperation::EstablishTrivialAffineLocal {
                        psi_operation: *psi_operation,
                        place: place.clone(),
                        structural_type: structural_type.clone(),
                    },
                    TerminalTargetUnitOperation::Call {
                        psi_operation,
                        callee,
                        arguments,
                        claim_transfers,
                    } => TerminalAssignedUnitOperation::Call {
                        psi_operation: *psi_operation,
                        callee: *callee,
                        result: None,
                        copies: arguments
                            .iter()
                            .map(|argument| TerminalAssignedAggregateCopy {
                                place: argument.place,
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
                    TerminalTargetUnitOperation::PortWrite {
                        psi_operation,
                        service,
                        port,
                        value,
                    } => TerminalAssignedUnitOperation::PortWrite {
                        psi_operation: *psi_operation,
                        service: *service,
                        port: *port,
                        value: *value,
                    },
                    TerminalTargetUnitOperation::BoundarySettlement {
                        psi_operation,
                        boundary,
                        provider_execution,
                        realization,
                        arguments,
                        completion_claim_sources,
                        completion_receipts,
                    } => TerminalAssignedUnitOperation::BoundarySettlement {
                        psi_operation: *psi_operation,
                        boundary: *boundary,
                        provider_execution: *provider_execution,
                        realization: (*realization).into(),
                        arguments: arguments.clone(),
                        completion_claim_sources: completion_claim_sources.clone(),
                        completion_receipts: completion_receipts.clone(),
                    },
                    TerminalTargetUnitOperation::Return {
                        psi_edge,
                        cleanup_actions,
                    } => TerminalAssignedUnitOperation::Return {
                        psi_edge: *psi_edge,
                        cleanup_actions: cleanup_actions.clone(),
                    },
                })
                .collect();
            TerminalAssignedOperation::UnitBody(TerminalAssignedUnitBody {
                structural_types: body.structural_types.clone(),
                call_plan: body.call_plan.clone(),
                parameters: body.parameters.clone(),
                operations,
            })
        }
        TerminalTargetOperation::ReturnStructuralParameter {
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
                validate_structural_placement(parameter.place, placement, architecture)?;
            }
            validate_structural_placement(result.place, result_placement, architecture)?;
            TerminalAssignedOperation::ReturnStructuralParameter {
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
        TerminalTargetOperation::Crash {
            psi_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        } => TerminalAssignedOperation::Crash {
            psi_edge: *psi_edge,
            cause: *cause,
            site_guard: site_guard.clone(),
            frontier_lower_bound: frontier_lower_bound.clone(),
        },
        TerminalTargetOperation::ReturnIntegerImmediate {
            psi_edge,
            source_value,
            scalar_type,
            value,
        } => TerminalAssignedOperation::ReturnIntegerImmediate {
            psi_edge: *psi_edge,
            source_value: *source_value,
            scalar_type: *scalar_type,
            value: *value,
        },
        TerminalTargetOperation::ReturnBooleanImmediate {
            psi_edge,
            source_value,
            value,
        } => TerminalAssignedOperation::ReturnBooleanImmediate {
            psi_edge: *psi_edge,
            source_value: *source_value,
            value: *value,
        },
        TerminalTargetOperation::ReturnIntegerParameter {
            psi_edge,
            source_value,
            scalar_type,
            parameter_index,
            location,
        } => TerminalAssignedOperation::ReturnIntegerParameter {
            psi_edge: *psi_edge,
            source_value: *source_value,
            scalar_type: *scalar_type,
            parameter_index: *parameter_index,
            location: assign_direct_location(*source_value, *location, architecture)?,
        },
        TerminalTargetOperation::ReturnBooleanParameter {
            psi_edge,
            source_value,
            parameter_index,
            location,
        } => TerminalAssignedOperation::ReturnBooleanParameter {
            psi_edge: *psi_edge,
            source_value: *source_value,
            parameter_index: *parameter_index,
            location: assign_direct_location(*source_value, *location, architecture)?,
        },
        TerminalTargetOperation::ReturnBooleanNotParameter {
            psi_edge,
            source_value,
            parameter_index,
            location,
        } => TerminalAssignedOperation::ReturnBooleanNotParameter {
            psi_edge: *psi_edge,
            source_value: *source_value,
            parameter_index: *parameter_index,
            location: assign_direct_location(*source_value, *location, architecture)?,
        },
        TerminalTargetOperation::ReturnBooleanSharedConvergence { psi_edge, control } => {
            TerminalAssignedOperation::ReturnBooleanSharedConvergence {
                psi_edge: *psi_edge,
                control: assign_boolean_control(control, architecture)?,
            }
        }
        TerminalTargetOperation::ReturnBooleanExpression {
            psi_edge,
            source_value,
            expression,
        } => {
            let (frame, expression) = assign_boolean_expression_frame(expression, architecture)?;
            TerminalAssignedOperation::ReturnBooleanExpression {
                psi_edge: *psi_edge,
                source_value: *source_value,
                frame,
                expression,
            }
        }
        TerminalTargetOperation::ReturnIntegerExpression {
            psi_edge,
            source_value,
            scalar_type,
            expression,
        } => {
            let (frame, expression) = assign_integer_expression_frame(expression, architecture)?;
            TerminalAssignedOperation::ReturnIntegerExpression {
                psi_edge: *psi_edge,
                source_value: *source_value,
                scalar_type: *scalar_type,
                frame,
                expression,
            }
        }
        TerminalTargetOperation::ReturnIntegerConditionalControl {
            condition_source,
            condition_parameter_index,
            condition_location,
            scalar_type,
            when_true,
            when_false,
        } => TerminalAssignedOperation::ReturnIntegerConditionalControl {
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
        TerminalTargetOperation::ReturnIntegerExpressionConditionalControl {
            condition_source,
            condition,
            scalar_type,
            when_true,
            when_false,
        } => {
            let preserved = integer_control_arms_parameter_locations(when_true, when_false)?;
            let (condition_frame, condition) =
                assign_boolean_expression_frame_preserving(condition, architecture, preserved)?;
            TerminalAssignedOperation::ReturnIntegerExpressionConditionalControl {
                condition_source: *condition_source,
                condition_frame,
                condition,
                scalar_type: *scalar_type,
                when_true: assign_control_arm(when_true, architecture)?,
                when_false: assign_control_arm(when_false, architecture)?,
            }
        }
        TerminalTargetOperation::ReturnBooleanConditionalControl {
            condition_source,
            condition_parameter_index,
            condition_location,
            when_true,
            when_false,
        } => TerminalAssignedOperation::ReturnBooleanConditionalControl {
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
        TerminalTargetOperation::ReturnBooleanExpressionConditionalControl {
            condition_source,
            condition,
            when_true,
            when_false,
        } => {
            let preserved = boolean_control_arms_parameter_locations(when_true, when_false)?;
            let (condition_frame, condition) =
                assign_boolean_expression_frame_preserving(condition, architecture, preserved)?;
            TerminalAssignedOperation::ReturnBooleanExpressionConditionalControl {
                condition_source: *condition_source,
                condition_frame,
                condition,
                when_true: assign_boolean_control_arm(when_true, architecture)?,
                when_false: assign_boolean_control_arm(when_false, architecture)?,
            }
        }
    };
    Ok(TerminalAssignedFunction {
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
    structural_parameters: &[omega_terminal_target_operations::TerminalTargetStructuralParameter],
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
fn finite_boolean_cleanup_edges(control: &TerminalTargetBooleanControl) -> Option<Vec<EdgeId>> {
    fn collect(control: &TerminalTargetBooleanControl, edges: &mut Vec<EdgeId>) -> Option<()> {
        match control {
            TerminalTargetBooleanControl::ReturnImmediate {
                psi_return_edge, ..
            }
            | TerminalTargetBooleanControl::ReturnParameter {
                psi_return_edge, ..
            }
            | TerminalTargetBooleanControl::ReturnNotParameter {
                psi_return_edge, ..
            }
            | TerminalTargetBooleanControl::ReturnExpression {
                psi_return_edge, ..
            } => edges.push(*psi_return_edge),
            TerminalTargetBooleanControl::Conditional {
                when_true,
                when_false,
                ..
            }
            | TerminalTargetBooleanControl::ConditionalExpression {
                when_true,
                when_false,
                ..
            } => {
                collect(&when_true.control, edges)?;
                collect(&when_false.control, edges)?;
            }
            TerminalTargetBooleanControl::Crash { .. } => return None,
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
    let matches_architecture = match (architecture, register) {
        (Architecture::X86_64, omega_terminal_target_operations::MachineRegister::X86Rax)
        | (Architecture::X86_64, omega_terminal_target_operations::MachineRegister::X86Rcx)
        | (Architecture::X86_64, omega_terminal_target_operations::MachineRegister::X86Rdx)
        | (Architecture::X86_64, omega_terminal_target_operations::MachineRegister::X86Rbx)
        | (Architecture::X86_64, omega_terminal_target_operations::MachineRegister::X86Rsp)
        | (Architecture::X86_64, omega_terminal_target_operations::MachineRegister::X86Rbp)
        | (Architecture::X86_64, omega_terminal_target_operations::MachineRegister::X86Rsi)
        | (Architecture::X86_64, omega_terminal_target_operations::MachineRegister::X86Rdi)
        | (Architecture::X86_64, omega_terminal_target_operations::MachineRegister::X86R8)
        | (Architecture::X86_64, omega_terminal_target_operations::MachineRegister::X86R9)
        | (Architecture::X86_64, omega_terminal_target_operations::MachineRegister::X86R10)
        | (Architecture::X86_64, omega_terminal_target_operations::MachineRegister::X86R11)
        | (Architecture::X86_64, omega_terminal_target_operations::MachineRegister::X86R12)
        | (Architecture::X86_64, omega_terminal_target_operations::MachineRegister::X86R13)
        | (Architecture::X86_64, omega_terminal_target_operations::MachineRegister::X86R14)
        | (Architecture::X86_64, omega_terminal_target_operations::MachineRegister::X86R15)
        | (
            Architecture::Aarch64,
            omega_terminal_target_operations::MachineRegister::Aarch64X(0..=30),
        ) => true,
        _ => false,
    };
    if !matches_architecture {
        return Err(AssignmentError::StructuralRegisterArchitectureMismatch {
            place,
            register: *register,
            architecture,
        });
    }
    Ok(())
}

fn assign_boolean_control_arm(
    arm: &omega_terminal_target_operations::TerminalTargetConditionalBooleanArm,
    architecture: Architecture,
) -> Result<TerminalAssignedConditionalBooleanArm, AssignmentError> {
    Ok(TerminalAssignedConditionalBooleanArm {
        psi_edge: arm.psi_edge,
        control: Box::new(assign_boolean_control(&arm.control, architecture)?),
    })
}

fn assign_boolean_control(
    control: &TerminalTargetBooleanControl,
    architecture: Architecture,
) -> Result<TerminalAssignedBooleanControl, AssignmentError> {
    Ok(match control {
        TerminalTargetBooleanControl::Crash {
            psi_crash_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        } => TerminalAssignedBooleanControl::Crash {
            psi_crash_edge: *psi_crash_edge,
            cause: *cause,
            site_guard: site_guard.clone(),
            frontier_lower_bound: frontier_lower_bound.clone(),
        },
        TerminalTargetBooleanControl::ReturnImmediate {
            psi_return_edge,
            source_value,
            value,
        } => TerminalAssignedBooleanControl::ReturnImmediate {
            psi_return_edge: *psi_return_edge,
            source_value: *source_value,
            value: *value,
        },
        TerminalTargetBooleanControl::ReturnParameter {
            psi_return_edge,
            source_value,
            parameter_index,
            location,
        } => TerminalAssignedBooleanControl::ReturnParameter {
            psi_return_edge: *psi_return_edge,
            source_value: *source_value,
            parameter_index: *parameter_index,
            location: assign_direct_location(*source_value, *location, architecture)?,
        },
        TerminalTargetBooleanControl::ReturnNotParameter {
            psi_return_edge,
            source_value,
            parameter_index,
            location,
        } => TerminalAssignedBooleanControl::ReturnNotParameter {
            psi_return_edge: *psi_return_edge,
            source_value: *source_value,
            parameter_index: *parameter_index,
            location: assign_direct_location(*source_value, *location, architecture)?,
        },
        TerminalTargetBooleanControl::ReturnExpression {
            psi_return_edge,
            source_value,
            expression,
        } => {
            let (frame, expression) = assign_boolean_expression_frame(expression, architecture)?;
            TerminalAssignedBooleanControl::ReturnExpression {
                psi_return_edge: *psi_return_edge,
                source_value: *source_value,
                frame,
                expression,
            }
        }
        TerminalTargetBooleanControl::Conditional {
            condition_source,
            condition_parameter_index,
            condition_location,
            when_true,
            when_false,
        } => TerminalAssignedBooleanControl::Conditional {
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
        TerminalTargetBooleanControl::ConditionalExpression {
            condition_source,
            condition,
            when_true,
            when_false,
        } => {
            let preserved = boolean_control_arms_parameter_locations(when_true, when_false)?;
            let (condition_frame, condition) =
                assign_boolean_expression_frame_preserving(condition, architecture, preserved)?;
            TerminalAssignedBooleanControl::ConditionalExpression {
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
    arm: &omega_terminal_target_operations::TerminalTargetConditionalIntegerArm,
    architecture: Architecture,
) -> Result<TerminalAssignedConditionalIntegerArm, AssignmentError> {
    Ok(TerminalAssignedConditionalIntegerArm {
        psi_edge: arm.psi_edge,
        control: Box::new(assign_integer_control(&arm.control, architecture)?),
    })
}

fn assign_integer_control(
    control: &TerminalTargetIntegerControl,
    architecture: Architecture,
) -> Result<TerminalAssignedIntegerControl, AssignmentError> {
    Ok(match control {
        TerminalTargetIntegerControl::Crash {
            psi_crash_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        } => TerminalAssignedIntegerControl::Crash {
            psi_crash_edge: *psi_crash_edge,
            cause: *cause,
            site_guard: site_guard.clone(),
            frontier_lower_bound: frontier_lower_bound.clone(),
        },
        TerminalTargetIntegerControl::Return {
            psi_return_edge,
            source_value,
            expression,
        } => {
            let (frame, expression) = assign_integer_expression_frame(expression, architecture)?;
            TerminalAssignedIntegerControl::Return {
                psi_return_edge: *psi_return_edge,
                source_value: *source_value,
                frame,
                expression,
            }
        }
        TerminalTargetIntegerControl::Conditional {
            condition_source,
            condition_parameter_index,
            condition_location,
            when_true,
            when_false,
        } => TerminalAssignedIntegerControl::Conditional {
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
        TerminalTargetIntegerControl::ConditionalExpression {
            condition_source,
            condition,
            when_true,
            when_false,
        } => {
            let preserved = integer_control_arms_parameter_locations(when_true, when_false)?;
            let (condition_frame, condition) =
                assign_boolean_expression_frame_preserving(condition, architecture, preserved)?;
            TerminalAssignedIntegerControl::ConditionalExpression {
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
    location: TerminalScalarParameterLocation,
    architecture: Architecture,
) -> Result<TerminalAssignedScalarLocation, AssignmentError> {
    Ok(match location {
        TerminalScalarParameterLocation::Register(register) => {
            require_register_architecture(source_value, register, architecture)?;
            TerminalAssignedScalarLocation::Register(register)
        }
        TerminalScalarParameterLocation::IncomingStack { byte_offset } => {
            TerminalAssignedScalarLocation::IncomingStack { byte_offset }
        }
    })
}

fn assign_expression_locations(
    architecture: Architecture,
    locations: &BTreeMap<usize, (ValueId, TerminalScalarParameterLocation)>,
    force_register_spills: bool,
) -> Result<
    (
        TerminalExpressionFrame,
        BTreeMap<usize, TerminalAssignedScalarLocation>,
    ),
    AssignmentError,
> {
    let mut register_spills = Vec::new();
    let mut assigned = BTreeMap::new();
    for (&parameter_index, &(source_value, location)) in locations {
        match location {
            TerminalScalarParameterLocation::Register(register) => {
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
                    register_spills.push(TerminalEntryRegisterSpill {
                        source_value,
                        parameter_index,
                        register,
                        byte_offset,
                    });
                    assigned.insert(
                        parameter_index,
                        TerminalAssignedScalarLocation::FrameSpill { byte_offset },
                    );
                } else {
                    assigned.insert(
                        parameter_index,
                        TerminalAssignedScalarLocation::Register(register),
                    );
                }
            }
            TerminalScalarParameterLocation::IncomingStack { byte_offset } => {
                assigned.insert(
                    parameter_index,
                    TerminalAssignedScalarLocation::IncomingStack { byte_offset },
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
        TerminalExpressionFrame {
            byte_size,
            register_spills,
        },
        assigned,
    ))
}

fn assign_integer_expression_frame(
    expression: &TerminalTargetIntegerExpression,
    architecture: Architecture,
) -> Result<(TerminalExpressionFrame, TerminalAssignedIntegerExpression), AssignmentError> {
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
    expression: &TerminalTargetBooleanExpression,
    architecture: Architecture,
) -> Result<(TerminalExpressionFrame, TerminalAssignedBooleanExpression), AssignmentError> {
    assign_boolean_expression_frame_preserving(expression, architecture, BTreeMap::new())
}

fn assign_boolean_expression_frame_preserving(
    expression: &TerminalTargetBooleanExpression,
    architecture: Architecture,
    preserved: BTreeMap<usize, (ValueId, TerminalScalarParameterLocation)>,
) -> Result<(TerminalExpressionFrame, TerminalAssignedBooleanExpression), AssignmentError> {
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

fn integer_expression_contains_call(expression: &TerminalTargetIntegerExpression) -> bool {
    match expression {
        TerminalTargetIntegerExpression::Call { .. } => true,
        TerminalTargetIntegerExpression::Immediate { .. }
        | TerminalTargetIntegerExpression::Parameter { .. } => false,
        TerminalTargetIntegerExpression::BitwiseNot { operand, .. }
        | TerminalTargetIntegerExpression::IntegerWiden { operand, .. }
        | TerminalTargetIntegerExpression::IntegerExactCast { operand, .. } => {
            integer_expression_contains_call(operand)
        }
        TerminalTargetIntegerExpression::WrappingAdd { left, right, .. }
        | TerminalTargetIntegerExpression::BitwiseAnd { left, right, .. }
        | TerminalTargetIntegerExpression::BitwiseOr { left, right, .. }
        | TerminalTargetIntegerExpression::BitwiseXor { left, right, .. }
        | TerminalTargetIntegerExpression::WrappingShiftLeft {
            value: left,
            count: right,
            ..
        }
        | TerminalTargetIntegerExpression::WrappingShiftRight {
            value: left,
            count: right,
            ..
        }
        | TerminalTargetIntegerExpression::ExactShiftLeft {
            value: left,
            count: right,
            ..
        }
        | TerminalTargetIntegerExpression::ExactShiftRight {
            value: left,
            count: right,
            ..
        }
        | TerminalTargetIntegerExpression::SaturatingAdd { left, right, .. }
        | TerminalTargetIntegerExpression::WrappingSubtract { left, right, .. }
        | TerminalTargetIntegerExpression::SaturatingSubtract { left, right, .. }
        | TerminalTargetIntegerExpression::WrappingMultiply { left, right, .. }
        | TerminalTargetIntegerExpression::SaturatingMultiply { left, right, .. }
        | TerminalTargetIntegerExpression::ExactDivide { left, right, .. }
        | TerminalTargetIntegerExpression::ExactRemainder { left, right, .. }
        | TerminalTargetIntegerExpression::WrappingDivide { left, right, .. }
        | TerminalTargetIntegerExpression::WrappingRemainder { left, right, .. }
        | TerminalTargetIntegerExpression::SaturatingDivide { left, right, .. }
        | TerminalTargetIntegerExpression::SaturatingRemainder { left, right, .. } => {
            integer_expression_contains_call(left) || integer_expression_contains_call(right)
        }
    }
}

fn boolean_expression_contains_call(expression: &TerminalTargetBooleanExpression) -> bool {
    match expression {
        TerminalTargetBooleanExpression::Call { .. } => true,
        TerminalTargetBooleanExpression::Immediate { .. }
        | TerminalTargetBooleanExpression::Parameter { .. }
        | TerminalTargetBooleanExpression::StructuralField { .. } => false,
        TerminalTargetBooleanExpression::Not { operand, .. } => {
            boolean_expression_contains_call(operand)
        }
        TerminalTargetBooleanExpression::Equal { left, right, .. } => {
            boolean_expression_contains_call(left) || boolean_expression_contains_call(right)
        }
        TerminalTargetBooleanExpression::IntegerEqual { left, right, .. }
        | TerminalTargetBooleanExpression::IntegerLessThan { left, right, .. }
        | TerminalTargetBooleanExpression::IntegerLessOrEqual { left, right, .. } => {
            integer_expression_contains_call(left) || integer_expression_contains_call(right)
        }
    }
}

fn assign_call_arguments(
    arguments: &[TerminalTargetCallArgument],
    locations: &BTreeMap<usize, TerminalAssignedScalarLocation>,
    architecture: Architecture,
    next_spill: &mut u32,
) -> Result<Vec<TerminalAssignedCallArgument>, AssignmentError> {
    arguments
        .iter()
        .map(|argument| {
            let expression = match &argument.expression {
                TerminalTargetScalarExpression::Boolean(expression) => {
                    TerminalAssignedScalarExpression::Boolean(assign_boolean_expression(
                        expression,
                        locations,
                        architecture,
                        next_spill,
                    )?)
                }
                TerminalTargetScalarExpression::Integer {
                    scalar_type,
                    expression,
                } => TerminalAssignedScalarExpression::Integer {
                    scalar_type: *scalar_type,
                    expression: assign_expression(expression, locations, architecture, next_spill)?,
                },
            };
            let destination = match argument.location {
                TerminalScalarParameterLocation::Register(register) => {
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
                    TerminalAssignedCallDestination::Register(register)
                }
                TerminalScalarParameterLocation::IncomingStack { byte_offset } => {
                    TerminalAssignedCallDestination::OutgoingStack { byte_offset }
                }
            };
            let spill_byte_offset = *next_spill;
            *next_spill = next_spill
                .checked_add(8)
                .ok_or(AssignmentError::ExpressionStackFrameNotEncodable)?;
            Ok(TerminalAssignedCallArgument {
                scalar_type: argument.scalar_type,
                destination,
                spill_byte_offset,
                expression,
            })
        })
        .collect()
}

fn assign_expression(
    expression: &TerminalTargetIntegerExpression,
    locations: &BTreeMap<usize, TerminalAssignedScalarLocation>,
    architecture: Architecture,
    next_spill: &mut u32,
) -> Result<TerminalAssignedIntegerExpression, AssignmentError> {
    fn binary(
        psi_operation: OperationId,
        left: &TerminalTargetIntegerExpression,
        right: &TerminalTargetIntegerExpression,
        locations: &BTreeMap<usize, TerminalAssignedScalarLocation>,
        architecture: Architecture,
        next_spill: &mut u32,
        constructor: fn(
            OperationId,
            Box<TerminalAssignedIntegerExpression>,
            Box<TerminalAssignedIntegerExpression>,
        ) -> TerminalAssignedIntegerExpression,
    ) -> Result<TerminalAssignedIntegerExpression, AssignmentError> {
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
        TerminalTargetIntegerExpression::Call {
            psi_operation,
            source_value,
            callee,
            arguments,
        } => Ok(TerminalAssignedIntegerExpression::Call {
            psi_operation: *psi_operation,
            source_value: *source_value,
            callee: *callee,
            arguments: assign_call_arguments(arguments, locations, architecture, next_spill)?,
        }),
        TerminalTargetIntegerExpression::Immediate {
            source_value,
            value,
        } => Ok(TerminalAssignedIntegerExpression::Immediate {
            source_value: *source_value,
            value: *value,
        }),
        TerminalTargetIntegerExpression::Parameter {
            source_value,
            parameter_index,
            ..
        } => Ok(TerminalAssignedIntegerExpression::Parameter {
            source_value: *source_value,
            parameter_index: *parameter_index,
            location: *locations.get(parameter_index).ok_or(
                AssignmentError::ExpressionParameterAssignmentMissing {
                    value: *source_value,
                    parameter_index: *parameter_index,
                },
            )?,
        }),
        TerminalTargetIntegerExpression::BitwiseNot {
            psi_operation,
            operand,
        } => Ok(TerminalAssignedIntegerExpression::BitwiseNot {
            psi_operation: *psi_operation,
            operand: Box::new(assign_expression(
                operand,
                locations,
                architecture,
                next_spill,
            )?),
        }),
        TerminalTargetIntegerExpression::IntegerWiden {
            psi_operation,
            source_type,
            operand,
        } => Ok(TerminalAssignedIntegerExpression::IntegerWiden {
            psi_operation: *psi_operation,
            source_type: *source_type,
            operand: Box::new(assign_expression(
                operand,
                locations,
                architecture,
                next_spill,
            )?),
        }),
        TerminalTargetIntegerExpression::IntegerExactCast {
            psi_operation,
            source_type,
            operand,
        } => Ok(TerminalAssignedIntegerExpression::IntegerExactCast {
            psi_operation: *psi_operation,
            source_type: *source_type,
            operand: Box::new(assign_expression(
                operand,
                locations,
                architecture,
                next_spill,
            )?),
        }),
        TerminalTargetIntegerExpression::BitwiseAnd {
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
            |psi_operation, left, right| TerminalAssignedIntegerExpression::BitwiseAnd {
                psi_operation,
                left,
                right,
            },
        ),
        TerminalTargetIntegerExpression::BitwiseOr {
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
            |psi_operation, left, right| TerminalAssignedIntegerExpression::BitwiseOr {
                psi_operation,
                left,
                right,
            },
        ),
        TerminalTargetIntegerExpression::BitwiseXor {
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
            |psi_operation, left, right| TerminalAssignedIntegerExpression::BitwiseXor {
                psi_operation,
                left,
                right,
            },
        ),
        TerminalTargetIntegerExpression::WrappingShiftLeft {
            psi_operation,
            count_type,
            value,
            count,
        } => Ok(TerminalAssignedIntegerExpression::WrappingShiftLeft {
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
        TerminalTargetIntegerExpression::WrappingShiftRight {
            psi_operation,
            count_type,
            value,
            count,
        } => Ok(TerminalAssignedIntegerExpression::WrappingShiftRight {
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
        TerminalTargetIntegerExpression::ExactShiftRight {
            psi_operation,
            count_type,
            value,
            count,
        } => Ok(TerminalAssignedIntegerExpression::ExactShiftRight {
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
        TerminalTargetIntegerExpression::ExactShiftLeft {
            psi_operation,
            count_type,
            value,
            count,
        } => Ok(TerminalAssignedIntegerExpression::ExactShiftLeft {
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
        TerminalTargetIntegerExpression::WrappingAdd {
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
            |psi_operation, left, right| TerminalAssignedIntegerExpression::WrappingAdd {
                psi_operation,
                left,
                right,
            },
        ),
        TerminalTargetIntegerExpression::SaturatingAdd {
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
            |psi_operation, left, right| TerminalAssignedIntegerExpression::SaturatingAdd {
                psi_operation,
                left,
                right,
            },
        ),
        TerminalTargetIntegerExpression::WrappingSubtract {
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
            |psi_operation, left, right| TerminalAssignedIntegerExpression::WrappingSubtract {
                psi_operation,
                left,
                right,
            },
        ),
        TerminalTargetIntegerExpression::SaturatingSubtract {
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
            |psi_operation, left, right| TerminalAssignedIntegerExpression::SaturatingSubtract {
                psi_operation,
                left,
                right,
            },
        ),
        TerminalTargetIntegerExpression::WrappingMultiply {
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
            |psi_operation, left, right| TerminalAssignedIntegerExpression::WrappingMultiply {
                psi_operation,
                left,
                right,
            },
        ),
        TerminalTargetIntegerExpression::SaturatingMultiply {
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
            |psi_operation, left, right| TerminalAssignedIntegerExpression::SaturatingMultiply {
                psi_operation,
                left,
                right,
            },
        ),
        TerminalTargetIntegerExpression::ExactDivide {
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
            |psi_operation, left, right| TerminalAssignedIntegerExpression::ExactDivide {
                psi_operation,
                left,
                right,
            },
        ),
        TerminalTargetIntegerExpression::ExactRemainder {
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
            |psi_operation, left, right| TerminalAssignedIntegerExpression::ExactRemainder {
                psi_operation,
                left,
                right,
            },
        ),
        TerminalTargetIntegerExpression::WrappingDivide {
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
            |psi_operation, left, right| TerminalAssignedIntegerExpression::WrappingDivide {
                psi_operation,
                left,
                right,
            },
        ),
        TerminalTargetIntegerExpression::WrappingRemainder {
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
            |psi_operation, left, right| TerminalAssignedIntegerExpression::WrappingRemainder {
                psi_operation,
                left,
                right,
            },
        ),
        TerminalTargetIntegerExpression::SaturatingDivide {
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
            |psi_operation, left, right| TerminalAssignedIntegerExpression::SaturatingDivide {
                psi_operation,
                left,
                right,
            },
        ),
        TerminalTargetIntegerExpression::SaturatingRemainder {
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
            |psi_operation, left, right| TerminalAssignedIntegerExpression::SaturatingRemainder {
                psi_operation,
                left,
                right,
            },
        ),
    }
}

fn assign_boolean_expression(
    expression: &TerminalTargetBooleanExpression,
    locations: &BTreeMap<usize, TerminalAssignedScalarLocation>,
    architecture: Architecture,
    next_spill: &mut u32,
) -> Result<TerminalAssignedBooleanExpression, AssignmentError> {
    match expression {
        TerminalTargetBooleanExpression::Call {
            psi_operation,
            source_value,
            callee,
            arguments,
        } => Ok(TerminalAssignedBooleanExpression::Call {
            psi_operation: *psi_operation,
            source_value: *source_value,
            callee: *callee,
            arguments: assign_call_arguments(arguments, locations, architecture, next_spill)?,
        }),
        TerminalTargetBooleanExpression::Immediate {
            source_value,
            value,
        } => Ok(TerminalAssignedBooleanExpression::Immediate {
            source_value: *source_value,
            value: *value,
        }),
        TerminalTargetBooleanExpression::Parameter {
            source_value,
            parameter_index,
            ..
        } => Ok(TerminalAssignedBooleanExpression::Parameter {
            source_value: *source_value,
            parameter_index: *parameter_index,
            location: *locations.get(parameter_index).ok_or(
                AssignmentError::ExpressionParameterAssignmentMissing {
                    value: *source_value,
                    parameter_index: *parameter_index,
                },
            )?,
        }),
        TerminalTargetBooleanExpression::StructuralField {
            psi_operation,
            source_value,
            source,
            field,
            source_placement,
            field_byte_offset,
        } => Ok(TerminalAssignedBooleanExpression::StructuralField {
            psi_operation: *psi_operation,
            source_value: *source_value,
            source: *source,
            field: *field,
            source_placement: source_placement.clone(),
            field_byte_offset: *field_byte_offset,
        }),
        TerminalTargetBooleanExpression::Not {
            psi_operation,
            operand,
        } => Ok(TerminalAssignedBooleanExpression::Not {
            psi_operation: *psi_operation,
            operand: Box::new(assign_boolean_expression(
                operand,
                locations,
                architecture,
                next_spill,
            )?),
        }),
        TerminalTargetBooleanExpression::Equal {
            psi_operation,
            left,
            right,
        } => Ok(TerminalAssignedBooleanExpression::Equal {
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
        TerminalTargetBooleanExpression::IntegerEqual {
            psi_operation,
            scalar_type,
            left,
            right,
        } => Ok(TerminalAssignedBooleanExpression::IntegerEqual {
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
        TerminalTargetBooleanExpression::IntegerLessThan {
            psi_operation,
            scalar_type,
            left,
            right,
        } => Ok(TerminalAssignedBooleanExpression::IntegerLessThan {
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
        TerminalTargetBooleanExpression::IntegerLessOrEqual {
            psi_operation,
            scalar_type,
            left,
            right,
        } => Ok(TerminalAssignedBooleanExpression::IntegerLessOrEqual {
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
    expression: &TerminalTargetIntegerExpression,
) -> Result<BTreeMap<usize, (ValueId, TerminalScalarParameterLocation)>, AssignmentError> {
    fn collect(
        expression: &TerminalTargetIntegerExpression,
        locations: &mut BTreeMap<usize, (ValueId, TerminalScalarParameterLocation)>,
    ) -> Result<(), AssignmentError> {
        match expression {
            TerminalTargetIntegerExpression::Call { arguments, .. } => {
                for argument in arguments {
                    let nested = match &argument.expression {
                        TerminalTargetScalarExpression::Boolean(expression) => {
                            boolean_expression_parameter_locations(expression)?
                        }
                        TerminalTargetScalarExpression::Integer { expression, .. } => {
                            expression_parameter_locations(expression)?
                        }
                    };
                    merge_expression_locations(locations, nested)?;
                }
            }
            TerminalTargetIntegerExpression::Immediate { .. } => {}
            TerminalTargetIntegerExpression::Parameter {
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
            TerminalTargetIntegerExpression::BitwiseNot { operand, .. } => {
                collect(operand, locations)?;
            }
            TerminalTargetIntegerExpression::IntegerWiden { operand, .. } => {
                collect(operand, locations)?;
            }
            TerminalTargetIntegerExpression::IntegerExactCast { operand, .. } => {
                collect(operand, locations)?;
            }
            TerminalTargetIntegerExpression::WrappingAdd { left, right, .. }
            | TerminalTargetIntegerExpression::BitwiseAnd { left, right, .. }
            | TerminalTargetIntegerExpression::BitwiseOr { left, right, .. }
            | TerminalTargetIntegerExpression::BitwiseXor { left, right, .. }
            | TerminalTargetIntegerExpression::WrappingShiftLeft {
                value: left,
                count: right,
                ..
            }
            | TerminalTargetIntegerExpression::WrappingShiftRight {
                value: left,
                count: right,
                ..
            }
            | TerminalTargetIntegerExpression::ExactShiftRight {
                value: left,
                count: right,
                ..
            }
            | TerminalTargetIntegerExpression::ExactShiftLeft {
                value: left,
                count: right,
                ..
            }
            | TerminalTargetIntegerExpression::SaturatingAdd { left, right, .. }
            | TerminalTargetIntegerExpression::WrappingSubtract { left, right, .. }
            | TerminalTargetIntegerExpression::SaturatingSubtract { left, right, .. }
            | TerminalTargetIntegerExpression::WrappingMultiply { left, right, .. }
            | TerminalTargetIntegerExpression::SaturatingMultiply { left, right, .. } => {
                collect(left, locations)?;
                collect(right, locations)?;
            }
            TerminalTargetIntegerExpression::ExactDivide { left, right, .. } => {
                collect(left, locations)?;
                collect(right, locations)?;
            }
            TerminalTargetIntegerExpression::ExactRemainder { left, right, .. } => {
                collect(left, locations)?;
                collect(right, locations)?;
            }
            TerminalTargetIntegerExpression::WrappingDivide { left, right, .. } => {
                collect(left, locations)?;
                collect(right, locations)?;
            }
            TerminalTargetIntegerExpression::WrappingRemainder { left, right, .. } => {
                collect(left, locations)?;
                collect(right, locations)?;
            }
            TerminalTargetIntegerExpression::SaturatingDivide { left, right, .. } => {
                collect(left, locations)?;
                collect(right, locations)?;
            }
            TerminalTargetIntegerExpression::SaturatingRemainder { left, right, .. } => {
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
    expression: &TerminalTargetBooleanExpression,
) -> Result<BTreeMap<usize, (ValueId, TerminalScalarParameterLocation)>, AssignmentError> {
    fn collect(
        expression: &TerminalTargetBooleanExpression,
        locations: &mut BTreeMap<usize, (ValueId, TerminalScalarParameterLocation)>,
    ) -> Result<(), AssignmentError> {
        match expression {
            TerminalTargetBooleanExpression::Call { arguments, .. } => {
                for argument in arguments {
                    let nested = match &argument.expression {
                        TerminalTargetScalarExpression::Boolean(expression) => {
                            boolean_expression_parameter_locations(expression)?
                        }
                        TerminalTargetScalarExpression::Integer { expression, .. } => {
                            expression_parameter_locations(expression)?
                        }
                    };
                    merge_expression_locations(locations, nested)?;
                }
            }
            TerminalTargetBooleanExpression::Immediate { .. } => {}
            TerminalTargetBooleanExpression::StructuralField { .. } => {}
            TerminalTargetBooleanExpression::Parameter {
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
            TerminalTargetBooleanExpression::Not { operand, .. } => {
                collect(operand, locations)?;
            }
            TerminalTargetBooleanExpression::Equal { left, right, .. } => {
                collect(left, locations)?;
                collect(right, locations)?;
            }
            TerminalTargetBooleanExpression::IntegerEqual { left, right, .. }
            | TerminalTargetBooleanExpression::IntegerLessThan { left, right, .. }
            | TerminalTargetBooleanExpression::IntegerLessOrEqual { left, right, .. } => {
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
    when_true: &omega_terminal_target_operations::TerminalTargetConditionalIntegerArm,
    when_false: &omega_terminal_target_operations::TerminalTargetConditionalIntegerArm,
) -> Result<BTreeMap<usize, (ValueId, TerminalScalarParameterLocation)>, AssignmentError> {
    let mut locations = integer_control_parameter_locations(&when_true.control)?;
    merge_expression_locations(
        &mut locations,
        integer_control_parameter_locations(&when_false.control)?,
    )?;
    Ok(locations)
}

fn integer_control_parameter_locations(
    control: &TerminalTargetIntegerControl,
) -> Result<BTreeMap<usize, (ValueId, TerminalScalarParameterLocation)>, AssignmentError> {
    let mut locations = BTreeMap::new();
    match control {
        TerminalTargetIntegerControl::Crash { .. } => {}
        TerminalTargetIntegerControl::Return { expression, .. } => {
            locations = expression_parameter_locations(expression)?;
        }
        TerminalTargetIntegerControl::Conditional {
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
        TerminalTargetIntegerControl::ConditionalExpression {
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
    when_true: &omega_terminal_target_operations::TerminalTargetConditionalBooleanArm,
    when_false: &omega_terminal_target_operations::TerminalTargetConditionalBooleanArm,
) -> Result<BTreeMap<usize, (ValueId, TerminalScalarParameterLocation)>, AssignmentError> {
    let mut locations = boolean_control_parameter_locations(&when_true.control)?;
    merge_expression_locations(
        &mut locations,
        boolean_control_parameter_locations(&when_false.control)?,
    )?;
    Ok(locations)
}

fn boolean_control_parameter_locations(
    control: &TerminalTargetBooleanControl,
) -> Result<BTreeMap<usize, (ValueId, TerminalScalarParameterLocation)>, AssignmentError> {
    let mut locations = BTreeMap::new();
    match control {
        TerminalTargetBooleanControl::Crash { .. }
        | TerminalTargetBooleanControl::ReturnImmediate { .. } => {}
        TerminalTargetBooleanControl::ReturnParameter {
            source_value,
            parameter_index,
            location,
            ..
        }
        | TerminalTargetBooleanControl::ReturnNotParameter {
            source_value,
            parameter_index,
            location,
            ..
        } => {
            locations.insert(*parameter_index, (*source_value, *location));
        }
        TerminalTargetBooleanControl::ReturnExpression { expression, .. } => {
            locations = boolean_expression_parameter_locations(expression)?;
        }
        TerminalTargetBooleanControl::Conditional {
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
        TerminalTargetBooleanControl::ConditionalExpression {
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
    locations: &mut BTreeMap<usize, (ValueId, TerminalScalarParameterLocation)>,
    nested: BTreeMap<usize, (ValueId, TerminalScalarParameterLocation)>,
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
    BoundaryPortReadUnsupported {
        machine: MachineId,
        architecture: Architecture,
    },
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
    use omega_target::NativeTarget;
    use omega_terminal_assigned_target_operations::{
        TerminalAssignedBooleanControl, TerminalAssignedIntegerExpression,
        TerminalAssignedOperation, TerminalAssignedScalarExpression,
        TerminalAssignedScalarLocation,
    };
    use omega_terminal_target_operations::{
        TerminalPsiProvenance, TerminalTargetBooleanControl, TerminalTargetCallArgument,
        TerminalTargetConditionalBooleanArm, TerminalTargetFunction,
        TerminalTargetIntegerExpression, TerminalTargetOperation, TerminalTargetScalarExpression,
        TerminalTargetStructuralParameter,
    };
    use psi_core::{
        EdgeId, IntegerSign, IntegerType, OperationId, PlaceId, ScalarType, StructuralTypeId,
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
            let TerminalAssignedOperation::BooleanControlWithCleanup {
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
            let TerminalAssignedBooleanControl::Conditional {
                when_true,
                when_false,
                ..
            } = control
            else {
                panic!("root decision must survive assignment")
            };
            let TerminalAssignedBooleanControl::Conditional {
                when_true: nested_true,
                when_false: nested_false,
                ..
            } = when_true.control.as_ref()
            else {
                panic!("true arm must retain the nested decision")
            };
            assert!(matches!(
                nested_true.control.as_ref(),
                TerminalAssignedBooleanControl::ReturnImmediate {
                    psi_return_edge,
                    ..
                } if *psi_return_edge == EdgeId::new(10).unwrap()
            ));
            assert!(matches!(
                nested_false.control.as_ref(),
                TerminalAssignedBooleanControl::ReturnParameter {
                    psi_return_edge,
                    ..
                } if *psi_return_edge == EdgeId::new(11).unwrap()
            ));
            assert!(matches!(
                when_false.control.as_ref(),
                TerminalAssignedBooleanControl::ReturnNotParameter {
                    psi_return_edge,
                    ..
                } if *psi_return_edge == EdgeId::new(12).unwrap()
            ));
        }
    }

    #[test]
    fn finite_boolean_cleanup_accepts_two_leaf_and_wider_trees() {
        let mut two_leaf = boolean_cleanup_plan(NativeTarget::linux_x64());
        let TerminalTargetOperation::BooleanControlWithCleanup { control, .. } =
            &mut two_leaf.functions[0].operation
        else {
            unreachable!()
        };
        let TerminalTargetBooleanControl::Conditional { when_true, .. } = control else {
            unreachable!()
        };
        when_true.control = Box::new(boolean_immediate_return(13));
        assign_registers(&two_leaf).expect("assign two-leaf Boolean cleanup");

        let mut wider = boolean_cleanup_plan(NativeTarget::linux_x64());
        let location = boolean_cleanup_condition_location(&wider);
        let TerminalTargetOperation::BooleanControlWithCleanup { control, .. } =
            &mut wider.functions[0].operation
        else {
            unreachable!()
        };
        let TerminalTargetBooleanControl::Conditional { when_true, .. } = control else {
            unreachable!()
        };
        let TerminalTargetBooleanControl::Conditional {
            when_true: nested_true,
            ..
        } = when_true.control.as_mut()
        else {
            unreachable!()
        };
        nested_true.control = Box::new(TerminalTargetBooleanControl::Conditional {
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
        let TerminalTargetOperation::BooleanControlWithCleanup { control, .. } =
            &mut plan.functions[0].operation
        else {
            unreachable!()
        };
        let TerminalTargetBooleanControl::Conditional { when_true, .. } = control else {
            unreachable!()
        };
        let TerminalTargetBooleanControl::Conditional { when_false, .. } =
            when_true.control.as_mut()
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
        let TerminalTargetOperation::BooleanControlWithCleanup {
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
            TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
            TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
        );
        let assigned = assign_registers(&plan).expect("assign AArch64 homes");
        let TerminalAssignedOperation::ReturnIntegerExpression {
            frame, expression, ..
        } = &assigned.functions[0].operation
        else {
            panic!("fixture must remain an expression")
        };
        assert_eq!(frame.byte_size, 16);
        assert_eq!(frame.register_spills.len(), 2);
        assert_eq!(frame.register_spills[0].byte_offset, 0);
        assert_eq!(frame.register_spills[1].byte_offset, 8);
        let TerminalAssignedIntegerExpression::WrappingAdd { left, right, .. } = expression else {
            panic!("fixture must remain wrapping addition")
        };
        assert!(matches!(
            left.as_ref(),
            TerminalAssignedIntegerExpression::Parameter {
                location: TerminalAssignedScalarLocation::FrameSpill { byte_offset: 0 },
                ..
            }
        ));
        assert!(matches!(
            right.as_ref(),
            TerminalAssignedIntegerExpression::Parameter {
                location: TerminalAssignedScalarLocation::FrameSpill { byte_offset: 8 },
                ..
            }
        ));
    }

    #[test]
    fn x86_expression_registers_remain_explicit_without_a_frame() {
        let plan = expression_plan(
            NativeTarget::linux_x64(),
            TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
            TerminalScalarParameterLocation::IncomingStack { byte_offset: 16 },
        );
        let assigned = assign_registers(&plan).expect("assign x86-64 homes");
        let TerminalAssignedOperation::ReturnIntegerExpression {
            frame, expression, ..
        } = &assigned.functions[0].operation
        else {
            panic!("fixture must remain an expression")
        };
        assert_eq!(frame.byte_size, 0);
        assert!(frame.register_spills.is_empty());
        let TerminalAssignedIntegerExpression::WrappingAdd { left, right, .. } = expression else {
            panic!("fixture must remain wrapping addition")
        };
        assert!(matches!(
            left.as_ref(),
            TerminalAssignedIntegerExpression::Parameter {
                location: TerminalAssignedScalarLocation::Register(MachineRegister::X86Rdi),
                ..
            }
        ));
        assert!(matches!(
            right.as_ref(),
            TerminalAssignedIntegerExpression::Parameter {
                location: TerminalAssignedScalarLocation::IncomingStack { byte_offset: 16 },
                ..
            }
        ));
    }

    #[test]
    fn x86_scratch_conflicting_parameter_receives_a_frame_spill() {
        let plan = expression_plan(
            NativeTarget::linux_x64(),
            TerminalScalarParameterLocation::Register(MachineRegister::X86R10),
            TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
        );
        let assigned = assign_registers(&plan).expect("assign x86-64 scratch conflict");
        let TerminalAssignedOperation::ReturnIntegerExpression {
            frame, expression, ..
        } = &assigned.functions[0].operation
        else {
            panic!("fixture must remain an expression")
        };
        assert_eq!(frame.byte_size, 16);
        assert_eq!(frame.register_spills.len(), 1);
        assert_eq!(frame.register_spills[0].register, MachineRegister::X86R10);
        let TerminalAssignedIntegerExpression::WrappingAdd { left, .. } = expression else {
            panic!("fixture must remain wrapping addition")
        };
        assert!(matches!(
            left.as_ref(),
            TerminalAssignedIntegerExpression::Parameter {
                location: TerminalAssignedScalarLocation::FrameSpill { byte_offset: 0 },
                ..
            }
        ));
    }

    #[test]
    fn x86_calling_expression_spills_live_caller_registers() {
        let scalar_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let mut plan = expression_plan(
            NativeTarget::linux_x64(),
            TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
            TerminalScalarParameterLocation::Register(MachineRegister::X86Rsi),
        );
        let TerminalTargetOperation::ReturnIntegerExpression { expression, .. } =
            &mut plan.functions[0].operation
        else {
            unreachable!()
        };
        *expression = TerminalTargetIntegerExpression::WrappingAdd {
            psi_operation: OperationId::new(8).unwrap(),
            left: Box::new(TerminalTargetIntegerExpression::Call {
                psi_operation: OperationId::new(7).unwrap(),
                source_value: ValueId::new(4).unwrap(),
                callee: MachineId::new(2).unwrap(),
                arguments: vec![TerminalTargetCallArgument {
                    scalar_type: ScalarType::Integer(scalar_type),
                    location: TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
                    expression: TerminalTargetScalarExpression::Integer {
                        scalar_type,
                        expression: TerminalTargetIntegerExpression::Parameter {
                            source_value: ValueId::new(1).unwrap(),
                            parameter_index: 0,
                            location: TerminalScalarParameterLocation::Register(
                                MachineRegister::X86Rdi,
                            ),
                        },
                    },
                }],
            }),
            right: Box::new(TerminalTargetIntegerExpression::Parameter {
                source_value: ValueId::new(1).unwrap(),
                parameter_index: 0,
                location: TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
            }),
        };

        let assigned = assign_registers(&plan).expect("assign call-preserved parameter");
        let TerminalAssignedOperation::ReturnIntegerExpression {
            frame, expression, ..
        } = &assigned.functions[0].operation
        else {
            unreachable!()
        };
        assert_eq!(frame.byte_size, 32);
        assert_eq!(frame.register_spills.len(), 1);
        let TerminalAssignedIntegerExpression::WrappingAdd { left, right, .. } = expression else {
            unreachable!()
        };
        let TerminalAssignedIntegerExpression::Call { arguments, .. } = left.as_ref() else {
            unreachable!()
        };
        assert!(matches!(
            &arguments[0].expression,
            TerminalAssignedScalarExpression::Integer {
                expression: TerminalAssignedIntegerExpression::Parameter {
                    location: TerminalAssignedScalarLocation::FrameSpill { byte_offset: 0 },
                    ..
                },
                ..
            }
        ));
        assert!(matches!(
            right.as_ref(),
            TerminalAssignedIntegerExpression::Parameter {
                location: TerminalAssignedScalarLocation::FrameSpill { byte_offset: 0 },
                ..
            }
        ));
    }

    #[test]
    fn call_stack_arguments_receive_concrete_outgoing_homes() {
        let scalar_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let mut plan = expression_plan(
            NativeTarget::linux_x64(),
            TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
            TerminalScalarParameterLocation::Register(MachineRegister::X86Rsi),
        );
        let TerminalTargetOperation::ReturnIntegerExpression { expression, .. } =
            &mut plan.functions[0].operation
        else {
            unreachable!()
        };
        *expression = TerminalTargetIntegerExpression::Call {
            psi_operation: OperationId::new(7).unwrap(),
            source_value: ValueId::new(4).unwrap(),
            callee: MachineId::new(2).unwrap(),
            arguments: vec![TerminalTargetCallArgument {
                scalar_type: ScalarType::Integer(scalar_type),
                location: TerminalScalarParameterLocation::IncomingStack { byte_offset: 8 },
                expression: TerminalTargetScalarExpression::Integer {
                    scalar_type,
                    expression: TerminalTargetIntegerExpression::Immediate {
                        source_value: ValueId::new(5).unwrap(),
                        value: psi_core::IntegerValue::Unsigned(9),
                    },
                },
            }],
        };

        let assigned = assign_registers(&plan).expect("assign outgoing stack argument");
        let TerminalAssignedOperation::ReturnIntegerExpression {
            frame, expression, ..
        } = &assigned.functions[0].operation
        else {
            unreachable!()
        };
        assert_eq!(frame.byte_size, 16);
        let TerminalAssignedIntegerExpression::Call { arguments, .. } = expression else {
            unreachable!()
        };
        assert_eq!(arguments[0].spill_byte_offset, 0);
        assert_eq!(
            arguments[0].destination,
            TerminalAssignedCallDestination::OutgoingStack { byte_offset: 8 }
        );
    }

    #[test]
    fn x86_stack_pointer_cannot_be_an_expression_parameter_home() {
        let plan = expression_plan(
            NativeTarget::linux_x64(),
            TerminalScalarParameterLocation::Register(MachineRegister::X86Rsp),
            TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
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
            TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
            TerminalScalarParameterLocation::Register(MachineRegister::X86Rsi),
        );
        let TerminalTargetOperation::ReturnIntegerExpression { expression, .. } =
            &mut plan.functions[0].operation
        else {
            panic!("fixture must contain an expression")
        };
        let TerminalTargetIntegerExpression::WrappingAdd { right, .. } = expression else {
            panic!("fixture must contain wrapping addition")
        };
        let TerminalTargetIntegerExpression::Parameter {
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
            TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
            TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
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
        let plan = TerminalTargetOperationPlan {
            terminal_psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([3; 32]),
            },
            target,
            entry: MachineId::new(1).unwrap(),
            functions: vec![TerminalTargetFunction {
                machine: MachineId::new(1).unwrap(),
                attachment: None,
                provenance: TerminalPsiProvenance::default(),
                operation: TerminalTargetOperation::UnitBody(
                    omega_terminal_target_operations::TerminalTargetUnitBody {
                        structural_types: Vec::new(),
                        call_plan: call_plan.clone(),
                        parameters: Vec::new(),
                        operations: vec![TerminalTargetUnitOperation::Call {
                            psi_operation: OperationId::new(1).unwrap(),
                            callee: MachineId::new(2).unwrap(),
                            arguments: vec![
                                omega_terminal_target_operations::TerminalTargetStructuralArgument {
                                    place,
                                    path: path.clone(),
                                    root_structural_type: structural_type,
                                    structural_type,
                                    shape,
                                    source_byte_offset: 0,
                                    fixed_array_length: None,
                                    element_stride: None,
                                    source: call_plan.parameters[0].clone(),
                                    destination: call_plan.parameters[0].clone(),
                                },
                            ],
                            claim_transfers: Vec::new(),
                        }],
                    },
                ),
            }],
        };

        let assigned = assign_registers(&plan).unwrap();
        let TerminalAssignedOperation::UnitBody(body) = &assigned.functions[0].operation else {
            panic!("Unit body")
        };
        let TerminalAssignedUnitOperation::Call { copies, .. } = &body.operations[0] else {
            panic!("Unit call")
        };
        assert_eq!(copies[0].path, path);
    }

    fn expression_plan(
        target: NativeTarget,
        left_location: TerminalScalarParameterLocation,
        right_location: TerminalScalarParameterLocation,
    ) -> TerminalTargetOperationPlan {
        TerminalTargetOperationPlan {
            terminal_psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([3; 32]),
            },
            target,
            entry: MachineId::new(1).expect("machine"),
            functions: vec![TerminalTargetFunction {
                machine: MachineId::new(1).expect("machine"),
                attachment: None,
                provenance: TerminalPsiProvenance::default(),
                operation: TerminalTargetOperation::ReturnIntegerExpression {
                    psi_edge: EdgeId::new(1).expect("edge"),
                    source_value: ValueId::new(3).expect("result"),
                    scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
                    expression: TerminalTargetIntegerExpression::WrappingAdd {
                        psi_operation: OperationId::new(1).expect("operation"),
                        left: Box::new(TerminalTargetIntegerExpression::Parameter {
                            source_value: ValueId::new(1).expect("left"),
                            parameter_index: 0,
                            location: left_location,
                        }),
                        right: Box::new(TerminalTargetIntegerExpression::Parameter {
                            source_value: ValueId::new(2).expect("right"),
                            parameter_index: 1,
                            location: right_location,
                        }),
                    },
                },
            }],
        }
    }

    fn boolean_cleanup_plan(target: NativeTarget) -> TerminalTargetOperationPlan {
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
        let condition_location = TerminalScalarParameterLocation::Register(*register);
        let nested = TerminalTargetBooleanControl::Conditional {
            condition_source: ValueId::new(1).unwrap(),
            condition_parameter_index: 0,
            condition_location,
            when_true: boolean_arm(4, boolean_immediate_return(10)),
            when_false: boolean_arm(
                5,
                TerminalTargetBooleanControl::ReturnParameter {
                    psi_return_edge: EdgeId::new(11).unwrap(),
                    source_value: ValueId::new(1).unwrap(),
                    parameter_index: 0,
                    location: condition_location,
                },
            ),
        };
        let place = PlaceId::new(1).unwrap();
        TerminalTargetOperationPlan {
            terminal_psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
            },
            target,
            entry: MachineId::new(1).unwrap(),
            functions: vec![TerminalTargetFunction {
                machine: MachineId::new(1).unwrap(),
                attachment: None,
                provenance: TerminalPsiProvenance {
                    operations: Vec::new(),
                    edges: (1..=5)
                        .chain(10..=12)
                        .map(|edge| EdgeId::new(edge).unwrap())
                        .collect(),
                },
                operation: TerminalTargetOperation::BooleanControlWithCleanup {
                    control: TerminalTargetBooleanControl::Conditional {
                        condition_source: ValueId::new(1).unwrap(),
                        condition_parameter_index: 0,
                        condition_location,
                        when_true: boolean_arm(2, nested),
                        when_false: boolean_arm(
                            3,
                            TerminalTargetBooleanControl::ReturnNotParameter {
                                psi_return_edge: EdgeId::new(12).unwrap(),
                                source_value: ValueId::new(1).unwrap(),
                                parameter_index: 0,
                                location: condition_location,
                            },
                        ),
                    },
                    structural_types: Vec::new(),
                    call_plan: call_plan.clone(),
                    structural_parameters: vec![TerminalTargetStructuralParameter {
                        place,
                        structural_type: StructuralTypeId::new(1).unwrap(),
                        multiplicity: StructuralMultiplicity::Affine,
                        shape: structural_shape,
                        placement: call_plan.parameters[1].clone(),
                    }],
                    cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(place)],
                },
            }],
        }
    }

    fn boolean_arm(
        edge: u64,
        control: TerminalTargetBooleanControl,
    ) -> TerminalTargetConditionalBooleanArm {
        TerminalTargetConditionalBooleanArm {
            psi_edge: EdgeId::new(edge).unwrap(),
            control: Box::new(control),
        }
    }

    fn boolean_immediate_return(edge: u64) -> TerminalTargetBooleanControl {
        TerminalTargetBooleanControl::ReturnImmediate {
            psi_return_edge: EdgeId::new(edge).unwrap(),
            source_value: ValueId::new(edge).unwrap(),
            value: edge % 2 == 0,
        }
    }

    fn boolean_cleanup_condition_location(
        plan: &TerminalTargetOperationPlan,
    ) -> TerminalScalarParameterLocation {
        let TerminalTargetOperation::BooleanControlWithCleanup { control, .. } =
            &plan.functions[0].operation
        else {
            unreachable!()
        };
        let TerminalTargetBooleanControl::Conditional {
            condition_location, ..
        } = control
        else {
            unreachable!()
        };
        *condition_location
    }
}
