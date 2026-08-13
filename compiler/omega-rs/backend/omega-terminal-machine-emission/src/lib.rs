#![forbid(unsafe_code)]

//! Machine-code emission for the first source-independent terminal-Psi target
//! operation slice.

use omega_calling_conventions::{
    IndirectPointerLocation, ValueLocation, ValuePlacement, ValueShape,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_assigned_target_operations::{
    TerminalAssignedAggregateCopy, TerminalAssignedBooleanControl,
    TerminalAssignedBooleanExpression, TerminalAssignedCallArgument,
    TerminalAssignedCallDestination, TerminalAssignedConditionalBooleanArm,
    TerminalAssignedConditionalIntegerArm, TerminalAssignedFunction,
    TerminalAssignedIntegerControl, TerminalAssignedIntegerExpression, TerminalAssignedOperation,
    TerminalAssignedOperationPlan, TerminalAssignedScalarExpression,
    TerminalAssignedScalarLocation, TerminalAssignedUnitBody, TerminalAssignedUnitOperation,
    TerminalExpressionFrame,
};
use omega_terminal_machine_code::{
    TerminalAarch64ReturnLinkEvidence, TerminalBoundarySettlementRecord,
    TerminalInternalCallRelocation, TerminalInternalUnitCallArgumentRecord,
    TerminalInternalUnitCallRecord, TerminalMachineCodeFunction, TerminalMachineCodePlan,
    TerminalNativeFuelAttribution, TerminalNativeFuelSite, TerminalPortEffectRecord,
    TerminalScalarCallStackEvidence, TerminalScalarConditionalCondition,
    TerminalScalarControlFlowEvidence, TerminalScalarStackEvidence, TerminalScalarStackMutation,
    TerminalScalarStackMutationKind, TerminalStackAdjustmentPair, TerminalStructuralReturnRecord,
    TerminalUnitCallStackEvidence, TerminalUnitStackEvidence,
};
use omega_terminal_target_operations::MachineRegister;
use psi_core::{IntegerSign, IntegerType, IntegerValue, MachineId, ValueId};

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
            .map(|function| emit_function(function, plan.target))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn emit_function(
    function: &TerminalAssignedFunction,
    target: NativeTarget,
) -> Result<TerminalMachineCodeFunction, EmissionError> {
    let architecture = target.architecture;
    let mut internal_calls = Vec::new();
    let mut internal_unit_calls = Vec::new();
    let mut fuel_attribution = Vec::new();
    let mut port_effects = Vec::new();
    let mut boundary_settlements = Vec::new();
    let mut structural_return = None;
    let mut unit_stack = None;
    let mut unit_parameter_homes = Vec::new();
    let mut scalar_stack_eligible = false;
    let mut scalar_control_flow = TerminalScalarControlFlowEvidence::Linear;
    let bytes = match &function.operation {
        TerminalAssignedOperation::UnitBody(body) => {
            let emitted = emit_unit_body(body, target)?;
            internal_calls = emitted.internal_calls;
            internal_unit_calls = emitted.internal_unit_calls;
            fuel_attribution = emitted.fuel_attribution;
            port_effects = emitted.port_effects;
            boundary_settlements = emitted.boundary_settlements;
            unit_stack = Some(emitted.stack);
            unit_parameter_homes = emitted.parameter_homes;
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
            scalar_stack_eligible = linear_integer_expression(expression);
            require_native_integer_width(*source_value, *scalar_type)?;
            match architecture {
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
            }
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
            scalar_stack_eligible =
                direct_linear_integer_arm(when_true) && direct_linear_integer_arm(when_false);
            if scalar_stack_eligible {
                scalar_control_flow = fragment
                    .conditional
                    .expect("top-level integer conditional retains its branch evidence");
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
            scalar_stack_eligible = linear_boolean_expression(condition)
                && direct_linear_integer_arm(when_true)
                && direct_linear_integer_arm(when_false);
            if scalar_stack_eligible {
                scalar_control_flow = fragment
                    .conditional
                    .expect("top-level integer expression conditional retains its branch evidence");
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
        } => match architecture {
            Architecture::Aarch64 => {
                let fragment = emit_aarch64_conditional_boolean_control(
                    *condition_source,
                    *condition_location,
                    when_true,
                    when_false,
                    target,
                )?;
                internal_calls = fragment.internal_calls;
                fragment.bytes
            }
            Architecture::X86_64 => {
                let fragment = emit_x86_64_conditional_boolean_control(
                    *condition_source,
                    *condition_location,
                    when_true,
                    when_false,
                    target,
                )?;
                internal_calls = fragment.internal_calls;
                fragment.bytes
            }
        },
        TerminalAssignedOperation::ReturnBooleanExpressionConditionalControl {
            condition_frame,
            condition,
            when_true,
            when_false,
            ..
        } => match architecture {
            Architecture::Aarch64 => {
                let fragment = emit_aarch64_conditional_boolean_expression_control(
                    condition_frame,
                    condition,
                    when_true,
                    when_false,
                    target,
                )?;
                internal_calls = fragment.internal_calls;
                fragment.bytes
            }
            Architecture::X86_64 => {
                let fragment = emit_x86_64_conditional_boolean_expression_control(
                    condition_frame,
                    condition,
                    when_true,
                    when_false,
                    target,
                )?;
                internal_calls = fragment.internal_calls;
                fragment.bytes
            }
        },
    };
    let scalar_stack = scalar_stack_eligible
        .then(|| collect_scalar_stack_evidence(architecture, &bytes, scalar_control_flow))
        .transpose()?;
    if !scalar_stack_eligible {
        for call in &mut internal_calls {
            call.scalar_stack = None;
        }
    }
    Ok(TerminalMachineCodeFunction {
        machine: function.machine,
        provenance: function.provenance.clone(),
        bytes,
        unit_stack,
        unit_parameter_homes,
        scalar_stack,
        internal_calls,
        internal_unit_calls,
        fuel_attribution,
        port_effects,
        boundary_settlements,
        structural_return,
    })
}

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

struct UnitEmission {
    bytes: Vec<u8>,
    internal_calls: Vec<TerminalInternalCallRelocation>,
    internal_unit_calls: Vec<TerminalInternalUnitCallRecord>,
    fuel_attribution: Vec<TerminalNativeFuelAttribution>,
    port_effects: Vec<TerminalPortEffectRecord>,
    boundary_settlements: Vec<TerminalBoundarySettlementRecord>,
    stack: TerminalUnitStackEvidence,
    parameter_homes: Vec<omega_terminal_machine_code::TerminalUnitParameterHomeRecord>,
}

#[derive(Debug, Clone)]
struct X86UnitParameterHome {
    place: psi_core::PlaceId,
    shape: ValueShape,
    source: ValuePlacement,
    byte_offset: u32,
    indirect: bool,
}

#[derive(Debug, Clone)]
struct Aarch64UnitParameterHome {
    place: psi_core::PlaceId,
    shape: ValueShape,
    source: ValuePlacement,
    byte_offset: u32,
    indirect: bool,
}

fn emit_unit_body(
    body: &TerminalAssignedUnitBody,
    target: NativeTarget,
) -> Result<UnitEmission, EmissionError> {
    let mut bytes = Vec::new();
    let mut internal_calls = Vec::new();
    let mut internal_unit_calls = Vec::new();
    let mut fuel_attribution = Vec::new();
    let mut port_effects = Vec::new();
    let mut boundary_settlements = Vec::new();
    let mut x86_homes = Vec::new();
    let mut x86_frame_bytes = 0;
    let mut aarch64_homes = Vec::new();
    let mut aarch64_frame_bytes = 0;
    let mut aarch64_lr_offset = 0;
    let mut frame_allocation = None;
    let mut frame_release = None;
    let mut aarch64_link_store = None;
    let mut aarch64_link_load = None;
    let parameter_homes;
    match target.architecture {
        Architecture::X86_64 => {
            (x86_homes, x86_frame_bytes) = x86_unit_parameter_homes(body)?;
            parameter_homes = body
                .parameters
                .iter()
                .zip(&x86_homes)
                .map(|(parameter, home)| {
                    omega_terminal_machine_code::TerminalUnitParameterHomeRecord {
                        place: parameter.place,
                        structural_type: parameter.structural_type,
                        shape: parameter.shape,
                        source: parameter.placement.clone(),
                        byte_offset: home.byte_offset,
                        indirect: home.indirect,
                    }
                })
                .collect();
            if x86_frame_bytes != 0 {
                let offset = bytes.len();
                emit_x86_64_adjust_sp(&mut bytes, x86_frame_bytes, false);
                frame_allocation = Some((offset, bytes.len() - offset));
                emit_x86_64_stage_unit_parameters(&mut bytes, &x86_homes, x86_frame_bytes)?;
            }
        }
        Architecture::Aarch64 => {
            let (homes, frame_bytes, lr_offset) = aarch64_unit_parameter_homes(body)?;
            aarch64_homes = homes;
            aarch64_frame_bytes = frame_bytes;
            aarch64_lr_offset = lr_offset;
            parameter_homes = body
                .parameters
                .iter()
                .zip(&aarch64_homes)
                .map(|(parameter, home)| {
                    omega_terminal_machine_code::TerminalUnitParameterHomeRecord {
                        place: parameter.place,
                        structural_type: parameter.structural_type,
                        shape: parameter.shape,
                        source: parameter.placement.clone(),
                        byte_offset: home.byte_offset,
                        indirect: home.indirect,
                    }
                })
                .collect();
            let mut instructions = Vec::new();
            emit_aarch64_adjust_sp(&mut instructions, frame_bytes, false)?;
            frame_allocation = Some((0, 4));
            aarch64_link_store = Some(4);
            instructions.push(aarch64_unit_stack_access(0xf900_0000, 30, lr_offset, 8)?);
            emit_aarch64_stage_unit_parameters(&mut instructions, &aarch64_homes, frame_bytes)?;
            append_aarch64_instructions(&mut bytes, instructions);
        }
    };
    let mut returned = false;
    for (operation_ordinal, operation) in body.operations.iter().enumerate() {
        if returned {
            return Err(EmissionError::UnitOperationAfterReturn);
        }
        let code_offset = bytes.len();
        let mut operation_site = None;
        let mut edge_site = None;
        match operation {
            TerminalAssignedUnitOperation::Call {
                psi_operation,
                callee,
                copies,
                claim_transfers,
            } => {
                operation_site = Some(*psi_operation);
                let argument_intervals = match target.architecture {
                    Architecture::X86_64 => emit_x86_64_unit_call(
                        &mut bytes,
                        *psi_operation,
                        *callee,
                        copies,
                        target,
                        &x86_homes,
                        &mut internal_calls,
                    )?,
                    Architecture::Aarch64 => emit_aarch64_unit_call(
                        &mut bytes,
                        *psi_operation,
                        *callee,
                        copies,
                        &aarch64_homes,
                        &mut internal_calls,
                    )?,
                };
                internal_unit_calls.push(TerminalInternalUnitCallRecord {
                    psi_operation: *psi_operation,
                    target: *callee,
                    arguments: copies
                        .iter()
                        .zip(argument_intervals)
                        .map(
                            |(
                                copy,
                                (
                                    code_offset,
                                    byte_count,
                                    source_home_byte_offset,
                                    call_stack_bytes,
                                ),
                            )| {
                                TerminalInternalUnitCallArgumentRecord {
                                    place: copy.place,
                                    path: copy.path.clone(),
                                    root_structural_type: copy.root_structural_type,
                                    structural_type: copy.structural_type,
                                    shape: copy.shape,
                                    source_byte_offset: copy.source_byte_offset,
                                    source_home_byte_offset,
                                    call_stack_bytes,
                                    fixed_array_length: copy.fixed_array_length,
                                    element_stride: copy.element_stride,
                                    source: copy.source.clone(),
                                    destination: copy.destination.clone(),
                                    code_offset,
                                    byte_count,
                                    bytes: bytes[code_offset..code_offset + byte_count].to_vec(),
                                }
                            },
                        )
                        .collect(),
                    claim_transfers: claim_transfers.clone(),
                    operation_ordinal,
                    code_offset,
                    byte_count: bytes.len() - code_offset,
                });
            }
            TerminalAssignedUnitOperation::PortWrite {
                psi_operation,
                service,
                port,
                value,
            } => {
                operation_site = Some(*psi_operation);
                if target.architecture != Architecture::X86_64 {
                    return Err(EmissionError::PortWriteUnsupportedOnArchitecture(
                        target.architecture,
                    ));
                }
                let code_offset = bytes.len();
                bytes.extend_from_slice(&omega_x86_encoding::encode_immediate_port_write(
                    *port, *value,
                ));
                port_effects.push(TerminalPortEffectRecord {
                    psi_operation: *psi_operation,
                    service: *service,
                    port: *port,
                    value: *value,
                    operation_ordinal,
                    code_offset,
                    byte_count: bytes.len() - code_offset,
                });
            }
            TerminalAssignedUnitOperation::BoundarySettlement {
                psi_operation,
                boundary,
                provider_execution,
                realization,
                arguments,
                completion_receipts,
            } => {
                operation_site = Some(*psi_operation);
                boundary_settlements.push(TerminalBoundarySettlementRecord {
                    psi_operation: *psi_operation,
                    boundary: *boundary,
                    provider_execution: (*provider_execution).into(),
                    realization: *realization,
                    arguments: arguments.clone(),
                    completion_receipts: completion_receipts.clone(),
                    operation_ordinal,
                    code_offset: bytes.len(),
                });
            }
            TerminalAssignedUnitOperation::Return { psi_edge } => {
                edge_site = Some(*psi_edge);
                match target.architecture {
                    Architecture::X86_64 => {
                        if x86_frame_bytes != 0 {
                            let offset = bytes.len();
                            emit_x86_64_adjust_sp(&mut bytes, x86_frame_bytes, true);
                            frame_release = Some((offset, bytes.len() - offset));
                        }
                        bytes.push(0xc3)
                    }
                    Architecture::Aarch64 => {
                        let mut instructions = Vec::new();
                        aarch64_link_load = Some(bytes.len());
                        instructions.push(aarch64_unit_stack_access(
                            0xf940_0000,
                            30,
                            aarch64_lr_offset,
                            8,
                        )?);
                        let release_offset = bytes.len() + 4;
                        emit_aarch64_adjust_sp(&mut instructions, aarch64_frame_bytes, true)?;
                        frame_release = Some((release_offset, 4));
                        append_aarch64_instructions(&mut bytes, instructions);
                        bytes.extend_from_slice(&0xd65f_03c0_u32.to_le_bytes())
                    }
                }
                returned = true;
            }
        }
        let site = match (operation_site, edge_site) {
            (Some(operation), None) => TerminalNativeFuelSite::Operation(operation),
            (None, Some(edge)) => TerminalNativeFuelSite::Edge(edge),
            _ => unreachable!("one Unit operation owns exactly one fuel site"),
        };
        fuel_attribution.push(TerminalNativeFuelAttribution {
            schedule: psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
            site,
            units: 1,
            operation_ordinal,
            code_offset,
            byte_count: bytes.len() - code_offset,
        });
    }
    if !returned {
        return Err(EmissionError::UnitFunctionHasNoReturn);
    }
    Ok(UnitEmission {
        bytes,
        internal_calls,
        internal_unit_calls,
        fuel_attribution,
        port_effects,
        boundary_settlements,
        stack: TerminalUnitStackEvidence {
            frame: match (frame_allocation, frame_release) {
                (
                    Some((allocation_offset, allocation_byte_count)),
                    Some((release_offset, release_byte_count)),
                ) => Some(TerminalStackAdjustmentPair {
                    byte_size: match target.architecture {
                        Architecture::X86_64 => x86_frame_bytes,
                        Architecture::Aarch64 => aarch64_frame_bytes,
                    },
                    allocation_offset,
                    allocation_byte_count,
                    release_offset,
                    release_byte_count,
                }),
                (None, None) => None,
                _ => unreachable!("Unit frame allocation and release are paired"),
            },
            aarch64_return_link: match (aarch64_link_store, aarch64_link_load) {
                (Some(store_offset), Some(load_offset)) => {
                    Some(TerminalAarch64ReturnLinkEvidence {
                        frame_byte_offset: aarch64_lr_offset,
                        store_offset,
                        load_offset,
                    })
                }
                (None, None) => None,
                _ => unreachable!("AArch64 Unit link save and restore are paired"),
            },
            stack_alignment: 16,
        },
        parameter_homes,
    })
}

fn emit_x86_64_unit_call(
    bytes: &mut Vec<u8>,
    psi_operation: psi_core::OperationId,
    callee: MachineId,
    copies: &[TerminalAssignedAggregateCopy],
    target: NativeTarget,
    homes: &[X86UnitParameterHome],
    internal_calls: &mut Vec<TerminalInternalCallRelocation>,
) -> Result<Vec<(usize, usize, u32, u32)>, EmissionError> {
    let outgoing_bytes = copies
        .iter()
        .map(|copy| outgoing_placement_extent(&copy.destination))
        .try_fold(0_u32, |extent, candidate| {
            candidate.map(|value| extent.max(value))
        })?
        .max(if target.object_format == ObjectFormat::Coff {
            32
        } else {
            0
        });
    // Function-entry RSP is 8 mod 16. Before CALL it must be 0 mod 16.
    let padding = (8 + 16 - (outgoing_bytes % 16)) % 16;
    let call_stack_bytes = outgoing_bytes
        .checked_add(padding)
        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
    let mut allocation = None;
    if call_stack_bytes != 0 {
        let allocation_offset = bytes.len();
        emit_x86_64_adjust_sp(bytes, call_stack_bytes, false);
        allocation = Some((allocation_offset, bytes.len() - allocation_offset));
    }
    let mut argument_intervals = Vec::with_capacity(copies.len());
    for copy in copies {
        let copy_offset = bytes.len();
        let home = homes
            .iter()
            .find(|home| home.place == copy.place)
            .ok_or(EmissionError::MissingUnitParameterHome(copy.place))?;
        if home.source != copy.source
            || copy
                .source_byte_offset
                .checked_add(u32::from(copy.shape.byte_size))
                .is_none_or(|end| end > u32::from(home.shape.byte_size))
        {
            return Err(EmissionError::UnitParameterHomeMismatch(copy.place));
        }
        emit_x86_64_aggregate_copy_from_home(bytes, copy, home, call_stack_bytes)?;
        argument_intervals.push((
            copy_offset,
            bytes.len() - copy_offset,
            home.byte_offset,
            call_stack_bytes,
        ));
    }
    bytes.push(0xe8);
    let offset = bytes.len();
    bytes.extend_from_slice(&0_i32.to_le_bytes());
    let mut release = None;
    if call_stack_bytes != 0 {
        let release_offset = bytes.len();
        emit_x86_64_adjust_sp(bytes, call_stack_bytes, true);
        release = Some((release_offset, bytes.len() - release_offset));
    }
    internal_calls.push(TerminalInternalCallRelocation {
        psi_operation,
        target: callee,
        unit_stack: Some(TerminalUnitCallStackEvidence {
            outbound: stack_adjustment_pair(call_stack_bytes, allocation, release),
        }),
        scalar_stack: None,
        offset,
    });
    Ok(argument_intervals)
}

fn emit_aarch64_unit_call(
    bytes: &mut Vec<u8>,
    psi_operation: psi_core::OperationId,
    callee: MachineId,
    copies: &[TerminalAssignedAggregateCopy],
    homes: &[Aarch64UnitParameterHome],
    internal_calls: &mut Vec<TerminalInternalCallRelocation>,
) -> Result<Vec<(usize, usize, u32, u32)>, EmissionError> {
    let outgoing_bytes = copies
        .iter()
        .map(|copy| aarch64_outgoing_placement_extent(&copy.destination))
        .try_fold(0_u32, |extent, candidate| {
            candidate.map(|value| extent.max(value))
        })?;
    let call_stack_bytes = align_u32(outgoing_bytes, 16)?;
    let mut instructions = Vec::new();
    let mut allocation = None;
    if call_stack_bytes != 0 {
        allocation = Some((bytes.len(), 4));
        emit_aarch64_adjust_sp(&mut instructions, call_stack_bytes, false)?;
    }
    let mut argument_intervals = Vec::with_capacity(copies.len());
    for copy in copies {
        let instruction_offset = instructions.len();
        let home = homes
            .iter()
            .find(|home| home.place == copy.place)
            .ok_or(EmissionError::MissingUnitParameterHome(copy.place))?;
        if home.source != copy.source
            || copy
                .source_byte_offset
                .checked_add(u32::from(copy.shape.byte_size))
                .is_none_or(|end| end > u32::from(home.shape.byte_size))
        {
            return Err(EmissionError::UnitParameterHomeMismatch(copy.place));
        }
        emit_aarch64_aggregate_copy_from_home(&mut instructions, copy, home, call_stack_bytes)?;
        argument_intervals.push((
            bytes.len() + instruction_offset * 4,
            (instructions.len() - instruction_offset) * 4,
            home.byte_offset,
            call_stack_bytes,
        ));
    }
    append_aarch64_instructions(bytes, instructions);
    let offset = bytes.len();
    bytes.extend_from_slice(&0x9400_0000_u32.to_le_bytes()); // bl #0
    let mut release = None;
    if call_stack_bytes != 0 {
        let mut instructions = Vec::new();
        release = Some((bytes.len(), 4));
        emit_aarch64_adjust_sp(&mut instructions, call_stack_bytes, true)?;
        append_aarch64_instructions(bytes, instructions);
    }
    internal_calls.push(TerminalInternalCallRelocation {
        psi_operation,
        target: callee,
        unit_stack: Some(TerminalUnitCallStackEvidence {
            outbound: stack_adjustment_pair(call_stack_bytes, allocation, release),
        }),
        scalar_stack: None,
        offset,
    });
    Ok(argument_intervals)
}

fn stack_adjustment_pair(
    byte_size: u32,
    allocation: Option<(usize, usize)>,
    release: Option<(usize, usize)>,
) -> Option<TerminalStackAdjustmentPair> {
    match (byte_size, allocation, release) {
        (0, None, None) => None,
        (
            byte_size,
            Some((allocation_offset, allocation_byte_count)),
            Some((release_offset, release_byte_count)),
        ) => Some(TerminalStackAdjustmentPair {
            byte_size,
            allocation_offset,
            allocation_byte_count,
            release_offset,
            release_byte_count,
        }),
        _ => unreachable!("nonzero stack adjustment must retain both encoded operations"),
    }
}

fn x86_unit_parameter_homes(
    body: &TerminalAssignedUnitBody,
) -> Result<(Vec<X86UnitParameterHome>, u32), EmissionError> {
    let mut homes = Vec::with_capacity(body.parameters.len());
    let mut cursor = 0_u32;
    for parameter in &body.parameters {
        cursor = align_u32(cursor, 8)?;
        let indirect = matches!(
            parameter.placement.locations.as_slice(),
            [ValueLocation::Indirect { .. }]
        );
        if parameter
            .placement
            .locations
            .iter()
            .any(|location| matches!(location, ValueLocation::Indirect { .. }))
            && !indirect
        {
            return Err(EmissionError::UnsupportedAggregatePlacement);
        }
        let byte_size = if indirect {
            8
        } else {
            u32::from(parameter.shape.byte_size)
        };
        homes.push(X86UnitParameterHome {
            place: parameter.place,
            shape: parameter.shape,
            source: parameter.placement.clone(),
            byte_offset: cursor,
            indirect,
        });
        cursor = cursor
            .checked_add(byte_size)
            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
    }
    Ok((homes, align_u32(cursor, 16)?))
}

fn aarch64_unit_parameter_homes(
    body: &TerminalAssignedUnitBody,
) -> Result<(Vec<Aarch64UnitParameterHome>, u32, u32), EmissionError> {
    let mut homes = Vec::with_capacity(body.parameters.len());
    let mut cursor = 0_u32;
    for parameter in &body.parameters {
        cursor = align_u32(cursor, u32::from(parameter.shape.alignment.clamp(8, 16)))?;
        let indirect = matches!(
            parameter.placement.locations.as_slice(),
            [ValueLocation::Indirect { .. }]
        );
        if parameter
            .placement
            .locations
            .iter()
            .any(|location| matches!(location, ValueLocation::Indirect { .. }))
            && !indirect
        {
            return Err(EmissionError::UnsupportedAggregatePlacement);
        }
        let byte_size = if indirect {
            8
        } else {
            u32::from(parameter.shape.byte_size)
        };
        homes.push(Aarch64UnitParameterHome {
            place: parameter.place,
            shape: parameter.shape,
            source: parameter.placement.clone(),
            byte_offset: cursor,
            indirect,
        });
        cursor = cursor
            .checked_add(byte_size)
            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
    }
    let lr_offset = align_u32(cursor, 8)?;
    let frame_bytes = lr_offset
        .checked_add(8)
        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)
        .and_then(|size| align_u32(size, 16))?;
    Ok((homes, frame_bytes, lr_offset))
}

fn emit_aarch64_stage_unit_parameters(
    instructions: &mut Vec<u32>,
    homes: &[Aarch64UnitParameterHome],
    frame_bytes: u32,
) -> Result<(), EmissionError> {
    for home in homes {
        if home.indirect {
            let [ValueLocation::Indirect { pointer, .. }] = home.source.locations.as_slice() else {
                return Err(EmissionError::UnsupportedAggregatePlacement);
            };
            match *pointer {
                IndirectPointerLocation::Register(register) => {
                    instructions.push(aarch64_unit_stack_access(
                        0xf900_0000,
                        aarch64_unit_register(register)?,
                        home.byte_offset,
                        8,
                    )?)
                }
                IndirectPointerLocation::Stack {
                    stack_byte_offset, ..
                } => {
                    let incoming = frame_bytes
                        .checked_add(stack_byte_offset)
                        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                    instructions.push(aarch64_unit_stack_access(0xf940_0000, 9, incoming, 8)?);
                    instructions.push(aarch64_unit_stack_access(
                        0xf900_0000,
                        9,
                        home.byte_offset,
                        8,
                    )?);
                }
            }
            continue;
        }
        for source in &home.source.locations {
            let (value_offset, byte_size) = placement_fragment(source)?;
            let destination = home
                .byte_offset
                .checked_add(u32::from(value_offset))
                .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
            match *source {
                ValueLocation::Register { register, .. } => {
                    instructions.push(aarch64_unit_stack_access(
                        aarch64_store_base(byte_size)?,
                        aarch64_unit_register(register)?,
                        destination,
                        byte_size,
                    )?)
                }
                ValueLocation::Stack {
                    stack_byte_offset, ..
                } => {
                    let incoming = frame_bytes
                        .checked_add(stack_byte_offset)
                        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                    instructions.push(aarch64_unit_stack_access(
                        aarch64_load_base(byte_size)?,
                        9,
                        incoming,
                        byte_size,
                    )?);
                    instructions.push(aarch64_unit_stack_access(
                        aarch64_store_base(byte_size)?,
                        9,
                        destination,
                        byte_size,
                    )?);
                }
                ValueLocation::Indirect { .. } => unreachable!(),
            }
        }
    }
    Ok(())
}

fn align_u32(value: u32, alignment: u32) -> Result<u32, EmissionError> {
    let remainder = value % alignment;
    if remainder == 0 {
        return Ok(value);
    }
    value
        .checked_add(alignment - remainder)
        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)
}

fn emit_x86_64_stage_unit_parameters(
    bytes: &mut Vec<u8>,
    homes: &[X86UnitParameterHome],
    frame_bytes: u32,
) -> Result<(), EmissionError> {
    for home in homes {
        if home.indirect {
            let [ValueLocation::Indirect { pointer, .. }] = home.source.locations.as_slice() else {
                return Err(EmissionError::UnsupportedAggregatePlacement);
            };
            match *pointer {
                IndirectPointerLocation::Register(register) => emit_x86_64_stack_store_width(
                    bytes,
                    x86_unit_register(register)?,
                    home.byte_offset,
                    8,
                )?,
                IndirectPointerLocation::Stack {
                    stack_byte_offset, ..
                } => {
                    let incoming = frame_bytes
                        .checked_add(8)
                        .and_then(|offset| offset.checked_add(stack_byte_offset))
                        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                    emit_x86_64_stack_load_width(bytes, 0, incoming, 8)?;
                    emit_x86_64_stack_store_width(bytes, 0, home.byte_offset, 8)?;
                }
            }
            continue;
        }
        for source in &home.source.locations {
            let (value_offset, byte_size) = placement_fragment(source)?;
            let destination = home
                .byte_offset
                .checked_add(u32::from(value_offset))
                .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
            match *source {
                ValueLocation::Register { register, .. } => emit_x86_64_stack_store_width(
                    bytes,
                    x86_unit_register(register)?,
                    destination,
                    byte_size,
                )?,
                ValueLocation::Stack {
                    stack_byte_offset, ..
                } => {
                    let incoming = frame_bytes
                        .checked_add(8)
                        .and_then(|offset| offset.checked_add(stack_byte_offset))
                        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                    emit_x86_64_stack_load_width(bytes, 0, incoming, byte_size)?;
                    emit_x86_64_stack_store_width(bytes, 0, destination, byte_size)?;
                }
                ValueLocation::Indirect { .. } => unreachable!(),
            }
        }
    }
    Ok(())
}

fn outgoing_placement_extent(placement: &ValuePlacement) -> Result<u32, EmissionError> {
    placement
        .locations
        .iter()
        .try_fold(0_u32, |extent, location| {
            let end = match *location {
                ValueLocation::Register { .. } => 0,
                ValueLocation::Stack {
                    stack_byte_offset,
                    byte_size,
                    ..
                } => stack_byte_offset
                    .checked_add(u32::from(byte_size.max(8)))
                    .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?,
                // Forwarding a by-value structural argument may reuse the exact
                // caller-owned copy. Only a stack-resident pointer needs outgoing
                // space; no second aggregate copy is fabricated.
                ValueLocation::Indirect { pointer, .. } => match pointer {
                    IndirectPointerLocation::Register(_) => 0,
                    IndirectPointerLocation::Stack {
                        stack_byte_offset, ..
                    } => stack_byte_offset
                        .checked_add(8)
                        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?,
                },
            };
            Ok(extent.max(end))
        })
}

fn aarch64_outgoing_placement_extent(placement: &ValuePlacement) -> Result<u32, EmissionError> {
    placement
        .locations
        .iter()
        .try_fold(0_u32, |extent, location| {
            let end = match *location {
                ValueLocation::Register { .. } => 0,
                ValueLocation::Stack {
                    stack_byte_offset,
                    byte_size,
                    ..
                } => stack_byte_offset
                    .checked_add(u32::from(byte_size.max(8)))
                    .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?,
                ValueLocation::Indirect {
                    pointer,
                    copy_stack_byte_offset,
                    byte_size,
                    ..
                } => {
                    let pointer_end = match pointer {
                        IndirectPointerLocation::Register(_) => 0,
                        IndirectPointerLocation::Stack {
                            stack_byte_offset, ..
                        } => stack_byte_offset
                            .checked_add(8)
                            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?,
                    };
                    let copy_end = copy_stack_byte_offset
                        .ok_or(EmissionError::UnsupportedAggregatePlacement)?
                        .checked_add(u32::from(byte_size).next_multiple_of(8))
                        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                    pointer_end.max(copy_end)
                }
            };
            Ok(extent.max(end))
        })
}

fn emit_x86_64_aggregate_copy_from_home(
    bytes: &mut Vec<u8>,
    copy: &TerminalAssignedAggregateCopy,
    home: &X86UnitParameterHome,
    call_stack_bytes: u32,
) -> Result<(), EmissionError> {
    if home.indirect {
        if !copy.path.is_empty() {
            if copy
                .destination
                .locations
                .iter()
                .any(|location| matches!(location, ValueLocation::Indirect { .. }))
            {
                return Err(EmissionError::UnsupportedAggregatePlacement);
            }
            let pointer_home = call_stack_bytes
                .checked_add(home.byte_offset)
                .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
            emit_x86_64_stack_load_width(bytes, 11, pointer_home, 8)?;
            for destination in &copy.destination.locations {
                let (value_offset, width) = placement_fragment(destination)?;
                let source_offset = copy
                    .source_byte_offset
                    .checked_add(u32::from(value_offset))
                    .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                match *destination {
                    ValueLocation::Register { register, .. } => {
                        emit_x86_64_memory_load_width(
                            bytes,
                            x86_unit_register(register)?,
                            11,
                            source_offset,
                            width,
                        )?;
                    }
                    ValueLocation::Stack {
                        stack_byte_offset, ..
                    } => {
                        emit_x86_64_memory_load_width(bytes, 0, 11, source_offset, width)?;
                        emit_x86_64_stack_store_width(bytes, 0, stack_byte_offset, width)?;
                    }
                    ValueLocation::Indirect { .. } => unreachable!(),
                }
            }
            return Ok(());
        }
        if copy.shape != home.shape {
            return Err(EmissionError::UnsupportedAggregatePlacement);
        }
        let [ValueLocation::Indirect { pointer, .. }] = copy.destination.locations.as_slice()
        else {
            return Err(EmissionError::UnsupportedAggregatePlacement);
        };
        let source_offset = call_stack_bytes
            .checked_add(home.byte_offset)
            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
        return match *pointer {
            IndirectPointerLocation::Register(register) => {
                emit_x86_64_stack_load_width(bytes, x86_unit_register(register)?, source_offset, 8)
            }
            IndirectPointerLocation::Stack {
                stack_byte_offset, ..
            } => {
                emit_x86_64_stack_load_width(bytes, 0, source_offset, 8)?;
                emit_x86_64_stack_store_width(bytes, 0, stack_byte_offset, 8)
            }
        };
    }
    if copy
        .destination
        .locations
        .iter()
        .any(|location| matches!(location, ValueLocation::Indirect { .. }))
    {
        return Err(EmissionError::UnsupportedAggregatePlacement);
    }
    for destination in &copy.destination.locations {
        let (destination_offset, destination_size) = placement_fragment(destination)?;
        let source_offset = call_stack_bytes
            .checked_add(home.byte_offset)
            .and_then(|offset| offset.checked_add(copy.source_byte_offset))
            .and_then(|offset| offset.checked_add(u32::from(destination_offset)))
            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
        match *destination {
            ValueLocation::Register { register, .. } => {
                let destination_register = x86_unit_register(register)?;
                emit_x86_64_stack_load_width(
                    bytes,
                    destination_register,
                    source_offset,
                    destination_size,
                )?;
            }
            ValueLocation::Stack {
                stack_byte_offset, ..
            } => {
                emit_x86_64_stack_load_width(bytes, 0, source_offset, destination_size)?;
                emit_x86_64_stack_store_width(bytes, 0, stack_byte_offset, destination_size)?;
            }
            ValueLocation::Indirect { .. } => unreachable!(),
        }
    }
    Ok(())
}

fn emit_aarch64_aggregate_copy_from_home(
    instructions: &mut Vec<u32>,
    copy: &TerminalAssignedAggregateCopy,
    home: &Aarch64UnitParameterHome,
    call_stack_bytes: u32,
) -> Result<(), EmissionError> {
    if home.indirect {
        if !copy.path.is_empty() {
            if copy
                .destination
                .locations
                .iter()
                .any(|location| matches!(location, ValueLocation::Indirect { .. }))
            {
                return Err(EmissionError::UnsupportedAggregatePlacement);
            }
            let pointer_home = call_stack_bytes
                .checked_add(home.byte_offset)
                .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
            instructions.push(aarch64_unit_stack_access(0xf940_0000, 9, pointer_home, 8)?);
            for destination in &copy.destination.locations {
                let (value_offset, width) = placement_fragment(destination)?;
                let source_offset = copy
                    .source_byte_offset
                    .checked_add(u32::from(value_offset))
                    .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                match *destination {
                    ValueLocation::Register { register, .. } => {
                        instructions.push(aarch64_unit_memory_access(
                            aarch64_load_base(width)?,
                            aarch64_unit_register(register)?,
                            9,
                            source_offset,
                            width,
                        )?)
                    }
                    ValueLocation::Stack {
                        stack_byte_offset, ..
                    } => {
                        instructions.push(aarch64_unit_memory_access(
                            aarch64_load_base(width)?,
                            10,
                            9,
                            source_offset,
                            width,
                        )?);
                        instructions.push(aarch64_unit_stack_access(
                            aarch64_store_base(width)?,
                            10,
                            stack_byte_offset,
                            width,
                        )?);
                    }
                    ValueLocation::Indirect { .. } => unreachable!(),
                }
            }
            return Ok(());
        }
        if copy.shape != home.shape {
            return Err(EmissionError::UnsupportedAggregatePlacement);
        }
        let [
            ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset: Some(copy_stack_byte_offset),
                byte_size,
                ..
            },
        ] = copy.destination.locations.as_slice()
        else {
            return Err(EmissionError::UnsupportedAggregatePlacement);
        };
        let source_offset = call_stack_bytes
            .checked_add(home.byte_offset)
            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
        instructions.push(aarch64_unit_stack_access(0xf940_0000, 9, source_offset, 8)?);
        let mut copied = 0_u32;
        while copied < u32::from(*byte_size) {
            let remaining = u32::from(*byte_size) - copied;
            let width = if remaining >= 8 {
                8_u16
            } else if remaining >= 4 {
                4
            } else if remaining >= 2 {
                2
            } else {
                1
            };
            instructions.push(aarch64_unit_memory_access(
                aarch64_load_base(width)?,
                10,
                9,
                copied,
                width,
            )?);
            instructions.push(aarch64_unit_stack_access(
                aarch64_store_base(width)?,
                10,
                copy_stack_byte_offset
                    .checked_add(copied)
                    .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?,
                width,
            )?);
            copied += u32::from(width);
        }
        match *pointer {
            IndirectPointerLocation::Register(register) => {
                emit_aarch64_sp_address(
                    instructions,
                    aarch64_unit_register(register)?,
                    *copy_stack_byte_offset,
                )?;
            }
            IndirectPointerLocation::Stack {
                stack_byte_offset, ..
            } => {
                emit_aarch64_sp_address(instructions, 10, *copy_stack_byte_offset)?;
                instructions.push(aarch64_unit_stack_access(
                    0xf900_0000,
                    10,
                    stack_byte_offset,
                    8,
                )?);
            }
        }
        return Ok(());
    }
    if copy
        .destination
        .locations
        .iter()
        .any(|location| matches!(location, ValueLocation::Indirect { .. }))
    {
        return Err(EmissionError::UnsupportedAggregatePlacement);
    }
    for destination in &copy.destination.locations {
        let (destination_offset, destination_size) = placement_fragment(destination)?;
        let source_offset = call_stack_bytes
            .checked_add(home.byte_offset)
            .and_then(|offset| offset.checked_add(copy.source_byte_offset))
            .and_then(|offset| offset.checked_add(u32::from(destination_offset)))
            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
        match *destination {
            ValueLocation::Register { register, .. } => {
                instructions.push(aarch64_unit_stack_access(
                    aarch64_load_base(destination_size)?,
                    aarch64_unit_register(register)?,
                    source_offset,
                    destination_size,
                )?)
            }
            ValueLocation::Stack {
                stack_byte_offset, ..
            } => {
                instructions.push(aarch64_unit_stack_access(
                    aarch64_load_base(destination_size)?,
                    9,
                    source_offset,
                    destination_size,
                )?);
                instructions.push(aarch64_unit_stack_access(
                    aarch64_store_base(destination_size)?,
                    9,
                    stack_byte_offset,
                    destination_size,
                )?);
            }
            ValueLocation::Indirect { .. } => unreachable!(),
        }
    }
    Ok(())
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

fn emit_native_crash(architecture: Architecture) -> Vec<u8> {
    match architecture {
        Architecture::Aarch64 => vec![0x00, 0x00, 0x20, 0xd4], // brk #0
        Architecture::X86_64 => vec![0x0f, 0x0b],              // ud2
    }
}

struct EmissionFragment {
    bytes: Vec<u8>,
    internal_calls: Vec<TerminalInternalCallRelocation>,
    conditional: Option<TerminalScalarControlFlowEvidence>,
}

impl EmissionFragment {
    fn without_calls(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            internal_calls: Vec::new(),
            conditional: None,
        }
    }

    fn append(&mut self, mut fragment: Self) -> Result<(), EmissionError> {
        let base = self.bytes.len();
        for relocation in &mut fragment.internal_calls {
            relocation.offset = relocation
                .offset
                .checked_add(base)
                .ok_or(EmissionError::InternalCallRelocationOffsetNotEncodable)?;
            if let Some(stack) = &mut relocation.scalar_stack {
                if let Some(outbound) = &mut stack.outbound {
                    outbound.allocation_offset = outbound
                        .allocation_offset
                        .checked_add(base)
                        .ok_or(EmissionError::InternalCallRelocationOffsetNotEncodable)?;
                    outbound.release_offset = outbound
                        .release_offset
                        .checked_add(base)
                        .ok_or(EmissionError::InternalCallRelocationOffsetNotEncodable)?;
                }
                if let Some(link) = &mut stack.aarch64_return_link {
                    link.store_offset = link
                        .store_offset
                        .checked_add(base)
                        .ok_or(EmissionError::InternalCallRelocationOffsetNotEncodable)?;
                    link.load_offset = link
                        .load_offset
                        .checked_add(base)
                        .ok_or(EmissionError::InternalCallRelocationOffsetNotEncodable)?;
                }
            }
        }
        self.bytes.append(&mut fragment.bytes);
        self.internal_calls.append(&mut fragment.internal_calls);
        Ok(())
    }
}

fn emit_x86_64_conditional_integer_control(
    condition_source: ValueId,
    condition_location: TerminalAssignedScalarLocation,
    scalar_type: IntegerType,
    when_true: &TerminalAssignedConditionalIntegerArm,
    when_false: &TerminalAssignedConditionalIntegerArm,
    target: NativeTarget,
) -> Result<EmissionFragment, EmissionError> {
    let mut bytes = emit_x86_64_parameter_return(condition_source, false, condition_location)?;
    if bytes.pop() != Some(0xc3) {
        return Err(EmissionError::ConditionalBranchEncodingInvalid);
    }
    bytes.extend_from_slice(&[0x85, 0xc0]); // test eax, eax
    let true_fragment = emit_x86_64_integer_control(scalar_type, &when_true.control, target)?;
    let false_fragment = emit_x86_64_integer_control(scalar_type, &when_false.control, target)?;
    let displacement = i32::try_from(true_fragment.bytes.len())
        .map_err(|_| EmissionError::ConditionalBranchDistanceNotEncodable)?;
    let branch_offset = bytes.len();
    bytes.extend_from_slice(&[0x0f, 0x84]); // jz false arm
    bytes.extend_from_slice(&displacement.to_le_bytes());
    let false_arm_offset = bytes
        .len()
        .checked_add(true_fragment.bytes.len())
        .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
    let mut fragment = EmissionFragment::without_calls(bytes);
    fragment.conditional = Some(TerminalScalarControlFlowEvidence::TopLevelTwoReturn {
        condition: TerminalScalarConditionalCondition::Parameter,
        branch_offset,
        branch_byte_count: 6,
        false_arm_offset,
    });
    fragment.append(true_fragment)?;
    fragment.append(false_fragment)?;
    Ok(fragment)
}

fn emit_x86_64_integer_control(
    scalar_type: IntegerType,
    control: &TerminalAssignedIntegerControl,
    target: NativeTarget,
) -> Result<EmissionFragment, EmissionError> {
    match control {
        TerminalAssignedIntegerControl::Crash { .. } => Ok(EmissionFragment::without_calls(
            emit_native_crash(Architecture::X86_64),
        )),
        TerminalAssignedIntegerControl::Return {
            source_value,
            frame,
            expression,
            ..
        } => {
            require_native_integer_width(*source_value, scalar_type)?;
            let mut internal_calls = Vec::new();
            let bytes = emit_x86_64_integer_expression(
                scalar_type,
                frame,
                expression,
                Some((&mut internal_calls, target)),
            )?;
            Ok(EmissionFragment {
                bytes,
                internal_calls,
                conditional: None,
            })
        }
        TerminalAssignedIntegerControl::Conditional {
            condition_source,
            condition_location,
            when_true,
            when_false,
            ..
        } => emit_x86_64_conditional_integer_control(
            *condition_source,
            *condition_location,
            scalar_type,
            when_true,
            when_false,
            target,
        ),
        TerminalAssignedIntegerControl::ConditionalExpression {
            condition_frame,
            condition,
            when_true,
            when_false,
            ..
        } => emit_x86_64_conditional_integer_expression_control(
            condition_frame,
            condition,
            scalar_type,
            when_true,
            when_false,
            target,
        ),
    }
}

fn emit_x86_64_conditional_integer_expression_control(
    condition_frame: &TerminalExpressionFrame,
    condition: &TerminalAssignedBooleanExpression,
    scalar_type: IntegerType,
    when_true: &TerminalAssignedConditionalIntegerArm,
    when_false: &TerminalAssignedConditionalIntegerArm,
    target: NativeTarget,
) -> Result<EmissionFragment, EmissionError> {
    let mut internal_calls = Vec::new();
    let mut bytes = emit_x86_64_boolean_condition_value(
        condition_frame,
        condition,
        Some((&mut internal_calls, target)),
    )?;
    let true_fragment = emit_x86_64_integer_control(scalar_type, &when_true.control, target)?;
    let false_fragment = emit_x86_64_integer_control(scalar_type, &when_false.control, target)?;
    let displacement = i32::try_from(true_fragment.bytes.len())
        .map_err(|_| EmissionError::ConditionalBranchDistanceNotEncodable)?;
    let branch_offset = bytes.len();
    bytes.extend_from_slice(&[0x0f, 0x84]); // jz false arm
    bytes.extend_from_slice(&displacement.to_le_bytes());
    let false_arm_offset = bytes
        .len()
        .checked_add(true_fragment.bytes.len())
        .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
    let mut fragment = EmissionFragment {
        bytes,
        internal_calls,
        conditional: Some(TerminalScalarControlFlowEvidence::TopLevelTwoReturn {
            condition: TerminalScalarConditionalCondition::Expression,
            branch_offset,
            branch_byte_count: 6,
            false_arm_offset,
        }),
    };
    fragment.append(true_fragment)?;
    fragment.append(false_fragment)?;
    Ok(fragment)
}

fn emit_x86_64_conditional_boolean_control(
    condition_source: ValueId,
    condition_location: TerminalAssignedScalarLocation,
    when_true: &TerminalAssignedConditionalBooleanArm,
    when_false: &TerminalAssignedConditionalBooleanArm,
    target: NativeTarget,
) -> Result<EmissionFragment, EmissionError> {
    let mut bytes = emit_x86_64_parameter_return(condition_source, false, condition_location)?;
    if bytes.pop() != Some(0xc3) {
        return Err(EmissionError::ConditionalBranchEncodingInvalid);
    }
    bytes.extend_from_slice(&[0x85, 0xc0]); // test eax, eax
    let true_fragment = emit_x86_64_boolean_control(&when_true.control, target)?;
    let false_fragment = emit_x86_64_boolean_control(&when_false.control, target)?;
    let displacement = i32::try_from(true_fragment.bytes.len())
        .map_err(|_| EmissionError::ConditionalBranchDistanceNotEncodable)?;
    bytes.extend_from_slice(&[0x0f, 0x84]); // jz false arm
    bytes.extend_from_slice(&displacement.to_le_bytes());
    let mut fragment = EmissionFragment::without_calls(bytes);
    fragment.append(true_fragment)?;
    fragment.append(false_fragment)?;
    Ok(fragment)
}

fn emit_x86_64_conditional_boolean_expression_control(
    condition_frame: &TerminalExpressionFrame,
    condition: &TerminalAssignedBooleanExpression,
    when_true: &TerminalAssignedConditionalBooleanArm,
    when_false: &TerminalAssignedConditionalBooleanArm,
    target: NativeTarget,
) -> Result<EmissionFragment, EmissionError> {
    let mut internal_calls = Vec::new();
    let mut bytes = emit_x86_64_boolean_condition_value(
        condition_frame,
        condition,
        Some((&mut internal_calls, target)),
    )?;
    let true_fragment = emit_x86_64_boolean_control(&when_true.control, target)?;
    let false_fragment = emit_x86_64_boolean_control(&when_false.control, target)?;
    let displacement = i32::try_from(true_fragment.bytes.len())
        .map_err(|_| EmissionError::ConditionalBranchDistanceNotEncodable)?;
    bytes.extend_from_slice(&[0x0f, 0x84]); // jz false arm
    bytes.extend_from_slice(&displacement.to_le_bytes());
    let mut fragment = EmissionFragment {
        bytes,
        internal_calls,
        conditional: None,
    };
    fragment.append(true_fragment)?;
    fragment.append(false_fragment)?;
    Ok(fragment)
}

fn emit_x86_64_boolean_control(
    control: &TerminalAssignedBooleanControl,
    target: NativeTarget,
) -> Result<EmissionFragment, EmissionError> {
    match control {
        TerminalAssignedBooleanControl::Crash { .. } => Ok(EmissionFragment::without_calls(
            emit_native_crash(Architecture::X86_64),
        )),
        TerminalAssignedBooleanControl::ReturnImmediate { value, .. } => Ok(
            EmissionFragment::without_calls(emit_x86_64_boolean_return(*value)),
        ),
        TerminalAssignedBooleanControl::ReturnParameter {
            source_value,
            location,
            ..
        } => Ok(EmissionFragment::without_calls(
            emit_x86_64_parameter_return(*source_value, false, *location)?,
        )),
        TerminalAssignedBooleanControl::ReturnNotParameter {
            source_value,
            location,
            ..
        } => Ok(EmissionFragment::without_calls(
            emit_x86_64_boolean_not_parameter_return(*source_value, *location)?,
        )),
        TerminalAssignedBooleanControl::ReturnExpression {
            frame, expression, ..
        } => {
            let mut internal_calls = Vec::new();
            let bytes = emit_x86_64_boolean_expression(
                frame,
                expression,
                Some((&mut internal_calls, target)),
            )?;
            Ok(EmissionFragment {
                bytes,
                internal_calls,
                conditional: None,
            })
        }
        TerminalAssignedBooleanControl::Conditional {
            condition_source,
            condition_location,
            when_true,
            when_false,
            ..
        } => emit_x86_64_conditional_boolean_control(
            *condition_source,
            *condition_location,
            when_true,
            when_false,
            target,
        ),
        TerminalAssignedBooleanControl::ConditionalExpression {
            condition_frame,
            condition,
            when_true,
            when_false,
            ..
        } => emit_x86_64_conditional_boolean_expression_control(
            condition_frame,
            condition,
            when_true,
            when_false,
            target,
        ),
    }
}

fn emit_aarch64_conditional_integer_control(
    condition_source: ValueId,
    condition_location: TerminalAssignedScalarLocation,
    scalar_type: IntegerType,
    when_true: &TerminalAssignedConditionalIntegerArm,
    when_false: &TerminalAssignedConditionalIntegerArm,
    target: NativeTarget,
) -> Result<EmissionFragment, EmissionError> {
    let (mut bytes, condition_register) =
        emit_aarch64_condition_load(condition_source, condition_location)?;
    let true_fragment = emit_aarch64_integer_control(scalar_type, &when_true.control, target)?;
    let false_fragment = emit_aarch64_integer_control(scalar_type, &when_false.control, target)?;
    let branch_words = true_fragment
        .bytes
        .len()
        .checked_div(4)
        .and_then(|words| words.checked_add(1))
        .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
    if branch_words > 0x3ffff {
        return Err(EmissionError::ConditionalBranchDistanceNotEncodable);
    }
    let cbz = 0x3400_0000_u32 | ((branch_words as u32) << 5) | u32::from(condition_register);
    let branch_offset = bytes.len();
    bytes.extend_from_slice(&cbz.to_le_bytes());
    let false_arm_offset = bytes
        .len()
        .checked_add(true_fragment.bytes.len())
        .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
    let mut fragment = EmissionFragment::without_calls(bytes);
    fragment.conditional = Some(TerminalScalarControlFlowEvidence::TopLevelTwoReturn {
        condition: TerminalScalarConditionalCondition::Parameter,
        branch_offset,
        branch_byte_count: 4,
        false_arm_offset,
    });
    fragment.append(true_fragment)?;
    fragment.append(false_fragment)?;
    Ok(fragment)
}

fn emit_aarch64_integer_control(
    scalar_type: IntegerType,
    control: &TerminalAssignedIntegerControl,
    target: NativeTarget,
) -> Result<EmissionFragment, EmissionError> {
    match control {
        TerminalAssignedIntegerControl::Crash { .. } => Ok(EmissionFragment::without_calls(
            emit_native_crash(Architecture::Aarch64),
        )),
        TerminalAssignedIntegerControl::Return {
            source_value,
            frame,
            expression,
            ..
        } => {
            require_native_integer_width(*source_value, scalar_type)?;
            let mut internal_calls = Vec::new();
            let bytes = emit_aarch64_integer_expression(
                scalar_type,
                frame,
                expression,
                Some((&mut internal_calls, target)),
            )?;
            Ok(EmissionFragment {
                bytes,
                internal_calls,
                conditional: None,
            })
        }
        TerminalAssignedIntegerControl::Conditional {
            condition_source,
            condition_location,
            when_true,
            when_false,
            ..
        } => emit_aarch64_conditional_integer_control(
            *condition_source,
            *condition_location,
            scalar_type,
            when_true,
            when_false,
            target,
        ),
        TerminalAssignedIntegerControl::ConditionalExpression {
            condition_frame,
            condition,
            when_true,
            when_false,
            ..
        } => emit_aarch64_conditional_integer_expression_control(
            condition_frame,
            condition,
            scalar_type,
            when_true,
            when_false,
            target,
        ),
    }
}

fn emit_aarch64_conditional_integer_expression_control(
    condition_frame: &TerminalExpressionFrame,
    condition: &TerminalAssignedBooleanExpression,
    scalar_type: IntegerType,
    when_true: &TerminalAssignedConditionalIntegerArm,
    when_false: &TerminalAssignedConditionalIntegerArm,
    target: NativeTarget,
) -> Result<EmissionFragment, EmissionError> {
    let mut internal_calls = Vec::new();
    let mut bytes = emit_aarch64_boolean_condition_value(
        condition_frame,
        condition,
        Some((&mut internal_calls, target)),
    )?;
    let true_fragment = emit_aarch64_integer_control(scalar_type, &when_true.control, target)?;
    let false_fragment = emit_aarch64_integer_control(scalar_type, &when_false.control, target)?;
    let branch_words = true_fragment
        .bytes
        .len()
        .checked_div(4)
        .and_then(|words| words.checked_add(1))
        .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
    if branch_words > 0x3ffff {
        return Err(EmissionError::ConditionalBranchDistanceNotEncodable);
    }
    let branch_equal = 0x5400_0000_u32 | ((branch_words as u32) << 5); // b.eq false
    let branch_offset = bytes.len();
    bytes.extend_from_slice(&branch_equal.to_le_bytes());
    let false_arm_offset = bytes
        .len()
        .checked_add(true_fragment.bytes.len())
        .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
    let mut fragment = EmissionFragment {
        bytes,
        internal_calls,
        conditional: Some(TerminalScalarControlFlowEvidence::TopLevelTwoReturn {
            condition: TerminalScalarConditionalCondition::Expression,
            branch_offset,
            branch_byte_count: 4,
            false_arm_offset,
        }),
    };
    fragment.append(true_fragment)?;
    fragment.append(false_fragment)?;
    Ok(fragment)
}

fn emit_aarch64_conditional_boolean_control(
    condition_source: ValueId,
    condition_location: TerminalAssignedScalarLocation,
    when_true: &TerminalAssignedConditionalBooleanArm,
    when_false: &TerminalAssignedConditionalBooleanArm,
    target: NativeTarget,
) -> Result<EmissionFragment, EmissionError> {
    let (mut bytes, condition_register) =
        emit_aarch64_condition_load(condition_source, condition_location)?;
    let true_fragment = emit_aarch64_boolean_control(&when_true.control, target)?;
    let false_fragment = emit_aarch64_boolean_control(&when_false.control, target)?;
    let branch_words = true_fragment
        .bytes
        .len()
        .checked_div(4)
        .and_then(|words| words.checked_add(1))
        .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
    if branch_words > 0x3ffff {
        return Err(EmissionError::ConditionalBranchDistanceNotEncodable);
    }
    let cbz = 0x3400_0000_u32 | ((branch_words as u32) << 5) | u32::from(condition_register);
    bytes.extend_from_slice(&cbz.to_le_bytes());
    let mut fragment = EmissionFragment::without_calls(bytes);
    fragment.append(true_fragment)?;
    fragment.append(false_fragment)?;
    Ok(fragment)
}

fn emit_aarch64_conditional_boolean_expression_control(
    condition_frame: &TerminalExpressionFrame,
    condition: &TerminalAssignedBooleanExpression,
    when_true: &TerminalAssignedConditionalBooleanArm,
    when_false: &TerminalAssignedConditionalBooleanArm,
    target: NativeTarget,
) -> Result<EmissionFragment, EmissionError> {
    let mut internal_calls = Vec::new();
    let mut bytes = emit_aarch64_boolean_condition_value(
        condition_frame,
        condition,
        Some((&mut internal_calls, target)),
    )?;
    let true_fragment = emit_aarch64_boolean_control(&when_true.control, target)?;
    let false_fragment = emit_aarch64_boolean_control(&when_false.control, target)?;
    let branch_words = true_fragment
        .bytes
        .len()
        .checked_div(4)
        .and_then(|words| words.checked_add(1))
        .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
    if branch_words > 0x3ffff {
        return Err(EmissionError::ConditionalBranchDistanceNotEncodable);
    }
    let branch_equal = 0x5400_0000_u32 | ((branch_words as u32) << 5); // b.eq false
    bytes.extend_from_slice(&branch_equal.to_le_bytes());
    let mut fragment = EmissionFragment {
        bytes,
        internal_calls,
        conditional: None,
    };
    fragment.append(true_fragment)?;
    fragment.append(false_fragment)?;
    Ok(fragment)
}

fn emit_aarch64_boolean_control(
    control: &TerminalAssignedBooleanControl,
    target: NativeTarget,
) -> Result<EmissionFragment, EmissionError> {
    match control {
        TerminalAssignedBooleanControl::Crash { .. } => Ok(EmissionFragment::without_calls(
            emit_native_crash(Architecture::Aarch64),
        )),
        TerminalAssignedBooleanControl::ReturnImmediate { value, .. } => Ok(
            EmissionFragment::without_calls(emit_aarch64_boolean_return(*value)),
        ),
        TerminalAssignedBooleanControl::ReturnParameter {
            source_value,
            location,
            ..
        } => Ok(EmissionFragment::without_calls(
            emit_aarch64_parameter_return(*source_value, false, *location)?,
        )),
        TerminalAssignedBooleanControl::ReturnNotParameter {
            source_value,
            location,
            ..
        } => Ok(EmissionFragment::without_calls(
            emit_aarch64_boolean_not_parameter_return(*source_value, *location)?,
        )),
        TerminalAssignedBooleanControl::ReturnExpression {
            frame, expression, ..
        } => {
            let mut internal_calls = Vec::new();
            let bytes = emit_aarch64_boolean_expression(
                frame,
                expression,
                Some((&mut internal_calls, target)),
            )?;
            Ok(EmissionFragment {
                bytes,
                internal_calls,
                conditional: None,
            })
        }
        TerminalAssignedBooleanControl::Conditional {
            condition_source,
            condition_location,
            when_true,
            when_false,
            ..
        } => emit_aarch64_conditional_boolean_control(
            *condition_source,
            *condition_location,
            when_true,
            when_false,
            target,
        ),
        TerminalAssignedBooleanControl::ConditionalExpression {
            condition_frame,
            condition,
            when_true,
            when_false,
            ..
        } => emit_aarch64_conditional_boolean_expression_control(
            condition_frame,
            condition,
            when_true,
            when_false,
            target,
        ),
    }
}

fn emit_x86_64_boolean_not_parameter_return(
    source: ValueId,
    location: TerminalAssignedScalarLocation,
) -> Result<Vec<u8>, EmissionError> {
    let mut bytes = emit_x86_64_parameter_return(source, false, location)?;
    if bytes.pop() != Some(0xc3) {
        return Err(EmissionError::BooleanNotEncodingInvalid);
    }
    bytes.extend_from_slice(&[0x83, 0xf0, 0x01]); // xor eax, 1
    bytes.push(0xc3); // ret
    Ok(bytes)
}

fn emit_aarch64_boolean_not_parameter_return(
    source: ValueId,
    location: TerminalAssignedScalarLocation,
) -> Result<Vec<u8>, EmissionError> {
    let mut bytes = emit_aarch64_parameter_return(source, false, location)?;
    if bytes.len() < 4 || bytes[bytes.len() - 4..] != 0xd65f_03c0_u32.to_le_bytes() {
        return Err(EmissionError::BooleanNotEncodingInvalid);
    }
    bytes.truncate(bytes.len() - 4);
    bytes.extend_from_slice(&0x5200_0000_u32.to_le_bytes()); // eor w0, w0, #1
    bytes.extend_from_slice(&0xd65f_03c0_u32.to_le_bytes()); // ret
    Ok(bytes)
}

fn emit_aarch64_condition_load(
    source: ValueId,
    location: TerminalAssignedScalarLocation,
) -> Result<(Vec<u8>, u8), EmissionError> {
    match location {
        TerminalAssignedScalarLocation::Register(MachineRegister::Aarch64X(register))
            if register < 31 =>
        {
            Ok((Vec::new(), register))
        }
        TerminalAssignedScalarLocation::Register(register) => {
            Err(EmissionError::ParameterRegisterArchitectureMismatch {
                value: source,
                register,
                architecture: Architecture::Aarch64,
            })
        }
        TerminalAssignedScalarLocation::IncomingStack { byte_offset } => {
            if byte_offset > 0xfff {
                return Err(EmissionError::IncomingStackOffsetNotEncodable {
                    value: source,
                    byte_offset,
                });
            }
            let register = 16_u8;
            let ldrb = 0x3940_0000_u32 | (byte_offset << 10) | (31 << 5) | u32::from(register);
            Ok((ldrb.to_le_bytes().to_vec(), register))
        }
        TerminalAssignedScalarLocation::FrameSpill { .. } => {
            Err(EmissionError::AssignedFrameSpillOutsideExpression(source))
        }
    }
}

fn emit_x86_64_boolean_return(value: bool) -> Vec<u8> {
    vec![0xb8, u8::from(value), 0, 0, 0, 0xc3] // mov eax, 0/1; ret
}

fn emit_aarch64_boolean_return(value: bool) -> Vec<u8> {
    let mov_w0 = 0x5280_0000_u32 | (u32::from(value) << 5);
    [mov_w0, 0xd65f_03c0]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect()
}

fn integer_bits(
    source: ValueId,
    scalar_type: IntegerType,
    value: IntegerValue,
) -> Result<u64, EmissionError> {
    let width = require_native_integer_width(source, scalar_type)?;
    if !scalar_type.admits(value) {
        return Err(EmissionError::IntegerOutsideType(source));
    }
    let mask = if width == 64 {
        u64::MAX
    } else {
        (1_u64 << width) - 1
    };
    let bits = match (scalar_type.sign(), value) {
        (IntegerSign::Signed, IntegerValue::Signed(value)) => value as u128 as u64,
        (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) => value as u64,
        _ => return Err(EmissionError::IntegerSignMismatch(source)),
    };
    Ok(bits & mask)
}

fn require_native_integer_width(
    source: ValueId,
    scalar_type: IntegerType,
) -> Result<u16, EmissionError> {
    let width = scalar_type.bits();
    if !matches!(width, 8 | 16 | 32 | 64) {
        return Err(EmissionError::IntegerWidthNotNativelySupported {
            value: source,
            bits: width,
        });
    }
    Ok(width)
}

fn emit_x86_64_parameter_return(
    source: ValueId,
    is_64: bool,
    location: TerminalAssignedScalarLocation,
) -> Result<Vec<u8>, EmissionError> {
    let mut bytes = Vec::new();
    match location {
        TerminalAssignedScalarLocation::Register(register) => {
            let register = x86_gpr_code(source, register)?;
            let rex = 0x40 | (u8::from(is_64) << 3) | (((register >> 3) & 1) << 2);
            if rex != 0x40 {
                bytes.push(rex);
            }
            bytes.push(0x89); // mov eax/rax, selected argument register
            bytes.push(0xc0 | ((register & 7) << 3));
        }
        TerminalAssignedScalarLocation::IncomingStack { byte_offset } => {
            let displacement = byte_offset.checked_add(8).ok_or(
                EmissionError::IncomingStackOffsetNotEncodable {
                    value: source,
                    byte_offset,
                },
            )?;
            if is_64 {
                bytes.push(0x48);
            }
            bytes.push(0x8b); // mov eax/rax, [rsp + displacement]
            if displacement <= i8::MAX as u32 {
                bytes.extend_from_slice(&[0x44, 0x24, displacement as u8]);
            } else {
                bytes.extend_from_slice(&[0x84, 0x24]);
                bytes.extend_from_slice(&displacement.to_le_bytes());
            }
        }
        TerminalAssignedScalarLocation::FrameSpill { .. } => {
            return Err(EmissionError::AssignedFrameSpillOutsideExpression(source));
        }
    }
    bytes.push(0xc3);
    Ok(bytes)
}

fn x86_gpr_code(source: ValueId, register: MachineRegister) -> Result<u8, EmissionError> {
    Ok(match register {
        MachineRegister::X86Rax => 0,
        MachineRegister::X86Rcx => 1,
        MachineRegister::X86Rdx => 2,
        MachineRegister::X86Rbx => 3,
        MachineRegister::X86Rsp => 4,
        MachineRegister::X86Rbp => 5,
        MachineRegister::X86Rsi => 6,
        MachineRegister::X86Rdi => 7,
        MachineRegister::X86R8 => 8,
        MachineRegister::X86R9 => 9,
        MachineRegister::X86R10 => 10,
        MachineRegister::X86R11 => 11,
        MachineRegister::X86R12 => 12,
        MachineRegister::X86R13 => 13,
        MachineRegister::X86R14 => 14,
        MachineRegister::X86R15 => 15,
        MachineRegister::X86Xmm(_)
        | MachineRegister::Aarch64X(_)
        | MachineRegister::Aarch64V(_) => {
            return Err(EmissionError::ParameterRegisterArchitectureMismatch {
                value: source,
                register,
                architecture: Architecture::X86_64,
            });
        }
    })
}

fn emit_aarch64_parameter_return(
    source: ValueId,
    is_64: bool,
    location: TerminalAssignedScalarLocation,
) -> Result<Vec<u8>, EmissionError> {
    let instruction = match location {
        TerminalAssignedScalarLocation::Register(MachineRegister::Aarch64X(register))
            if register < 31 =>
        {
            if register == 0 {
                None
            } else {
                let base = if is_64 { 0xaa00_03e0 } else { 0x2a00_03e0 };
                Some(base | (u32::from(register) << 16))
            }
        }
        TerminalAssignedScalarLocation::Register(register) => {
            return Err(EmissionError::ParameterRegisterArchitectureMismatch {
                value: source,
                register,
                architecture: Architecture::Aarch64,
            });
        }
        TerminalAssignedScalarLocation::IncomingStack { byte_offset } => {
            let scale = if is_64 { 8 } else { 4 };
            if byte_offset % scale != 0 || byte_offset / scale > 0xfff {
                return Err(EmissionError::IncomingStackOffsetNotEncodable {
                    value: source,
                    byte_offset,
                });
            }
            let base = if is_64 { 0xf940_0000 } else { 0xb940_0000 };
            Some(base | ((byte_offset / scale) << 10) | (31 << 5))
        }
        TerminalAssignedScalarLocation::FrameSpill { .. } => {
            return Err(EmissionError::AssignedFrameSpillOutsideExpression(source));
        }
    };
    Ok(instruction
        .into_iter()
        .chain([0xd65f_03c0])
        .flat_map(u32::to_le_bytes)
        .collect())
}

fn emit_x86_64_return(scalar_type: IntegerType, bits: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    if scalar_type.bits() <= 32 {
        bytes.push(0xb8); // mov eax, imm32
        bytes.extend_from_slice(&(bits as u32).to_le_bytes());
    } else {
        bytes.extend_from_slice(&[0x48, 0xb8]); // mov rax, imm64
        bytes.extend_from_slice(&bits.to_le_bytes());
    }
    bytes.push(0xc3); // ret
    bytes
}

fn emit_aarch64_return(scalar_type: IntegerType, bits: u64) -> Vec<u8> {
    let is_64 = scalar_type.bits() > 32;
    let chunk_count = if is_64 { 4 } else { 2 };
    let movz_base = if is_64 { 0xd280_0000 } else { 0x5280_0000 };
    let movk_base = if is_64 { 0xf280_0000 } else { 0x7280_0000 };
    let mut instructions = Vec::new();
    for chunk in 0..chunk_count {
        let immediate = ((bits >> (chunk * 16)) & 0xffff) as u32;
        if chunk == 0 || immediate != 0 {
            let base = if chunk == 0 { movz_base } else { movk_base };
            instructions.push(base | ((chunk as u32) << 21) | (immediate << 5));
        }
    }
    instructions.push(0xd65f_03c0); // ret x30
    instructions
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect()
}

fn emit_x86_64_boolean_expression(
    frame: &TerminalExpressionFrame,
    expression: &TerminalAssignedBooleanExpression,
    internal_calls: Option<(&mut Vec<TerminalInternalCallRelocation>, NativeTarget)>,
) -> Result<Vec<u8>, EmissionError> {
    let mut bytes = emit_x86_64_boolean_expression_value(frame, expression, internal_calls)?;
    bytes.push(0xc3); // ret
    Ok(bytes)
}

fn emit_x86_64_boolean_expression_value(
    frame: &TerminalExpressionFrame,
    expression: &TerminalAssignedBooleanExpression,
    mut internal_calls: Option<(&mut Vec<TerminalInternalCallRelocation>, NativeTarget)>,
) -> Result<Vec<u8>, EmissionError> {
    if frame.byte_size == 0 && !frame.register_spills.is_empty() {
        return Err(EmissionError::AssignedFrameSizeMismatch);
    }
    let mut bytes = Vec::new();
    if frame.byte_size != 0 {
        emit_x86_64_adjust_sp(&mut bytes, frame.byte_size, false);
        for spill in &frame.register_spills {
            let register = x86_gpr_code(spill.source_value, spill.register)?;
            if register == 4 {
                return Err(EmissionError::ExpressionScratchRegisterConflict {
                    value: spill.source_value,
                    register: spill.register,
                });
            }
            emit_x86_64_stack_store(&mut bytes, register, spill.byte_offset);
        }
    }
    emit_x86_64_boolean_expression_node(
        &mut bytes,
        expression,
        frame.byte_size,
        0,
        &mut internal_calls,
    )?;
    if frame.byte_size != 0 {
        emit_x86_64_adjust_sp(&mut bytes, frame.byte_size, true);
    }
    Ok(bytes)
}

fn emit_x86_64_boolean_condition_value(
    frame: &TerminalExpressionFrame,
    expression: &TerminalAssignedBooleanExpression,
    mut internal_calls: Option<(&mut Vec<TerminalInternalCallRelocation>, NativeTarget)>,
) -> Result<Vec<u8>, EmissionError> {
    if frame.byte_size == 0 && !frame.register_spills.is_empty() {
        return Err(EmissionError::AssignedFrameSizeMismatch);
    }
    let mut bytes = Vec::new();
    if frame.byte_size != 0 {
        emit_x86_64_adjust_sp(&mut bytes, frame.byte_size, false);
        for spill in &frame.register_spills {
            let register = x86_gpr_code(spill.source_value, spill.register)?;
            if register == 4 {
                return Err(EmissionError::ExpressionScratchRegisterConflict {
                    value: spill.source_value,
                    register: spill.register,
                });
            }
            emit_x86_64_stack_store(&mut bytes, register, spill.byte_offset);
        }
    }
    emit_x86_64_boolean_expression_node(
        &mut bytes,
        expression,
        frame.byte_size,
        0,
        &mut internal_calls,
    )?;
    bytes.extend_from_slice(&[0x85, 0xc0]); // test eax, eax
    for spill in &frame.register_spills {
        let register = x86_gpr_code(spill.source_value, spill.register)?;
        emit_x86_64_stack_load(&mut bytes, register, spill.byte_offset);
    }
    if frame.byte_size != 0 {
        emit_x86_64_restore_sp_preserving_flags(&mut bytes, frame.byte_size);
    }
    Ok(bytes)
}

fn emit_x86_64_restore_sp_preserving_flags(bytes: &mut Vec<u8>, byte_size: u32) {
    if byte_size <= i8::MAX as u32 {
        bytes.extend_from_slice(&[0x48, 0x8d, 0x64, 0x24, byte_size as u8]);
    } else {
        bytes.extend_from_slice(&[0x48, 0x8d, 0xa4, 0x24]);
        bytes.extend_from_slice(&byte_size.to_le_bytes());
    }
}

fn emit_x86_64_boolean_expression_node(
    bytes: &mut Vec<u8>,
    expression: &TerminalAssignedBooleanExpression,
    frame_byte_size: u32,
    stack_depth: u32,
    internal_calls: &mut Option<(&mut Vec<TerminalInternalCallRelocation>, NativeTarget)>,
) -> Result<(), EmissionError> {
    match expression {
        TerminalAssignedBooleanExpression::Call {
            psi_operation,
            source_value,
            callee,
            arguments,
        } => {
            emit_x86_64_call(
                bytes,
                *psi_operation,
                *source_value,
                *callee,
                arguments,
                frame_byte_size,
                stack_depth,
                internal_calls,
            )?;
            bytes.extend_from_slice(&[0x83, 0xe0, 0x01]); // and eax, 1
        }
        TerminalAssignedBooleanExpression::Immediate { value, .. } => {
            bytes.push(0xb8); // mov eax, imm32
            bytes.extend_from_slice(&u32::from(*value).to_le_bytes());
        }
        TerminalAssignedBooleanExpression::Parameter {
            source_value,
            location,
            ..
        } => {
            match location {
                TerminalAssignedScalarLocation::Register(register) => {
                    let register_code = x86_gpr_code(*source_value, *register)?;
                    if matches!(register_code, 0 | 4 | 10 | 11) {
                        return Err(EmissionError::ExpressionScratchRegisterConflict {
                            value: *source_value,
                            register: *register,
                        });
                    }
                    let rex = 0x48 | (((register_code >> 3) & 1) << 2);
                    bytes.extend_from_slice(&[rex, 0x89, 0xc0 | ((register_code & 7) << 3)]);
                }
                TerminalAssignedScalarLocation::FrameSpill { byte_offset } => {
                    let displacement = byte_offset.checked_add(stack_depth).ok_or(
                        EmissionError::IncomingStackOffsetNotEncodable {
                            value: *source_value,
                            byte_offset: *byte_offset,
                        },
                    )?;
                    bytes.extend_from_slice(&[0x48, 0x8b]);
                    if displacement <= i8::MAX as u32 {
                        bytes.extend_from_slice(&[0x44, 0x24, displacement as u8]);
                    } else {
                        bytes.extend_from_slice(&[0x84, 0x24]);
                        bytes.extend_from_slice(&displacement.to_le_bytes());
                    }
                }
                TerminalAssignedScalarLocation::IncomingStack { byte_offset } => {
                    let displacement = byte_offset
                        .checked_add(8)
                        .and_then(|offset| offset.checked_add(frame_byte_size))
                        .and_then(|offset| offset.checked_add(stack_depth))
                        .ok_or(EmissionError::IncomingStackOffsetNotEncodable {
                            value: *source_value,
                            byte_offset: *byte_offset,
                        })?;
                    bytes.extend_from_slice(&[0x48, 0x8b]);
                    if displacement <= i8::MAX as u32 {
                        bytes.extend_from_slice(&[0x44, 0x24, displacement as u8]);
                    } else {
                        bytes.extend_from_slice(&[0x84, 0x24]);
                        bytes.extend_from_slice(&displacement.to_le_bytes());
                    }
                }
            }
            bytes.extend_from_slice(&[0x83, 0xe0, 0x01]); // and eax, 1
        }
        TerminalAssignedBooleanExpression::Not { operand, .. } => {
            emit_x86_64_boolean_expression_node(
                bytes,
                operand,
                frame_byte_size,
                stack_depth,
                internal_calls,
            )?;
            bytes.extend_from_slice(&[0x83, 0xf0, 0x01]); // xor eax, 1
        }
        TerminalAssignedBooleanExpression::Equal { left, right, .. } => {
            emit_x86_64_boolean_expression_node(
                bytes,
                left,
                frame_byte_size,
                stack_depth,
                internal_calls,
            )?;
            bytes.push(0x50); // push rax
            let nested_depth = stack_depth.checked_add(8).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: boolean_expression_source(left),
                },
            )?;
            emit_x86_64_boolean_expression_node(
                bytes,
                right,
                frame_byte_size,
                nested_depth,
                internal_calls,
            )?;
            bytes.extend_from_slice(&[0x41, 0x5a]); // pop r10
            bytes.extend_from_slice(&[0x49, 0x39, 0xc2]); // cmp r10, rax
            bytes.extend_from_slice(&[0x0f, 0x94, 0xc0]); // sete al
            bytes.extend_from_slice(&[0x0f, 0xb6, 0xc0]); // movzx eax, al
        }
        TerminalAssignedBooleanExpression::IntegerEqual {
            scalar_type,
            left,
            right,
            ..
        } => {
            emit_x86_64_expression_node(
                bytes,
                *scalar_type,
                left,
                frame_byte_size,
                stack_depth,
                internal_calls,
            )?;
            bytes.push(0x50); // push rax
            let nested_depth = stack_depth.checked_add(8).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_x86_64_expression_node(
                bytes,
                *scalar_type,
                right,
                frame_byte_size,
                nested_depth,
                internal_calls,
            )?;
            bytes.extend_from_slice(&[0x41, 0x5a]); // pop r10
            bytes.extend_from_slice(&[0x49, 0x39, 0xc2]); // cmp r10, rax
            bytes.extend_from_slice(&[0x0f, 0x94, 0xc0]); // sete al
            bytes.extend_from_slice(&[0x0f, 0xb6, 0xc0]); // movzx eax, al
        }
        TerminalAssignedBooleanExpression::IntegerLessThan {
            scalar_type,
            left,
            right,
            ..
        }
        | TerminalAssignedBooleanExpression::IntegerLessOrEqual {
            scalar_type,
            left,
            right,
            ..
        } => {
            emit_x86_64_expression_node(
                bytes,
                *scalar_type,
                left,
                frame_byte_size,
                stack_depth,
                internal_calls,
            )?;
            bytes.push(0x50); // push rax
            let nested_depth = stack_depth.checked_add(8).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_x86_64_expression_node(
                bytes,
                *scalar_type,
                right,
                frame_byte_size,
                nested_depth,
                internal_calls,
            )?;
            bytes.extend_from_slice(&[0x41, 0x5a]); // pop r10
            bytes.extend_from_slice(&[0x49, 0x39, 0xc2]); // cmp r10, rax
            let inclusive = matches!(
                expression,
                TerminalAssignedBooleanExpression::IntegerLessOrEqual { .. }
            );
            let setcc = match (scalar_type.sign(), inclusive) {
                (IntegerSign::Signed, false) => 0x9c,   // setl al
                (IntegerSign::Unsigned, false) => 0x92, // setb al
                (IntegerSign::Signed, true) => 0x9e,    // setle al
                (IntegerSign::Unsigned, true) => 0x96,  // setbe al
            };
            bytes.extend_from_slice(&[0x0f, setcc, 0xc0]);
            bytes.extend_from_slice(&[0x0f, 0xb6, 0xc0]); // movzx eax, al
        }
    }
    Ok(())
}

fn emit_x86_64_integer_expression(
    scalar_type: IntegerType,
    frame: &TerminalExpressionFrame,
    expression: &TerminalAssignedIntegerExpression,
    mut internal_calls: Option<(&mut Vec<TerminalInternalCallRelocation>, NativeTarget)>,
) -> Result<Vec<u8>, EmissionError> {
    if frame.byte_size == 0 && !frame.register_spills.is_empty() {
        return Err(EmissionError::AssignedFrameSizeMismatch);
    }
    let mut bytes = Vec::new();
    if frame.byte_size != 0 {
        emit_x86_64_adjust_sp(&mut bytes, frame.byte_size, false);
        for spill in &frame.register_spills {
            let register = x86_gpr_code(spill.source_value, spill.register)?;
            if register == 4 {
                return Err(EmissionError::ExpressionScratchRegisterConflict {
                    value: spill.source_value,
                    register: spill.register,
                });
            }
            emit_x86_64_stack_store(&mut bytes, register, spill.byte_offset);
        }
    }
    emit_x86_64_expression_node(
        &mut bytes,
        scalar_type,
        expression,
        frame.byte_size,
        0,
        &mut internal_calls,
    )?;
    if frame.byte_size != 0 {
        emit_x86_64_adjust_sp(&mut bytes, frame.byte_size, true);
    }
    bytes.push(0xc3); // ret
    Ok(bytes)
}

fn emit_x86_64_adjust_sp(bytes: &mut Vec<u8>, byte_size: u32, add: bool) {
    if byte_size <= i8::MAX as u32 {
        bytes.extend_from_slice(&[0x48, 0x83, if add { 0xc4 } else { 0xec }, byte_size as u8]);
    } else {
        bytes.extend_from_slice(&[0x48, 0x81, if add { 0xc4 } else { 0xec }]);
        bytes.extend_from_slice(&byte_size.to_le_bytes());
    }
}

fn emit_x86_64_stack_store(bytes: &mut Vec<u8>, register: u8, byte_offset: u32) {
    bytes.push(0x48 | (((register >> 3) & 1) << 2));
    bytes.push(0x89); // mov [rsp + displacement], selected incoming register
    if byte_offset <= i8::MAX as u32 {
        bytes.extend_from_slice(&[0x44 | ((register & 7) << 3), 0x24, byte_offset as u8]);
    } else {
        bytes.extend_from_slice(&[0x84 | ((register & 7) << 3), 0x24]);
        bytes.extend_from_slice(&byte_offset.to_le_bytes());
    }
}

fn emit_x86_64_stack_load(bytes: &mut Vec<u8>, register: u8, byte_offset: u32) {
    bytes.push(0x48 | (((register >> 3) & 1) << 2));
    bytes.push(0x8b); // mov selected register, [rsp + displacement]
    if byte_offset <= i8::MAX as u32 {
        bytes.extend_from_slice(&[0x44 | ((register & 7) << 3), 0x24, byte_offset as u8]);
    } else {
        bytes.extend_from_slice(&[0x84 | ((register & 7) << 3), 0x24]);
        bytes.extend_from_slice(&byte_offset.to_le_bytes());
    }
}

fn emit_x86_64_stack_store_width(
    bytes: &mut Vec<u8>,
    register: u8,
    byte_offset: u32,
    byte_size: u16,
) -> Result<(), EmissionError> {
    match byte_size {
        1 => bytes.push(0x40 | (((register >> 3) & 1) << 2)),
        2 => {
            bytes.push(0x66);
            bytes.push(0x40 | (((register >> 3) & 1) << 2));
        }
        4 => bytes.push(0x40 | (((register >> 3) & 1) << 2)),
        8 => bytes.push(0x48 | (((register >> 3) & 1) << 2)),
        width => return Err(EmissionError::UnsupportedAggregateFragmentWidth(width)),
    }
    bytes.push(if byte_size == 1 { 0x88 } else { 0x89 });
    emit_x86_64_rsp_modrm(bytes, register, byte_offset);
    Ok(())
}

fn emit_x86_64_stack_load_width(
    bytes: &mut Vec<u8>,
    register: u8,
    byte_offset: u32,
    byte_size: u16,
) -> Result<(), EmissionError> {
    match byte_size {
        1 => {
            bytes.push(0x40 | (((register >> 3) & 1) << 2));
            bytes.extend_from_slice(&[0x0f, 0xb6]);
        }
        2 => {
            bytes.push(0x66);
            bytes.push(0x40 | (((register >> 3) & 1) << 2));
            bytes.extend_from_slice(&[0x0f, 0xb7]);
        }
        4 => {
            bytes.push(0x40 | (((register >> 3) & 1) << 2));
            bytes.push(0x8b);
        }
        8 => {
            bytes.push(0x48 | (((register >> 3) & 1) << 2));
            bytes.push(0x8b);
        }
        width => return Err(EmissionError::UnsupportedAggregateFragmentWidth(width)),
    }
    emit_x86_64_rsp_modrm(bytes, register, byte_offset);
    Ok(())
}

fn emit_x86_64_memory_load_width(
    bytes: &mut Vec<u8>,
    destination: u8,
    base: u8,
    byte_offset: u32,
    byte_size: u16,
) -> Result<(), EmissionError> {
    match byte_size {
        1 => {
            bytes.push(0x40 | (((destination >> 3) & 1) << 2) | ((base >> 3) & 1));
            bytes.extend_from_slice(&[0x0f, 0xb6]);
        }
        2 => {
            bytes.push(0x66);
            bytes.push(0x40 | (((destination >> 3) & 1) << 2) | ((base >> 3) & 1));
            bytes.extend_from_slice(&[0x0f, 0xb7]);
        }
        4 => {
            bytes.push(0x40 | (((destination >> 3) & 1) << 2) | ((base >> 3) & 1));
            bytes.push(0x8b);
        }
        8 => {
            bytes.push(0x48 | (((destination >> 3) & 1) << 2) | ((base >> 3) & 1));
            bytes.push(0x8b);
        }
        width => return Err(EmissionError::UnsupportedAggregateFragmentWidth(width)),
    }
    if byte_offset == 0 && (base & 7) != 5 {
        bytes.push(((destination & 7) << 3) | (base & 7));
    } else if byte_offset <= i8::MAX as u32 {
        bytes.push(0x40 | ((destination & 7) << 3) | (base & 7));
        bytes.push(byte_offset as u8);
    } else {
        bytes.push(0x80 | ((destination & 7) << 3) | (base & 7));
        bytes.extend_from_slice(&byte_offset.to_le_bytes());
    }
    Ok(())
}

fn emit_x86_64_rsp_modrm(bytes: &mut Vec<u8>, register: u8, byte_offset: u32) {
    if byte_offset <= i8::MAX as u32 {
        bytes.extend_from_slice(&[0x44 | ((register & 7) << 3), 0x24, byte_offset as u8]);
    } else {
        bytes.extend_from_slice(&[0x84 | ((register & 7) << 3), 0x24]);
        bytes.extend_from_slice(&byte_offset.to_le_bytes());
    }
}

fn emit_x86_64_call(
    bytes: &mut Vec<u8>,
    psi_operation: psi_core::OperationId,
    source_value: ValueId,
    callee: MachineId,
    arguments: &[TerminalAssignedCallArgument],
    frame_byte_size: u32,
    stack_depth: u32,
    internal_calls: &mut Option<(&mut Vec<TerminalInternalCallRelocation>, NativeTarget)>,
) -> Result<(), EmissionError> {
    for argument in arguments {
        match &argument.expression {
            TerminalAssignedScalarExpression::Boolean(expression) => {
                emit_x86_64_boolean_expression_node(
                    bytes,
                    expression,
                    frame_byte_size,
                    stack_depth,
                    internal_calls,
                )?;
            }
            TerminalAssignedScalarExpression::Integer {
                scalar_type,
                expression,
            } => emit_x86_64_expression_node(
                bytes,
                *scalar_type,
                expression,
                frame_byte_size,
                stack_depth,
                internal_calls,
            )?,
        }
        let byte_offset = argument.spill_byte_offset.checked_add(stack_depth).ok_or(
            EmissionError::IncomingStackOffsetNotEncodable {
                value: source_value,
                byte_offset: argument.spill_byte_offset,
            },
        )?;
        emit_x86_64_stack_store(bytes, 0, byte_offset);
    }
    let Some((relocations, target)) = internal_calls.as_mut() else {
        return Err(EmissionError::CallOutsideDirectReturnExpression);
    };
    let shadow_bytes = if target.object_format == ObjectFormat::Coff {
        32
    } else {
        0
    };
    let outgoing_stack_bytes = outgoing_stack_bytes(source_value, arguments)?.max(shadow_bytes);
    let unaligned_depth = stack_depth.checked_add(outgoing_stack_bytes).ok_or(
        EmissionError::CallStackAreaNotEncodable {
            value: source_value,
            byte_size: outgoing_stack_bytes,
        },
    )?;
    // Entry RSP is 8 modulo 16 after the return address. Expression frames are
    // 16-byte aligned, so the call-time allocation must make the cumulative
    // depth 8 modulo 16 before `call` pushes the next return address.
    let alignment_padding = (8 + 16 - (unaligned_depth % 16)) % 16;
    let call_stack_bytes = outgoing_stack_bytes.checked_add(alignment_padding).ok_or(
        EmissionError::CallStackAreaNotEncodable {
            value: source_value,
            byte_size: outgoing_stack_bytes,
        },
    )?;
    let mut allocation = None;
    if call_stack_bytes != 0 {
        let allocation_offset = bytes.len();
        emit_x86_64_adjust_sp(bytes, call_stack_bytes, false);
        allocation = Some((allocation_offset, bytes.len() - allocation_offset));
    }
    for argument in arguments {
        let TerminalAssignedCallDestination::OutgoingStack { byte_offset } = argument.destination
        else {
            continue;
        };
        let spill_byte_offset = argument
            .spill_byte_offset
            .checked_add(stack_depth)
            .and_then(|offset| offset.checked_add(call_stack_bytes))
            .ok_or(EmissionError::CallStackAreaNotEncodable {
                value: source_value,
                byte_size: call_stack_bytes,
            })?;
        emit_x86_64_stack_load(bytes, 0, spill_byte_offset);
        emit_x86_64_stack_store(bytes, 0, byte_offset);
    }
    for argument in arguments {
        let TerminalAssignedCallDestination::Register(register) = argument.destination else {
            continue;
        };
        let register = x86_gpr_code(source_value, register)?;
        if register == 4 {
            return Err(EmissionError::UnsupportedCallArgumentRegister(
                MachineRegister::X86Rsp,
            ));
        }
        let byte_offset = argument
            .spill_byte_offset
            .checked_add(stack_depth)
            .and_then(|offset| offset.checked_add(call_stack_bytes))
            .ok_or(EmissionError::CallStackAreaNotEncodable {
                value: source_value,
                byte_size: call_stack_bytes,
            })?;
        emit_x86_64_stack_load(bytes, register, byte_offset);
    }
    bytes.push(0xe8); // call rel32
    let offset = bytes.len();
    bytes.extend_from_slice(&0_i32.to_le_bytes());
    let mut release = None;
    if call_stack_bytes != 0 {
        let release_offset = bytes.len();
        emit_x86_64_adjust_sp(bytes, call_stack_bytes, true);
        release = Some((release_offset, bytes.len() - release_offset));
    }
    relocations.push(TerminalInternalCallRelocation {
        psi_operation,
        target: callee,
        unit_stack: None,
        scalar_stack: Some(TerminalScalarCallStackEvidence {
            outbound: stack_adjustment_pair(call_stack_bytes, allocation, release),
            aarch64_return_link: None,
        }),
        offset,
    });
    Ok(())
}

fn emit_x86_64_expression_node(
    bytes: &mut Vec<u8>,
    scalar_type: IntegerType,
    expression: &TerminalAssignedIntegerExpression,
    frame_byte_size: u32,
    stack_depth: u32,
    internal_calls: &mut Option<(&mut Vec<TerminalInternalCallRelocation>, NativeTarget)>,
) -> Result<(), EmissionError> {
    match expression {
        TerminalAssignedIntegerExpression::Call {
            psi_operation,
            source_value,
            callee,
            arguments,
        } => {
            emit_x86_64_call(
                bytes,
                *psi_operation,
                *source_value,
                *callee,
                arguments,
                frame_byte_size,
                stack_depth,
                internal_calls,
            )?;
            emit_x86_64_normalize(bytes, scalar_type);
        }
        TerminalAssignedIntegerExpression::Immediate {
            source_value,
            value,
        } => {
            let bits = integer_bits(*source_value, scalar_type, *value)?;
            bytes.extend_from_slice(&[0x48, 0xb8]); // mov rax, imm64
            bytes.extend_from_slice(&bits.to_le_bytes());
            emit_x86_64_normalize(bytes, scalar_type);
        }
        TerminalAssignedIntegerExpression::Parameter {
            source_value,
            location,
            ..
        } => {
            match location {
                TerminalAssignedScalarLocation::Register(register) => {
                    let register_code = x86_gpr_code(*source_value, *register)?;
                    if matches!(register_code, 0 | 4 | 10 | 11) {
                        return Err(EmissionError::ExpressionScratchRegisterConflict {
                            value: *source_value,
                            register: *register,
                        });
                    }
                    let rex = 0x48 | (((register_code >> 3) & 1) << 2);
                    bytes.extend_from_slice(&[rex, 0x89, 0xc0 | ((register_code & 7) << 3)]);
                    // mov rax, selected argument register
                }
                TerminalAssignedScalarLocation::FrameSpill { byte_offset } => {
                    let displacement = byte_offset.checked_add(stack_depth).ok_or(
                        EmissionError::IncomingStackOffsetNotEncodable {
                            value: *source_value,
                            byte_offset: *byte_offset,
                        },
                    )?;
                    bytes.extend_from_slice(&[0x48, 0x8b]); // mov rax, [rsp + spill]
                    if displacement <= i8::MAX as u32 {
                        bytes.extend_from_slice(&[0x44, 0x24, displacement as u8]);
                    } else {
                        bytes.extend_from_slice(&[0x84, 0x24]);
                        bytes.extend_from_slice(&displacement.to_le_bytes());
                    }
                }
                TerminalAssignedScalarLocation::IncomingStack { byte_offset } => {
                    let displacement = byte_offset
                        .checked_add(8)
                        .and_then(|offset| offset.checked_add(frame_byte_size))
                        .and_then(|offset| offset.checked_add(stack_depth))
                        .ok_or(EmissionError::IncomingStackOffsetNotEncodable {
                            value: *source_value,
                            byte_offset: *byte_offset,
                        })?;
                    bytes.extend_from_slice(&[0x48, 0x8b]); // mov rax, [rsp + displacement]
                    if displacement <= i8::MAX as u32 {
                        bytes.extend_from_slice(&[0x44, 0x24, displacement as u8]);
                    } else {
                        bytes.extend_from_slice(&[0x84, 0x24]);
                        bytes.extend_from_slice(&displacement.to_le_bytes());
                    }
                }
            }
            emit_x86_64_normalize(bytes, scalar_type);
        }
        TerminalAssignedIntegerExpression::BitwiseNot { operand, .. } => {
            emit_x86_64_expression_node(
                bytes,
                scalar_type,
                operand,
                frame_byte_size,
                stack_depth,
                internal_calls,
            )?;
            bytes.extend_from_slice(&[0x48, 0xf7, 0xd0]); // not rax
            emit_x86_64_normalize(bytes, scalar_type);
        }
        TerminalAssignedIntegerExpression::IntegerWiden {
            source_type,
            operand,
            ..
        }
        | TerminalAssignedIntegerExpression::IntegerExactCast {
            source_type,
            operand,
            ..
        } => {
            emit_x86_64_expression_node(
                bytes,
                *source_type,
                operand,
                frame_byte_size,
                stack_depth,
                internal_calls,
            )?;
            emit_x86_64_normalize(bytes, scalar_type);
        }
        TerminalAssignedIntegerExpression::WrappingShiftLeft {
            count_type,
            value,
            count,
            ..
        }
        | TerminalAssignedIntegerExpression::WrappingShiftRight {
            count_type,
            value,
            count,
            ..
        }
        | TerminalAssignedIntegerExpression::ExactShiftLeft {
            count_type,
            value,
            count,
            ..
        }
        | TerminalAssignedIntegerExpression::ExactShiftRight {
            count_type,
            value,
            count,
            ..
        } => {
            emit_x86_64_expression_node(
                bytes,
                scalar_type,
                value,
                frame_byte_size,
                stack_depth,
                internal_calls,
            )?;
            bytes.push(0x50); // push rax
            let nested_depth = stack_depth.checked_add(8).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(value),
                },
            )?;
            emit_x86_64_expression_node(
                bytes,
                *count_type,
                count,
                frame_byte_size,
                nested_depth,
                internal_calls,
            )?;
            bytes.extend_from_slice(&[0x41, 0x5a]); // pop r10
            bytes.extend_from_slice(&[0x48, 0x89, 0xc1]); // mov rcx, rax
            bytes.extend_from_slice(&[0x83, 0xe1, (scalar_type.bits() - 1) as u8]); // and ecx, width - 1
            match expression {
                TerminalAssignedIntegerExpression::WrappingShiftLeft { .. } => {
                    bytes.extend_from_slice(&[0x49, 0xd3, 0xe2]); // shl r10, cl
                }
                TerminalAssignedIntegerExpression::ExactShiftLeft { .. } => {
                    bytes.extend_from_slice(&[0x49, 0xd3, 0xe2]); // shl r10, cl
                }
                TerminalAssignedIntegerExpression::WrappingShiftRight { .. } => {
                    match scalar_type.sign() {
                        IntegerSign::Signed => {
                            bytes.extend_from_slice(&[0x49, 0xd3, 0xfa]); // sar r10, cl
                        }
                        IntegerSign::Unsigned => {
                            bytes.extend_from_slice(&[0x49, 0xd3, 0xea]); // shr r10, cl
                        }
                    }
                }
                TerminalAssignedIntegerExpression::ExactShiftRight { .. } => {
                    match scalar_type.sign() {
                        IntegerSign::Signed => {
                            bytes.extend_from_slice(&[0x49, 0xd3, 0xfa]); // sar r10, cl
                        }
                        IntegerSign::Unsigned => {
                            bytes.extend_from_slice(&[0x49, 0xd3, 0xea]); // shr r10, cl
                        }
                    }
                }
                _ => unreachable!("outer match admits only integer shifts"),
            }
            bytes.extend_from_slice(&[0x4c, 0x89, 0xd0]); // mov rax, r10
            emit_x86_64_normalize(bytes, scalar_type);
        }
        TerminalAssignedIntegerExpression::WrappingAdd { left, right, .. }
        | TerminalAssignedIntegerExpression::BitwiseAnd { left, right, .. }
        | TerminalAssignedIntegerExpression::BitwiseOr { left, right, .. }
        | TerminalAssignedIntegerExpression::BitwiseXor { left, right, .. }
        | TerminalAssignedIntegerExpression::SaturatingAdd { left, right, .. }
        | TerminalAssignedIntegerExpression::WrappingSubtract { left, right, .. }
        | TerminalAssignedIntegerExpression::SaturatingSubtract { left, right, .. }
        | TerminalAssignedIntegerExpression::WrappingMultiply { left, right, .. }
        | TerminalAssignedIntegerExpression::SaturatingMultiply { left, right, .. } => {
            emit_x86_64_expression_node(
                bytes,
                scalar_type,
                left,
                frame_byte_size,
                stack_depth,
                internal_calls,
            )?;
            bytes.push(0x50); // push rax
            let nested_depth = stack_depth.checked_add(8).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_x86_64_expression_node(
                bytes,
                scalar_type,
                right,
                frame_byte_size,
                nested_depth,
                internal_calls,
            )?;
            bytes.extend_from_slice(&[0x41, 0x5a]); // pop r10
            match expression {
                TerminalAssignedIntegerExpression::BitwiseAnd { .. } => {
                    bytes.extend_from_slice(&[0x4c, 0x21, 0xd0]); // and rax, r10
                    emit_x86_64_normalize(bytes, scalar_type);
                }
                TerminalAssignedIntegerExpression::BitwiseOr { .. } => {
                    bytes.extend_from_slice(&[0x4c, 0x09, 0xd0]); // or rax, r10
                    emit_x86_64_normalize(bytes, scalar_type);
                }
                TerminalAssignedIntegerExpression::BitwiseXor { .. } => {
                    bytes.extend_from_slice(&[0x4c, 0x31, 0xd0]); // xor rax, r10
                    emit_x86_64_normalize(bytes, scalar_type);
                }
                TerminalAssignedIntegerExpression::WrappingAdd { .. } => {
                    bytes.extend_from_slice(&[0x4c, 0x01, 0xd0]); // add rax, r10
                    emit_x86_64_normalize(bytes, scalar_type);
                }
                TerminalAssignedIntegerExpression::SaturatingAdd { .. } => {
                    emit_x86_64_saturating_add(bytes, scalar_type);
                }
                TerminalAssignedIntegerExpression::WrappingSubtract { .. } => {
                    bytes.extend_from_slice(&[0x49, 0x29, 0xc2]); // sub r10, rax
                    bytes.extend_from_slice(&[0x4c, 0x89, 0xd0]); // mov rax, r10
                    emit_x86_64_normalize(bytes, scalar_type);
                }
                TerminalAssignedIntegerExpression::SaturatingSubtract { .. } => {
                    emit_x86_64_saturating_subtract(bytes, scalar_type);
                }
                TerminalAssignedIntegerExpression::WrappingMultiply { .. } => {
                    bytes.extend_from_slice(&[0x49, 0x0f, 0xaf, 0xc2]); // imul rax, r10
                    emit_x86_64_normalize(bytes, scalar_type);
                }
                TerminalAssignedIntegerExpression::SaturatingMultiply { .. } => {
                    emit_x86_64_saturating_multiply(bytes, scalar_type);
                }
                _ => unreachable!("outer match admits only binary arithmetic nodes"),
            }
        }
        TerminalAssignedIntegerExpression::ExactDivide { left, right, .. } => {
            emit_x86_64_expression_node(
                bytes,
                scalar_type,
                left,
                frame_byte_size,
                stack_depth,
                internal_calls,
            )?;
            bytes.push(0x50); // push rax
            let nested_depth = stack_depth.checked_add(8).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_x86_64_expression_node(
                bytes,
                scalar_type,
                right,
                frame_byte_size,
                nested_depth,
                internal_calls,
            )?;
            bytes.extend_from_slice(&[0x41, 0x5a]); // pop r10
            bytes.push(0x50); // push divisor
            bytes.extend_from_slice(&[0x4c, 0x89, 0xd0]); // mov rax, r10
            match scalar_type.sign() {
                IntegerSign::Signed => {
                    bytes.extend_from_slice(&[0x48, 0x99]); // cqo
                    bytes.extend_from_slice(&[0x48, 0xf7, 0x3c, 0x24]); // idiv qword [rsp]
                }
                IntegerSign::Unsigned => {
                    bytes.extend_from_slice(&[0x31, 0xd2]); // xor edx, edx
                    bytes.extend_from_slice(&[0x48, 0xf7, 0x34, 0x24]); // div qword [rsp]
                }
            }
            bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]); // add rsp, 8
            emit_x86_64_normalize(bytes, scalar_type);
        }
        TerminalAssignedIntegerExpression::ExactRemainder { left, right, .. } => {
            emit_x86_64_expression_node(
                bytes,
                scalar_type,
                left,
                frame_byte_size,
                stack_depth,
                internal_calls,
            )?;
            bytes.push(0x50);
            let nested_depth = stack_depth.checked_add(8).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_x86_64_expression_node(
                bytes,
                scalar_type,
                right,
                frame_byte_size,
                nested_depth,
                internal_calls,
            )?;
            bytes.extend_from_slice(&[0x41, 0x5a]); // pop r10
            bytes.push(0x50); // push divisor
            bytes.extend_from_slice(&[0x4c, 0x89, 0xd0]); // mov rax, r10
            match scalar_type.sign() {
                IntegerSign::Signed => {
                    bytes.extend_from_slice(&[0x48, 0x99]);
                    bytes.extend_from_slice(&[0x48, 0xf7, 0x3c, 0x24]);
                }
                IntegerSign::Unsigned => {
                    bytes.extend_from_slice(&[0x31, 0xd2]);
                    bytes.extend_from_slice(&[0x48, 0xf7, 0x34, 0x24]);
                }
            }
            bytes.extend_from_slice(&[0x48, 0x89, 0xd0]); // mov rax, rdx
            bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]);
            emit_x86_64_normalize(bytes, scalar_type);
        }
        TerminalAssignedIntegerExpression::WrappingDivide { left, right, .. } => {
            emit_x86_64_expression_node(
                bytes,
                scalar_type,
                left,
                frame_byte_size,
                stack_depth,
                internal_calls,
            )?;
            bytes.push(0x50);
            let nested_depth = stack_depth.checked_add(8).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_x86_64_expression_node(
                bytes,
                scalar_type,
                right,
                frame_byte_size,
                nested_depth,
                internal_calls,
            )?;
            bytes.extend_from_slice(&[0x41, 0x5a]); // pop r10
            bytes.push(0x50); // push divisor
            bytes.extend_from_slice(&[0x4c, 0x89, 0xd0]); // mov rax, r10
            match scalar_type.sign() {
                IntegerSign::Unsigned => {
                    bytes.extend_from_slice(&[0x31, 0xd2]);
                    bytes.extend_from_slice(&[0x48, 0xf7, 0x34, 0x24]);
                    bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]);
                    emit_x86_64_normalize(bytes, scalar_type);
                }
                IntegerSign::Signed => {
                    let mut negative_one = vec![0x48, 0xf7, 0xd8]; // neg rax
                    negative_one.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]);
                    emit_x86_64_normalize(&mut negative_one, scalar_type);

                    let mut ordinary = vec![0x48, 0x99]; // cqo
                    ordinary.extend_from_slice(&[0x48, 0xf7, 0x3c, 0x24]);
                    ordinary.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]);
                    emit_x86_64_normalize(&mut ordinary, scalar_type);

                    bytes.extend_from_slice(&[0x48, 0x83, 0x3c, 0x24, 0xff]); // cmp [rsp], -1
                    bytes.extend_from_slice(&[0x0f, 0x85]); // jne ordinary
                    let ordinary_offset = i32::try_from(negative_one.len() + 5)
                        .expect("wrapping-divide branch is small");
                    bytes.extend_from_slice(&ordinary_offset.to_le_bytes());
                    bytes.extend_from_slice(&negative_one);
                    bytes.push(0xe9); // jmp done
                    let done_offset =
                        i32::try_from(ordinary.len()).expect("wrapping-divide branch is small");
                    bytes.extend_from_slice(&done_offset.to_le_bytes());
                    bytes.extend_from_slice(&ordinary);
                }
            }
        }
        TerminalAssignedIntegerExpression::WrappingRemainder { left, right, .. }
        | TerminalAssignedIntegerExpression::SaturatingRemainder { left, right, .. } => {
            emit_x86_64_expression_node(
                bytes,
                scalar_type,
                left,
                frame_byte_size,
                stack_depth,
                internal_calls,
            )?;
            bytes.push(0x50);
            let nested_depth = stack_depth.checked_add(8).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_x86_64_expression_node(
                bytes,
                scalar_type,
                right,
                frame_byte_size,
                nested_depth,
                internal_calls,
            )?;
            bytes.extend_from_slice(&[0x41, 0x5a]); // pop r10
            bytes.push(0x50); // push divisor
            bytes.extend_from_slice(&[0x4c, 0x89, 0xd0]); // mov rax, r10
            match scalar_type.sign() {
                IntegerSign::Unsigned => {
                    bytes.extend_from_slice(&[0x31, 0xd2]);
                    bytes.extend_from_slice(&[0x48, 0xf7, 0x34, 0x24]);
                    bytes.extend_from_slice(&[0x48, 0x89, 0xd0]); // mov rax, rdx
                    bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]);
                    emit_x86_64_normalize(bytes, scalar_type);
                }
                IntegerSign::Signed => {
                    let mut negative_one = vec![0x31, 0xc0]; // xor eax, eax
                    negative_one.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]);
                    emit_x86_64_normalize(&mut negative_one, scalar_type);

                    let mut ordinary = vec![0x48, 0x99]; // cqo
                    ordinary.extend_from_slice(&[0x48, 0xf7, 0x3c, 0x24]);
                    ordinary.extend_from_slice(&[0x48, 0x89, 0xd0]); // mov rax, rdx
                    ordinary.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]);
                    emit_x86_64_normalize(&mut ordinary, scalar_type);

                    bytes.extend_from_slice(&[0x48, 0x83, 0x3c, 0x24, 0xff]); // cmp [rsp], -1
                    bytes.extend_from_slice(&[0x0f, 0x85]); // jne ordinary
                    let ordinary_offset = i32::try_from(negative_one.len() + 5)
                        .expect("wrapping-remainder branch is small");
                    bytes.extend_from_slice(&ordinary_offset.to_le_bytes());
                    bytes.extend_from_slice(&negative_one);
                    bytes.push(0xe9); // jmp done
                    let done_offset =
                        i32::try_from(ordinary.len()).expect("wrapping-remainder branch is small");
                    bytes.extend_from_slice(&done_offset.to_le_bytes());
                    bytes.extend_from_slice(&ordinary);
                }
            }
        }
        TerminalAssignedIntegerExpression::SaturatingDivide { left, right, .. } => {
            emit_x86_64_expression_node(
                bytes,
                scalar_type,
                left,
                frame_byte_size,
                stack_depth,
                internal_calls,
            )?;
            bytes.push(0x50);
            let nested_depth = stack_depth.checked_add(8).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_x86_64_expression_node(
                bytes,
                scalar_type,
                right,
                frame_byte_size,
                nested_depth,
                internal_calls,
            )?;
            bytes.extend_from_slice(&[0x41, 0x5a]); // pop r10
            bytes.push(0x50); // push divisor
            bytes.extend_from_slice(&[0x4c, 0x89, 0xd0]); // mov rax, r10
            match scalar_type.sign() {
                IntegerSign::Unsigned => {
                    bytes.extend_from_slice(&[0x31, 0xd2]);
                    bytes.extend_from_slice(&[0x48, 0xf7, 0x34, 0x24]);
                    bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]);
                    emit_x86_64_normalize(bytes, scalar_type);
                }
                IntegerSign::Signed => {
                    let (_, maximum) = native_integer_bounds(scalar_type);
                    let mut negative_one = vec![0x48, 0xf7, 0xd8]; // neg rax
                    emit_x86_64_mov_r10(&mut negative_one, maximum);
                    if scalar_type.bits() == 64 {
                        negative_one.extend_from_slice(&[0x49, 0x0f, 0x40, 0xc2]); // cmovo rax, r10
                    } else {
                        negative_one.extend_from_slice(&[0x4c, 0x39, 0xd0]); // cmp rax, r10
                        negative_one.extend_from_slice(&[0x49, 0x0f, 0x4f, 0xc2]); // cmovg rax, r10
                    }
                    negative_one.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]);
                    emit_x86_64_normalize(&mut negative_one, scalar_type);

                    let mut ordinary = vec![0x48, 0x99]; // cqo
                    ordinary.extend_from_slice(&[0x48, 0xf7, 0x3c, 0x24]);
                    ordinary.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]);
                    emit_x86_64_normalize(&mut ordinary, scalar_type);

                    bytes.extend_from_slice(&[0x48, 0x83, 0x3c, 0x24, 0xff]); // cmp [rsp], -1
                    bytes.extend_from_slice(&[0x0f, 0x85]); // jne ordinary
                    let ordinary_offset = i32::try_from(negative_one.len() + 5)
                        .expect("saturating-divide branch is small");
                    bytes.extend_from_slice(&ordinary_offset.to_le_bytes());
                    bytes.extend_from_slice(&negative_one);
                    bytes.push(0xe9); // jmp done
                    let done_offset =
                        i32::try_from(ordinary.len()).expect("saturating-divide branch is small");
                    bytes.extend_from_slice(&done_offset.to_le_bytes());
                    bytes.extend_from_slice(&ordinary);
                }
            }
        }
    }
    Ok(())
}

fn emit_x86_64_normalize(bytes: &mut Vec<u8>, scalar_type: IntegerType) {
    match (scalar_type.sign(), scalar_type.bits()) {
        (_, 64) => {}
        (IntegerSign::Unsigned, 8) => bytes.extend_from_slice(&[0x25, 0xff, 0, 0, 0]),
        (IntegerSign::Unsigned, 16) => bytes.extend_from_slice(&[0x25, 0xff, 0xff, 0, 0]),
        (IntegerSign::Unsigned, 32) => bytes.extend_from_slice(&[0x89, 0xc0]),
        (IntegerSign::Signed, 8) => bytes.extend_from_slice(&[0x48, 0x0f, 0xbe, 0xc0]),
        (IntegerSign::Signed, 16) => bytes.extend_from_slice(&[0x48, 0x0f, 0xbf, 0xc0]),
        (IntegerSign::Signed, 32) => bytes.extend_from_slice(&[0x48, 0x63, 0xc0]),
        _ => unreachable!("native integer width was checked before expression emission"),
    }
}

fn emit_x86_64_saturating_add(bytes: &mut Vec<u8>, scalar_type: IntegerType) {
    let (minimum, maximum) = native_integer_bounds(scalar_type);
    match (scalar_type.sign(), scalar_type.bits()) {
        (IntegerSign::Unsigned, 64) => {
            bytes.extend_from_slice(&[0x4c, 0x01, 0xd0]); // add rax, r10
            bytes.extend_from_slice(&[0x4d, 0x19, 0xd2]); // sbb r10, r10
            bytes.extend_from_slice(&[0x4c, 0x09, 0xd0]); // or rax, r10
        }
        (IntegerSign::Unsigned, _) => {
            bytes.extend_from_slice(&[0x4c, 0x01, 0xd0]); // add rax, r10
            emit_x86_64_mov_r10(bytes, maximum);
            bytes.extend_from_slice(&[0x4c, 0x39, 0xd0]); // cmp rax, r10
            bytes.extend_from_slice(&[0x49, 0x0f, 0x47, 0xc2]); // cmova rax, r10
        }
        (IntegerSign::Signed, 64) => {
            bytes.extend_from_slice(&[0x4d, 0x89, 0xd3]); // mov r11, r10
            bytes.extend_from_slice(&[0x49, 0xc1, 0xfb, 0x3f]); // sar r11, 63
            bytes.extend_from_slice(&[0x49, 0xf7, 0xd3]); // not r11
            bytes.extend_from_slice(&[0x49, 0x0f, 0xba, 0xfb, 0x3f]); // btc r11, 63
            bytes.extend_from_slice(&[0x4c, 0x01, 0xd0]); // add rax, r10
            bytes.extend_from_slice(&[0x49, 0x0f, 0x40, 0xc3]); // cmovo rax, r11
        }
        (IntegerSign::Signed, _) => {
            bytes.extend_from_slice(&[0x4c, 0x01, 0xd0]); // add rax, r10
            emit_x86_64_mov_r10(bytes, maximum);
            bytes.extend_from_slice(&[0x4c, 0x39, 0xd0]); // cmp rax, r10
            bytes.extend_from_slice(&[0x49, 0x0f, 0x4f, 0xc2]); // cmovg rax, r10
            emit_x86_64_mov_r10(bytes, minimum);
            bytes.extend_from_slice(&[0x4c, 0x39, 0xd0]); // cmp rax, r10
            bytes.extend_from_slice(&[0x49, 0x0f, 0x4c, 0xc2]); // cmovl rax, r10
        }
    }
}

fn emit_x86_64_saturating_subtract(bytes: &mut Vec<u8>, scalar_type: IntegerType) {
    let (minimum, maximum) = native_integer_bounds(scalar_type);
    match (scalar_type.sign(), scalar_type.bits()) {
        (IntegerSign::Unsigned, _) => {
            bytes.extend_from_slice(&[0x49, 0x29, 0xc2]); // sub r10, rax
            bytes.extend_from_slice(&[0xb8, 0, 0, 0, 0]); // mov eax, 0 (flags unchanged)
            bytes.extend_from_slice(&[0x49, 0x0f, 0x43, 0xc2]); // cmovae rax, r10
        }
        (IntegerSign::Signed, 64) => {
            bytes.extend_from_slice(&[0x4d, 0x89, 0xd3]); // mov r11, r10
            bytes.extend_from_slice(&[0x49, 0xc1, 0xfb, 0x3f]); // sar r11, 63
            bytes.extend_from_slice(&[0x49, 0xf7, 0xd3]); // not r11
            bytes.extend_from_slice(&[0x49, 0x0f, 0xba, 0xfb, 0x3f]); // btc r11, 63
            bytes.extend_from_slice(&[0x49, 0x29, 0xc2]); // sub r10, rax
            bytes.extend_from_slice(&[0x4c, 0x89, 0xd0]); // mov rax, r10
            bytes.extend_from_slice(&[0x49, 0x0f, 0x40, 0xc3]); // cmovo rax, r11
        }
        (IntegerSign::Signed, _) => {
            bytes.extend_from_slice(&[0x49, 0x29, 0xc2]); // sub r10, rax
            bytes.extend_from_slice(&[0x4c, 0x89, 0xd0]); // mov rax, r10
            emit_x86_64_mov_r10(bytes, maximum);
            bytes.extend_from_slice(&[0x4c, 0x39, 0xd0]); // cmp rax, r10
            bytes.extend_from_slice(&[0x49, 0x0f, 0x4f, 0xc2]); // cmovg rax, r10
            emit_x86_64_mov_r10(bytes, minimum);
            bytes.extend_from_slice(&[0x4c, 0x39, 0xd0]); // cmp rax, r10
            bytes.extend_from_slice(&[0x49, 0x0f, 0x4c, 0xc2]); // cmovl rax, r10
        }
    }
}

fn emit_x86_64_saturating_multiply(bytes: &mut Vec<u8>, scalar_type: IntegerType) {
    let (minimum, maximum) = native_integer_bounds(scalar_type);
    match (scalar_type.sign(), scalar_type.bits()) {
        (IntegerSign::Unsigned, 64) => {
            bytes.push(0x52); // push rdx
            bytes.extend_from_slice(&[0x49, 0xf7, 0xe2]); // mul r10 -> rdx:rax
            bytes.extend_from_slice(&[0x48, 0x85, 0xd2]); // test rdx, rdx
            bytes.extend_from_slice(&[0x49, 0xbb]); // mov r11, u64::MAX
            bytes.extend_from_slice(&maximum.to_le_bytes());
            bytes.extend_from_slice(&[0x49, 0x0f, 0x45, 0xc3]); // cmovne rax, r11
            bytes.push(0x5a); // pop rdx
        }
        (IntegerSign::Unsigned, _) => {
            bytes.extend_from_slice(&[0x49, 0x0f, 0xaf, 0xc2]); // imul rax, r10
            emit_x86_64_mov_r10(bytes, maximum);
            bytes.extend_from_slice(&[0x4c, 0x39, 0xd0]); // cmp rax, r10
            bytes.extend_from_slice(&[0x49, 0x0f, 0x47, 0xc2]); // cmova rax, r10
        }
        (IntegerSign::Signed, 64) => {
            bytes.push(0x52); // push rdx
            bytes.extend_from_slice(&[0x41, 0x52]); // push r10
            bytes.extend_from_slice(&[0x49, 0xbb]); // mov r11, maximum
            bytes.extend_from_slice(&maximum.to_le_bytes());
            bytes.extend_from_slice(&[0x48, 0x89, 0xc2]); // mov rdx, rax
            bytes.extend_from_slice(&[0x4c, 0x31, 0xd2]); // xor rdx, r10
            emit_x86_64_mov_r10(bytes, minimum);
            bytes.extend_from_slice(&[0x4d, 0x0f, 0x48, 0xda]); // cmovs r11, r10
            bytes.extend_from_slice(&[0x41, 0x5a]); // pop r10
            bytes.extend_from_slice(&[0x49, 0xf7, 0xea]); // imul r10 -> rdx:rax
            bytes.extend_from_slice(&[0x49, 0x0f, 0x40, 0xc3]); // cmovo rax, r11
            bytes.push(0x5a); // pop rdx
        }
        (IntegerSign::Signed, _) => {
            bytes.extend_from_slice(&[0x49, 0x0f, 0xaf, 0xc2]); // imul rax, r10
            emit_x86_64_mov_r10(bytes, maximum);
            bytes.extend_from_slice(&[0x4c, 0x39, 0xd0]); // cmp rax, r10
            bytes.extend_from_slice(&[0x49, 0x0f, 0x4f, 0xc2]); // cmovg rax, r10
            emit_x86_64_mov_r10(bytes, minimum);
            bytes.extend_from_slice(&[0x4c, 0x39, 0xd0]); // cmp rax, r10
            bytes.extend_from_slice(&[0x49, 0x0f, 0x4c, 0xc2]); // cmovl rax, r10
        }
    }
    emit_x86_64_normalize(bytes, scalar_type);
}

fn emit_x86_64_mov_r10(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&[0x49, 0xba]);
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn emit_aarch64_boolean_expression(
    frame: &TerminalExpressionFrame,
    expression: &TerminalAssignedBooleanExpression,
    internal_calls: Option<(&mut Vec<TerminalInternalCallRelocation>, NativeTarget)>,
) -> Result<Vec<u8>, EmissionError> {
    let mut bytes = emit_aarch64_boolean_expression_value(frame, expression, internal_calls)?;
    bytes.extend_from_slice(&0xd65f_03c0_u32.to_le_bytes()); // ret x30
    Ok(bytes)
}

fn emit_aarch64_boolean_expression_value(
    frame: &TerminalExpressionFrame,
    expression: &TerminalAssignedBooleanExpression,
    mut internal_calls: Option<(&mut Vec<TerminalInternalCallRelocation>, NativeTarget)>,
) -> Result<Vec<u8>, EmissionError> {
    if frame.byte_size == 0 && !frame.register_spills.is_empty() {
        return Err(EmissionError::AssignedFrameSizeMismatch);
    }
    let mut instructions = Vec::new();
    if frame.byte_size != 0 {
        emit_aarch64_adjust_sp(&mut instructions, frame.byte_size, false)?;
        for spill in &frame.register_spills {
            instructions.push(aarch64_stack_access(
                0xf900_0000,
                aarch64_spill_register(spill.source_value, spill.register)?,
                spill.source_value,
                spill.byte_offset,
            )?);
        }
    }
    emit_aarch64_boolean_expression_node(
        &mut instructions,
        expression,
        frame,
        0,
        &mut internal_calls,
    )?;
    if frame.byte_size != 0 {
        emit_aarch64_adjust_sp(&mut instructions, frame.byte_size, true)?;
    }
    Ok(instructions
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect())
}

fn emit_aarch64_boolean_condition_value(
    frame: &TerminalExpressionFrame,
    expression: &TerminalAssignedBooleanExpression,
    mut internal_calls: Option<(&mut Vec<TerminalInternalCallRelocation>, NativeTarget)>,
) -> Result<Vec<u8>, EmissionError> {
    if frame.byte_size == 0 && !frame.register_spills.is_empty() {
        return Err(EmissionError::AssignedFrameSizeMismatch);
    }
    let mut instructions = Vec::new();
    if frame.byte_size != 0 {
        emit_aarch64_adjust_sp(&mut instructions, frame.byte_size, false)?;
        for spill in &frame.register_spills {
            instructions.push(aarch64_stack_access(
                0xf900_0000,
                aarch64_spill_register(spill.source_value, spill.register)?,
                spill.source_value,
                spill.byte_offset,
            )?);
        }
    }
    emit_aarch64_boolean_expression_node(
        &mut instructions,
        expression,
        frame,
        0,
        &mut internal_calls,
    )?;
    instructions.push(0x7100_001f); // cmp w0, #0
    for spill in &frame.register_spills {
        instructions.push(aarch64_stack_access(
            0xf940_0000,
            aarch64_spill_register(spill.source_value, spill.register)?,
            spill.source_value,
            spill.byte_offset,
        )?);
    }
    if frame.byte_size != 0 {
        emit_aarch64_adjust_sp(&mut instructions, frame.byte_size, true)?;
    }
    Ok(instructions
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect())
}

fn emit_aarch64_boolean_expression_node(
    instructions: &mut Vec<u32>,
    expression: &TerminalAssignedBooleanExpression,
    frame: &TerminalExpressionFrame,
    stack_depth: u32,
    internal_calls: &mut Option<(&mut Vec<TerminalInternalCallRelocation>, NativeTarget)>,
) -> Result<(), EmissionError> {
    match expression {
        TerminalAssignedBooleanExpression::Call {
            psi_operation,
            source_value,
            callee,
            arguments,
        } => {
            emit_aarch64_call(
                instructions,
                *psi_operation,
                *source_value,
                *callee,
                arguments,
                frame,
                stack_depth,
                internal_calls,
            )?;
            instructions.push(0x1200_0000); // and w0, w0, #1
        }
        TerminalAssignedBooleanExpression::Immediate { value, .. } => {
            emit_aarch64_mov_immediate(instructions, 0, u64::from(*value));
        }
        TerminalAssignedBooleanExpression::Parameter {
            source_value,
            location,
            ..
        } => {
            let byte_offset = match location {
                TerminalAssignedScalarLocation::FrameSpill { byte_offset } => {
                    stack_depth.checked_add(*byte_offset)
                }
                TerminalAssignedScalarLocation::IncomingStack { byte_offset } => stack_depth
                    .checked_add(frame.byte_size)
                    .and_then(|offset| offset.checked_add(*byte_offset)),
                TerminalAssignedScalarLocation::Register(_) => {
                    return Err(EmissionError::AssignedFrameArchitectureMismatch(
                        Architecture::Aarch64,
                    ));
                }
            }
            .ok_or(EmissionError::IncomingStackOffsetNotEncodable {
                value: *source_value,
                byte_offset: match location {
                    TerminalAssignedScalarLocation::Register(_)
                    | TerminalAssignedScalarLocation::FrameSpill { .. } => 0,
                    TerminalAssignedScalarLocation::IncomingStack { byte_offset } => *byte_offset,
                },
            })?;
            instructions.push(aarch64_stack_access(
                0xf940_0000,
                0,
                *source_value,
                byte_offset,
            )?);
            instructions.push(0x1200_0000); // and w0, w0, #1
        }
        TerminalAssignedBooleanExpression::Not { operand, .. } => {
            emit_aarch64_boolean_expression_node(
                instructions,
                operand,
                frame,
                stack_depth,
                internal_calls,
            )?;
            instructions.push(0x5200_0000); // eor w0, w0, #1
        }
        TerminalAssignedBooleanExpression::Equal { left, right, .. } => {
            emit_aarch64_boolean_expression_node(
                instructions,
                left,
                frame,
                stack_depth,
                internal_calls,
            )?;
            emit_aarch64_adjust_sp(instructions, 16, false)?;
            instructions.push(aarch64_stack_access(
                0xf900_0000,
                0,
                boolean_expression_source(left),
                0,
            )?);
            let nested_depth = stack_depth.checked_add(16).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: boolean_expression_source(left),
                },
            )?;
            emit_aarch64_boolean_expression_node(
                instructions,
                right,
                frame,
                nested_depth,
                internal_calls,
            )?;
            instructions.push(aarch64_stack_access(
                0xf940_0000,
                9,
                boolean_expression_source(left),
                0,
            )?);
            emit_aarch64_adjust_sp(instructions, 16, true)?;
            instructions.push(0x6b00_013f); // cmp w9, w0
            instructions.push(0x1a9f_17e0); // cset w0, eq
        }
        TerminalAssignedBooleanExpression::IntegerEqual {
            scalar_type,
            left,
            right,
            ..
        } => {
            emit_aarch64_expression_node(
                instructions,
                *scalar_type,
                left,
                frame,
                stack_depth,
                internal_calls,
            )?;
            emit_aarch64_adjust_sp(instructions, 16, false)?;
            instructions.push(aarch64_stack_access(
                0xf900_0000,
                0,
                expression_source(left),
                0,
            )?);
            let nested_depth = stack_depth.checked_add(16).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_aarch64_expression_node(
                instructions,
                *scalar_type,
                right,
                frame,
                nested_depth,
                internal_calls,
            )?;
            instructions.push(aarch64_stack_access(
                0xf940_0000,
                9,
                expression_source(left),
                0,
            )?);
            emit_aarch64_adjust_sp(instructions, 16, true)?;
            instructions.push(0xeb00_013f); // cmp x9, x0
            instructions.push(0x1a9f_17e0); // cset w0, eq
        }
        TerminalAssignedBooleanExpression::IntegerLessThan {
            scalar_type,
            left,
            right,
            ..
        }
        | TerminalAssignedBooleanExpression::IntegerLessOrEqual {
            scalar_type,
            left,
            right,
            ..
        } => {
            emit_aarch64_expression_node(
                instructions,
                *scalar_type,
                left,
                frame,
                stack_depth,
                internal_calls,
            )?;
            emit_aarch64_adjust_sp(instructions, 16, false)?;
            instructions.push(aarch64_stack_access(
                0xf900_0000,
                0,
                expression_source(left),
                0,
            )?);
            let nested_depth = stack_depth.checked_add(16).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_aarch64_expression_node(
                instructions,
                *scalar_type,
                right,
                frame,
                nested_depth,
                internal_calls,
            )?;
            instructions.push(aarch64_stack_access(
                0xf940_0000,
                9,
                expression_source(left),
                0,
            )?);
            emit_aarch64_adjust_sp(instructions, 16, true)?;
            instructions.push(0xeb00_013f); // cmp x9, x0
            let inclusive = matches!(
                expression,
                TerminalAssignedBooleanExpression::IntegerLessOrEqual { .. }
            );
            instructions.push(match (scalar_type.sign(), inclusive) {
                (IntegerSign::Signed, false) => 0x1a9f_a7e0, // cset w0, lt
                (IntegerSign::Unsigned, false) => 0x1a9f_27e0, // cset w0, lo
                (IntegerSign::Signed, true) => 0x1a9f_c7e0,  // cset w0, le
                (IntegerSign::Unsigned, true) => 0x1a9f_87e0, // cset w0, ls
            });
        }
    }
    Ok(())
}

fn emit_aarch64_integer_expression(
    scalar_type: IntegerType,
    frame: &TerminalExpressionFrame,
    expression: &TerminalAssignedIntegerExpression,
    mut internal_calls: Option<(&mut Vec<TerminalInternalCallRelocation>, NativeTarget)>,
) -> Result<Vec<u8>, EmissionError> {
    let mut instructions = Vec::new();
    if frame.byte_size != 0 {
        emit_aarch64_adjust_sp(&mut instructions, frame.byte_size, false)?;
        for spill in &frame.register_spills {
            instructions.push(aarch64_stack_access(
                0xf900_0000,
                aarch64_spill_register(spill.source_value, spill.register)?,
                spill.source_value,
                spill.byte_offset,
            )?); // str xN, [sp, #spill]
        }
    }
    emit_aarch64_expression_node(
        &mut instructions,
        scalar_type,
        expression,
        frame,
        0,
        &mut internal_calls,
    )?;
    if frame.byte_size != 0 {
        emit_aarch64_adjust_sp(&mut instructions, frame.byte_size, true)?;
    }
    instructions.push(0xd65f_03c0); // ret x30
    Ok(instructions
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect())
}

fn aarch64_spill_register(
    source_value: ValueId,
    register: MachineRegister,
) -> Result<u8, EmissionError> {
    match register {
        MachineRegister::Aarch64X(register) if register < 31 => Ok(register),
        _ => Err(EmissionError::ParameterRegisterArchitectureMismatch {
            value: source_value,
            register,
            architecture: Architecture::Aarch64,
        }),
    }
}

fn emit_aarch64_expression_node(
    instructions: &mut Vec<u32>,
    scalar_type: IntegerType,
    expression: &TerminalAssignedIntegerExpression,
    frame: &TerminalExpressionFrame,
    stack_depth: u32,
    internal_calls: &mut Option<(&mut Vec<TerminalInternalCallRelocation>, NativeTarget)>,
) -> Result<(), EmissionError> {
    match expression {
        TerminalAssignedIntegerExpression::Call {
            psi_operation,
            source_value,
            callee,
            arguments,
        } => {
            emit_aarch64_call(
                instructions,
                *psi_operation,
                *source_value,
                *callee,
                arguments,
                frame,
                stack_depth,
                internal_calls,
            )?;
            emit_aarch64_normalize(instructions, scalar_type);
        }
        TerminalAssignedIntegerExpression::Immediate {
            source_value,
            value,
        } => {
            let bits = integer_bits(*source_value, scalar_type, *value)?;
            emit_aarch64_mov_immediate(instructions, 0, bits);
            emit_aarch64_normalize(instructions, scalar_type);
        }
        TerminalAssignedIntegerExpression::Parameter {
            source_value,
            parameter_index: _,
            location,
        } => {
            let byte_offset = match location {
                TerminalAssignedScalarLocation::FrameSpill { byte_offset } => {
                    stack_depth.checked_add(*byte_offset)
                }
                TerminalAssignedScalarLocation::IncomingStack { byte_offset } => stack_depth
                    .checked_add(frame.byte_size)
                    .and_then(|offset| offset.checked_add(*byte_offset)),
                TerminalAssignedScalarLocation::Register(_) => {
                    return Err(EmissionError::AssignedFrameArchitectureMismatch(
                        Architecture::Aarch64,
                    ));
                }
            }
            .ok_or(EmissionError::IncomingStackOffsetNotEncodable {
                value: *source_value,
                byte_offset: match location {
                    TerminalAssignedScalarLocation::Register(_)
                    | TerminalAssignedScalarLocation::FrameSpill { .. } => 0,
                    TerminalAssignedScalarLocation::IncomingStack { byte_offset } => *byte_offset,
                },
            })?;
            instructions.push(aarch64_stack_access(
                0xf940_0000,
                0,
                *source_value,
                byte_offset,
            )?); // ldr x0, [sp, #value]
            emit_aarch64_normalize(instructions, scalar_type);
        }
        TerminalAssignedIntegerExpression::BitwiseNot { operand, .. } => {
            emit_aarch64_expression_node(
                instructions,
                scalar_type,
                operand,
                frame,
                stack_depth,
                internal_calls,
            )?;
            instructions.push(0xaa20_03e0); // mvn x0, x0
            emit_aarch64_normalize(instructions, scalar_type);
        }
        TerminalAssignedIntegerExpression::IntegerWiden {
            source_type,
            operand,
            ..
        }
        | TerminalAssignedIntegerExpression::IntegerExactCast {
            source_type,
            operand,
            ..
        } => {
            emit_aarch64_expression_node(
                instructions,
                *source_type,
                operand,
                frame,
                stack_depth,
                internal_calls,
            )?;
            emit_aarch64_normalize(instructions, scalar_type);
        }
        TerminalAssignedIntegerExpression::WrappingShiftLeft {
            count_type,
            value,
            count,
            ..
        }
        | TerminalAssignedIntegerExpression::WrappingShiftRight {
            count_type,
            value,
            count,
            ..
        }
        | TerminalAssignedIntegerExpression::ExactShiftLeft {
            count_type,
            value,
            count,
            ..
        }
        | TerminalAssignedIntegerExpression::ExactShiftRight {
            count_type,
            value,
            count,
            ..
        } => {
            emit_aarch64_expression_node(
                instructions,
                scalar_type,
                value,
                frame,
                stack_depth,
                internal_calls,
            )?;
            emit_aarch64_adjust_sp(instructions, 16, false)?;
            instructions.push(aarch64_stack_access(
                0xf900_0000,
                0,
                expression_source(value),
                0,
            )?); // str x0, [sp]
            let nested_depth = stack_depth.checked_add(16).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(value),
                },
            )?;
            emit_aarch64_expression_node(
                instructions,
                *count_type,
                count,
                frame,
                nested_depth,
                internal_calls,
            )?;
            instructions.push(aarch64_stack_access(
                0xf940_0000,
                9,
                expression_source(value),
                0,
            )?); // ldr x9, [sp]
            emit_aarch64_adjust_sp(instructions, 16, true)?;
            let count_mask_bits = scalar_type.bits().trailing_zeros();
            instructions.push(0x9240_0000 | ((count_mask_bits - 1) << 10)); // and x0, x0, #width-1
            match expression {
                TerminalAssignedIntegerExpression::WrappingShiftLeft { .. } => {
                    instructions.push(0x9ac0_2120); // lslv x0, x9, x0
                }
                TerminalAssignedIntegerExpression::ExactShiftLeft { .. } => {
                    instructions.push(0x9ac0_2120); // lslv x0, x9, x0
                }
                TerminalAssignedIntegerExpression::WrappingShiftRight { .. } => {
                    instructions.push(match scalar_type.sign() {
                        IntegerSign::Signed => 0x9ac0_2920,   // asrv x0, x9, x0
                        IntegerSign::Unsigned => 0x9ac0_2520, // lsrv x0, x9, x0
                    });
                }
                TerminalAssignedIntegerExpression::ExactShiftRight { .. } => {
                    instructions.push(match scalar_type.sign() {
                        IntegerSign::Signed => 0x9ac0_2920,   // asrv x0, x9, x0
                        IntegerSign::Unsigned => 0x9ac0_2520, // lsrv x0, x9, x0
                    });
                }
                _ => unreachable!("outer match admits only integer shifts"),
            }
            emit_aarch64_normalize(instructions, scalar_type);
        }
        TerminalAssignedIntegerExpression::WrappingAdd { left, right, .. }
        | TerminalAssignedIntegerExpression::BitwiseAnd { left, right, .. }
        | TerminalAssignedIntegerExpression::BitwiseOr { left, right, .. }
        | TerminalAssignedIntegerExpression::BitwiseXor { left, right, .. }
        | TerminalAssignedIntegerExpression::SaturatingAdd { left, right, .. }
        | TerminalAssignedIntegerExpression::WrappingSubtract { left, right, .. }
        | TerminalAssignedIntegerExpression::SaturatingSubtract { left, right, .. }
        | TerminalAssignedIntegerExpression::WrappingMultiply { left, right, .. }
        | TerminalAssignedIntegerExpression::SaturatingMultiply { left, right, .. } => {
            emit_aarch64_expression_node(
                instructions,
                scalar_type,
                left,
                frame,
                stack_depth,
                internal_calls,
            )?;
            emit_aarch64_adjust_sp(instructions, 16, false)?;
            instructions.push(aarch64_stack_access(
                0xf900_0000,
                0,
                expression_source(left),
                0,
            )?); // str x0, [sp]
            let nested_depth = stack_depth.checked_add(16).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_aarch64_expression_node(
                instructions,
                scalar_type,
                right,
                frame,
                nested_depth,
                internal_calls,
            )?;
            instructions.push(aarch64_stack_access(
                0xf940_0000,
                9,
                expression_source(left),
                0,
            )?); // ldr x9, [sp]
            emit_aarch64_adjust_sp(instructions, 16, true)?;
            match expression {
                TerminalAssignedIntegerExpression::BitwiseAnd { .. } => {
                    instructions.push(0x8a00_0120); // and x0, x9, x0
                    emit_aarch64_normalize(instructions, scalar_type);
                }
                TerminalAssignedIntegerExpression::BitwiseOr { .. } => {
                    instructions.push(0xaa00_0120); // orr x0, x9, x0
                    emit_aarch64_normalize(instructions, scalar_type);
                }
                TerminalAssignedIntegerExpression::BitwiseXor { .. } => {
                    instructions.push(0xca00_0120); // eor x0, x9, x0
                    emit_aarch64_normalize(instructions, scalar_type);
                }
                TerminalAssignedIntegerExpression::WrappingAdd { .. } => {
                    instructions.push(0x8b00_0120); // add x0, x9, x0
                    emit_aarch64_normalize(instructions, scalar_type);
                }
                TerminalAssignedIntegerExpression::SaturatingAdd { .. } => {
                    emit_aarch64_saturating_add(instructions, scalar_type);
                }
                TerminalAssignedIntegerExpression::WrappingSubtract { .. } => {
                    instructions.push(0xcb00_0120); // sub x0, x9, x0
                    emit_aarch64_normalize(instructions, scalar_type);
                }
                TerminalAssignedIntegerExpression::SaturatingSubtract { .. } => {
                    emit_aarch64_saturating_subtract(instructions, scalar_type);
                }
                TerminalAssignedIntegerExpression::WrappingMultiply { .. } => {
                    instructions.push(0x9b00_7d20); // mul x0, x9, x0
                    emit_aarch64_normalize(instructions, scalar_type);
                }
                TerminalAssignedIntegerExpression::SaturatingMultiply { .. } => {
                    emit_aarch64_saturating_multiply(instructions, scalar_type);
                }
                _ => unreachable!("outer match admits only binary arithmetic nodes"),
            }
        }
        TerminalAssignedIntegerExpression::ExactDivide { left, right, .. } => {
            emit_aarch64_expression_node(
                instructions,
                scalar_type,
                left,
                frame,
                stack_depth,
                internal_calls,
            )?;
            emit_aarch64_adjust_sp(instructions, 16, false)?;
            instructions.push(aarch64_stack_access(
                0xf900_0000,
                0,
                expression_source(left),
                0,
            )?); // str x0, [sp]
            let nested_depth = stack_depth.checked_add(16).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_aarch64_expression_node(
                instructions,
                scalar_type,
                right,
                frame,
                nested_depth,
                internal_calls,
            )?;
            instructions.push(aarch64_stack_access(
                0xf940_0000,
                9,
                expression_source(left),
                0,
            )?); // ldr x9, [sp]
            emit_aarch64_adjust_sp(instructions, 16, true)?;
            instructions.push(match scalar_type.sign() {
                IntegerSign::Signed => 0x9ac0_0d20,   // sdiv x0, x9, x0
                IntegerSign::Unsigned => 0x9ac0_0920, // udiv x0, x9, x0
            });
            emit_aarch64_normalize(instructions, scalar_type);
        }
        TerminalAssignedIntegerExpression::ExactRemainder { left, right, .. } => {
            emit_aarch64_expression_node(
                instructions,
                scalar_type,
                left,
                frame,
                stack_depth,
                internal_calls,
            )?;
            emit_aarch64_adjust_sp(instructions, 16, false)?;
            instructions.push(aarch64_stack_access(
                0xf900_0000,
                0,
                expression_source(left),
                0,
            )?);
            let nested_depth = stack_depth.checked_add(16).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_aarch64_expression_node(
                instructions,
                scalar_type,
                right,
                frame,
                nested_depth,
                internal_calls,
            )?;
            instructions.push(aarch64_stack_access(
                0xf940_0000,
                9,
                expression_source(left),
                0,
            )?);
            emit_aarch64_adjust_sp(instructions, 16, true)?;
            instructions.push(match scalar_type.sign() {
                IntegerSign::Signed => 0x9ac0_0d2a,   // sdiv x10, x9, x0
                IntegerSign::Unsigned => 0x9ac0_092a, // udiv x10, x9, x0
            });
            instructions.push(0x9b00_a540); // msub x0, x10, x0, x9
            emit_aarch64_normalize(instructions, scalar_type);
        }
        TerminalAssignedIntegerExpression::WrappingDivide { left, right, .. } => {
            emit_aarch64_expression_node(
                instructions,
                scalar_type,
                left,
                frame,
                stack_depth,
                internal_calls,
            )?;
            emit_aarch64_adjust_sp(instructions, 16, false)?;
            instructions.push(aarch64_stack_access(
                0xf900_0000,
                0,
                expression_source(left),
                0,
            )?);
            let nested_depth = stack_depth.checked_add(16).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_aarch64_expression_node(
                instructions,
                scalar_type,
                right,
                frame,
                nested_depth,
                internal_calls,
            )?;
            instructions.push(aarch64_stack_access(
                0xf940_0000,
                9,
                expression_source(left),
                0,
            )?);
            emit_aarch64_adjust_sp(instructions, 16, true)?;
            instructions.push(match scalar_type.sign() {
                IntegerSign::Signed => 0x9ac0_0d20,   // sdiv x0, x9, x0
                IntegerSign::Unsigned => 0x9ac0_0920, // udiv x0, x9, x0
            });
            emit_aarch64_normalize(instructions, scalar_type);
        }
        TerminalAssignedIntegerExpression::WrappingRemainder { left, right, .. }
        | TerminalAssignedIntegerExpression::SaturatingRemainder { left, right, .. } => {
            emit_aarch64_expression_node(
                instructions,
                scalar_type,
                left,
                frame,
                stack_depth,
                internal_calls,
            )?;
            emit_aarch64_adjust_sp(instructions, 16, false)?;
            instructions.push(aarch64_stack_access(
                0xf900_0000,
                0,
                expression_source(left),
                0,
            )?);
            let nested_depth = stack_depth.checked_add(16).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_aarch64_expression_node(
                instructions,
                scalar_type,
                right,
                frame,
                nested_depth,
                internal_calls,
            )?;
            instructions.push(aarch64_stack_access(
                0xf940_0000,
                9,
                expression_source(left),
                0,
            )?);
            emit_aarch64_adjust_sp(instructions, 16, true)?;
            instructions.push(match scalar_type.sign() {
                IntegerSign::Signed => 0x9ac0_0d2a,   // sdiv x10, x9, x0
                IntegerSign::Unsigned => 0x9ac0_092a, // udiv x10, x9, x0
            });
            instructions.push(0x9b00_a540); // msub x0, x10, x0, x9
            emit_aarch64_normalize(instructions, scalar_type);
        }
        TerminalAssignedIntegerExpression::SaturatingDivide { left, right, .. } => {
            emit_aarch64_expression_node(
                instructions,
                scalar_type,
                left,
                frame,
                stack_depth,
                internal_calls,
            )?;
            emit_aarch64_adjust_sp(instructions, 16, false)?;
            instructions.push(aarch64_stack_access(
                0xf900_0000,
                0,
                expression_source(left),
                0,
            )?);
            let nested_depth = stack_depth.checked_add(16).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_aarch64_expression_node(
                instructions,
                scalar_type,
                right,
                frame,
                nested_depth,
                internal_calls,
            )?;
            instructions.push(aarch64_stack_access(
                0xf940_0000,
                9,
                expression_source(left),
                0,
            )?);
            emit_aarch64_adjust_sp(instructions, 16, true)?;
            match scalar_type.sign() {
                IntegerSign::Unsigned => instructions.push(0x9ac0_0920), // udiv x0, x9, x0
                IntegerSign::Signed => {
                    let (minimum, maximum) = native_integer_bounds(scalar_type);
                    instructions.push(0x9ac0_0d2a); // sdiv x10, x9, x0
                    instructions.push(0xcb09_03eb); // neg x11, x9
                    emit_aarch64_mov_immediate(instructions, 12, maximum);
                    if scalar_type.bits() == 64 {
                        emit_aarch64_mov_immediate(instructions, 13, minimum);
                        instructions.push(0xeb0d_013f); // cmp x9, x13
                        instructions.push(aarch64_csel(11, 12, 11, 0)); // min ? max : -value
                    } else {
                        instructions.push(0xeb0c_017f); // cmp x11, x12
                        instructions.push(aarch64_csel(11, 11, 12, 13)); // min(-value, max)
                    }
                    emit_aarch64_mov_immediate(instructions, 13, u64::MAX);
                    instructions.push(0xeb0d_001f); // cmp x0, x13
                    instructions.push(aarch64_csel(0, 11, 10, 0)); // divisor -1 ? clamp : quotient
                }
            }
            emit_aarch64_normalize(instructions, scalar_type);
        }
    }
    Ok(())
}

fn emit_aarch64_normalize(instructions: &mut Vec<u32>, scalar_type: IntegerType) {
    if scalar_type.bits() == 64 {
        return;
    }
    let base = match scalar_type.sign() {
        IntegerSign::Signed => 0x9340_0000,   // sbfm
        IntegerSign::Unsigned => 0xd340_0000, // ubfm
    };
    instructions.push(base | (u32::from(scalar_type.bits() - 1) << 10));
}

fn emit_aarch64_call(
    instructions: &mut Vec<u32>,
    psi_operation: psi_core::OperationId,
    source_value: ValueId,
    callee: MachineId,
    arguments: &[TerminalAssignedCallArgument],
    frame: &TerminalExpressionFrame,
    stack_depth: u32,
    internal_calls: &mut Option<(&mut Vec<TerminalInternalCallRelocation>, NativeTarget)>,
) -> Result<(), EmissionError> {
    for argument in arguments {
        match &argument.expression {
            TerminalAssignedScalarExpression::Boolean(expression) => {
                emit_aarch64_boolean_expression_node(
                    instructions,
                    expression,
                    frame,
                    stack_depth,
                    internal_calls,
                )?;
            }
            TerminalAssignedScalarExpression::Integer {
                scalar_type,
                expression,
            } => emit_aarch64_expression_node(
                instructions,
                *scalar_type,
                expression,
                frame,
                stack_depth,
                internal_calls,
            )?,
        }
        let byte_offset = argument.spill_byte_offset.checked_add(stack_depth).ok_or(
            EmissionError::IncomingStackOffsetNotEncodable {
                value: source_value,
                byte_offset: argument.spill_byte_offset,
            },
        )?;
        instructions.push(aarch64_stack_access(
            0xf900_0000,
            0,
            source_value,
            byte_offset,
        )?);
    }
    let Some((relocations, _)) = internal_calls.as_mut() else {
        return Err(EmissionError::CallOutsideDirectReturnExpression);
    };
    let outgoing_stack_bytes = outgoing_stack_bytes(source_value, arguments)?;
    let outgoing_stack_bytes = outgoing_stack_bytes
        .checked_add(15)
        .map(|bytes| bytes & !15)
        .ok_or(EmissionError::CallStackAreaNotEncodable {
            value: source_value,
            byte_size: outgoing_stack_bytes,
        })?;
    let call_stack_bytes =
        outgoing_stack_bytes
            .checked_add(16)
            .ok_or(EmissionError::CallStackAreaNotEncodable {
                value: source_value,
                byte_size: outgoing_stack_bytes,
            })?;
    let allocation_offset = instructions.len() * 4;
    emit_aarch64_adjust_sp(instructions, call_stack_bytes, false)?;
    let link_store_offset = instructions.len() * 4;
    instructions.push(aarch64_stack_access(
        0xf900_0000,
        30,
        source_value,
        outgoing_stack_bytes,
    )?); // str x30 above outgoing arguments
    for argument in arguments {
        let TerminalAssignedCallDestination::OutgoingStack { byte_offset } = argument.destination
        else {
            continue;
        };
        let spill_byte_offset = argument
            .spill_byte_offset
            .checked_add(stack_depth)
            .and_then(|offset| offset.checked_add(call_stack_bytes))
            .ok_or(EmissionError::CallStackAreaNotEncodable {
                value: source_value,
                byte_size: call_stack_bytes,
            })?;
        instructions.push(aarch64_stack_access(
            0xf940_0000,
            0,
            source_value,
            spill_byte_offset,
        )?);
        instructions.push(aarch64_stack_access(
            0xf900_0000,
            0,
            source_value,
            byte_offset,
        )?);
    }
    for argument in arguments {
        let TerminalAssignedCallDestination::Register(register) = argument.destination else {
            continue;
        };
        let register = aarch64_spill_register(source_value, register)?;
        let byte_offset = argument
            .spill_byte_offset
            .checked_add(stack_depth)
            .and_then(|offset| offset.checked_add(call_stack_bytes))
            .ok_or(EmissionError::CallStackAreaNotEncodable {
                value: source_value,
                byte_size: call_stack_bytes,
            })?;
        instructions.push(aarch64_stack_access(
            0xf940_0000,
            register,
            source_value,
            byte_offset,
        )?);
    }
    let offset = instructions.len() * 4;
    instructions.push(0x9400_0000); // bl #0
    let link_load_offset = instructions.len() * 4;
    instructions.push(aarch64_stack_access(
        0xf940_0000,
        30,
        source_value,
        outgoing_stack_bytes,
    )?); // ldr x30 above outgoing arguments
    let release_offset = instructions.len() * 4;
    emit_aarch64_adjust_sp(instructions, call_stack_bytes, true)?;
    relocations.push(TerminalInternalCallRelocation {
        psi_operation,
        target: callee,
        unit_stack: None,
        scalar_stack: Some(TerminalScalarCallStackEvidence {
            outbound: stack_adjustment_pair(
                call_stack_bytes,
                Some((allocation_offset, 4)),
                Some((release_offset, 4)),
            ),
            aarch64_return_link: Some(TerminalAarch64ReturnLinkEvidence {
                frame_byte_offset: outgoing_stack_bytes,
                store_offset: link_store_offset,
                load_offset: link_load_offset,
            }),
        }),
        offset,
    });
    Ok(())
}

fn outgoing_stack_bytes(
    source_value: ValueId,
    arguments: &[TerminalAssignedCallArgument],
) -> Result<u32, EmissionError> {
    arguments.iter().try_fold(0, |byte_size, argument| {
        let TerminalAssignedCallDestination::OutgoingStack { byte_offset } = argument.destination
        else {
            return Ok(byte_size);
        };
        let end = byte_offset
            .checked_add(8)
            .ok_or(EmissionError::CallStackAreaNotEncodable {
                value: source_value,
                byte_size: byte_offset,
            })?;
        Ok(byte_size.max(end))
    })
}

fn emit_aarch64_saturating_add(instructions: &mut Vec<u32>, scalar_type: IntegerType) {
    let (minimum, maximum) = native_integer_bounds(scalar_type);
    match (scalar_type.sign(), scalar_type.bits()) {
        (IntegerSign::Unsigned, 64) => {
            instructions.push(0xab00_0120); // adds x0, x9, x0
            emit_aarch64_mov_immediate(instructions, 10, maximum);
            instructions.push(aarch64_csel(0, 0, 10, 3)); // csel x0, x0, x10, cc
        }
        (IntegerSign::Unsigned, _) => {
            instructions.push(0x8b00_0120); // add x0, x9, x0
            emit_aarch64_mov_immediate(instructions, 10, maximum);
            instructions.push(0xeb0a_001f); // cmp x0, x10
            instructions.push(aarch64_csel(0, 0, 10, 9)); // csel x0, x0, x10, ls
        }
        (IntegerSign::Signed, 64) => {
            instructions.push(0x937f_fd2a); // asr x10, x9, 63
            emit_aarch64_mov_immediate(instructions, 11, maximum);
            instructions.push(0xca0b_014a); // eor x10, x10, x11
            instructions.push(0xab00_0120); // adds x0, x9, x0
            instructions.push(aarch64_csel(0, 0, 10, 7)); // csel x0, x0, x10, vc
        }
        (IntegerSign::Signed, _) => {
            instructions.push(0x8b00_0120); // add x0, x9, x0
            emit_aarch64_mov_immediate(instructions, 10, maximum);
            instructions.push(0xeb0a_001f); // cmp x0, x10
            instructions.push(aarch64_csel(0, 0, 10, 13)); // csel x0, x0, x10, le
            emit_aarch64_mov_immediate(instructions, 10, minimum);
            instructions.push(0xeb0a_001f); // cmp x0, x10
            instructions.push(aarch64_csel(0, 0, 10, 10)); // csel x0, x0, x10, ge
        }
    }
}

fn emit_aarch64_saturating_subtract(instructions: &mut Vec<u32>, scalar_type: IntegerType) {
    let (minimum, maximum) = native_integer_bounds(scalar_type);
    match (scalar_type.sign(), scalar_type.bits()) {
        (IntegerSign::Unsigned, _) => {
            instructions.push(0xeb00_0129); // subs x9, x9, x0
            instructions.push(aarch64_csel(0, 9, 31, 2)); // csel x0, x9, xzr, cs
        }
        (IntegerSign::Signed, 64) => {
            instructions.push(0x937f_fd2a); // asr x10, x9, 63
            emit_aarch64_mov_immediate(instructions, 11, maximum);
            instructions.push(0xca0b_014a); // eor x10, x10, x11
            instructions.push(0xeb00_0120); // subs x0, x9, x0
            instructions.push(aarch64_csel(0, 0, 10, 7)); // csel x0, x0, x10, vc
        }
        (IntegerSign::Signed, _) => {
            instructions.push(0xcb00_0120); // sub x0, x9, x0
            emit_aarch64_mov_immediate(instructions, 10, maximum);
            instructions.push(0xeb0a_001f); // cmp x0, x10
            instructions.push(aarch64_csel(0, 0, 10, 13)); // csel x0, x0, x10, le
            emit_aarch64_mov_immediate(instructions, 10, minimum);
            instructions.push(0xeb0a_001f); // cmp x0, x10
            instructions.push(aarch64_csel(0, 0, 10, 10)); // csel x0, x0, x10, ge
        }
    }
}

fn emit_aarch64_saturating_multiply(instructions: &mut Vec<u32>, scalar_type: IntegerType) {
    let (minimum, maximum) = native_integer_bounds(scalar_type);
    match (scalar_type.sign(), scalar_type.bits()) {
        (IntegerSign::Unsigned, 64) => {
            instructions.push(0x9bc0_7d2a); // umulh x10, x9, x0
            instructions.push(0x9b00_7d20); // mul x0, x9, x0
            instructions.push(0xf100_015f); // cmp x10, #0
            emit_aarch64_mov_immediate(instructions, 11, maximum);
            instructions.push(aarch64_csel(0, 0, 11, 0)); // csel x0, x0, x11, eq
        }
        (IntegerSign::Unsigned, _) => {
            instructions.push(0x9b00_7d20); // mul x0, x9, x0
            emit_aarch64_mov_immediate(instructions, 10, maximum);
            instructions.push(0xeb0a_001f); // cmp x0, x10
            instructions.push(aarch64_csel(0, 0, 10, 9)); // csel x0, x0, x10, ls
        }
        (IntegerSign::Signed, 64) => {
            instructions.push(0xca00_012b); // eor x11, x9, x0
            emit_aarch64_mov_immediate(instructions, 10, maximum);
            emit_aarch64_mov_immediate(instructions, 12, minimum);
            instructions.push(0xf100_017f); // cmp x11, #0
            instructions.push(aarch64_csel(11, 12, 10, 4)); // csel x11, x12, x10, mi
            instructions.push(0x9b40_7d2a); // smulh x10, x9, x0
            instructions.push(0x9b00_7d20); // mul x0, x9, x0
            instructions.push(0x937f_fc0c); // asr x12, x0, 63
            instructions.push(0xeb0c_015f); // cmp x10, x12
            instructions.push(aarch64_csel(0, 0, 11, 0)); // csel x0, x0, x11, eq
        }
        (IntegerSign::Signed, _) => {
            instructions.push(0x9b00_7d20); // mul x0, x9, x0
            emit_aarch64_mov_immediate(instructions, 10, maximum);
            instructions.push(0xeb0a_001f); // cmp x0, x10
            instructions.push(aarch64_csel(0, 0, 10, 13)); // csel x0, x0, x10, le
            emit_aarch64_mov_immediate(instructions, 10, minimum);
            instructions.push(0xeb0a_001f); // cmp x0, x10
            instructions.push(aarch64_csel(0, 0, 10, 10)); // csel x0, x0, x10, ge
        }
    }
    emit_aarch64_normalize(instructions, scalar_type);
}

fn emit_aarch64_mov_immediate(instructions: &mut Vec<u32>, register: u8, bits: u64) {
    for chunk in 0..4 {
        let immediate = ((bits >> (chunk * 16)) & 0xffff) as u32;
        if chunk == 0 || immediate != 0 {
            let base = if chunk == 0 { 0xd280_0000 } else { 0xf280_0000 };
            instructions
                .push(base | ((chunk as u32) << 21) | (immediate << 5) | u32::from(register));
        }
    }
}

fn emit_aarch64_adjust_sp(
    instructions: &mut Vec<u32>,
    byte_size: u32,
    add: bool,
) -> Result<(), EmissionError> {
    if byte_size > 0xfff {
        return Err(EmissionError::ExpressionStackFrameNotEncodable);
    }
    let base = if add { 0x9100_03ff } else { 0xd100_03ff };
    instructions.push(base | (byte_size << 10));
    Ok(())
}

fn linear_boolean_expression(expression: &TerminalAssignedBooleanExpression) -> bool {
    match expression {
        TerminalAssignedBooleanExpression::Call { arguments, .. } => arguments
            .iter()
            .all(|argument| linear_scalar_expression(&argument.expression)),
        TerminalAssignedBooleanExpression::Immediate { .. }
        | TerminalAssignedBooleanExpression::Parameter { .. } => true,
        TerminalAssignedBooleanExpression::Not { operand, .. } => {
            linear_boolean_expression(operand)
        }
        TerminalAssignedBooleanExpression::Equal { left, right, .. } => {
            linear_boolean_expression(left) && linear_boolean_expression(right)
        }
        TerminalAssignedBooleanExpression::IntegerEqual { left, right, .. }
        | TerminalAssignedBooleanExpression::IntegerLessThan { left, right, .. }
        | TerminalAssignedBooleanExpression::IntegerLessOrEqual { left, right, .. } => {
            linear_integer_expression(left) && linear_integer_expression(right)
        }
    }
}

fn linear_integer_expression(expression: &TerminalAssignedIntegerExpression) -> bool {
    match expression {
        TerminalAssignedIntegerExpression::Call { arguments, .. } => arguments
            .iter()
            .all(|argument| linear_scalar_expression(&argument.expression)),
        TerminalAssignedIntegerExpression::ExactDivide { .. }
        | TerminalAssignedIntegerExpression::ExactRemainder { .. }
        | TerminalAssignedIntegerExpression::WrappingDivide { .. }
        | TerminalAssignedIntegerExpression::WrappingRemainder { .. }
        | TerminalAssignedIntegerExpression::SaturatingDivide { .. }
        | TerminalAssignedIntegerExpression::SaturatingRemainder { .. } => false,
        TerminalAssignedIntegerExpression::Immediate { .. }
        | TerminalAssignedIntegerExpression::Parameter { .. } => true,
        TerminalAssignedIntegerExpression::BitwiseNot { operand, .. }
        | TerminalAssignedIntegerExpression::IntegerWiden { operand, .. }
        | TerminalAssignedIntegerExpression::IntegerExactCast { operand, .. } => {
            linear_integer_expression(operand)
        }
        TerminalAssignedIntegerExpression::WrappingAdd { left, right, .. }
        | TerminalAssignedIntegerExpression::BitwiseAnd { left, right, .. }
        | TerminalAssignedIntegerExpression::BitwiseOr { left, right, .. }
        | TerminalAssignedIntegerExpression::BitwiseXor { left, right, .. }
        | TerminalAssignedIntegerExpression::WrappingShiftLeft {
            value: left,
            count: right,
            ..
        }
        | TerminalAssignedIntegerExpression::WrappingShiftRight {
            value: left,
            count: right,
            ..
        }
        | TerminalAssignedIntegerExpression::ExactShiftLeft {
            value: left,
            count: right,
            ..
        }
        | TerminalAssignedIntegerExpression::ExactShiftRight {
            value: left,
            count: right,
            ..
        }
        | TerminalAssignedIntegerExpression::SaturatingAdd { left, right, .. }
        | TerminalAssignedIntegerExpression::WrappingSubtract { left, right, .. }
        | TerminalAssignedIntegerExpression::SaturatingSubtract { left, right, .. }
        | TerminalAssignedIntegerExpression::WrappingMultiply { left, right, .. }
        | TerminalAssignedIntegerExpression::SaturatingMultiply { left, right, .. } => {
            linear_integer_expression(left) && linear_integer_expression(right)
        }
    }
}

fn linear_scalar_expression(expression: &TerminalAssignedScalarExpression) -> bool {
    match expression {
        TerminalAssignedScalarExpression::Boolean(expression) => {
            linear_boolean_expression(expression)
        }
        TerminalAssignedScalarExpression::Integer { expression, .. } => {
            linear_integer_expression(expression)
        }
    }
}

fn direct_linear_integer_arm(arm: &TerminalAssignedConditionalIntegerArm) -> bool {
    matches!(
        arm.control.as_ref(),
        TerminalAssignedIntegerControl::Return { expression, .. }
            if linear_integer_expression(expression)
    )
}

fn collect_scalar_stack_evidence(
    architecture: Architecture,
    bytes: &[u8],
    control_flow: TerminalScalarControlFlowEvidence,
) -> Result<TerminalScalarStackEvidence, EmissionError> {
    let mutations = match architecture {
        Architecture::X86_64 => {
            let mut decoder =
                iced_x86::Decoder::with_ip(64, bytes, 0, iced_x86::DecoderOptions::NONE);
            let mut mutations = Vec::new();
            while decoder.can_decode() {
                let instruction = decoder.decode();
                if instruction.is_invalid() {
                    return Err(EmissionError::ScalarStackInstructionEncodingInvalid);
                }
                let offset = usize::try_from(instruction.ip())
                    .map_err(|_| EmissionError::ScalarStackInstructionEncodingInvalid)?;
                let kind = match instruction.mnemonic() {
                    iced_x86::Mnemonic::Sub
                        if instruction.op0_register() == iced_x86::Register::RSP =>
                    {
                        Some(TerminalScalarStackMutationKind::Allocate {
                            byte_size: x86_adjustment_immediate(bytes, offset, instruction.len())?,
                        })
                    }
                    iced_x86::Mnemonic::Add
                        if instruction.op0_register() == iced_x86::Register::RSP =>
                    {
                        Some(TerminalScalarStackMutationKind::Release {
                            byte_size: x86_adjustment_immediate(bytes, offset, instruction.len())?,
                        })
                    }
                    iced_x86::Mnemonic::Lea
                        if instruction.op0_register() == iced_x86::Register::RSP =>
                    {
                        Some(TerminalScalarStackMutationKind::X86ReleasePreservingFlags {
                            byte_size: x86_preserving_release_immediate(
                                bytes,
                                offset,
                                instruction.len(),
                            )?,
                        })
                    }
                    iced_x86::Mnemonic::Push => Some(TerminalScalarStackMutationKind::X86Push),
                    iced_x86::Mnemonic::Pop => Some(TerminalScalarStackMutationKind::X86Pop),
                    _ => None,
                };
                if let Some(kind) = kind {
                    mutations.push(TerminalScalarStackMutation {
                        offset,
                        byte_count: instruction.len(),
                        kind,
                    });
                }
            }
            mutations
        }
        Architecture::Aarch64 => {
            if !bytes.len().is_multiple_of(4) {
                return Err(EmissionError::ScalarStackInstructionEncodingInvalid);
            }
            bytes
                .chunks_exact(4)
                .enumerate()
                .filter_map(|(index, encoded)| {
                    let encoded = u32::from_le_bytes(encoded.try_into().expect("four-byte word"));
                    let base = encoded & !(0xfff << 10);
                    let kind = match base {
                        0xd100_03ff => TerminalScalarStackMutationKind::Allocate {
                            byte_size: (encoded >> 10) & 0xfff,
                        },
                        0x9100_03ff => TerminalScalarStackMutationKind::Release {
                            byte_size: (encoded >> 10) & 0xfff,
                        },
                        _ => return None,
                    };
                    Some(TerminalScalarStackMutation {
                        offset: index * 4,
                        byte_count: 4,
                        kind,
                    })
                })
                .collect()
        }
    };
    Ok(TerminalScalarStackEvidence {
        mutations,
        control_flow,
        stack_alignment: 16,
    })
}

fn x86_preserving_release_immediate(
    bytes: &[u8],
    offset: usize,
    byte_count: usize,
) -> Result<u32, EmissionError> {
    let instruction = bytes
        .get(offset..offset.saturating_add(byte_count))
        .ok_or(EmissionError::ScalarStackInstructionEncodingInvalid)?;
    match instruction {
        [0x48, 0x8d, 0x64, 0x24, immediate] if *immediate != 0 && *immediate <= i8::MAX as u8 => {
            Ok(u32::from(*immediate))
        }
        [0x48, 0x8d, 0xa4, 0x24, immediate @ ..] if immediate.len() == 4 => {
            let byte_size = u32::from_le_bytes(
                immediate
                    .try_into()
                    .map_err(|_| EmissionError::ScalarStackInstructionEncodingInvalid)?,
            );
            (byte_size > i8::MAX as u32)
                .then_some(byte_size)
                .ok_or(EmissionError::ScalarStackInstructionEncodingInvalid)
        }
        _ => Err(EmissionError::ScalarStackInstructionEncodingInvalid),
    }
}

fn x86_adjustment_immediate(
    bytes: &[u8],
    offset: usize,
    byte_count: usize,
) -> Result<u32, EmissionError> {
    match byte_count {
        4 => bytes
            .get(offset + 3)
            .copied()
            .map(u32::from)
            .ok_or(EmissionError::ScalarStackInstructionEncodingInvalid),
        7 => bytes
            .get(offset + 3..offset + 7)
            .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
            .map(u32::from_le_bytes)
            .ok_or(EmissionError::ScalarStackInstructionEncodingInvalid),
        _ => Err(EmissionError::ScalarStackInstructionEncodingInvalid),
    }
}

fn aarch64_stack_access(
    base: u32,
    register: u8,
    source_value: ValueId,
    byte_offset: u32,
) -> Result<u32, EmissionError> {
    if !byte_offset.is_multiple_of(8) || byte_offset / 8 > 0xfff {
        return Err(EmissionError::IncomingStackOffsetNotEncodable {
            value: source_value,
            byte_offset,
        });
    }
    Ok(base | ((byte_offset / 8) << 10) | (31 << 5) | u32::from(register))
}

const fn aarch64_csel(destination: u8, left: u8, right: u8, condition: u8) -> u32 {
    0x9a80_0000
        | ((right as u32) << 16)
        | ((condition as u32) << 12)
        | ((left as u32) << 5)
        | destination as u32
}

fn expression_source(expression: &TerminalAssignedIntegerExpression) -> ValueId {
    match expression {
        TerminalAssignedIntegerExpression::Call { source_value, .. } => *source_value,
        TerminalAssignedIntegerExpression::Immediate { source_value, .. }
        | TerminalAssignedIntegerExpression::Parameter { source_value, .. } => *source_value,
        TerminalAssignedIntegerExpression::BitwiseNot { operand, .. }
        | TerminalAssignedIntegerExpression::IntegerWiden { operand, .. }
        | TerminalAssignedIntegerExpression::IntegerExactCast { operand, .. } => {
            expression_source(operand)
        }
        TerminalAssignedIntegerExpression::WrappingAdd { left, .. }
        | TerminalAssignedIntegerExpression::BitwiseAnd { left, .. }
        | TerminalAssignedIntegerExpression::BitwiseOr { left, .. }
        | TerminalAssignedIntegerExpression::BitwiseXor { left, .. }
        | TerminalAssignedIntegerExpression::WrappingShiftLeft { value: left, .. }
        | TerminalAssignedIntegerExpression::WrappingShiftRight { value: left, .. }
        | TerminalAssignedIntegerExpression::ExactShiftLeft { value: left, .. }
        | TerminalAssignedIntegerExpression::ExactShiftRight { value: left, .. }
        | TerminalAssignedIntegerExpression::SaturatingAdd { left, .. }
        | TerminalAssignedIntegerExpression::WrappingSubtract { left, .. }
        | TerminalAssignedIntegerExpression::SaturatingSubtract { left, .. }
        | TerminalAssignedIntegerExpression::WrappingMultiply { left, .. }
        | TerminalAssignedIntegerExpression::SaturatingMultiply { left, .. } => {
            expression_source(left)
        }
        TerminalAssignedIntegerExpression::ExactDivide { left, .. } => expression_source(left),
        TerminalAssignedIntegerExpression::ExactRemainder { left, .. } => expression_source(left),
        TerminalAssignedIntegerExpression::WrappingDivide { left, .. } => expression_source(left),
        TerminalAssignedIntegerExpression::WrappingRemainder { left, .. } => {
            expression_source(left)
        }
        TerminalAssignedIntegerExpression::SaturatingDivide { left, .. } => expression_source(left),
        TerminalAssignedIntegerExpression::SaturatingRemainder { left, .. } => {
            expression_source(left)
        }
    }
}

fn boolean_expression_source(expression: &TerminalAssignedBooleanExpression) -> ValueId {
    match expression {
        TerminalAssignedBooleanExpression::Call { source_value, .. } => *source_value,
        TerminalAssignedBooleanExpression::Immediate { source_value, .. }
        | TerminalAssignedBooleanExpression::Parameter { source_value, .. } => *source_value,
        TerminalAssignedBooleanExpression::Not { operand, .. } => {
            boolean_expression_source(operand)
        }
        TerminalAssignedBooleanExpression::Equal { left, .. } => boolean_expression_source(left),
        TerminalAssignedBooleanExpression::IntegerEqual { left, .. }
        | TerminalAssignedBooleanExpression::IntegerLessThan { left, .. }
        | TerminalAssignedBooleanExpression::IntegerLessOrEqual { left, .. } => {
            expression_source(left)
        }
    }
}

fn native_integer_bounds(scalar_type: IntegerType) -> (u64, u64) {
    let width = scalar_type.bits();
    match scalar_type.sign() {
        IntegerSign::Unsigned => {
            let maximum = if width == 64 {
                u64::MAX
            } else {
                (1_u64 << width) - 1
            };
            (0, maximum)
        }
        IntegerSign::Signed => {
            let maximum = if width == 64 {
                i64::MAX as u64
            } else {
                (1_u64 << (width - 1)) - 1
            };
            let minimum = if width == 64 {
                i64::MIN as u64
            } else {
                u64::MAX << (width - 1)
            };
            (minimum, maximum)
        }
    }
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
}

impl std::fmt::Display for EmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for EmissionError {}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_calling_conventions::{CallSignature, CallingPolicy, evaluate_call_plan};
    use omega_target::NativeTarget;
    use omega_terminal_target_operations::{
        TerminalMetadataOnlyPortRealization, TerminalProviderExecutionBinding,
        TerminalProviderPlanIdentity, TerminalPsiProvenance, TerminalScalarParameterLocation,
        TerminalTargetBooleanControl, TerminalTargetBooleanExpression, TerminalTargetCallArgument,
        TerminalTargetConditionalBooleanArm, TerminalTargetConditionalIntegerArm,
        TerminalTargetFunction, TerminalTargetIntegerControl, TerminalTargetIntegerExpression,
        TerminalTargetOperation, TerminalTargetOperationPlan, TerminalTargetScalarExpression,
        TerminalTargetStructuralArgument, TerminalTargetStructuralParameter,
        TerminalTargetUnitBody, TerminalTargetUnitOperation,
    };
    use omega_terminal_target_operations_to_assigned_target_operations::assign_registers;
    use psi_core::{
        BoundaryMachineId, EdgeId, MachineId, OperationId, PlaceId, ServiceId, StructuralTypeId,
    };
    use psi_terminal::{
        SemanticFingerprint, StructuralArgument, StructuralPathSegment, TerminalPsiIdentity,
        VocabularyMarker,
    };

    fn emit_machine_code(
        plan: &TerminalTargetOperationPlan,
    ) -> Result<TerminalMachineCodePlan, EmissionError> {
        let assigned = assign_registers(plan).expect("test target operations must assign");
        super::emit_machine_code(&assigned)
    }

    #[test]
    fn x86_unit_call_port_write_and_settlement_keep_exact_order() {
        let target = NativeTarget::linux_x64();
        let empty_call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature::default(),
        )
        .expect("empty Unit ABI");
        let call_operation = OperationId::new(1).expect("call");
        let port_operation = OperationId::new(2).expect("port");
        let settlement_operation = OperationId::new(3).expect("settlement");
        let root_return = EdgeId::new(1).expect("root return");
        let leaf_return = EdgeId::new(2).expect("leaf return");
        let boundary = BoundaryMachineId::new(1).expect("boundary");
        let provider_plan = TerminalProviderPlanIdentity::new(7).expect("provider");
        let provider_execution =
            TerminalProviderExecutionBinding::from_execution_record(provider_plan, 8, 9, 10, 11)
                .expect("provider execution");
        let realization = TerminalMetadataOnlyPortRealization {
            effect_operation: port_operation,
            service: ServiceId::new(1).expect("PortIo"),
            port: 0x20,
            value: 0x20,
        };
        let settlement_arguments = vec![StructuralArgument {
            place: PlaceId::new(41).expect("custody argument"),
            path: vec![
                StructuralPathSegment::Field("#payload".into()),
                StructuralPathSegment::FixedIndex(3),
            ],
        }];
        let plan = TerminalTargetOperationPlan {
            terminal_psi: identity(),
            target,
            entry: MachineId::new(1).expect("root"),
            functions: vec![
                TerminalTargetFunction {
                    machine: MachineId::new(1).expect("root"),
                    provenance: TerminalPsiProvenance {
                        operations: vec![call_operation],
                        edges: vec![root_return],
                    },
                    operation: TerminalTargetOperation::UnitBody(TerminalTargetUnitBody {
                        call_plan: empty_call_plan.clone(),
                        parameters: Vec::new(),
                        operations: vec![
                            TerminalTargetUnitOperation::Call {
                                psi_operation: call_operation,
                                callee: MachineId::new(2).expect("leaf"),
                                arguments: Vec::new(),
                                claim_transfers: Vec::new(),
                            },
                            TerminalTargetUnitOperation::Return {
                                psi_edge: root_return,
                            },
                        ],
                    }),
                },
                TerminalTargetFunction {
                    machine: MachineId::new(2).expect("leaf"),
                    provenance: TerminalPsiProvenance {
                        operations: vec![port_operation, settlement_operation],
                        edges: vec![leaf_return],
                    },
                    operation: TerminalTargetOperation::UnitBody(TerminalTargetUnitBody {
                        call_plan: empty_call_plan,
                        parameters: Vec::new(),
                        operations: vec![
                            TerminalTargetUnitOperation::PortWrite {
                                psi_operation: port_operation,
                                service: ServiceId::new(1).expect("PortIo"),
                                port: 0x20,
                                value: 0x20,
                            },
                            TerminalTargetUnitOperation::BoundarySettlement {
                                psi_operation: settlement_operation,
                                boundary,
                                provider_execution,
                                realization,
                                arguments: settlement_arguments.clone(),
                                completion_receipts: Vec::new(),
                            },
                            TerminalTargetUnitOperation::Return {
                                psi_edge: leaf_return,
                            },
                        ],
                    }),
                },
            ],
        };

        let emitted = emit_machine_code(&plan).expect("Unit/effect emission");
        let root = &emitted.functions[0];
        assert_eq!(
            root.bytes,
            [
                0x48, 0x83, 0xec, 0x08, 0xe8, 0, 0, 0, 0, 0x48, 0x83, 0xc4, 0x08, 0xc3,
            ]
        );
        assert_eq!(root.internal_calls[0].offset, 5);
        assert_eq!(root.internal_calls[0].target, MachineId::new(2).unwrap());
        assert_eq!(
            root.unit_stack,
            Some(TerminalUnitStackEvidence {
                frame: None,
                aarch64_return_link: None,
                stack_alignment: 16,
            })
        );
        assert_eq!(
            root.internal_calls[0].unit_stack,
            Some(TerminalUnitCallStackEvidence {
                outbound: Some(TerminalStackAdjustmentPair {
                    byte_size: 8,
                    allocation_offset: 0,
                    allocation_byte_count: 4,
                    release_offset: 9,
                    release_byte_count: 4,
                }),
            })
        );

        let leaf = &emitted.functions[1];
        let mut expected = omega_x86_encoding::encode_immediate_port_write(0x20, 0x20).to_vec();
        expected.push(0xc3);
        assert_eq!(leaf.bytes, expected);
        assert_eq!(
            leaf.unit_stack,
            Some(TerminalUnitStackEvidence {
                frame: None,
                aarch64_return_link: None,
                stack_alignment: 16,
            })
        );
        assert_eq!(leaf.bytes.iter().filter(|byte| **byte == 0xee).count(), 1);
        assert_eq!(leaf.boundary_settlements.len(), 1);
        assert_eq!(leaf.boundary_settlements[0].code_offset, 27);
        assert_eq!(leaf.boundary_settlements[0].boundary, boundary);
        assert_eq!(
            leaf.boundary_settlements[0].provider_execution,
            provider_execution.into()
        );
        assert_eq!(leaf.boundary_settlements[0].realization, realization);
        assert_eq!(leaf.boundary_settlements[0].arguments, settlement_arguments);
        assert_eq!(leaf.port_effects.len(), 1);
        assert_eq!(leaf.port_effects[0].service, realization.service);
        assert_eq!(leaf.fuel_attribution.len(), 3);
        assert_eq!(
            leaf.fuel_attribution
                .iter()
                .map(|row| (row.site, row.units, row.code_offset, row.byte_count))
                .collect::<Vec<_>>(),
            [
                (TerminalNativeFuelSite::Operation(port_operation), 1, 0, 27,),
                (
                    TerminalNativeFuelSite::Operation(settlement_operation),
                    1,
                    27,
                    0,
                ),
                (TerminalNativeFuelSite::Edge(leaf_return), 1, 27, 1),
            ]
        );
        assert!(leaf.fuel_attribution.iter().all(|row| {
            row.schedule == psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity()
        }));
    }

    #[test]
    fn aarch64_rejects_port_write_before_emitting_a_partial_body() {
        let target = NativeTarget::linux_arm64();
        let call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature::default(),
        )
        .expect("empty Unit ABI");
        let plan = TerminalTargetOperationPlan {
            terminal_psi: identity(),
            target,
            entry: MachineId::new(1).unwrap(),
            functions: vec![TerminalTargetFunction {
                machine: MachineId::new(1).unwrap(),
                provenance: TerminalPsiProvenance::default(),
                operation: TerminalTargetOperation::UnitBody(TerminalTargetUnitBody {
                    call_plan,
                    parameters: Vec::new(),
                    operations: vec![
                        TerminalTargetUnitOperation::PortWrite {
                            psi_operation: OperationId::new(1).unwrap(),
                            service: ServiceId::new(1).unwrap(),
                            port: 0x20,
                            value: 0x20,
                        },
                        TerminalTargetUnitOperation::Return {
                            psi_edge: EdgeId::new(1).unwrap(),
                        },
                    ],
                }),
            }],
        };
        assert_eq!(
            emit_machine_code(&plan),
            Err(EmissionError::PortWriteUnsupportedOnArchitecture(
                Architecture::Aarch64
            ))
        );
    }

    #[test]
    fn forty_byte_unit_argument_is_copied_for_sysv_and_forwarded_indirectly_elsewhere() {
        for (target, expected_length, expected_relocation) in [
            (NativeTarget::linux_x64(), 122, 109),
            (NativeTarget::windows_x64(), 32, 19),
            (NativeTarget::linux_arm64(), 84, 64),
        ] {
            let shape = omega_calling_conventions::ValueShape::integer(40, 8);
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
            let argument = TerminalTargetStructuralArgument {
                place,
                path: Vec::new(),
                root_structural_type: structural_type,
                structural_type,
                shape,
                source_byte_offset: 0,
                fixed_array_length: None,
                element_stride: None,
                source: call_plan.parameters[0].clone(),
                destination: call_plan.parameters[0].clone(),
            };
            let parameter = TerminalTargetStructuralParameter {
                place,
                structural_type,
                shape,
                placement: call_plan.parameters[0].clone(),
            };
            let plan = TerminalTargetOperationPlan {
                terminal_psi: identity(),
                target,
                entry: MachineId::new(1).unwrap(),
                functions: vec![
                    TerminalTargetFunction {
                        machine: MachineId::new(1).unwrap(),
                        provenance: TerminalPsiProvenance::default(),
                        operation: TerminalTargetOperation::UnitBody(TerminalTargetUnitBody {
                            call_plan: call_plan.clone(),
                            parameters: vec![parameter.clone()],
                            operations: vec![
                                TerminalTargetUnitOperation::Call {
                                    psi_operation: OperationId::new(1).unwrap(),
                                    callee: MachineId::new(2).unwrap(),
                                    arguments: vec![argument],
                                    claim_transfers: Vec::new(),
                                },
                                TerminalTargetUnitOperation::Return {
                                    psi_edge: EdgeId::new(1).unwrap(),
                                },
                            ],
                        }),
                    },
                    TerminalTargetFunction {
                        machine: MachineId::new(2).unwrap(),
                        provenance: TerminalPsiProvenance::default(),
                        operation: TerminalTargetOperation::UnitBody(TerminalTargetUnitBody {
                            call_plan,
                            parameters: vec![parameter],
                            operations: vec![TerminalTargetUnitOperation::Return {
                                psi_edge: EdgeId::new(2).unwrap(),
                            }],
                        }),
                    },
                ],
            };
            let emitted = emit_machine_code(&plan).unwrap();
            assert_eq!(emitted.functions[0].bytes.len(), expected_length);
            assert_eq!(
                emitted.functions[0].internal_calls[0].offset,
                expected_relocation
            );
        }
    }

    #[test]
    fn x86_unit_parameter_homes_survive_effects_and_parallel_reordering() {
        let target = NativeTarget::linux_x64();
        let shape = omega_calling_conventions::ValueShape::integer(8, 8);
        let call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![shape, shape],
                result: None,
            },
        )
        .unwrap();
        let first = PlaceId::new(1).unwrap();
        let second = PlaceId::new(2).unwrap();
        let ty = StructuralTypeId::new(1).unwrap();
        let parameter = |place: PlaceId, index: usize| TerminalTargetStructuralParameter {
            place,
            structural_type: ty,
            shape,
            placement: call_plan.parameters[index].clone(),
        };
        let argument =
            |place: PlaceId, source: usize, destination: usize| TerminalTargetStructuralArgument {
                place,
                path: Vec::new(),
                root_structural_type: ty,
                structural_type: ty,
                shape,
                source_byte_offset: 0,
                fixed_array_length: None,
                element_stride: None,
                source: call_plan.parameters[source].clone(),
                destination: call_plan.parameters[destination].clone(),
            };
        let plan = TerminalTargetOperationPlan {
            terminal_psi: identity(),
            target,
            entry: MachineId::new(1).unwrap(),
            functions: vec![
                TerminalTargetFunction {
                    machine: MachineId::new(1).unwrap(),
                    provenance: TerminalPsiProvenance::default(),
                    operation: TerminalTargetOperation::UnitBody(TerminalTargetUnitBody {
                        call_plan: call_plan.clone(),
                        parameters: vec![parameter(first, 0), parameter(second, 1)],
                        operations: vec![
                            TerminalTargetUnitOperation::PortWrite {
                                psi_operation: OperationId::new(1).unwrap(),
                                service: ServiceId::new(1).unwrap(),
                                port: 0x20,
                                value: 0x20,
                            },
                            TerminalTargetUnitOperation::Call {
                                psi_operation: OperationId::new(2).unwrap(),
                                callee: MachineId::new(2).unwrap(),
                                arguments: vec![argument(second, 1, 0), argument(first, 0, 1)],
                                claim_transfers: Vec::new(),
                            },
                            TerminalTargetUnitOperation::Return {
                                psi_edge: EdgeId::new(1).unwrap(),
                            },
                        ],
                    }),
                },
                TerminalTargetFunction {
                    machine: MachineId::new(2).unwrap(),
                    provenance: TerminalPsiProvenance::default(),
                    operation: TerminalTargetOperation::UnitBody(TerminalTargetUnitBody {
                        call_plan: call_plan.clone(),
                        parameters: vec![parameter(first, 0), parameter(second, 1)],
                        operations: vec![TerminalTargetUnitOperation::Return {
                            psi_edge: EdgeId::new(2).unwrap(),
                        }],
                    }),
                },
            ],
        };
        let emitted = emit_machine_code(&plan).unwrap();
        let bytes = &emitted.functions[0].bytes;
        let out = bytes.iter().position(|byte| *byte == 0xee).unwrap();
        let load_second_into_first = bytes
            .windows(5)
            .position(|window| window == [0x48, 0x8b, 0x7c, 0x24, 0x10])
            .unwrap();
        let load_first_into_second = bytes
            .windows(5)
            .position(|window| window == [0x48, 0x8b, 0x74, 0x24, 0x08])
            .unwrap();
        assert!(out < load_second_into_first);
        assert!(out < load_first_into_second);
    }

    #[test]
    fn aarch64_unit_parameter_homes_survive_parallel_reordering_and_restore_lr() {
        let target = NativeTarget::linux_arm64();
        let shape = omega_calling_conventions::ValueShape::integer(8, 8);
        let call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![shape, shape],
                result: None,
            },
        )
        .unwrap();
        let first = PlaceId::new(1).unwrap();
        let second = PlaceId::new(2).unwrap();
        let ty = StructuralTypeId::new(1).unwrap();
        let parameter = |place: PlaceId, index: usize| TerminalTargetStructuralParameter {
            place,
            structural_type: ty,
            shape,
            placement: call_plan.parameters[index].clone(),
        };
        let argument =
            |place: PlaceId, source: usize, destination: usize| TerminalTargetStructuralArgument {
                place,
                path: Vec::new(),
                root_structural_type: ty,
                structural_type: ty,
                shape,
                source_byte_offset: 0,
                fixed_array_length: None,
                element_stride: None,
                source: call_plan.parameters[source].clone(),
                destination: call_plan.parameters[destination].clone(),
            };
        let plan = TerminalTargetOperationPlan {
            terminal_psi: identity(),
            target,
            entry: MachineId::new(1).unwrap(),
            functions: vec![
                TerminalTargetFunction {
                    machine: MachineId::new(1).unwrap(),
                    provenance: TerminalPsiProvenance::default(),
                    operation: TerminalTargetOperation::UnitBody(TerminalTargetUnitBody {
                        call_plan: call_plan.clone(),
                        parameters: vec![parameter(first, 0), parameter(second, 1)],
                        operations: vec![
                            TerminalTargetUnitOperation::Call {
                                psi_operation: OperationId::new(1).unwrap(),
                                callee: MachineId::new(2).unwrap(),
                                arguments: vec![argument(second, 1, 0), argument(first, 0, 1)],
                                claim_transfers: Vec::new(),
                            },
                            TerminalTargetUnitOperation::Return {
                                psi_edge: EdgeId::new(1).unwrap(),
                            },
                        ],
                    }),
                },
                TerminalTargetFunction {
                    machine: MachineId::new(2).unwrap(),
                    provenance: TerminalPsiProvenance::default(),
                    operation: TerminalTargetOperation::UnitBody(TerminalTargetUnitBody {
                        call_plan: call_plan.clone(),
                        parameters: vec![parameter(first, 0), parameter(second, 1)],
                        operations: vec![TerminalTargetUnitOperation::Return {
                            psi_edge: EdgeId::new(2).unwrap(),
                        }],
                    }),
                },
            ],
        };
        let emitted = emit_machine_code(&plan).unwrap();
        let caller = &emitted.functions[0];
        let instructions = aarch64_instructions(&caller.bytes);
        assert_eq!(caller.internal_calls[0].offset, 24);
        assert_eq!(caller.internal_calls[0].target, MachineId::new(2).unwrap());
        assert_eq!(
            caller.unit_stack,
            Some(TerminalUnitStackEvidence {
                frame: Some(TerminalStackAdjustmentPair {
                    byte_size: 32,
                    allocation_offset: 0,
                    allocation_byte_count: 4,
                    release_offset: 32,
                    release_byte_count: 4,
                }),
                aarch64_return_link: Some(TerminalAarch64ReturnLinkEvidence {
                    frame_byte_offset: 16,
                    store_offset: 4,
                    load_offset: 28,
                }),
                stack_alignment: 16,
            })
        );
        assert_eq!(
            caller.internal_calls[0].unit_stack,
            Some(TerminalUnitCallStackEvidence { outbound: None })
        );
        assert_eq!(instructions[0], 0xd100_83ff); // sub sp, sp, #32
        assert_eq!(instructions[1], 0xf900_0bfe); // str x30, [sp, #16]
        assert_eq!(instructions[2], 0xf900_03e0); // str x0, [sp]
        assert_eq!(instructions[3], 0xf900_07e1); // str x1, [sp, #8]
        assert_eq!(instructions[4], 0xf940_07e0); // ldr x0, [sp, #8]
        assert_eq!(instructions[5], 0xf940_03e1); // ldr x1, [sp]
        assert_eq!(instructions[6], 0x9400_0000); // bl #0
        assert_eq!(instructions[7], 0xf940_0bfe); // ldr x30, [sp, #16]
        assert_eq!(instructions[8], 0x9100_83ff); // add sp, sp, #32
        assert_eq!(instructions[9], 0xd65f_03c0); // ret x30
        assert_eq!(caller.fuel_attribution[0].code_offset, 16);
        assert_eq!(caller.fuel_attribution[0].byte_count, 12);
        assert_eq!(caller.fuel_attribution[1].code_offset, 28);
        assert_eq!(caller.fuel_attribution[1].byte_count, 12);
    }

    #[test]
    fn aarch64_unit_calls_cover_stack_fragments_and_stack_indirect_copies() {
        for final_shape in [
            omega_calling_conventions::ValueShape::integer(16, 8),
            omega_calling_conventions::ValueShape::integer(24, 16),
        ] {
            let target = NativeTarget::linux_arm64();
            let word = omega_calling_conventions::ValueShape::integer(8, 8);
            let mut shapes = vec![word; 8];
            shapes.push(final_shape);
            let call_plan = evaluate_call_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature {
                    parameters: shapes.clone(),
                    result: None,
                },
            )
            .unwrap();
            let ty = StructuralTypeId::new(1).unwrap();
            let parameters = shapes
                .iter()
                .enumerate()
                .map(|(index, shape)| TerminalTargetStructuralParameter {
                    place: PlaceId::new(index as u64 + 1).unwrap(),
                    structural_type: ty,
                    shape: *shape,
                    placement: call_plan.parameters[index].clone(),
                })
                .collect::<Vec<_>>();
            let arguments = parameters
                .iter()
                .map(|parameter| TerminalTargetStructuralArgument {
                    place: parameter.place,
                    path: Vec::new(),
                    root_structural_type: ty,
                    structural_type: ty,
                    shape: parameter.shape,
                    source_byte_offset: 0,
                    fixed_array_length: None,
                    element_stride: None,
                    source: parameter.placement.clone(),
                    destination: parameter.placement.clone(),
                })
                .collect::<Vec<_>>();
            let plan = TerminalTargetOperationPlan {
                terminal_psi: identity(),
                target,
                entry: MachineId::new(1).unwrap(),
                functions: vec![
                    TerminalTargetFunction {
                        machine: MachineId::new(1).unwrap(),
                        provenance: TerminalPsiProvenance::default(),
                        operation: TerminalTargetOperation::UnitBody(TerminalTargetUnitBody {
                            call_plan: call_plan.clone(),
                            parameters: parameters.clone(),
                            operations: vec![
                                TerminalTargetUnitOperation::Call {
                                    psi_operation: OperationId::new(1).unwrap(),
                                    callee: MachineId::new(2).unwrap(),
                                    arguments,
                                    claim_transfers: Vec::new(),
                                },
                                TerminalTargetUnitOperation::Return {
                                    psi_edge: EdgeId::new(1).unwrap(),
                                },
                            ],
                        }),
                    },
                    TerminalTargetFunction {
                        machine: MachineId::new(2).unwrap(),
                        provenance: TerminalPsiProvenance::default(),
                        operation: TerminalTargetOperation::UnitBody(TerminalTargetUnitBody {
                            call_plan,
                            parameters,
                            operations: vec![TerminalTargetUnitOperation::Return {
                                psi_edge: EdgeId::new(2).unwrap(),
                            }],
                        }),
                    },
                ],
            };
            let emitted = emit_machine_code(&plan).unwrap_or_else(|error| {
                panic!(
                    "AAPCS64 {}-byte exhausted Unit argument failed: {error:?}",
                    final_shape.byte_size
                )
            });
            let caller = &emitted.functions[0];
            assert_eq!(caller.internal_calls.len(), 1);
            assert_eq!(caller.internal_calls[0].target, MachineId::new(2).unwrap());
            assert!(aarch64_instructions(&caller.bytes).contains(&0x9400_0000));
            assert_eq!(
                *aarch64_instructions(&caller.bytes).last().unwrap(),
                0xd65f_03c0
            );
        }
    }

    #[test]
    fn unit_argument_fragments_cover_native_scalar_widths() {
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::windows_x64(),
            NativeTarget::linux_arm64(),
        ] {
            for byte_size in [1_u16, 2, 4, 8, 12, 16] {
                let alignment = byte_size.min(8);
                let shape = omega_calling_conventions::ValueShape::integer(byte_size, alignment);
                let call_plan = evaluate_call_plan(
                    CallingPolicy::native_for_target(target),
                    &CallSignature {
                        parameters: vec![shape],
                        result: None,
                    },
                )
                .unwrap();
                let place = PlaceId::new(1).unwrap();
                let ty = StructuralTypeId::new(1).unwrap();
                let parameter = TerminalTargetStructuralParameter {
                    place,
                    structural_type: ty,
                    shape,
                    placement: call_plan.parameters[0].clone(),
                };
                let argument = TerminalTargetStructuralArgument {
                    place,
                    path: Vec::new(),
                    root_structural_type: ty,
                    structural_type: ty,
                    shape,
                    source_byte_offset: 0,
                    fixed_array_length: None,
                    element_stride: None,
                    source: call_plan.parameters[0].clone(),
                    destination: call_plan.parameters[0].clone(),
                };
                let plan = TerminalTargetOperationPlan {
                    terminal_psi: identity(),
                    target,
                    entry: MachineId::new(1).unwrap(),
                    functions: vec![
                        TerminalTargetFunction {
                            machine: MachineId::new(1).unwrap(),
                            provenance: TerminalPsiProvenance::default(),
                            operation: TerminalTargetOperation::UnitBody(TerminalTargetUnitBody {
                                call_plan: call_plan.clone(),
                                parameters: vec![parameter.clone()],
                                operations: vec![
                                    TerminalTargetUnitOperation::Call {
                                        psi_operation: OperationId::new(1).unwrap(),
                                        callee: MachineId::new(2).unwrap(),
                                        arguments: vec![argument],
                                        claim_transfers: Vec::new(),
                                    },
                                    TerminalTargetUnitOperation::Return {
                                        psi_edge: EdgeId::new(1).unwrap(),
                                    },
                                ],
                            }),
                        },
                        TerminalTargetFunction {
                            machine: MachineId::new(2).unwrap(),
                            provenance: TerminalPsiProvenance::default(),
                            operation: TerminalTargetOperation::UnitBody(TerminalTargetUnitBody {
                                call_plan,
                                parameters: vec![parameter],
                                operations: vec![TerminalTargetUnitOperation::Return {
                                    psi_edge: EdgeId::new(2).unwrap(),
                                }],
                            }),
                        },
                    ],
                };
                emit_machine_code(&plan).unwrap_or_else(|error| {
                    panic!("{target:?} {byte_size}-byte Unit argument failed: {error:?}")
                });
            }
        }
    }

    fn plan(target: NativeTarget) -> TerminalTargetOperationPlan {
        let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
        TerminalTargetOperationPlan {
            terminal_psi: identity(),
            target,
            entry: MachineId::new(1).expect("machine"),
            functions: vec![TerminalTargetFunction {
                machine: MachineId::new(1).expect("machine"),
                provenance: TerminalPsiProvenance::default(),
                operation: TerminalTargetOperation::ReturnIntegerImmediate {
                    psi_edge: EdgeId::new(1).expect("edge"),
                    source_value: ValueId::new(1).expect("value"),
                    scalar_type: i32_type,
                    value: IntegerValue::Signed(7),
                },
            }],
        }
    }

    fn conditional_plan(target: NativeTarget) -> TerminalTargetOperationPlan {
        let locations = match target.architecture {
            Architecture::X86_64 => [
                MachineRegister::X86Rdi,
                MachineRegister::X86Rsi,
                MachineRegister::X86Rdx,
            ],
            Architecture::Aarch64 => [
                MachineRegister::Aarch64X(0),
                MachineRegister::Aarch64X(1),
                MachineRegister::Aarch64X(2),
            ],
        };
        let arm = |edge, return_edge, source_value, parameter_index, register| {
            TerminalTargetConditionalIntegerArm {
                psi_edge: EdgeId::new(edge).expect("edge"),
                control: Box::new(TerminalTargetIntegerControl::Return {
                    psi_return_edge: EdgeId::new(return_edge).expect("return edge"),
                    source_value: ValueId::new(source_value).expect("source value"),
                    expression: TerminalTargetIntegerExpression::Parameter {
                        source_value: ValueId::new(source_value).expect("argument value"),
                        parameter_index,
                        location: TerminalScalarParameterLocation::Register(register),
                    },
                }),
            }
        };
        TerminalTargetOperationPlan {
            terminal_psi: identity(),
            target,
            entry: MachineId::new(1).expect("machine"),
            functions: vec![TerminalTargetFunction {
                machine: MachineId::new(1).expect("machine"),
                provenance: TerminalPsiProvenance::default(),
                operation: TerminalTargetOperation::ReturnIntegerConditionalControl {
                    condition_source: ValueId::new(1).expect("condition"),
                    condition_parameter_index: 0,
                    condition_location: TerminalScalarParameterLocation::Register(locations[0]),
                    scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
                    when_true: arm(1, 3, 2, 1, locations[1]),
                    when_false: arm(2, 4, 3, 2, locations[2]),
                },
            }],
        }
    }

    #[test]
    fn emits_x86_64_return_immediate() {
        let emitted = emit_machine_code(&plan(NativeTarget::linux_x64())).expect("emit");
        assert_eq!(emitted.functions[0].bytes, [0xb8, 7, 0, 0, 0, 0xc3]);
    }

    #[test]
    fn emits_aarch64_return_immediate() {
        let emitted = emit_machine_code(&plan(NativeTarget::linux_arm64())).expect("emit");
        assert_eq!(
            emitted.functions[0].bytes,
            [0xe0, 0x00, 0x80, 0x52, 0xc0, 0x03, 0x5f, 0xd6]
        );
    }

    #[test]
    fn emits_canonical_boolean_returns_for_both_architectures() {
        let boolean_plan = |target, value| TerminalTargetOperationPlan {
            terminal_psi: identity(),
            target,
            entry: MachineId::new(1).expect("machine"),
            functions: vec![TerminalTargetFunction {
                machine: MachineId::new(1).expect("machine"),
                provenance: TerminalPsiProvenance::default(),
                operation: TerminalTargetOperation::ReturnBooleanImmediate {
                    psi_edge: EdgeId::new(1).expect("edge"),
                    source_value: ValueId::new(1).expect("value"),
                    value,
                },
            }],
        };

        assert_eq!(
            emit_machine_code(&boolean_plan(NativeTarget::linux_x64(), true))
                .unwrap()
                .functions[0]
                .bytes,
            [0xb8, 1, 0, 0, 0, 0xc3]
        );
        assert_eq!(
            emit_machine_code(&boolean_plan(NativeTarget::linux_arm64(), false))
                .unwrap()
                .functions[0]
                .bytes,
            [0x00, 0x00, 0x80, 0x52, 0xc0, 0x03, 0x5f, 0xd6]
        );
    }

    #[test]
    fn emits_runtime_boolean_equality_for_both_architectures() {
        let x86 = emit_machine_code(&boolean_equality_plan(
            NativeTarget::linux_x64(),
            MachineRegister::X86Rdi,
            MachineRegister::X86Rsi,
        ))
        .unwrap();
        assert_eq!(
            x86.functions[0].bytes,
            [
                0x48, 0x89, 0xf8, // mov rax, rdi
                0x83, 0xe0, 0x01, // and eax, 1
                0x50, // push rax
                0x48, 0x89, 0xf0, // mov rax, rsi
                0x83, 0xe0, 0x01, // and eax, 1
                0x41, 0x5a, // pop r10
                0x49, 0x39, 0xc2, // cmp r10, rax
                0x0f, 0x94, 0xc0, // sete al
                0x0f, 0xb6, 0xc0, // movzx eax, al
                0xc3,
            ]
        );

        let aarch64 = emit_machine_code(&boolean_equality_plan(
            NativeTarget::linux_arm64(),
            MachineRegister::Aarch64X(0),
            MachineRegister::Aarch64X(1),
        ))
        .unwrap();
        assert_eq!(
            aarch64_instructions(&aarch64.functions[0].bytes),
            [
                0xd100_43ff, // sub sp, sp, #16
                0xf900_03e0, // str x0, [sp]
                0xf900_07e1, // str x1, [sp, #8]
                0xf940_03e0, // ldr x0, [sp]
                0x1200_0000, // and w0, w0, #1
                0xd100_43ff, // sub sp, sp, #16
                0xf900_03e0, // str x0, [sp]
                0xf940_0fe0, // ldr x0, [sp, #24]
                0x1200_0000, // and w0, w0, #1
                0xf940_03e9, // ldr x9, [sp]
                0x9100_43ff, // add sp, sp, #16
                0x6b00_013f, // cmp w9, w0
                0x1a9f_17e0, // cset w0, eq
                0x9100_43ff, // add sp, sp, #16
                0xd65f_03c0, // ret
            ]
        );
    }

    #[test]
    fn retains_ordered_linear_scalar_stack_mutations() {
        let x86 = emit_machine_code(&boolean_equality_plan(
            NativeTarget::linux_x64(),
            MachineRegister::X86Rdi,
            MachineRegister::X86Rsi,
        ))
        .expect("x86 scalar expression");
        let x86_stack = x86.functions[0]
            .scalar_stack
            .as_ref()
            .expect("linear scalar evidence");
        assert_eq!(x86_stack.stack_alignment, 16);
        assert_eq!(x86_stack.mutations.len(), 2);
        assert_eq!(
            x86_stack.mutations[0].kind,
            TerminalScalarStackMutationKind::X86Push
        );
        assert_eq!(
            x86_stack.mutations[1].kind,
            TerminalScalarStackMutationKind::X86Pop
        );

        let aarch64 = emit_machine_code(&boolean_equality_plan(
            NativeTarget::linux_arm64(),
            MachineRegister::Aarch64X(0),
            MachineRegister::Aarch64X(1),
        ))
        .expect("AArch64 scalar expression");
        let aarch64_stack = aarch64.functions[0]
            .scalar_stack
            .as_ref()
            .expect("linear scalar evidence");
        assert_eq!(
            aarch64_stack
                .mutations
                .iter()
                .map(|mutation| mutation.kind)
                .collect::<Vec<_>>(),
            [
                TerminalScalarStackMutationKind::Allocate { byte_size: 16 },
                TerminalScalarStackMutationKind::Allocate { byte_size: 16 },
                TerminalScalarStackMutationKind::Release { byte_size: 16 },
                TerminalScalarStackMutationKind::Release { byte_size: 16 },
            ]
        );
    }

    #[test]
    fn emits_runtime_u8_integer_equality_for_both_architectures() {
        let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let x86 = emit_machine_code(&integer_equality_plan(
            NativeTarget::linux_x64(),
            scalar_type,
            MachineRegister::X86Rdi,
            MachineRegister::X86Rsi,
        ))
        .unwrap();
        assert_eq!(
            x86.functions[0].bytes,
            [
                0x48, 0x89, 0xf8, // mov rax, rdi
                0x25, 0xff, 0, 0, 0,    // and eax, 0xff
                0x50, // push rax
                0x48, 0x89, 0xf0, // mov rax, rsi
                0x25, 0xff, 0, 0, 0, // and eax, 0xff
                0x41, 0x5a, // pop r10
                0x49, 0x39, 0xc2, // cmp r10, rax
                0x0f, 0x94, 0xc0, // sete al
                0x0f, 0xb6, 0xc0, // movzx eax, al
                0xc3,
            ]
        );

        let aarch64 = emit_machine_code(&integer_equality_plan(
            NativeTarget::linux_arm64(),
            scalar_type,
            MachineRegister::Aarch64X(0),
            MachineRegister::Aarch64X(1),
        ))
        .unwrap();
        assert_eq!(
            aarch64_instructions(&aarch64.functions[0].bytes),
            [
                0xd100_43ff, // sub sp, sp, #16
                0xf900_03e0, // str x0, [sp]
                0xf900_07e1, // str x1, [sp, #8]
                0xf940_03e0, // ldr x0, [sp]
                0xd340_1c00, // uxtb x0, x0
                0xd100_43ff, // sub sp, sp, #16
                0xf900_03e0, // str x0, [sp]
                0xf940_0fe0, // ldr x0, [sp, #24]
                0xd340_1c00, // uxtb x0, x0
                0xf940_03e9, // ldr x9, [sp]
                0x9100_43ff, // add sp, sp, #16
                0xeb00_013f, // cmp x9, x0
                0x1a9f_17e0, // cset w0, eq
                0x9100_43ff, // add sp, sp, #16
                0xd65f_03c0, // ret
            ]
        );
    }

    #[test]
    fn emits_exact_signed_and_unsigned_integer_ordering_conditions() {
        for (sign, inclusive, x86_setcc, aarch64_cset) in [
            (IntegerSign::Signed, false, 0x9c, 0x1a9f_a7e0),
            (IntegerSign::Unsigned, false, 0x92, 0x1a9f_27e0),
            (IntegerSign::Signed, true, 0x9e, 0x1a9f_c7e0),
            (IntegerSign::Unsigned, true, 0x96, 0x1a9f_87e0),
        ] {
            let scalar_type = IntegerType::new(sign, 8).expect("8-bit ordering type");
            let x86 = emit_machine_code(&integer_ordering_plan(
                NativeTarget::linux_x64(),
                scalar_type,
                inclusive,
                MachineRegister::X86Rdi,
                MachineRegister::X86Rsi,
            ))
            .unwrap();
            assert!(
                x86.functions[0]
                    .bytes
                    .windows(3)
                    .any(|bytes| bytes == [0x0f, x86_setcc, 0xc0]),
                "x86-64 ordering must select the exact signedness-aware condition"
            );

            let aarch64 = emit_machine_code(&integer_ordering_plan(
                NativeTarget::linux_arm64(),
                scalar_type,
                inclusive,
                MachineRegister::Aarch64X(0),
                MachineRegister::Aarch64X(1),
            ))
            .unwrap();
            assert!(
                aarch64_instructions(&aarch64.functions[0].bytes).contains(&aarch64_cset),
                "AArch64 ordering must select the exact signedness-aware condition"
            );
        }
    }

    #[test]
    fn emits_boolean_expression_conditions_for_both_architectures() {
        let x86 = emit_machine_code(&boolean_expression_conditional_plan(
            NativeTarget::linux_x64(),
            MachineRegister::X86Rdi,
            MachineRegister::X86Rsi,
        ))
        .unwrap();
        assert_eq!(x86.functions[0].scalar_stack, None);
        assert!(
            x86.functions[0]
                .bytes
                .windows(8)
                .any(|window| window == [0x0f, 0xb6, 0xc0, 0x85, 0xc0, 0x0f, 0x84, 6])
        );

        let aarch64 = emit_machine_code(&boolean_expression_conditional_plan(
            NativeTarget::linux_arm64(),
            MachineRegister::Aarch64X(0),
            MachineRegister::Aarch64X(1),
        ))
        .unwrap();
        assert_eq!(aarch64.functions[0].scalar_stack, None);
        let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
        assert!(instructions.windows(6).any(|window| window
            == [
                0x1a9f_17e0,
                0x7100_001f,
                0xf940_03e0,
                0xf940_07e1,
                0x9100_43ff,
                0x5400_0060,
            ]));
    }

    #[test]
    fn emits_and_rebases_calls_across_conditional_control() {
        for (target, argument_register) in [
            (NativeTarget::linux_x64(), MachineRegister::X86Rdi),
            (NativeTarget::linux_arm64(), MachineRegister::Aarch64X(0)),
        ] {
            let emitted = emit_machine_code(&calling_conditional_plan(target, argument_register))
                .expect("emit conditional calls");
            let caller = &emitted.functions[0];
            assert_eq!(caller.scalar_stack, None);
            assert_eq!(caller.internal_calls.len(), 3);
            assert_eq!(
                caller
                    .internal_calls
                    .iter()
                    .map(|relocation| relocation.psi_operation.get())
                    .collect::<Vec<_>>(),
                [1, 2, 3]
            );
            assert!(
                caller
                    .internal_calls
                    .windows(2)
                    .all(|pair| pair[0].offset < pair[1].offset)
            );
            for relocation in &caller.internal_calls {
                assert_eq!(relocation.target, MachineId::new(2).unwrap());
                assert_eq!(relocation.scalar_stack, None);
                match target.architecture {
                    Architecture::X86_64 => {
                        assert_eq!(caller.bytes[relocation.offset - 1], 0xe8);
                    }
                    Architecture::Aarch64 => assert_eq!(
                        &caller.bytes[relocation.offset..relocation.offset + 4],
                        &0x9400_0000_u32.to_le_bytes()
                    ),
                }
            }
            match target.architecture {
                Architecture::X86_64 => {
                    assert!(
                        caller
                            .bytes
                            .windows(5)
                            .any(|window| window == [0x48, 0x8d, 0x64, 0x24, 32])
                    );
                }
                Architecture::Aarch64 => {
                    let instructions = aarch64_instructions(&caller.bytes);
                    assert!(instructions.contains(&0xf940_03e0)); // restore x0 from outer frame
                    assert!(
                        instructions
                            .iter()
                            .any(|instruction| instruction & 0xff00_001f == 0x5400_0000)
                    ); // b.eq false arm
                }
            }
        }
    }

    #[test]
    fn emits_parameter_expression_conditionals_for_both_architectures() {
        let x86 = emit_machine_code(&conditional_plan(NativeTarget::linux_x64())).unwrap();
        assert_eq!(
            x86.functions[0].bytes,
            [
                0x89, 0xf8, // mov eax, edi
                0x85, 0xc0, // test eax, eax
                0x0f, 0x84, 9, 0, 0, 0, // jz false
                0x48, 0x89, 0xf0, // mov rax, rsi
                0x25, 0xff, 0, 0, 0, 0xc3, // mask to u8; ret
                0x48, 0x89, 0xd0, // mov rax, rdx
                0x25, 0xff, 0, 0, 0, 0xc3, // mask to u8; ret
            ]
        );
        let x86_stack = x86.functions[0]
            .scalar_stack
            .as_ref()
            .expect("top-level two-return x86 conditional stack evidence");
        assert_eq!(x86_stack.mutations, []);
        assert_eq!(
            x86_stack.control_flow,
            TerminalScalarControlFlowEvidence::TopLevelTwoReturn {
                condition: TerminalScalarConditionalCondition::Parameter,
                branch_offset: 4,
                branch_byte_count: 6,
                false_arm_offset: 19,
            }
        );
        let aarch64 = emit_machine_code(&conditional_plan(NativeTarget::linux_arm64())).unwrap();
        assert_eq!(
            aarch64_instructions(&aarch64.functions[0].bytes),
            [
                0x3400_00e0, // cbz w0, false
                0xd100_43ff, // sub sp, sp, #16
                0xf900_03e1, // str x1, [sp]
                0xf940_03e0, // ldr x0, [sp]
                0xd340_1c00, // mask to u8
                0x9100_43ff, // add sp, sp, #16
                0xd65f_03c0, // ret
                0xd100_43ff, // sub sp, sp, #16
                0xf900_03e2, // str x2, [sp]
                0xf940_03e0, // ldr x0, [sp]
                0xd340_1c00, // mask to u8
                0x9100_43ff, // add sp, sp, #16
                0xd65f_03c0, // ret
            ]
        );
        let aarch64_stack = aarch64.functions[0]
            .scalar_stack
            .as_ref()
            .expect("top-level two-return AArch64 conditional stack evidence");
        assert_eq!(
            aarch64_stack.control_flow,
            TerminalScalarControlFlowEvidence::TopLevelTwoReturn {
                condition: TerminalScalarConditionalCondition::Parameter,
                branch_offset: 0,
                branch_byte_count: 4,
                false_arm_offset: 28,
            }
        );
        assert_eq!(aarch64_stack.mutations.len(), 4);
    }

    #[test]
    fn broader_conditional_shapes_remain_outside_scalar_stack_evidence() {
        let mut division_arm = conditional_plan(NativeTarget::linux_x64());
        let TerminalTargetOperation::ReturnIntegerConditionalControl { when_true, .. } =
            &mut division_arm.functions[0].operation
        else {
            unreachable!()
        };
        let TerminalTargetIntegerControl::Return { expression, .. } = when_true.control.as_mut()
        else {
            unreachable!()
        };
        *expression = TerminalTargetIntegerExpression::WrappingDivide {
            psi_operation: OperationId::new(8).expect("divide operation"),
            left: Box::new(TerminalTargetIntegerExpression::Immediate {
                source_value: ValueId::new(8).expect("left"),
                value: IntegerValue::Unsigned(8),
            }),
            right: Box::new(TerminalTargetIntegerExpression::Immediate {
                source_value: ValueId::new(9).expect("right"),
                value: IntegerValue::Unsigned(2),
            }),
        };
        assert_eq!(
            emit_machine_code(&division_arm)
                .expect("conditional division still emits")
                .functions[0]
                .scalar_stack,
            None
        );

        let mut crash_arm = conditional_plan(NativeTarget::linux_arm64());
        let TerminalTargetOperation::ReturnIntegerConditionalControl { when_false, .. } =
            &mut crash_arm.functions[0].operation
        else {
            unreachable!()
        };
        when_false.control = Box::new(TerminalTargetIntegerControl::Crash {
            psi_crash_edge: EdgeId::new(9).expect("crash edge"),
            cause: psi_terminal::CrashCause::Trap,
            site_guard: Vec::new(),
            frontier_lower_bound: Vec::new(),
        });
        assert_eq!(
            emit_machine_code(&crash_arm)
                .expect("conditional crash still emits")
                .functions[0]
                .scalar_stack,
            None
        );
    }

    #[test]
    fn parameter_conditional_retains_typed_calls_inside_direct_linear_arms() {
        for (target, condition_register, argument_register) in [
            (
                NativeTarget::linux_x64(),
                MachineRegister::X86Rdi,
                MachineRegister::X86Rsi,
            ),
            (
                NativeTarget::linux_arm64(),
                MachineRegister::Aarch64X(0),
                MachineRegister::Aarch64X(1),
            ),
        ] {
            let emitted = emit_machine_code(&calling_arm_conditional_plan(
                target,
                condition_register,
                argument_register,
            ))
            .expect("emit conditional arm call");
            let caller = &emitted.functions[0];
            assert!(matches!(
                caller
                    .scalar_stack
                    .as_ref()
                    .expect("conditional call stack evidence")
                    .control_flow,
                TerminalScalarControlFlowEvidence::TopLevelTwoReturn { .. }
            ));
            assert_eq!(caller.internal_calls.len(), 1);
            assert!(caller.internal_calls[0].scalar_stack.is_some());
            assert_eq!(caller.internal_calls[0].target, MachineId::new(2).unwrap());
        }

        let mut excluded = calling_arm_conditional_plan(
            NativeTarget::linux_x64(),
            MachineRegister::X86Rdi,
            MachineRegister::X86Rsi,
        );
        let TerminalTargetOperation::ReturnIntegerConditionalControl { when_true, .. } =
            &mut excluded.functions[0].operation
        else {
            unreachable!()
        };
        let TerminalTargetIntegerControl::Return { expression, .. } = when_true.control.as_mut()
        else {
            unreachable!()
        };
        let TerminalTargetIntegerExpression::WrappingAdd { right, .. } = expression else {
            unreachable!()
        };
        let TerminalTargetIntegerExpression::Call { arguments, .. } = right.as_mut() else {
            unreachable!()
        };
        let TerminalTargetScalarExpression::Integer { expression, .. } =
            &mut arguments[0].expression
        else {
            unreachable!()
        };
        *expression = TerminalTargetIntegerExpression::WrappingDivide {
            psi_operation: OperationId::new(9).unwrap(),
            left: Box::new(TerminalTargetIntegerExpression::Immediate {
                source_value: ValueId::new(8).unwrap(),
                value: IntegerValue::Unsigned(8),
            }),
            right: Box::new(TerminalTargetIntegerExpression::Immediate {
                source_value: ValueId::new(9).unwrap(),
                value: IntegerValue::Unsigned(2),
            }),
        };
        assert_eq!(
            emit_machine_code(&excluded)
                .expect("excluded conditional call argument still emits")
                .functions[0]
                .scalar_stack,
            None,
            "branch-producing call arguments remain outside this slice"
        );
    }

    #[test]
    fn expression_conditional_retains_typed_call_in_linear_condition_prefix() {
        for (target, argument_register) in [
            (NativeTarget::linux_x64(), MachineRegister::X86Rdi),
            (NativeTarget::linux_arm64(), MachineRegister::Aarch64X(0)),
        ] {
            let emitted = emit_machine_code(&calling_expression_condition_plan(
                target,
                argument_register,
            ))
            .expect("emit expression-condition call");
            let caller = &emitted.functions[0];
            assert!(matches!(
                caller
                    .scalar_stack
                    .as_ref()
                    .expect("expression-condition call stack evidence")
                    .control_flow,
                TerminalScalarControlFlowEvidence::TopLevelTwoReturn {
                    condition: TerminalScalarConditionalCondition::Expression,
                    ..
                }
            ));
            assert_eq!(caller.internal_calls.len(), 1);
            assert!(caller.internal_calls[0].scalar_stack.is_some());
            if target.architecture == Architecture::X86_64 {
                assert!(caller.scalar_stack.as_ref().unwrap().mutations.iter().any(
                    |mutation| matches!(
                        mutation.kind,
                        TerminalScalarStackMutationKind::X86ReleasePreservingFlags { .. }
                    )
                ));
            }
        }
    }

    #[test]
    fn emits_selected_register_parameter_returns_for_all_native_policies() {
        assert_eq!(
            emit_machine_code(&parameter_plan(
                NativeTarget::linux_x64(),
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
                false,
            ))
            .unwrap()
            .functions[0]
                .bytes,
            [0x89, 0xf8, 0xc3]
        );
        assert_eq!(
            emit_machine_code(&parameter_plan(
                NativeTarget::windows_x64(),
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rcx),
                false,
            ))
            .unwrap()
            .functions[0]
                .bytes,
            [0x89, 0xc8, 0xc3]
        );
        assert_eq!(
            emit_machine_code(&parameter_plan(
                NativeTarget::linux_arm64(),
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                false,
            ))
            .unwrap()
            .functions[0]
                .bytes,
            [0xc0, 0x03, 0x5f, 0xd6]
        );
        assert_eq!(
            emit_machine_code(&parameter_plan(
                NativeTarget::linux_arm64(),
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
                false,
            ))
            .unwrap()
            .functions[0]
                .bytes,
            [0xe0, 0x03, 0x01, 0x2a, 0xc0, 0x03, 0x5f, 0xd6]
        );
        assert_eq!(
            emit_machine_code(&parameter_plan(
                NativeTarget::linux_x64(),
                TerminalScalarParameterLocation::Register(MachineRegister::X86R9),
                true,
            ))
            .unwrap()
            .functions[0]
                .bytes,
            [0x4c, 0x89, 0xc8, 0xc3]
        );
        assert_eq!(
            emit_machine_code(&parameter_plan(
                NativeTarget::linux_arm64(),
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(3)),
                true,
            ))
            .unwrap()
            .functions[0]
                .bytes,
            [0xe0, 0x03, 0x03, 0xaa, 0xc0, 0x03, 0x5f, 0xd6]
        );
    }

    #[test]
    fn emits_selected_incoming_stack_parameter_returns_for_both_architectures() {
        assert_eq!(
            emit_machine_code(&parameter_plan(
                NativeTarget::linux_x64(),
                TerminalScalarParameterLocation::IncomingStack { byte_offset: 16 },
                false,
            ))
            .unwrap()
            .functions[0]
                .bytes,
            [0x8b, 0x44, 0x24, 24, 0xc3]
        );
        assert_eq!(
            emit_machine_code(&parameter_plan(
                NativeTarget::linux_arm64(),
                TerminalScalarParameterLocation::IncomingStack { byte_offset: 0 },
                false,
            ))
            .unwrap()
            .functions[0]
                .bytes,
            [0xe0, 0x03, 0x40, 0xb9, 0xc0, 0x03, 0x5f, 0xd6]
        );
        assert_eq!(
            emit_machine_code(&parameter_plan(
                NativeTarget::linux_x64(),
                TerminalScalarParameterLocation::IncomingStack { byte_offset: 16 },
                true,
            ))
            .unwrap()
            .functions[0]
                .bytes,
            [0x48, 0x8b, 0x44, 0x24, 24, 0xc3]
        );
        assert_eq!(
            emit_machine_code(&parameter_plan(
                NativeTarget::linux_arm64(),
                TerminalScalarParameterLocation::IncomingStack { byte_offset: 0 },
                true,
            ))
            .unwrap()
            .functions[0]
                .bytes,
            [0xe0, 0x03, 0x40, 0xf9, 0xc0, 0x03, 0x5f, 0xd6]
        );
    }

    #[test]
    fn emits_a_canonical_boolean_parameter_return() {
        let mut plan = parameter_plan(
            NativeTarget::linux_x64(),
            TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
            false,
        );
        plan.functions[0].operation = TerminalTargetOperation::ReturnBooleanParameter {
            psi_edge: EdgeId::new(1).expect("edge"),
            source_value: ValueId::new(1).expect("value"),
            parameter_index: 0,
            location: TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
        };
        assert_eq!(
            emit_machine_code(&plan).unwrap().functions[0].bytes,
            [0x89, 0xf8, 0xc3]
        );
    }

    #[test]
    fn emits_boolean_not_parameter_returns_for_both_architectures() {
        let mut x86 = parameter_plan(
            NativeTarget::linux_x64(),
            TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
            false,
        );
        x86.functions[0].operation = TerminalTargetOperation::ReturnBooleanNotParameter {
            psi_edge: EdgeId::new(1).expect("edge"),
            source_value: ValueId::new(1).expect("value"),
            parameter_index: 0,
            location: TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
        };
        assert_eq!(
            emit_machine_code(&x86).unwrap().functions[0].bytes,
            [0x89, 0xf8, 0x83, 0xf0, 0x01, 0xc3]
        );

        let mut aarch64 = parameter_plan(
            NativeTarget::linux_arm64(),
            TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
            false,
        );
        aarch64.functions[0].operation = TerminalTargetOperation::ReturnBooleanNotParameter {
            psi_edge: EdgeId::new(1).expect("edge"),
            source_value: ValueId::new(1).expect("value"),
            parameter_index: 0,
            location: TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
        };
        assert_eq!(
            aarch64_instructions(&emit_machine_code(&aarch64).unwrap().functions[0].bytes),
            [0x5200_0000, 0xd65f_03c0]
        );
    }

    #[test]
    fn emits_parameter_fed_wrapping_add_for_both_architectures() {
        let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let x86 = emit_machine_code(&expression_plan(
            NativeTarget::linux_x64(),
            scalar_type,
            wrapping_expression(
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rsi),
            ),
        ))
        .unwrap();
        assert_eq!(
            x86.functions[0].bytes,
            [
                0x48, 0x89, 0xf8, 0x25, 0xff, 0, 0, 0, 0x50, 0x48, 0x89, 0xf0, 0x25, 0xff, 0, 0, 0,
                0x41, 0x5a, 0x4c, 0x01, 0xd0, 0x25, 0xff, 0, 0, 0, 0xc3,
            ]
        );

        let aarch64 = emit_machine_code(&expression_plan(
            NativeTarget::linux_arm64(),
            scalar_type,
            wrapping_expression(
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
            ),
        ))
        .unwrap();
        assert_eq!(
            aarch64_instructions(&aarch64.functions[0].bytes),
            [
                0xd100_43ff,
                0xf900_03e0,
                0xf900_07e1,
                0xf940_03e0,
                0xd340_1c00,
                0xd100_43ff,
                0xf900_03e0,
                0xf940_0fe0,
                0xd340_1c00,
                0xf940_03e9,
                0x9100_43ff,
                0x8b00_0120,
                0xd340_1c00,
                0x9100_43ff,
                0xd65f_03c0,
            ]
        );
    }

    #[test]
    fn emits_exact_parameter_fed_bitwise_instructions_for_both_architectures() {
        let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        for (kind, x86_opcode, aarch64_opcode) in [
            (0_u8, 0x21_u8, 0x8a00_0120_u32),
            (1, 0x09, 0xaa00_0120),
            (2, 0x31, 0xca00_0120),
        ] {
            let x86 = emit_machine_code(&expression_plan(
                NativeTarget::linux_x64(),
                scalar_type,
                bitwise_expression(
                    kind,
                    TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
                    TerminalScalarParameterLocation::Register(MachineRegister::X86Rsi),
                ),
            ))
            .expect("x86-64 bitwise expression emits");
            let bytes = &x86.functions[0].bytes;
            assert!(
                bytes
                    .windows(3)
                    .any(|window| window == [0x4c, x86_opcode, 0xd0])
            );

            let aarch64 = emit_machine_code(&expression_plan(
                NativeTarget::linux_arm64(),
                scalar_type,
                bitwise_expression(
                    kind,
                    TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                    TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
                ),
            ))
            .expect("AArch64 bitwise expression emits");
            assert!(
                aarch64_instructions(&aarch64.functions[0].bytes).contains(&aarch64_opcode),
                "bitwise kind {kind} must retain its exact AArch64 instruction"
            );
        }
    }

    #[test]
    fn emits_modulo_count_wrapping_shifts_for_both_architectures() {
        let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
        let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
        for (left_shift, value_type, x86_opcode, aarch64_opcode) in [
            (true, u64_type, [0x49_u8, 0xd3, 0xe2], 0x9ac0_2120_u32),
            (false, u64_type, [0x49, 0xd3, 0xea], 0x9ac0_2520),
            (false, i64_type, [0x49, 0xd3, 0xfa], 0x9ac0_2920),
        ] {
            let x86 = emit_machine_code(&expression_plan(
                NativeTarget::linux_x64(),
                value_type,
                shift_expression(
                    left_shift,
                    i64_type,
                    TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
                    TerminalScalarParameterLocation::Register(MachineRegister::X86Rsi),
                ),
            ))
            .expect("x86-64 wrapping shift emits");
            let bytes = &x86.functions[0].bytes;
            assert!(bytes.windows(3).any(|window| window == [0x83, 0xe1, 63]));
            assert!(bytes.windows(3).any(|window| window == x86_opcode));

            let aarch64 = emit_machine_code(&expression_plan(
                NativeTarget::linux_arm64(),
                value_type,
                shift_expression(
                    left_shift,
                    i64_type,
                    TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                    TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
                ),
            ))
            .expect("AArch64 wrapping shift emits");
            let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
            assert!(instructions.contains(&(0x9240_0000 | (5 << 10))));
            assert!(instructions.contains(&aarch64_opcode));
        }
    }

    #[test]
    fn emits_x86_expression_after_assignment_spills_a_scratch_conflict() {
        let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let emitted = emit_machine_code(&expression_plan(
            NativeTarget::linux_x64(),
            scalar_type,
            wrapping_expression(
                TerminalScalarParameterLocation::Register(MachineRegister::X86R10),
                TerminalScalarParameterLocation::IncomingStack { byte_offset: 0 },
            ),
        ))
        .expect("assigned scratch conflict should emit");
        let bytes = &emitted.functions[0].bytes;
        assert_eq!(&bytes[..4], &[0x48, 0x83, 0xec, 16]); // sub rsp, frame
        assert_eq!(&bytes[4..9], &[0x4c, 0x89, 0x54, 0x24, 0]); // spill r10
        assert!(
            bytes
                .windows(5)
                .any(|window| window == [0x48, 0x8b, 0x44, 0x24, 32])
        ); // frame + return + expression push
        assert_eq!(&bytes[bytes.len() - 5..], &[0x48, 0x83, 0xc4, 16, 0xc3]);
    }

    #[test]
    fn emits_parameter_fed_wrapping_subtract_for_both_architectures() {
        let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let expression = |left, right| TerminalTargetIntegerExpression::WrappingSubtract {
            psi_operation: OperationId::new(3).expect("operation"),
            left: Box::new(TerminalTargetIntegerExpression::Parameter {
                source_value: ValueId::new(1).expect("left"),
                parameter_index: 0,
                location: left,
            }),
            right: Box::new(TerminalTargetIntegerExpression::Parameter {
                source_value: ValueId::new(2).expect("right"),
                parameter_index: 1,
                location: right,
            }),
        };
        let x86 = emit_machine_code(&expression_plan(
            NativeTarget::linux_x64(),
            scalar_type,
            expression(
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rsi),
            ),
        ))
        .unwrap();
        assert_eq!(
            x86.functions[0].bytes,
            [
                0x48, 0x89, 0xf8, 0x25, 0xff, 0, 0, 0, 0x50, 0x48, 0x89, 0xf0, 0x25, 0xff, 0, 0, 0,
                0x41, 0x5a, 0x49, 0x29, 0xc2, 0x4c, 0x89, 0xd0, 0x25, 0xff, 0, 0, 0, 0xc3,
            ]
        );

        let aarch64 = emit_machine_code(&expression_plan(
            NativeTarget::linux_arm64(),
            scalar_type,
            expression(
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
            ),
        ))
        .unwrap();
        let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
        assert!(instructions.contains(&0xcb00_0120)); // sub x0, x9, x0
        assert_eq!(instructions.last(), Some(&0xd65f_03c0));
    }

    #[test]
    fn emits_parameter_fed_wrapping_multiply_for_both_architectures() {
        let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let expression = |left, right| TerminalTargetIntegerExpression::WrappingMultiply {
            psi_operation: OperationId::new(3).expect("operation"),
            left: Box::new(TerminalTargetIntegerExpression::Parameter {
                source_value: ValueId::new(1).expect("left"),
                parameter_index: 0,
                location: left,
            }),
            right: Box::new(TerminalTargetIntegerExpression::Parameter {
                source_value: ValueId::new(2).expect("right"),
                parameter_index: 1,
                location: right,
            }),
        };
        let x86 = emit_machine_code(&expression_plan(
            NativeTarget::linux_x64(),
            scalar_type,
            expression(
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rsi),
            ),
        ))
        .unwrap();
        assert_eq!(
            x86.functions[0].bytes,
            [
                0x48, 0x89, 0xf8, 0x25, 0xff, 0, 0, 0, 0x50, 0x48, 0x89, 0xf0, 0x25, 0xff, 0, 0, 0,
                0x41, 0x5a, 0x49, 0x0f, 0xaf, 0xc2, 0x25, 0xff, 0, 0, 0, 0xc3,
            ]
        );

        let aarch64 = emit_machine_code(&expression_plan(
            NativeTarget::linux_arm64(),
            scalar_type,
            expression(
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
            ),
        ))
        .unwrap();
        let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
        assert!(instructions.contains(&0x9b00_7d20)); // mul x0, x9, x0
        assert_eq!(instructions.last(), Some(&0xd65f_03c0));
    }

    #[test]
    fn emits_parameter_fed_exact_divide_for_both_architectures() {
        for (sign, x86_opcode, aarch64_opcode) in [
            (IntegerSign::Unsigned, [0x48, 0xf7, 0x34], 0x9ac0_0920),
            (IntegerSign::Signed, [0x48, 0xf7, 0x3c], 0x9ac0_0d20),
        ] {
            let scalar_type = IntegerType::new(sign, 64).expect("64-bit integer");
            let expression = |left, right| TerminalTargetIntegerExpression::ExactDivide {
                psi_operation: OperationId::new(4).expect("operation"),
                left: Box::new(TerminalTargetIntegerExpression::Parameter {
                    source_value: ValueId::new(1).expect("left"),
                    parameter_index: 0,
                    location: left,
                }),
                right: Box::new(TerminalTargetIntegerExpression::Parameter {
                    source_value: ValueId::new(2).expect("right"),
                    parameter_index: 1,
                    location: right,
                }),
            };
            let x86 = emit_machine_code(&expression_plan(
                NativeTarget::linux_x64(),
                scalar_type,
                expression(
                    TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
                    TerminalScalarParameterLocation::Register(MachineRegister::X86Rsi),
                ),
            ))
            .expect("x86-64 exact divide emits");
            assert!(
                x86.functions[0]
                    .bytes
                    .windows(x86_opcode.len())
                    .any(|window| window == x86_opcode)
            );

            let aarch64 = emit_machine_code(&expression_plan(
                NativeTarget::linux_arm64(),
                scalar_type,
                expression(
                    TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                    TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
                ),
            ))
            .expect("AArch64 exact divide emits");
            let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
            assert!(instructions.contains(&aarch64_opcode));
            assert_eq!(instructions.last(), Some(&0xd65f_03c0));
        }
    }

    #[test]
    fn emits_parameter_fed_exact_remainder_for_both_architectures() {
        for (sign, x86_opcode, aarch64_divide) in [
            (IntegerSign::Unsigned, [0x48, 0xf7, 0x34], 0x9ac0_092a),
            (IntegerSign::Signed, [0x48, 0xf7, 0x3c], 0x9ac0_0d2a),
        ] {
            let scalar_type = IntegerType::new(sign, 64).expect("64-bit integer");
            let expression = |left, right| TerminalTargetIntegerExpression::ExactRemainder {
                psi_operation: OperationId::new(5).expect("operation"),
                left: Box::new(TerminalTargetIntegerExpression::Parameter {
                    source_value: ValueId::new(1).expect("left"),
                    parameter_index: 0,
                    location: left,
                }),
                right: Box::new(TerminalTargetIntegerExpression::Parameter {
                    source_value: ValueId::new(2).expect("right"),
                    parameter_index: 1,
                    location: right,
                }),
            };
            let x86 = emit_machine_code(&expression_plan(
                NativeTarget::linux_x64(),
                scalar_type,
                expression(
                    TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
                    TerminalScalarParameterLocation::Register(MachineRegister::X86Rsi),
                ),
            ))
            .expect("x86-64 exact remainder emits");
            assert!(
                x86.functions[0]
                    .bytes
                    .windows(x86_opcode.len())
                    .any(|window| window == x86_opcode)
            );
            assert!(
                x86.functions[0]
                    .bytes
                    .windows(3)
                    .any(|window| window == [0x48, 0x89, 0xd0])
            ); // mov rax, rdx

            let aarch64 = emit_machine_code(&expression_plan(
                NativeTarget::linux_arm64(),
                scalar_type,
                expression(
                    TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                    TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
                ),
            ))
            .expect("AArch64 exact remainder emits");
            let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
            assert!(instructions.contains(&aarch64_divide));
            assert!(instructions.contains(&0x9b00_a540)); // msub x0, x10, x0, x9
            assert_eq!(instructions.last(), Some(&0xd65f_03c0));
        }
    }

    #[test]
    fn emits_parameter_fed_wrapping_divide_for_both_architectures() {
        for (sign, x86_opcode, aarch64_opcode) in [
            (IntegerSign::Unsigned, [0x48, 0xf7, 0x34], 0x9ac0_0920),
            (IntegerSign::Signed, [0x48, 0xf7, 0x3c], 0x9ac0_0d20),
        ] {
            let scalar_type = IntegerType::new(sign, 64).expect("64-bit integer");
            let expression = |left, right| TerminalTargetIntegerExpression::WrappingDivide {
                psi_operation: OperationId::new(6).expect("operation"),
                left: Box::new(TerminalTargetIntegerExpression::Parameter {
                    source_value: ValueId::new(1).expect("left"),
                    parameter_index: 0,
                    location: left,
                }),
                right: Box::new(TerminalTargetIntegerExpression::Parameter {
                    source_value: ValueId::new(2).expect("right"),
                    parameter_index: 1,
                    location: right,
                }),
            };
            let x86 = emit_machine_code(&expression_plan(
                NativeTarget::linux_x64(),
                scalar_type,
                expression(
                    TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
                    TerminalScalarParameterLocation::Register(MachineRegister::X86Rsi),
                ),
            ))
            .expect("x86-64 wrapping divide emits");
            assert!(
                x86.functions[0]
                    .bytes
                    .windows(x86_opcode.len())
                    .any(|window| window == x86_opcode)
            );
            if sign == IntegerSign::Signed {
                assert!(
                    x86.functions[0]
                        .bytes
                        .windows(5)
                        .any(|window| window == [0x48, 0x83, 0x3c, 0x24, 0xff])
                );
                assert!(
                    x86.functions[0]
                        .bytes
                        .windows(3)
                        .any(|window| window == [0x48, 0xf7, 0xd8])
                );
            }

            let aarch64 = emit_machine_code(&expression_plan(
                NativeTarget::linux_arm64(),
                scalar_type,
                expression(
                    TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                    TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
                ),
            ))
            .expect("AArch64 wrapping divide emits");
            let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
            assert!(instructions.contains(&aarch64_opcode));
            assert_eq!(instructions.last(), Some(&0xd65f_03c0));
        }
    }

    #[test]
    fn emits_parameter_fed_wrapping_remainder_for_both_architectures() {
        for (sign, x86_opcode, aarch64_opcode) in [
            (IntegerSign::Unsigned, [0x48, 0xf7, 0x34], 0x9ac0_092a),
            (IntegerSign::Signed, [0x48, 0xf7, 0x3c], 0x9ac0_0d2a),
        ] {
            let scalar_type = IntegerType::new(sign, 64).expect("64-bit integer");
            let expression = |left, right| TerminalTargetIntegerExpression::WrappingRemainder {
                psi_operation: OperationId::new(7).expect("operation"),
                left: Box::new(TerminalTargetIntegerExpression::Parameter {
                    source_value: ValueId::new(1).expect("left"),
                    parameter_index: 0,
                    location: left,
                }),
                right: Box::new(TerminalTargetIntegerExpression::Parameter {
                    source_value: ValueId::new(2).expect("right"),
                    parameter_index: 1,
                    location: right,
                }),
            };
            let x86 = emit_machine_code(&expression_plan(
                NativeTarget::linux_x64(),
                scalar_type,
                expression(
                    TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
                    TerminalScalarParameterLocation::Register(MachineRegister::X86Rsi),
                ),
            ))
            .expect("x86-64 wrapping remainder emits");
            assert!(
                x86.functions[0]
                    .bytes
                    .windows(x86_opcode.len())
                    .any(|window| window == x86_opcode)
            );
            assert!(
                x86.functions[0]
                    .bytes
                    .windows(3)
                    .any(|window| window == [0x48, 0x89, 0xd0])
            );
            if sign == IntegerSign::Signed {
                assert!(
                    x86.functions[0]
                        .bytes
                        .windows(5)
                        .any(|window| window == [0x48, 0x83, 0x3c, 0x24, 0xff])
                );
                assert!(
                    x86.functions[0]
                        .bytes
                        .windows(2)
                        .any(|window| window == [0x31, 0xc0])
                );
            }

            let aarch64 = emit_machine_code(&expression_plan(
                NativeTarget::linux_arm64(),
                scalar_type,
                expression(
                    TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                    TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
                ),
            ))
            .expect("AArch64 wrapping remainder emits");
            let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
            assert!(instructions.contains(&aarch64_opcode));
            assert!(instructions.contains(&0x9b00_a540));
            assert_eq!(instructions.last(), Some(&0xd65f_03c0));
        }
    }

    #[test]
    fn emits_parameter_fed_saturating_divide_for_both_architectures() {
        let scalar_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
        let expression = |left, right| TerminalTargetIntegerExpression::SaturatingDivide {
            psi_operation: OperationId::new(8).expect("operation"),
            left: Box::new(TerminalTargetIntegerExpression::Parameter {
                source_value: ValueId::new(1).expect("left"),
                parameter_index: 0,
                location: left,
            }),
            right: Box::new(TerminalTargetIntegerExpression::Parameter {
                source_value: ValueId::new(2).expect("right"),
                parameter_index: 1,
                location: right,
            }),
        };
        let x86 = emit_machine_code(&expression_plan(
            NativeTarget::linux_x64(),
            scalar_type,
            expression(
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rsi),
            ),
        ))
        .expect("x86-64 saturating divide emits");
        assert!(
            x86.functions[0]
                .bytes
                .windows(3)
                .any(|window| window == [0x49, 0x0f, 0x40])
        ); // cmovo
        assert!(
            x86.functions[0]
                .bytes
                .windows(5)
                .any(|window| window == [0x48, 0x83, 0x3c, 0x24, 0xff])
        );

        let aarch64 = emit_machine_code(&expression_plan(
            NativeTarget::linux_arm64(),
            scalar_type,
            expression(
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
            ),
        ))
        .expect("AArch64 saturating divide emits");
        let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
        assert!(instructions.contains(&0x9ac0_0d2a)); // sdiv x10, x9, x0
        assert!(instructions.contains(&aarch64_csel(0, 11, 10, 0)));
        assert_eq!(instructions.last(), Some(&0xd65f_03c0));
    }

    #[test]
    fn emits_parameter_fed_saturating_multiply_for_both_architectures() {
        let scalar_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
        let expression = |left, right| TerminalTargetIntegerExpression::SaturatingMultiply {
            psi_operation: OperationId::new(3).expect("operation"),
            left: Box::new(TerminalTargetIntegerExpression::Parameter {
                source_value: ValueId::new(1).expect("left"),
                parameter_index: 0,
                location: left,
            }),
            right: Box::new(TerminalTargetIntegerExpression::Parameter {
                source_value: ValueId::new(2).expect("right"),
                parameter_index: 1,
                location: right,
            }),
        };
        let x86 = emit_machine_code(&expression_plan(
            NativeTarget::linux_x64(),
            scalar_type,
            expression(
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rsi),
            ),
        ))
        .unwrap();
        assert!(
            x86.functions[0]
                .bytes
                .windows(3)
                .any(|window| window == [0x49, 0xf7, 0xea])
        ); // imul r10 -> rdx:rax
        assert!(
            x86.functions[0]
                .bytes
                .windows(4)
                .any(|window| window == [0x49, 0x0f, 0x40, 0xc3])
        ); // cmovo rax, r11

        let aarch64 = emit_machine_code(&expression_plan(
            NativeTarget::linux_arm64(),
            scalar_type,
            expression(
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
            ),
        ))
        .unwrap();
        let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
        assert!(instructions.contains(&0x9b40_7d2a)); // smulh x10, x9, x0
        assert!(instructions.contains(&0x9b00_7d20)); // mul x0, x9, x0
        assert_eq!(instructions.last(), Some(&0xd65f_03c0));
    }

    #[test]
    fn emits_parameter_fed_saturating_subtract_for_both_architectures() {
        let expression = |left, right| TerminalTargetIntegerExpression::SaturatingSubtract {
            psi_operation: OperationId::new(3).expect("operation"),
            left: Box::new(TerminalTargetIntegerExpression::Parameter {
                source_value: ValueId::new(1).expect("left"),
                parameter_index: 0,
                location: left,
            }),
            right: Box::new(TerminalTargetIntegerExpression::Parameter {
                source_value: ValueId::new(2).expect("right"),
                parameter_index: 1,
                location: right,
            }),
        };
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let x86 = emit_machine_code(&expression_plan(
            NativeTarget::linux_x64(),
            u8_type,
            expression(
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rsi),
            ),
        ))
        .unwrap();
        assert!(
            x86.functions[0].bytes.windows(12).any(
                |window| window == [0x49, 0x29, 0xc2, 0xb8, 0, 0, 0, 0, 0x49, 0x0f, 0x43, 0xc2]
            )
        );

        let aarch64 = emit_machine_code(&expression_plan(
            NativeTarget::linux_arm64(),
            u8_type,
            expression(
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
            ),
        ))
        .unwrap();
        let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
        assert!(instructions.contains(&0xeb00_0129)); // subs x9, x9, x0
        assert!(instructions.contains(&aarch64_csel(0, 9, 31, 2))); // cs

        let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
        let x86 = emit_machine_code(&expression_plan(
            NativeTarget::linux_x64(),
            i64_type,
            expression(
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rsi),
            ),
        ))
        .unwrap();
        assert!(
            x86.functions[0]
                .bytes
                .windows(4)
                .any(|window| window == [0x49, 0x0f, 0x40, 0xc3])
        ); // cmovo

        let aarch64 = emit_machine_code(&expression_plan(
            NativeTarget::linux_arm64(),
            i64_type,
            expression(
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
            ),
        ))
        .unwrap();
        let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
        assert!(instructions.contains(&0xeb00_0120)); // subs x0, x9, x0
        assert!(instructions.contains(&aarch64_csel(0, 0, 10, 7))); // vc
    }

    #[test]
    fn runtime_expression_stack_loads_retain_the_incoming_stack_base() {
        let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let x86 = emit_machine_code(&expression_plan(
            NativeTarget::linux_x64(),
            scalar_type,
            wrapping_expression(
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
                TerminalScalarParameterLocation::IncomingStack { byte_offset: 0 },
            ),
        ))
        .unwrap();
        assert!(
            x86.functions[0]
                .bytes
                .windows(5)
                .any(|window| window == [0x48, 0x8b, 0x44, 0x24, 16])
        );

        let aarch64 = emit_machine_code(&expression_plan(
            NativeTarget::linux_arm64(),
            scalar_type,
            wrapping_expression(
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                TerminalScalarParameterLocation::IncomingStack { byte_offset: 0 },
            ),
        ))
        .unwrap();
        assert!(aarch64_instructions(&aarch64.functions[0].bytes).contains(&0xf940_13e0));
    }

    #[test]
    fn emits_signed_i64_saturation_for_both_architectures() {
        let scalar_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
        let expression = |left, right| TerminalTargetIntegerExpression::SaturatingAdd {
            psi_operation: OperationId::new(3).expect("operation"),
            left: Box::new(TerminalTargetIntegerExpression::Parameter {
                source_value: ValueId::new(1).expect("left"),
                parameter_index: 0,
                location: left,
            }),
            right: Box::new(TerminalTargetIntegerExpression::Parameter {
                source_value: ValueId::new(2).expect("right"),
                parameter_index: 1,
                location: right,
            }),
        };
        let x86 = emit_machine_code(&expression_plan(
            NativeTarget::linux_x64(),
            scalar_type,
            expression(
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rsi),
            ),
        ))
        .unwrap();
        let x86_bytes = &x86.functions[0].bytes;
        assert!(
            x86_bytes
                .windows(5)
                .any(|window| window == [0x49, 0x0f, 0xba, 0xfb, 0x3f])
        );
        assert!(
            x86_bytes
                .windows(4)
                .any(|window| window == [0x49, 0x0f, 0x40, 0xc3])
        );

        let aarch64 = emit_machine_code(&expression_plan(
            NativeTarget::linux_arm64(),
            scalar_type,
            expression(
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
            ),
        ))
        .unwrap();
        let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
        assert!(instructions.contains(&0x937f_fd2a)); // asr x10, x9, 63
        assert!(instructions.contains(&0xca0b_014a)); // eor x10, x10, x11
        assert!(instructions.contains(&0xab00_0120)); // adds x0, x9, x0
        assert!(instructions.contains(&aarch64_csel(0, 0, 10, 7))); // vc
    }

    #[test]
    fn emits_typed_direct_call_relocations_for_native_targets() {
        let scalar_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
        for (target, argument_register, stack_byte_offset) in [
            (NativeTarget::linux_x64(), MachineRegister::X86Rdi, 0),
            (NativeTarget::windows_x64(), MachineRegister::X86Rcx, 32),
            (NativeTarget::linux_arm64(), MachineRegister::Aarch64X(0), 0),
        ] {
            let caller = MachineId::new(1).expect("caller");
            let callee = MachineId::new(2).expect("callee");
            let call_operation = OperationId::new(3).expect("call operation");
            let call_result = ValueId::new(4).expect("call result");
            let argument = ValueId::new(5).expect("argument");
            let plan = TerminalTargetOperationPlan {
                terminal_psi: identity(),
                target,
                entry: caller,
                functions: vec![
                    TerminalTargetFunction {
                        machine: caller,
                        provenance: TerminalPsiProvenance::default(),
                        operation: TerminalTargetOperation::ReturnIntegerExpression {
                            psi_edge: EdgeId::new(1).expect("return edge"),
                            source_value: call_result,
                            scalar_type,
                            expression: TerminalTargetIntegerExpression::WrappingAdd {
                                psi_operation: OperationId::new(7).expect("add operation"),
                                left: Box::new(TerminalTargetIntegerExpression::Immediate {
                                    source_value: ValueId::new(7).expect("pending left value"),
                                    value: IntegerValue::Unsigned(1),
                                }),
                                right: Box::new(TerminalTargetIntegerExpression::Call {
                                    psi_operation: call_operation,
                                    source_value: call_result,
                                    callee,
                                    arguments: vec![
                                        TerminalTargetCallArgument {
                                            scalar_type: psi_core::ScalarType::Integer(scalar_type),
                                            location: TerminalScalarParameterLocation::Register(
                                                argument_register,
                                            ),
                                            expression: TerminalTargetScalarExpression::Integer {
                                                scalar_type,
                                                expression:
                                                    TerminalTargetIntegerExpression::Immediate {
                                                        source_value: argument,
                                                        value: IntegerValue::Unsigned(7),
                                                    },
                                            },
                                        },
                                        TerminalTargetCallArgument {
                                            scalar_type: psi_core::ScalarType::Integer(scalar_type),
                                            location:
                                                TerminalScalarParameterLocation::IncomingStack {
                                                    byte_offset: stack_byte_offset,
                                                },
                                            expression: TerminalTargetScalarExpression::Integer {
                                                scalar_type,
                                                expression:
                                                    TerminalTargetIntegerExpression::Immediate {
                                                        source_value: ValueId::new(6)
                                                            .expect("stack argument"),
                                                        value: IntegerValue::Unsigned(9),
                                                    },
                                            },
                                        },
                                    ],
                                }),
                            },
                        },
                    },
                    TerminalTargetFunction {
                        machine: callee,
                        provenance: TerminalPsiProvenance::default(),
                        operation: TerminalTargetOperation::ReturnIntegerParameter {
                            psi_edge: EdgeId::new(2).expect("callee return edge"),
                            source_value: argument,
                            scalar_type,
                            parameter_index: 0,
                            location: TerminalScalarParameterLocation::Register(argument_register),
                        },
                    },
                ],
            };
            let emitted = emit_machine_code(&plan).expect("emit direct call");
            let caller = &emitted.functions[0];
            assert!(caller.scalar_stack.is_some());
            assert_eq!(caller.internal_calls.len(), 1);
            let relocation = caller.internal_calls[0];
            assert_eq!(relocation.psi_operation, call_operation);
            assert_eq!(relocation.target, callee);
            let call_stack = relocation
                .scalar_stack
                .expect("linear scalar call stack evidence");
            assert_eq!(relocation.unit_stack, None);
            match target.architecture {
                Architecture::X86_64 => {
                    assert_eq!(call_stack.aarch64_return_link, None);
                    assert!(call_stack.outbound.is_some());
                    assert_eq!(caller.bytes[relocation.offset - 1], 0xe8);
                    assert_eq!(
                        &caller.bytes[relocation.offset..relocation.offset + 4],
                        &[0; 4]
                    );
                    assert!(caller.bytes.windows(5).any(|window| {
                        window
                            == [
                                0x48,
                                0x89,
                                0x44,
                                0x24,
                                u8::try_from(stack_byte_offset).unwrap(),
                            ]
                    }));
                    if target.object_format == ObjectFormat::Coff {
                        assert_eq!(call_stack.outbound.expect("COFF outbound").byte_size, 48);
                        assert!(
                            caller
                                .bytes
                                .windows(4)
                                .any(|window| window == [0x48, 0x83, 0xec, 48])
                        );
                    } else {
                        assert_eq!(call_stack.outbound.expect("SysV outbound").byte_size, 16);
                        assert!(
                            caller
                                .bytes
                                .windows(4)
                                .any(|window| window == [0x48, 0x83, 0xec, 16])
                        );
                    }
                }
                Architecture::Aarch64 => {
                    assert!(call_stack.outbound.is_some());
                    assert!(call_stack.aarch64_return_link.is_some());
                    assert_eq!(
                        &caller.bytes[relocation.offset..relocation.offset + 4],
                        &0x9400_0000_u32.to_le_bytes()
                    );
                    let instructions = aarch64_instructions(&caller.bytes);
                    assert!(instructions.contains(&0xf900_0bfe)); // str x30, [sp, #16]
                    assert!(instructions.contains(&0xf940_0bfe)); // ldr x30, [sp, #16]
                    assert!(instructions.contains(&0xf900_03e0)); // str x0, [sp]
                }
            }
        }
    }

    #[test]
    fn branch_producing_division_stays_outside_linear_scalar_stack_evidence() {
        let scalar_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
        let expression = |left, right| TerminalTargetIntegerExpression::WrappingDivide {
            psi_operation: OperationId::new(3).expect("operation"),
            left: Box::new(TerminalTargetIntegerExpression::Parameter {
                source_value: ValueId::new(1).expect("left"),
                parameter_index: 0,
                location: left,
            }),
            right: Box::new(TerminalTargetIntegerExpression::Parameter {
                source_value: ValueId::new(2).expect("right"),
                parameter_index: 1,
                location: right,
            }),
        };
        for (target, left, right) in [
            (
                NativeTarget::linux_x64(),
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rsi),
            ),
            (
                NativeTarget::linux_arm64(),
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
            ),
        ] {
            let emitted = emit_machine_code(&expression_plan(
                target,
                scalar_type,
                expression(left, right),
            ))
            .expect("division still emits outside linear WCSU");
            assert_eq!(emitted.functions[0].scalar_stack, None);
        }
    }

    #[test]
    fn rejects_integer_width_without_a_native_scalar_realization() {
        let mut plan = plan(NativeTarget::linux_x64());
        let TerminalTargetOperation::ReturnIntegerImmediate {
            scalar_type, value, ..
        } = &mut plan.functions[0].operation
        else {
            panic!("integer fixture must contain an integer return")
        };
        *scalar_type = IntegerType::new(IntegerSign::Signed, 128).expect("i128");
        *value = IntegerValue::Signed(7);
        assert!(matches!(
            emit_machine_code(&plan),
            Err(EmissionError::IntegerWidthNotNativelySupported { bits: 128, .. })
        ));
    }

    fn parameter_plan(
        target: NativeTarget,
        location: TerminalScalarParameterLocation,
        is_64: bool,
    ) -> TerminalTargetOperationPlan {
        let scalar_type = IntegerType::new(IntegerSign::Unsigned, if is_64 { 64 } else { 8 })
            .expect("integer type");
        TerminalTargetOperationPlan {
            terminal_psi: identity(),
            target,
            entry: MachineId::new(1).expect("machine"),
            functions: vec![TerminalTargetFunction {
                machine: MachineId::new(1).expect("machine"),
                provenance: TerminalPsiProvenance::default(),
                operation: TerminalTargetOperation::ReturnIntegerParameter {
                    psi_edge: EdgeId::new(1).expect("edge"),
                    source_value: ValueId::new(1).expect("value"),
                    scalar_type,
                    parameter_index: 0,
                    location,
                },
            }],
        }
    }

    fn expression_plan(
        target: NativeTarget,
        scalar_type: IntegerType,
        expression: TerminalTargetIntegerExpression,
    ) -> TerminalTargetOperationPlan {
        TerminalTargetOperationPlan {
            terminal_psi: identity(),
            target,
            entry: MachineId::new(1).expect("machine"),
            functions: vec![TerminalTargetFunction {
                machine: MachineId::new(1).expect("machine"),
                provenance: TerminalPsiProvenance::default(),
                operation: TerminalTargetOperation::ReturnIntegerExpression {
                    psi_edge: EdgeId::new(1).expect("edge"),
                    source_value: ValueId::new(3).expect("result"),
                    scalar_type,
                    expression,
                },
            }],
        }
    }

    fn boolean_equality_plan(
        target: NativeTarget,
        left_register: MachineRegister,
        right_register: MachineRegister,
    ) -> TerminalTargetOperationPlan {
        TerminalTargetOperationPlan {
            terminal_psi: identity(),
            target,
            entry: MachineId::new(1).expect("machine"),
            functions: vec![TerminalTargetFunction {
                machine: MachineId::new(1).expect("machine"),
                provenance: TerminalPsiProvenance::default(),
                operation: TerminalTargetOperation::ReturnBooleanExpression {
                    psi_edge: EdgeId::new(1).expect("edge"),
                    source_value: ValueId::new(3).expect("result"),
                    expression: TerminalTargetBooleanExpression::Equal {
                        psi_operation: OperationId::new(1).expect("operation"),
                        left: Box::new(TerminalTargetBooleanExpression::Parameter {
                            source_value: ValueId::new(1).expect("left"),
                            parameter_index: 0,
                            location: TerminalScalarParameterLocation::Register(left_register),
                        }),
                        right: Box::new(TerminalTargetBooleanExpression::Parameter {
                            source_value: ValueId::new(2).expect("right"),
                            parameter_index: 1,
                            location: TerminalScalarParameterLocation::Register(right_register),
                        }),
                    },
                },
            }],
        }
    }

    fn integer_equality_plan(
        target: NativeTarget,
        scalar_type: IntegerType,
        left_register: MachineRegister,
        right_register: MachineRegister,
    ) -> TerminalTargetOperationPlan {
        TerminalTargetOperationPlan {
            terminal_psi: identity(),
            target,
            entry: MachineId::new(1).expect("machine"),
            functions: vec![TerminalTargetFunction {
                machine: MachineId::new(1).expect("machine"),
                provenance: TerminalPsiProvenance::default(),
                operation: TerminalTargetOperation::ReturnBooleanExpression {
                    psi_edge: EdgeId::new(1).expect("edge"),
                    source_value: ValueId::new(3).expect("result"),
                    expression: TerminalTargetBooleanExpression::IntegerEqual {
                        psi_operation: OperationId::new(1).expect("operation"),
                        scalar_type,
                        left: Box::new(TerminalTargetIntegerExpression::Parameter {
                            source_value: ValueId::new(1).expect("left"),
                            parameter_index: 0,
                            location: TerminalScalarParameterLocation::Register(left_register),
                        }),
                        right: Box::new(TerminalTargetIntegerExpression::Parameter {
                            source_value: ValueId::new(2).expect("right"),
                            parameter_index: 1,
                            location: TerminalScalarParameterLocation::Register(right_register),
                        }),
                    },
                },
            }],
        }
    }

    fn integer_ordering_plan(
        target: NativeTarget,
        scalar_type: IntegerType,
        inclusive: bool,
        left_register: MachineRegister,
        right_register: MachineRegister,
    ) -> TerminalTargetOperationPlan {
        let left = Box::new(TerminalTargetIntegerExpression::Parameter {
            source_value: ValueId::new(1).expect("left"),
            parameter_index: 0,
            location: TerminalScalarParameterLocation::Register(left_register),
        });
        let right = Box::new(TerminalTargetIntegerExpression::Parameter {
            source_value: ValueId::new(2).expect("right"),
            parameter_index: 1,
            location: TerminalScalarParameterLocation::Register(right_register),
        });
        let expression = if inclusive {
            TerminalTargetBooleanExpression::IntegerLessOrEqual {
                psi_operation: OperationId::new(1).expect("operation"),
                scalar_type,
                left,
                right,
            }
        } else {
            TerminalTargetBooleanExpression::IntegerLessThan {
                psi_operation: OperationId::new(1).expect("operation"),
                scalar_type,
                left,
                right,
            }
        };
        TerminalTargetOperationPlan {
            terminal_psi: identity(),
            target,
            entry: MachineId::new(1).expect("machine"),
            functions: vec![TerminalTargetFunction {
                machine: MachineId::new(1).expect("machine"),
                provenance: TerminalPsiProvenance::default(),
                operation: TerminalTargetOperation::ReturnBooleanExpression {
                    psi_edge: EdgeId::new(1).expect("edge"),
                    source_value: ValueId::new(3).expect("result"),
                    expression,
                },
            }],
        }
    }

    fn boolean_expression_conditional_plan(
        target: NativeTarget,
        left_register: MachineRegister,
        right_register: MachineRegister,
    ) -> TerminalTargetOperationPlan {
        let arm = |edge, return_edge, value| TerminalTargetConditionalBooleanArm {
            psi_edge: EdgeId::new(edge).expect("control edge"),
            control: Box::new(TerminalTargetBooleanControl::ReturnImmediate {
                psi_return_edge: EdgeId::new(return_edge).expect("return edge"),
                source_value: ValueId::new(if value { 4 } else { 5 }).expect("leaf value"),
                value,
            }),
        };
        TerminalTargetOperationPlan {
            terminal_psi: identity(),
            target,
            entry: MachineId::new(1).expect("machine"),
            functions: vec![TerminalTargetFunction {
                machine: MachineId::new(1).expect("machine"),
                provenance: TerminalPsiProvenance::default(),
                operation: TerminalTargetOperation::ReturnBooleanExpressionConditionalControl {
                    condition_source: ValueId::new(3).expect("condition"),
                    condition: TerminalTargetBooleanExpression::Equal {
                        psi_operation: OperationId::new(1).expect("operation"),
                        left: Box::new(TerminalTargetBooleanExpression::Parameter {
                            source_value: ValueId::new(1).expect("left"),
                            parameter_index: 0,
                            location: TerminalScalarParameterLocation::Register(left_register),
                        }),
                        right: Box::new(TerminalTargetBooleanExpression::Parameter {
                            source_value: ValueId::new(2).expect("right"),
                            parameter_index: 1,
                            location: TerminalScalarParameterLocation::Register(right_register),
                        }),
                    },
                    when_true: arm(1, 3, true),
                    when_false: arm(2, 4, false),
                },
            }],
        }
    }

    fn calling_conditional_plan(
        target: NativeTarget,
        argument_register: MachineRegister,
    ) -> TerminalTargetOperationPlan {
        let caller = MachineId::new(1).unwrap();
        let callee = MachineId::new(2).unwrap();
        let parameter = ValueId::new(1).unwrap();
        let call = |operation: u64, result: u64| TerminalTargetBooleanExpression::Call {
            psi_operation: OperationId::new(operation).unwrap(),
            source_value: ValueId::new(result).unwrap(),
            callee,
            arguments: vec![TerminalTargetCallArgument {
                scalar_type: psi_core::ScalarType::Boolean,
                location: TerminalScalarParameterLocation::Register(argument_register),
                expression: TerminalTargetScalarExpression::Boolean(
                    TerminalTargetBooleanExpression::Parameter {
                        source_value: parameter,
                        parameter_index: 0,
                        location: TerminalScalarParameterLocation::Register(argument_register),
                    },
                ),
            }],
        };
        let arm = |edge, return_edge, operation, result| TerminalTargetConditionalBooleanArm {
            psi_edge: EdgeId::new(edge).unwrap(),
            control: Box::new(TerminalTargetBooleanControl::ReturnExpression {
                psi_return_edge: EdgeId::new(return_edge).unwrap(),
                source_value: ValueId::new(result).unwrap(),
                expression: call(operation, result),
            }),
        };
        TerminalTargetOperationPlan {
            terminal_psi: identity(),
            target,
            entry: caller,
            functions: vec![
                TerminalTargetFunction {
                    machine: caller,
                    provenance: TerminalPsiProvenance::default(),
                    operation: TerminalTargetOperation::ReturnBooleanExpressionConditionalControl {
                        condition_source: ValueId::new(10).unwrap(),
                        condition: call(1, 10),
                        when_true: arm(1, 3, 2, 11),
                        when_false: arm(2, 4, 3, 12),
                    },
                },
                TerminalTargetFunction {
                    machine: callee,
                    provenance: TerminalPsiProvenance::default(),
                    operation: TerminalTargetOperation::ReturnBooleanParameter {
                        psi_edge: EdgeId::new(5).unwrap(),
                        source_value: parameter,
                        parameter_index: 0,
                        location: TerminalScalarParameterLocation::Register(argument_register),
                    },
                },
            ],
        }
    }

    fn calling_expression_condition_plan(
        target: NativeTarget,
        argument_register: MachineRegister,
    ) -> TerminalTargetOperationPlan {
        let scalar_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
        let caller = MachineId::new(1).unwrap();
        let callee = MachineId::new(2).unwrap();
        let parameter = ValueId::new(1).unwrap();
        let arm = |edge, return_edge, source_value, value| TerminalTargetConditionalIntegerArm {
            psi_edge: EdgeId::new(edge).unwrap(),
            control: Box::new(TerminalTargetIntegerControl::Return {
                psi_return_edge: EdgeId::new(return_edge).unwrap(),
                source_value: ValueId::new(source_value).unwrap(),
                expression: TerminalTargetIntegerExpression::Immediate {
                    source_value: ValueId::new(source_value).unwrap(),
                    value: IntegerValue::Unsigned(value),
                },
            }),
        };
        TerminalTargetOperationPlan {
            terminal_psi: identity(),
            target,
            entry: caller,
            functions: vec![
                TerminalTargetFunction {
                    machine: caller,
                    provenance: TerminalPsiProvenance::default(),
                    operation: TerminalTargetOperation::ReturnIntegerExpressionConditionalControl {
                        condition_source: ValueId::new(2).unwrap(),
                        condition: TerminalTargetBooleanExpression::Call {
                            psi_operation: OperationId::new(1).unwrap(),
                            source_value: ValueId::new(2).unwrap(),
                            callee,
                            arguments: vec![TerminalTargetCallArgument {
                                scalar_type: psi_core::ScalarType::Boolean,
                                location: TerminalScalarParameterLocation::Register(
                                    argument_register,
                                ),
                                expression: TerminalTargetScalarExpression::Boolean(
                                    TerminalTargetBooleanExpression::Parameter {
                                        source_value: parameter,
                                        parameter_index: 0,
                                        location: TerminalScalarParameterLocation::Register(
                                            argument_register,
                                        ),
                                    },
                                ),
                            }],
                        },
                        scalar_type,
                        when_true: arm(1, 3, 3, 1),
                        when_false: arm(2, 4, 4, 2),
                    },
                },
                TerminalTargetFunction {
                    machine: callee,
                    provenance: TerminalPsiProvenance::default(),
                    operation: TerminalTargetOperation::ReturnBooleanParameter {
                        psi_edge: EdgeId::new(5).unwrap(),
                        source_value: parameter,
                        parameter_index: 0,
                        location: TerminalScalarParameterLocation::Register(argument_register),
                    },
                },
            ],
        }
    }

    fn calling_arm_conditional_plan(
        target: NativeTarget,
        condition_register: MachineRegister,
        argument_register: MachineRegister,
    ) -> TerminalTargetOperationPlan {
        let scalar_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
        let caller = MachineId::new(1).unwrap();
        let callee = MachineId::new(2).unwrap();
        let parameter = ValueId::new(1).unwrap();
        let true_arm = TerminalTargetConditionalIntegerArm {
            psi_edge: EdgeId::new(1).unwrap(),
            control: Box::new(TerminalTargetIntegerControl::Return {
                psi_return_edge: EdgeId::new(3).unwrap(),
                source_value: ValueId::new(5).unwrap(),
                expression: TerminalTargetIntegerExpression::WrappingAdd {
                    psi_operation: OperationId::new(2).unwrap(),
                    left: Box::new(TerminalTargetIntegerExpression::Immediate {
                        source_value: ValueId::new(2).unwrap(),
                        value: IntegerValue::Unsigned(1),
                    }),
                    right: Box::new(TerminalTargetIntegerExpression::Call {
                        psi_operation: OperationId::new(1).unwrap(),
                        source_value: ValueId::new(4).unwrap(),
                        callee,
                        arguments: vec![TerminalTargetCallArgument {
                            scalar_type: psi_core::ScalarType::Integer(scalar_type),
                            location: TerminalScalarParameterLocation::Register(argument_register),
                            expression: TerminalTargetScalarExpression::Integer {
                                scalar_type,
                                expression: TerminalTargetIntegerExpression::Immediate {
                                    source_value: ValueId::new(3).unwrap(),
                                    value: IntegerValue::Unsigned(7),
                                },
                            },
                        }],
                    }),
                },
            }),
        };
        let false_arm = TerminalTargetConditionalIntegerArm {
            psi_edge: EdgeId::new(2).unwrap(),
            control: Box::new(TerminalTargetIntegerControl::Return {
                psi_return_edge: EdgeId::new(4).unwrap(),
                source_value: ValueId::new(6).unwrap(),
                expression: TerminalTargetIntegerExpression::Immediate {
                    source_value: ValueId::new(6).unwrap(),
                    value: IntegerValue::Unsigned(2),
                },
            }),
        };
        TerminalTargetOperationPlan {
            terminal_psi: identity(),
            target,
            entry: caller,
            functions: vec![
                TerminalTargetFunction {
                    machine: caller,
                    provenance: TerminalPsiProvenance::default(),
                    operation: TerminalTargetOperation::ReturnIntegerConditionalControl {
                        condition_source: parameter,
                        condition_parameter_index: 0,
                        condition_location: TerminalScalarParameterLocation::Register(
                            condition_register,
                        ),
                        scalar_type,
                        when_true: true_arm,
                        when_false: false_arm,
                    },
                },
                TerminalTargetFunction {
                    machine: callee,
                    provenance: TerminalPsiProvenance::default(),
                    operation: TerminalTargetOperation::ReturnIntegerParameter {
                        psi_edge: EdgeId::new(5).unwrap(),
                        source_value: parameter,
                        scalar_type,
                        parameter_index: 0,
                        location: TerminalScalarParameterLocation::Register(argument_register),
                    },
                },
            ],
        }
    }

    fn wrapping_expression(
        left_location: TerminalScalarParameterLocation,
        right_location: TerminalScalarParameterLocation,
    ) -> TerminalTargetIntegerExpression {
        TerminalTargetIntegerExpression::WrappingAdd {
            psi_operation: OperationId::new(3).expect("operation"),
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
        }
    }

    fn bitwise_expression(
        kind: u8,
        left_location: TerminalScalarParameterLocation,
        right_location: TerminalScalarParameterLocation,
    ) -> TerminalTargetIntegerExpression {
        let left = Box::new(TerminalTargetIntegerExpression::Parameter {
            source_value: ValueId::new(1).expect("left"),
            parameter_index: 0,
            location: left_location,
        });
        let right = Box::new(TerminalTargetIntegerExpression::Parameter {
            source_value: ValueId::new(2).expect("right"),
            parameter_index: 1,
            location: right_location,
        });
        let psi_operation = OperationId::new(3).expect("operation");
        match kind {
            0 => TerminalTargetIntegerExpression::BitwiseAnd {
                psi_operation,
                left,
                right,
            },
            1 => TerminalTargetIntegerExpression::BitwiseOr {
                psi_operation,
                left,
                right,
            },
            2 => TerminalTargetIntegerExpression::BitwiseXor {
                psi_operation,
                left,
                right,
            },
            _ => panic!("unknown bitwise test kind"),
        }
    }

    fn shift_expression(
        left_shift: bool,
        count_type: IntegerType,
        value_location: TerminalScalarParameterLocation,
        count_location: TerminalScalarParameterLocation,
    ) -> TerminalTargetIntegerExpression {
        let value = Box::new(TerminalTargetIntegerExpression::Parameter {
            source_value: ValueId::new(1).expect("value"),
            parameter_index: 0,
            location: value_location,
        });
        let count = Box::new(TerminalTargetIntegerExpression::Parameter {
            source_value: ValueId::new(2).expect("count"),
            parameter_index: 1,
            location: count_location,
        });
        let psi_operation = OperationId::new(3).expect("operation");
        if left_shift {
            TerminalTargetIntegerExpression::WrappingShiftLeft {
                psi_operation,
                count_type,
                value,
                count,
            }
        } else {
            TerminalTargetIntegerExpression::WrappingShiftRight {
                psi_operation,
                count_type,
                value,
                count,
            }
        }
    }

    fn aarch64_instructions(bytes: &[u8]) -> Vec<u32> {
        bytes
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("instruction")))
            .collect()
    }

    fn identity() -> TerminalPsiIdentity {
        TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
        }
    }
}
