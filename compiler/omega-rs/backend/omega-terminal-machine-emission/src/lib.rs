#![forbid(unsafe_code)]

//! Machine-code emission for the first source-independent terminal-Psi target
//! operation slice.

#[cfg(test)]
use omega_calling_conventions::ValueShape;
use omega_calling_conventions::{ValueLocation, ValuePlacement};
#[cfg(test)]
use omega_target::ObjectFormat;
use omega_target::{Architecture, NativeTarget};
#[cfg(test)]
use omega_terminal_assigned_target_operations::TerminalAssignedBooleanControl;
use omega_terminal_assigned_target_operations::{
    TerminalAssignedFunction, TerminalAssignedOperation, TerminalAssignedOperationPlan,
};
#[cfg(test)]
use omega_terminal_machine_code::{
    TerminalAarch64ReturnLinkEvidence, TerminalScalarConditionalBranchEvidence,
    TerminalScalarConditionalCondition, TerminalScalarStackMutationKind,
    TerminalStackAdjustmentPair, TerminalUnitCallStackEvidence, TerminalUnitStackEvidence,
};
use omega_terminal_machine_code::{
    TerminalBoundaryResultRecord, TerminalBoundarySettlementRecord, TerminalMachineCodeFunction,
    TerminalMachineCodePlan, TerminalNativeFuelAttribution, TerminalNativeFuelSite,
    TerminalScalarControlFlowEvidence, TerminalStructuralReturnRecord,
};
use omega_terminal_target_operations::MachineRegister;
#[cfg(test)]
use omega_terminal_target_operations::TerminalCallSiteOwner;
#[cfg(test)]
use psi_core::{IntegerSign, IntegerType, IntegerValue};
use psi_core::{MachineId, ValueId};

mod unit;
use unit::{emit_aarch64_unit_call, emit_unit_body, emit_x86_64_unit_call};

mod structural_scalar;

mod cleanup;
use cleanup::{
    emit_boolean_control_with_cleanup, emit_scalar_return_with_cleanup,
    exact_partial_cleanup_partition, executable_nominal_cleanup, stack_adjustment_pair,
};

mod scalar;
#[cfg(test)]
use scalar::aarch64_csel;
use scalar::{
    accountable_conditional_boolean_expression, accountable_direct_integer_expression,
    collect_scalar_stack_evidence, collect_x86_division_branch_evidence,
    conditional_with_terminal_shape, direct_conditional_boolean_shape,
    direct_conditional_integer_shape, emit_aarch64_adjust_sp, emit_aarch64_boolean_condition_value,
    emit_aarch64_boolean_control, emit_aarch64_boolean_expression,
    emit_aarch64_boolean_not_parameter_return, emit_aarch64_boolean_return,
    emit_aarch64_condition_load, emit_aarch64_conditional_boolean_control,
    emit_aarch64_conditional_boolean_expression_control, emit_aarch64_conditional_integer_control,
    emit_aarch64_conditional_integer_expression_control, emit_aarch64_integer_expression,
    emit_aarch64_parameter_return, emit_aarch64_return, emit_boolean_shared_convergence,
    emit_native_crash, emit_x86_64_adjust_sp, emit_x86_64_boolean_condition_value,
    emit_x86_64_boolean_control, emit_x86_64_boolean_expression,
    emit_x86_64_boolean_not_parameter_return, emit_x86_64_boolean_return,
    emit_x86_64_conditional_boolean_control, emit_x86_64_conditional_boolean_expression_control,
    emit_x86_64_conditional_integer_control, emit_x86_64_conditional_integer_expression_control,
    emit_x86_64_integer_expression, emit_x86_64_memory_load_width, emit_x86_64_parameter_return,
    emit_x86_64_return, emit_x86_64_stack_load, emit_x86_64_stack_load_width,
    emit_x86_64_stack_store, emit_x86_64_stack_store_width, integer_bits,
    linear_boolean_expression, require_native_integer_width,
};

pub fn emit_machine_code(
    plan: &TerminalAssignedOperationPlan,
) -> Result<TerminalMachineCodePlan, EmissionError> {
    if !plan
        .functions
        .iter()
        .any(|function| function.machine == plan.entry)
    {
        return Err(EmissionError::EntryFunctionMissing(plan.entry));
    }
    Ok(TerminalMachineCodePlan {
        terminal_psi: plan.terminal_psi,
        target: plan.target,
        entry: plan.entry,
        functions: plan
            .functions
            .iter()
            .map(|function| emit_function(function, plan.target, &plan.functions))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn emit_function(
    function: &TerminalAssignedFunction,
    target: NativeTarget,
    functions: &[TerminalAssignedFunction],
) -> Result<TerminalMachineCodeFunction, EmissionError> {
    if let TerminalAssignedOperation::ScalarReturnWithCleanup {
        scalar,
        structural_types,
        call_plan,
        structural_parameters,
        cleanup_actions,
        psi_edge,
    } = &function.operation
    {
        return emit_scalar_return_with_cleanup(
            function,
            scalar,
            structural_types,
            call_plan,
            structural_parameters,
            cleanup_actions,
            *psi_edge,
            target,
            functions,
        );
    }
    if let TerminalAssignedOperation::BooleanControlWithCleanup {
        control,
        structural_types,
        call_plan,
        structural_parameters,
        cleanup_actions,
    } = &function.operation
    {
        return emit_boolean_control_with_cleanup(
            function,
            control,
            structural_types,
            call_plan,
            structural_parameters,
            cleanup_actions,
            target,
            functions,
        );
    }
    let architecture = target.architecture;
    let mut internal_calls = Vec::new();
    let mut internal_unit_calls = Vec::new();
    let mut unit_affine_cleanup = None;
    let mut fuel_attribution = Vec::new();
    let mut port_effects = Vec::new();
    let mut boundary_settlements = Vec::new();
    let mut structural_return = None;
    let mut unit_stack = None;
    let mut unit_parameter_homes = Vec::new();
    let mut unit_parameters = Vec::new();
    let mut scalar_structural_parameter_homes = Vec::new();
    let mut scalar_structural_parameters = Vec::new();
    let mut scalar_stack_eligible = false;
    let mut scalar_control_flow = TerminalScalarControlFlowEvidence::Linear;
    let bytes = match &function.operation {
        TerminalAssignedOperation::ScalarReturnWithCleanup { .. } => {
            unreachable!("scalar cleanup returns are emitted by the early carrier path")
        }
        TerminalAssignedOperation::BooleanControlWithCleanup { .. } => {
            unreachable!("Boolean-control cleanup is emitted by the early carrier path")
        }
        operation @ TerminalAssignedOperation::ReturnStructuralScalarCall { .. } => {
            let emitted = structural_scalar::emit(operation, target, functions)?;
            internal_calls = emitted.internal_calls;
            internal_unit_calls = emitted.internal_unit_calls;
            fuel_attribution = emitted.fuel_attribution;
            port_effects = emitted.port_effects;
            boundary_settlements = emitted.boundary_settlements;
            unit_stack = Some(emitted.stack);
            unit_parameter_homes = emitted.parameter_homes;
            unit_parameters = emitted.parameters;
            unit_affine_cleanup = emitted.affine_cleanup;
            emitted.bytes
        }
        TerminalAssignedOperation::ReturnBoundaryPortReadU8 {
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
            ..
        } => {
            if architecture != Architecture::X86_64
                || call_plan.result.is_none()
                || call_plan.parameters.len() < structural_parameters.len()
                || call_plan.parameters[call_plan.parameters.len() - structural_parameters.len()..]
                    .iter()
                    .zip(structural_parameters)
                    .any(|(placement, parameter)| placement != &parameter.placement)
            {
                return Err(EmissionError::BoundaryPortReadUnsupported(architecture));
            }
            scalar_stack_eligible = true;
            let mut bytes =
                omega_x86_encoding::encode_immediate_port_read_u8(realization.port).to_vec();
            let read_byte_count = bytes.len();
            bytes.push(0xc3);
            fuel_attribution.push(TerminalNativeFuelAttribution {
                schedule: psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
                site: TerminalNativeFuelSite::Operation(*psi_operation),
                units: 1,
                operation_ordinal: 0,
                code_offset: 0,
                byte_count: read_byte_count,
            });
            fuel_attribution.push(TerminalNativeFuelAttribution {
                schedule: psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
                site: TerminalNativeFuelSite::Edge(*psi_edge),
                units: 1,
                operation_ordinal: 1,
                code_offset: read_byte_count,
                byte_count: 1,
            });
            boundary_settlements.push(TerminalBoundarySettlementRecord {
                psi_operation: *psi_operation,
                boundary: *boundary,
                provider_execution: (*provider_execution).into(),
                realization:
                    omega_terminal_target_operations::TerminalBoundaryRealization::DirectPortReadU8(
                        *realization,
                    ),
                arguments: arguments.clone(),
                completion_claim_sources: completion_claim_sources.clone(),
                completion_receipts: completion_receipts.clone(),
                native_result: Some(TerminalBoundaryResultRecord {
                    value: *source_value,
                    scalar_type: psi_core::ScalarType::Integer(
                        psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8)
                            .expect("u8 is valid"),
                    ),
                    placement: call_plan.result.clone().expect("checked above"),
                    return_edge: *psi_edge,
                }),
                operation_ordinal: 0,
                code_offset: 0,
                byte_count: read_byte_count,
            });
            scalar_structural_parameters = structural_parameters
                .iter()
                .map(
                    |parameter| omega_terminal_machine_code::TerminalUnitParameterRecord {
                        place: parameter.place,
                        structural_type: parameter.structural_type,
                        multiplicity: parameter.multiplicity,
                        shape: parameter.shape,
                    },
                )
                .collect();
            scalar_structural_parameter_homes = structural_parameters
                .iter()
                .map(
                    |parameter| omega_terminal_machine_code::TerminalUnitParameterHomeRecord {
                        place: parameter.place,
                        structural_type: parameter.structural_type,
                        multiplicity: parameter.multiplicity,
                        shape: parameter.shape,
                        source: parameter.placement.clone(),
                        byte_offset: 0,
                        indirect: matches!(
                            parameter.placement.locations.as_slice(),
                            [ValueLocation::Indirect { .. }]
                        ),
                    },
                )
                .collect();
            bytes
        }
        TerminalAssignedOperation::UnitBody(body) => {
            let emitted = emit_unit_body(body, target, functions)?;
            internal_calls = emitted.internal_calls;
            internal_unit_calls = emitted.internal_unit_calls;
            fuel_attribution = emitted.fuel_attribution;
            port_effects = emitted.port_effects;
            boundary_settlements = emitted.boundary_settlements;
            unit_stack = Some(emitted.stack);
            unit_parameter_homes = emitted.parameter_homes;
            unit_parameters = emitted.parameters;
            unit_affine_cleanup = emitted.affine_cleanup;
            emitted.bytes
        }
        TerminalAssignedOperation::ReturnStructuralParameter {
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
            let bytes = emit_structural_parameter_return(
                source.place,
                source_placement,
                result_placement,
                target.architecture,
            )?;
            for (operation_ordinal, (operation, _, _)) in trivial_affine_locals.iter().enumerate() {
                fuel_attribution.push(TerminalNativeFuelAttribution {
                    schedule: psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
                    site: TerminalNativeFuelSite::Operation(*operation),
                    units: 1,
                    operation_ordinal,
                    code_offset: 0,
                    byte_count: 0,
                });
            }
            fuel_attribution.push(TerminalNativeFuelAttribution {
                schedule: psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
                site: TerminalNativeFuelSite::Edge(*psi_edge),
                units: 1,
                operation_ordinal: trivial_affine_locals.len(),
                code_offset: 0,
                byte_count: bytes.len(),
            });
            structural_return = Some(TerminalStructuralReturnRecord {
                psi_edge: *psi_edge,
                parameters: parameters.clone(),
                parameter_placements: call_plan.parameters.clone(),
                source: source.clone(),
                result: result.clone(),
                shape: *shape,
                source_placement: source_placement.clone(),
                result_placement: result_placement.clone(),
                returned_claims: returned_claims.clone(),
                trivial_affine_locals: trivial_affine_locals.clone(),
                trivial_affine_discards: trivial_affine_discards.clone(),
                code_offset: 0,
                byte_count: bytes.len(),
            });
            bytes
        }
        // The verified cause remains in the assigned operation and terminal
        // artifact identity. Both closed causes realize as the target's
        // unconditional synchronous fault until a platform crash dispatcher
        // supplies a cause-specific entry contract.
        TerminalAssignedOperation::Crash { .. } => emit_native_crash(architecture),
        TerminalAssignedOperation::ReturnIntegerImmediate {
            source_value,
            scalar_type,
            value,
            ..
        } => {
            scalar_stack_eligible = true;
            let bits = integer_bits(*source_value, *scalar_type, *value)?;
            match architecture {
                Architecture::Aarch64 => emit_aarch64_return(*scalar_type, bits),
                Architecture::X86_64 => emit_x86_64_return(*scalar_type, bits),
            }
        }
        TerminalAssignedOperation::ReturnBooleanImmediate { value, .. } => {
            scalar_stack_eligible = true;
            match architecture {
                Architecture::Aarch64 => emit_aarch64_boolean_return(*value),
                Architecture::X86_64 => emit_x86_64_boolean_return(*value),
            }
        }
        TerminalAssignedOperation::ReturnIntegerParameter {
            source_value,
            scalar_type,
            location,
            ..
        } => {
            scalar_stack_eligible = true;
            require_native_integer_width(*source_value, *scalar_type)?;
            match architecture {
                Architecture::Aarch64 => emit_aarch64_parameter_return(
                    *source_value,
                    scalar_type.bits() > 32,
                    *location,
                )?,
                Architecture::X86_64 => {
                    emit_x86_64_parameter_return(*source_value, scalar_type.bits() > 32, *location)?
                }
            }
        }
        TerminalAssignedOperation::ReturnBooleanParameter {
            source_value,
            location,
            ..
        } => {
            scalar_stack_eligible = true;
            match architecture {
                Architecture::Aarch64 => {
                    emit_aarch64_parameter_return(*source_value, false, *location)?
                }
                Architecture::X86_64 => {
                    emit_x86_64_parameter_return(*source_value, false, *location)?
                }
            }
        }
        TerminalAssignedOperation::ReturnBooleanNotParameter {
            source_value,
            location,
            ..
        } => {
            scalar_stack_eligible = true;
            match architecture {
                Architecture::Aarch64 => {
                    emit_aarch64_boolean_not_parameter_return(*source_value, *location)?
                }
                Architecture::X86_64 => {
                    emit_x86_64_boolean_not_parameter_return(*source_value, *location)?
                }
            }
        }
        TerminalAssignedOperation::ReturnBooleanSharedConvergence { control, .. } => {
            scalar_stack_eligible = true;
            let (emitted, control_flow) = emit_boolean_shared_convergence(architecture, control)?;
            scalar_control_flow = control_flow;
            emitted
        }
        TerminalAssignedOperation::ReturnBooleanExpression {
            frame, expression, ..
        } => {
            scalar_stack_eligible = linear_boolean_expression(expression);
            match architecture {
                Architecture::Aarch64 => emit_aarch64_boolean_expression(
                    frame,
                    expression,
                    Some((&mut internal_calls, target)),
                )?,
                Architecture::X86_64 => emit_x86_64_boolean_expression(
                    frame,
                    expression,
                    Some((&mut internal_calls, target)),
                )?,
            }
        }
        TerminalAssignedOperation::ReturnIntegerExpression {
            source_value,
            scalar_type,
            frame,
            expression,
            ..
        } => {
            scalar_stack_eligible = accountable_direct_integer_expression(expression);
            require_native_integer_width(*source_value, *scalar_type)?;
            let bytes = match architecture {
                Architecture::Aarch64 => emit_aarch64_integer_expression(
                    *scalar_type,
                    frame,
                    expression,
                    Some((&mut internal_calls, target)),
                )?,
                Architecture::X86_64 => emit_x86_64_integer_expression(
                    *scalar_type,
                    frame,
                    expression,
                    Some((&mut internal_calls, target)),
                )?,
            };
            if architecture == Architecture::X86_64 && scalar_stack_eligible {
                let branches = collect_x86_division_branch_evidence(&bytes)?;
                if !branches.is_empty() {
                    scalar_control_flow =
                        TerminalScalarControlFlowEvidence::LinearWithDivisionBranches { branches };
                }
            }
            bytes
        }
        TerminalAssignedOperation::ReturnIntegerConditionalControl {
            condition_source,
            condition_location,
            scalar_type,
            when_true,
            when_false,
            ..
        } => {
            let fragment = match architecture {
                Architecture::Aarch64 => emit_aarch64_conditional_integer_control(
                    *condition_source,
                    *condition_location,
                    *scalar_type,
                    when_true,
                    when_false,
                    target,
                )?,
                Architecture::X86_64 => emit_x86_64_conditional_integer_control(
                    *condition_source,
                    *condition_location,
                    *scalar_type,
                    when_true,
                    when_false,
                    target,
                )?,
            };
            let terminal_shape = direct_conditional_integer_shape(when_true, when_false);
            scalar_stack_eligible = terminal_shape.is_some();
            if let Some(terminal_shape) = terminal_shape {
                let conditional = fragment
                    .conditional
                    .expect("top-level integer conditional retains its branch evidence");
                let branches = if architecture == Architecture::X86_64 {
                    collect_x86_division_branch_evidence(&fragment.bytes)?
                } else {
                    Vec::new()
                };
                scalar_control_flow =
                    conditional_with_terminal_shape(conditional, terminal_shape, branches)?;
            }
            internal_calls = fragment.internal_calls;
            fragment.bytes
        }
        TerminalAssignedOperation::ReturnIntegerExpressionConditionalControl {
            condition_frame,
            condition,
            scalar_type,
            when_true,
            when_false,
            ..
        } => {
            let fragment = match architecture {
                Architecture::Aarch64 => emit_aarch64_conditional_integer_expression_control(
                    condition_frame,
                    condition,
                    *scalar_type,
                    when_true,
                    when_false,
                    target,
                )?,
                Architecture::X86_64 => emit_x86_64_conditional_integer_expression_control(
                    condition_frame,
                    condition,
                    *scalar_type,
                    when_true,
                    when_false,
                    target,
                )?,
            };
            let terminal_shape = direct_conditional_integer_shape(when_true, when_false);
            scalar_stack_eligible =
                terminal_shape.is_some() && accountable_conditional_boolean_expression(condition);
            if let Some(terminal_shape) = terminal_shape.filter(|_| scalar_stack_eligible) {
                let conditional = fragment
                    .conditional
                    .expect("top-level integer expression conditional retains its branch evidence");
                let branches = if architecture == Architecture::X86_64 {
                    collect_x86_division_branch_evidence(&fragment.bytes)?
                } else {
                    Vec::new()
                };
                scalar_control_flow =
                    conditional_with_terminal_shape(conditional, terminal_shape, branches)?;
            }
            internal_calls = fragment.internal_calls;
            fragment.bytes
        }
        TerminalAssignedOperation::ReturnBooleanConditionalControl {
            condition_source,
            condition_location,
            when_true,
            when_false,
            ..
        } => {
            let fragment = match architecture {
                Architecture::Aarch64 => emit_aarch64_conditional_boolean_control(
                    *condition_source,
                    *condition_location,
                    when_true,
                    when_false,
                    target,
                )?,
                Architecture::X86_64 => emit_x86_64_conditional_boolean_control(
                    *condition_source,
                    *condition_location,
                    when_true,
                    when_false,
                    target,
                )?,
            };
            let terminal_shape = direct_conditional_boolean_shape(when_true, when_false);
            scalar_stack_eligible = terminal_shape.is_some();
            if let Some(terminal_shape) = terminal_shape {
                let decisions = fragment
                    .conditional
                    .expect("top-level Boolean conditional retains its branch evidence");
                let branches = if architecture == Architecture::X86_64 {
                    collect_x86_division_branch_evidence(&fragment.bytes)?
                } else {
                    Vec::new()
                };
                scalar_control_flow =
                    conditional_with_terminal_shape(decisions, terminal_shape, branches)?;
            }
            internal_calls = fragment.internal_calls;
            fragment.bytes
        }
        TerminalAssignedOperation::ReturnBooleanExpressionConditionalControl {
            condition_frame,
            condition,
            when_true,
            when_false,
            ..
        } => {
            let fragment = match architecture {
                Architecture::Aarch64 => emit_aarch64_conditional_boolean_expression_control(
                    condition_frame,
                    condition,
                    when_true,
                    when_false,
                    target,
                )?,
                Architecture::X86_64 => emit_x86_64_conditional_boolean_expression_control(
                    condition_frame,
                    condition,
                    when_true,
                    when_false,
                    target,
                )?,
            };
            let terminal_shape = direct_conditional_boolean_shape(when_true, when_false);
            scalar_stack_eligible =
                terminal_shape.is_some() && accountable_conditional_boolean_expression(condition);
            if let Some(terminal_shape) = terminal_shape.filter(|_| scalar_stack_eligible) {
                let decisions = fragment
                    .conditional
                    .expect("top-level Boolean expression conditional retains branch evidence");
                let branches = if architecture == Architecture::X86_64 {
                    collect_x86_division_branch_evidence(&fragment.bytes)?
                } else {
                    Vec::new()
                };
                scalar_control_flow =
                    conditional_with_terminal_shape(decisions, terminal_shape, branches)?;
            }
            internal_calls = fragment.internal_calls;
            fragment.bytes
        }
    };
    let scalar_stack = scalar_stack_eligible
        .then(|| collect_scalar_stack_evidence(architecture, &bytes, scalar_control_flow, None))
        .transpose()?;
    if !scalar_stack_eligible {
        for call in &mut internal_calls {
            call.scalar_stack = None;
        }
    }
    Ok(TerminalMachineCodeFunction {
        machine: function.machine,
        attachment: function.attachment,
        provenance: function.provenance.clone(),
        bytes,
        unit_stack,
        unit_parameter_homes,
        unit_parameters,
        scalar_stack,
        internal_calls,
        internal_unit_calls,
        unit_affine_cleanup,
        scalar_affine_cleanup: None,
        scalar_control_affine_cleanups: Vec::new(),
        scalar_structural_parameters,
        scalar_structural_parameter_homes,
        fuel_attribution,
        port_effects,
        boundary_settlements,
        structural_return,
    })
}

#[allow(clippy::too_many_arguments)]

fn emit_structural_parameter_return(
    source: psi_core::PlaceId,
    source_placement: &ValuePlacement,
    result_placement: &ValuePlacement,
    architecture: Architecture,
) -> Result<Vec<u8>, EmissionError> {
    let [
        ValueLocation::Register {
            register: source_register,
            value_byte_offset: 0,
            byte_size: 8,
        },
    ] = source_placement.locations.as_slice()
    else {
        return Err(EmissionError::UnsupportedStructuralReturnPlacement(source));
    };
    let [
        ValueLocation::Register {
            register: result_register,
            value_byte_offset: 0,
            byte_size: 8,
        },
    ] = result_placement.locations.as_slice()
    else {
        return Err(EmissionError::UnsupportedStructuralReturnPlacement(source));
    };
    match architecture {
        Architecture::X86_64 => {
            let source_code = x86_unit_register(*source_register)?;
            let result_code = x86_unit_register(*result_register)?;
            if result_code != 0 {
                return Err(EmissionError::UnsupportedStructuralResultRegister(
                    *result_register,
                ));
            }
            Ok(vec![
                0x48 | (((source_code >> 3) & 1) << 2),
                0x89,
                0xc0 | ((source_code & 7) << 3),
                0xc3,
            ])
        }
        Architecture::Aarch64 => {
            let source_code = aarch64_unit_register(*source_register)?;
            let result_code = aarch64_unit_register(*result_register)?;
            if result_code != 0 {
                return Err(EmissionError::UnsupportedStructuralResultRegister(
                    *result_register,
                ));
            }
            let mut instructions = Vec::new();
            if source_code != 0 {
                instructions.push(0xaa00_03e0 | (u32::from(source_code) << 16));
            }
            instructions.push(0xd65f_03c0);
            Ok(instructions
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect())
        }
    }
}

fn placement_fragment(location: &ValueLocation) -> Result<(u16, u16), EmissionError> {
    match *location {
        ValueLocation::Register {
            value_byte_offset,
            byte_size,
            ..
        }
        | ValueLocation::Stack {
            value_byte_offset,
            byte_size,
            ..
        } => Ok((value_byte_offset, byte_size)),
        ValueLocation::Indirect { .. } => Err(EmissionError::UnsupportedAggregatePlacement),
    }
}

fn x86_unit_register(register: MachineRegister) -> Result<u8, EmissionError> {
    match register {
        MachineRegister::X86Rax => Ok(0),
        MachineRegister::X86Rcx => Ok(1),
        MachineRegister::X86Rdx => Ok(2),
        MachineRegister::X86Rbx => Ok(3),
        MachineRegister::X86Rsp => Ok(4),
        MachineRegister::X86Rbp => Ok(5),
        MachineRegister::X86Rsi => Ok(6),
        MachineRegister::X86Rdi => Ok(7),
        MachineRegister::X86R8 => Ok(8),
        MachineRegister::X86R9 => Ok(9),
        MachineRegister::X86R10 => Ok(10),
        MachineRegister::X86R11 => Ok(11),
        MachineRegister::X86R12 => Ok(12),
        MachineRegister::X86R13 => Ok(13),
        MachineRegister::X86R14 => Ok(14),
        MachineRegister::X86R15 => Ok(15),
        _ => Err(EmissionError::UnsupportedUnitRegister(register)),
    }
}

fn aarch64_unit_register(register: MachineRegister) -> Result<u8, EmissionError> {
    match register {
        MachineRegister::Aarch64X(register) if register < 31 => Ok(register),
        _ => Err(EmissionError::UnsupportedUnitRegister(register)),
    }
}

fn aarch64_load_base(byte_size: u16) -> Result<u32, EmissionError> {
    match byte_size {
        1 => Ok(0x3940_0000),
        2 => Ok(0x7940_0000),
        4 => Ok(0xb940_0000),
        8 => Ok(0xf940_0000),
        width => Err(EmissionError::UnsupportedAggregateFragmentWidth(width)),
    }
}

fn aarch64_store_base(byte_size: u16) -> Result<u32, EmissionError> {
    match byte_size {
        1 => Ok(0x3900_0000),
        2 => Ok(0x7900_0000),
        4 => Ok(0xb900_0000),
        8 => Ok(0xf900_0000),
        width => Err(EmissionError::UnsupportedAggregateFragmentWidth(width)),
    }
}

fn aarch64_unit_stack_access(
    base: u32,
    register: u8,
    byte_offset: u32,
    byte_size: u16,
) -> Result<u32, EmissionError> {
    let scale = u32::from(byte_size);
    if scale == 0 || !byte_offset.is_multiple_of(scale) || byte_offset / scale > 0xfff {
        return Err(EmissionError::UnitCallStackAreaNotEncodable);
    }
    Ok(base | ((byte_offset / scale) << 10) | (31 << 5) | u32::from(register))
}

fn aarch64_unit_memory_access(
    base: u32,
    register: u8,
    address_register: u8,
    byte_offset: u32,
    byte_size: u16,
) -> Result<u32, EmissionError> {
    let scale = u32::from(byte_size);
    if scale == 0 || !byte_offset.is_multiple_of(scale) || byte_offset / scale > 0xfff {
        return Err(EmissionError::UnitCallStackAreaNotEncodable);
    }
    Ok(base
        | ((byte_offset / scale) << 10)
        | (u32::from(address_register) << 5)
        | u32::from(register))
}

fn emit_aarch64_sp_address(
    instructions: &mut Vec<u32>,
    register: u8,
    byte_offset: u32,
) -> Result<(), EmissionError> {
    if byte_offset > 0xfff {
        return Err(EmissionError::UnitCallStackAreaNotEncodable);
    }
    instructions.push(0x9100_03e0 | (byte_offset << 10) | u32::from(register)); // add xd, sp, #imm
    Ok(())
}

fn append_aarch64_instructions(bytes: &mut Vec<u8>, instructions: Vec<u32>) {
    bytes.extend(instructions.into_iter().flat_map(u32::to_le_bytes));
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmissionError {
    UnitOperationAfterReturn,
    UnitFunctionHasNoReturn,
    UnitCallStackAreaNotEncodable,
    UnsupportedAggregatePlacement,
    AggregatePlacementCoverageMismatch,
    UnsupportedAggregateFragmentWidth(u16),
    MissingUnitParameterHome(psi_core::PlaceId),
    UnitParameterHomeMismatch(psi_core::PlaceId),
    UnsupportedUnitRegister(MachineRegister),
    UnsupportedStructuralReturnPlacement(psi_core::PlaceId),
    UnsupportedStructuralResultRegister(MachineRegister),
    PortWriteUnsupportedOnArchitecture(Architecture),
    BoundaryPortReadUnsupported(Architecture),
    IntegerWidthNotNativelySupported {
        value: ValueId,
        bits: u16,
    },
    IntegerOutsideType(ValueId),
    IntegerSignMismatch(ValueId),
    ParameterRegisterArchitectureMismatch {
        value: ValueId,
        register: MachineRegister,
        architecture: Architecture,
    },
    IncomingStackOffsetNotEncodable {
        value: ValueId,
        byte_offset: u32,
    },
    CallStackAreaNotEncodable {
        value: ValueId,
        byte_size: u32,
    },
    ExpressionScratchRegisterConflict {
        value: ValueId,
        register: MachineRegister,
    },
    ExpressionParameterLocationConflict {
        value: ValueId,
        parameter_index: usize,
    },
    ExpressionParameterSpillMissing {
        value: ValueId,
        parameter_index: usize,
    },
    ExpressionStackDepthNotEncodable {
        value: ValueId,
    },
    ExpressionStackFrameNotEncodable,
    AssignedFrameSpillOutsideExpression(ValueId),
    AssignedFrameArchitectureMismatch(Architecture),
    AssignedFrameSizeMismatch,
    ConditionalBranchDistanceNotEncodable,
    ConditionalBranchEncodingInvalid,
    InternalCallRelocationOffsetNotEncodable,
    BooleanNotEncodingInvalid,
    UnsupportedCallArgumentRegister(MachineRegister),
    CallOutsideDirectReturnExpression,
    ScalarStackInstructionEncodingInvalid,
    EntryFunctionMissing(MachineId),
    InvalidNominalCleanupTarget(MachineId),
    UnsupportedScalarCleanup,
}

impl std::fmt::Display for EmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for EmissionError {}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
