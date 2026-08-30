//! Boundary settlement and admitted-provider Unit-call lowering.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_boundary_call(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    installed_calls: &BTreeMap<
        (MachineId, OperationId, BoundaryMachineId),
        InstalledProviderUnitCallEvidence,
    >,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    mut shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    mut active: &mut BTreeSet<StructuralTypeId>,
    established_byte_sequences: &BTreeMap<
        PlaceId,
        (OperationId, StructuralTypeDeclaration, Vec<u8>),
    >,
    integer_constants: &BTreeMap<ValueId, (OperationId, IntegerType, IntegerValue)>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
    nonreturning_boundary: &mut bool,
) -> Result<(), LoweringError> {
    match operation {
        AbstractOperation::BoundaryCall {
            psi_operation,
            result,
            boundary,
            arguments,
            structural_arguments,
            completion_claim_sources,
            completion_receipts,
        } => {
            if let Some(installed) =
                installed_calls.get(&(function.machine, *psi_operation, *boundary))
            {
                let callee = functions
                    .get(&installed.provider.candidate)
                    .copied()
                    .ok_or(LoweringError::UnknownCallTarget(
                        installed.provider.candidate,
                    ))?;
                let declaration = boundary_machines
                    .get(boundary)
                    .copied()
                    .ok_or(LoweringError::UnknownBoundarySettlement(*boundary))?;
                if result.is_some()
                    || !arguments.is_empty()
                    || callee.result != AbstractFunctionResult::Unit
                    || !callee.parameters.is_empty()
                    || structural_arguments.len() != callee.structural_parameters.len()
                    || declaration.structural_parameters.len() != callee.structural_parameters.len()
                    || installed.provider.signature.parameters.len()
                        != callee.structural_parameters.len()
                {
                    return Err(LoweringError::InstalledProviderCallShapeMismatch {
                        machine: function.machine,
                        operation: *psi_operation,
                        boundary: *boundary,
                    });
                }
                let callee_shapes = callee
                    .structural_parameters
                    .iter()
                    .map(|parameter| {
                        structural_shape(
                            parameter.structural_type,
                            structural_types,
                            &mut shape_cache,
                            &mut active,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let callee_plan = evaluate_call_plan(
                    CallingPolicy::native_for_target(target),
                    &CallSignature {
                        parameters: callee_shapes.clone(),
                        result: None,
                    },
                )
                .map_err(LoweringError::AbiPlan)?;
                let mut target_arguments = Vec::with_capacity(structural_arguments.len());
                for (index, (((argument, boundary_parameter), callee_parameter), signature)) in
                    structural_arguments
                        .iter()
                        .zip(&declaration.structural_parameters)
                        .zip(&callee.structural_parameters)
                        .zip(&installed.provider.signature.parameters)
                        .enumerate()
                {
                    let source = parameters_by_place.get(&argument.place).copied().ok_or(
                        LoweringError::UnknownStructuralArgumentPlace {
                            machine: function.machine,
                            place: argument.place,
                        },
                    )?;
                    let caller_parameter = function
                        .structural_parameters
                        .iter()
                        .find(|parameter| parameter.place == argument.place)
                        .ok_or(LoweringError::UnknownStructuralArgumentPlace {
                            machine: function.machine,
                            place: argument.place,
                        })?;
                    if !argument.path.is_empty()
                        || argument.access != psi_terminal::StructuralAccess::Owned
                        || source.access != psi_terminal::StructuralAccess::Owned
                        || boundary_parameter.access != psi_terminal::StructuralAccess::Owned
                        || callee_parameter.access != psi_terminal::StructuralAccess::Owned
                        || boundary_parameter.position != index as u32
                        || callee_parameter.position != index as u32
                        || signature.position != index as u32
                        || boundary_parameter.is_self
                        || callee_parameter.is_self
                        || signature.is_self
                        || source.structural_type != boundary_parameter.structural_type
                        || source.structural_type != callee_parameter.structural_type
                        || source.structural_type != signature.structural_type
                        || source.multiplicity != boundary_parameter.multiplicity
                        || source.multiplicity != callee_parameter.multiplicity
                        || source.multiplicity != signature.multiplicity
                        || source.access != signature.access
                        || source.structural_type
                            != callee.structural_parameters[index].structural_type
                        || source.placement.shape != callee_shapes[index]
                        || source.placement.shape != callee_plan.parameters[index].shape
                        || caller_parameter.qualifications.iter().any(|qualification| {
                            !boundary_parameter.qualifications.contains(qualification)
                                || !callee_parameter.qualifications.contains(qualification)
                                || !signature.qualifications.contains(qualification)
                        })
                        || caller_parameter.qualifications != boundary_parameter.qualifications
                        || caller_parameter.qualifications != callee_parameter.qualifications
                        || caller_parameter.qualifications != signature.qualifications
                    {
                        return Err(LoweringError::InstalledProviderCallShapeMismatch {
                            machine: function.machine,
                            operation: *psi_operation,
                            boundary: *boundary,
                        });
                    }
                    target_arguments.push(TargetStructuralArgument {
                        place: argument.place,
                        access: argument.access,
                        path: Vec::new(),
                        root_structural_type: source.structural_type,
                        structural_type: source.structural_type,
                        shape: source.shape,
                        source_byte_offset: 0,
                        fixed_array_length: None,
                        element_stride: None,
                        source: source.placement.clone(),
                        destination: callee_plan.parameters[index].clone(),
                    });
                }
                let claim_transfers = completion_receipts
                    .iter()
                    .map(|receipt| psi_terminal::ClaimTransfer {
                        claim: receipt.claim,
                        argument_index: receipt.argument_index,
                    })
                    .collect::<Vec<_>>();
                if completion_receipts.len() != callee.entry_claims.len()
                    || callee.entry_claims.iter().any(|entry| {
                        let Some(index) = callee
                            .structural_parameters
                            .iter()
                            .position(|parameter| parameter.place == entry.input)
                        else {
                            return true;
                        };
                        let Some(receipt) = completion_receipts
                            .iter()
                            .find(|receipt| receipt.argument_index as usize == index)
                        else {
                            return true;
                        };
                        let Some(source) = completion_claim_sources
                            .iter()
                            .find(|source| source.claim == receipt.claim)
                        else {
                            return true;
                        };
                        source.entry.as_ref().is_none_or(|source| {
                            source.input != structural_arguments[index].place
                                || source.path != entry.path
                        })
                    })
                {
                    return Err(LoweringError::InstalledProviderClaimTransferMismatch {
                        machine: function.machine,
                        operation: *psi_operation,
                        boundary: *boundary,
                    });
                }
                operations.push(TargetUnitOperation::InstalledProviderCall {
                    psi_operation: *psi_operation,
                    boundary: *boundary,
                    provider: installed.provider.clone(),
                    source_arguments: structural_arguments.clone(),
                    arguments: target_arguments,
                    claim_transfers,
                    completion_claim_sources: completion_claim_sources.clone(),
                    completion_receipts: completion_receipts.clone(),
                });
                provenance.operations.push(*psi_operation);
                return Ok(());
            }
            if result.is_some() {
                return Err(
                    LoweringError::ResultBearingBoundarySettlementRequiresNativeRealization {
                        machine: function.machine,
                        operation: *psi_operation,
                        boundary: *boundary,
                    },
                );
            }
            let binding = settlements
                .get(boundary)
                .cloned()
                .ok_or(LoweringError::MissingBoundarySettlement(*boundary))?;
            let declaration = boundary_machines
                .get(boundary)
                .copied()
                .ok_or(LoweringError::UnknownBoundarySettlement(*boundary))?;
            if let omega_target_operations::BoundarySettlementRealization::NormalizedForeignCall(
                foreign,
            ) = &binding.realization
            {
                let scalar_arguments = lower_normalized_foreign_scalar_arguments(
                    *boundary,
                    declaration,
                    arguments,
                    &foreign.boundary_entry_plan,
                    integer_constants,
                )?;
                if arguments.len() != declaration.scalar_parameters.len()
                    || !structural_arguments.is_empty()
                    || !completion_claim_sources.is_empty()
                    || !completion_receipts.is_empty()
                    || !declaration.structural_parameters.is_empty()
                    || declaration.result.is_some()
                    || target.object_format != ObjectFormat::Elf
                    || !matches!(
                        foreign.locator.locator(),
                        omega_target::ForeignLocatorCandidate::ElfVersioned { .. }
                    )
                    || foreign.boundary_entry_plan.call.policy
                        != omega_calling_conventions::CallingPolicy::native_for_target(target)
                    || foreign.boundary_entry_plan.call.entry_control
                        != omega_calling_conventions::EntryControl::CallReturn
                    || foreign.locator.target().native_target() != target
                {
                    return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                }
                operations.push(TargetUnitOperation::NormalizedForeignCall {
                    psi_operation: *psi_operation,
                    boundary: *boundary,
                    provider_execution: binding.provider_execution,
                    binding: foreign.clone(),
                    scalar_arguments,
                });
                provenance.operations.push(*psi_operation);
                return Ok(());
            }
            let omega_target_operations::BoundarySettlementRealization::Builtin(realization) =
                binding.realization
            else {
                unreachable!("normalized foreign settlement returns above")
            };
            let mut scalar_arguments = Vec::new();
            let mut byte_sequence_arguments = Vec::new();
            match realization {
                BoundaryRealization::MetadataOnlyPort(realization) => {
                    if !arguments.is_empty()
                        || !matches!(
                            operations.last(),
                            Some(TargetUnitOperation::PortWrite {
                                psi_operation,
                                service,
                                port,
                                value,
                            }) if *psi_operation == realization.effect_operation
                                && *service == realization.service
                                && *port == realization.port
                                && *value == realization.value
                        )
                    {
                        return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                    }
                    for argument in structural_arguments {
                        if !parameters_by_place.contains_key(&argument.place) {
                            return Err(LoweringError::UnknownStructuralArgumentPlace {
                                machine: function.machine,
                                place: argument.place,
                            });
                        }
                    }
                }
                BoundaryRealization::ClaimCompletionOnly(_) => {
                    if !claim_completion_only_boundary_is_exact(
                        function,
                        declaration,
                        arguments,
                        structural_arguments,
                        completion_claim_sources,
                        completion_receipts,
                        &parameters_by_place,
                    ) {
                        return Err(LoweringError::InvalidClaimCompletionOnlyShape {
                            machine: function.machine,
                            operation: *psi_operation,
                            boundary: *boundary,
                        });
                    }
                }
                BoundaryRealization::LinuxWriteLine(_) => {
                    if target.object_format != ObjectFormat::Elf
                        || !matches!(
                            target.architecture,
                            Architecture::X86_64 | Architecture::Aarch64
                        )
                        || !arguments.is_empty()
                        || declaration.result.is_some()
                        || !declaration.scalar_parameters.is_empty()
                        || structural_arguments.len() != 1
                        || declaration.structural_parameters.len() != 1
                    {
                        return Err(LoweringError::LinuxWriteLineUnsupportedOrInvalid {
                            machine: function.machine,
                            boundary: *boundary,
                            target,
                        });
                    }
                    let argument = &structural_arguments[0];
                    let parameter = &declaration.structural_parameters[0];
                    let Some((literal_operation, structural_type, bytes)) =
                        established_byte_sequences.get(&argument.place)
                    else {
                        return Err(LoweringError::LinuxWriteLineUnsupportedOrInvalid {
                            machine: function.machine,
                            boundary: *boundary,
                            target,
                        });
                    };
                    if !argument.path.is_empty()
                        || parameter.position != 0
                        || parameter.is_self
                        || parameter.structural_type != structural_type.id
                        || parameter.multiplicity
                            != psi_terminal::StructuralMultiplicity::Unrestricted
                        || !parameter.qualifications.is_empty()
                    {
                        return Err(LoweringError::LinuxWriteLineUnsupportedOrInvalid {
                            machine: function.machine,
                            boundary: *boundary,
                            target,
                        });
                    }
                    byte_sequence_arguments.push(BoundaryByteSequenceArgument {
                        argument: argument.clone(),
                        literal_operation: *literal_operation,
                        structural_type: structural_type.clone(),
                        bytes: bytes.clone(),
                    });
                }
                BoundaryRealization::LinuxExitGroupI32(_) => {
                    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32 is valid");
                    let [argument] = arguments.as_slice() else {
                        return Err(LoweringError::InvalidLinuxExitGroupShape(function.machine));
                    };
                    let Some((_, actual_type, value)) = integer_constants.get(argument) else {
                        return Err(LoweringError::InvalidLinuxExitGroupShape(function.machine));
                    };
                    if target.object_format != ObjectFormat::Elf
                        || !matches!(
                            target.architecture,
                            Architecture::X86_64 | Architecture::Aarch64
                        )
                        || declaration.scalar_parameters.as_slice()
                            != [ScalarType::Integer(i32_type)]
                        || !declaration.structural_parameters.is_empty()
                        || declaration.result.is_some()
                        || *actual_type != i32_type
                        || !i32_type.admits(*value)
                        || !structural_arguments.is_empty()
                    {
                        return Err(LoweringError::InvalidLinuxExitGroupShape(function.machine));
                    }
                    scalar_arguments.push(BoundaryScalarArgument {
                        source_value: *argument,
                        scalar_type: ScalarType::Integer(*actual_type),
                        immediate: *value,
                        destination: match target.architecture {
                            Architecture::X86_64 => MachineRegister::X86Rdi,
                            Architecture::Aarch64 => MachineRegister::Aarch64X(0),
                        },
                    });
                    *nonreturning_boundary = true;
                }
                BoundaryRealization::DirectPortReadU8(_) => {
                    return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                }
            }
            operations.push(TargetUnitOperation::BoundarySettlement {
                psi_operation: *psi_operation,
                boundary: *boundary,
                provider_execution: binding.provider_execution,
                realization,
                scalar_arguments,
                arguments: structural_arguments.clone(),
                byte_sequence_arguments,
                completion_claim_sources: completion_claim_sources.clone(),
                completion_receipts: completion_receipts.clone(),
            });
            provenance.operations.push(*psi_operation);
        }
        _ => unreachable!("boundary-call routing admits only boundary calls"),
    }
    Ok(())
}

fn lower_normalized_foreign_scalar_arguments(
    boundary: BoundaryMachineId,
    declaration: &psi_terminal::BoundaryMachineDeclaration,
    arguments: &[ValueId],
    boundary_entry_plan: &omega_calling_conventions::BoundaryEntryPlan,
    integer_constants: &BTreeMap<ValueId, (OperationId, IntegerType, IntegerValue)>,
) -> Result<Vec<omega_target_operations::NormalizedForeignScalarArgument>, LoweringError> {
    if declaration.scalar_parameters.len() > 3 {
        return Err(LoweringError::BoundaryRealizationMismatch(boundary));
    }
    let scalar_parameter_shapes = declaration
        .scalar_parameters
        .iter()
        .map(|parameter| {
            let ScalarType::Integer(integer_type) = parameter else {
                return Err(LoweringError::BoundaryRealizationMismatch(boundary));
            };
            if integer_type.carrier() != psi_core::IntegerCarrier::Fixed
                || !matches!(integer_type.bits(), 8 | 16 | 32 | 64)
            {
                return Err(LoweringError::BoundaryRealizationMismatch(boundary));
            }
            let bytes = integer_type.bits().div_ceil(8);
            Ok(ValueShape::integer(bytes, bytes.next_power_of_two().min(8)))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let signature = CallSignature {
        parameters: scalar_parameter_shapes.clone(),
        result: None,
    };
    let validated = omega_calling_conventions::validate_boundary_entry_plan(
        boundary_entry_plan.clone(),
        &signature,
    )
    .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?;
    if arguments.len() != declaration.scalar_parameters.len()
        || validated.plan() != boundary_entry_plan
    {
        return Err(LoweringError::BoundaryRealizationMismatch(boundary));
    }
    arguments
        .iter()
        .zip(&declaration.scalar_parameters)
        .zip(&scalar_parameter_shapes)
        .zip(&boundary_entry_plan.call.parameters)
        .enumerate()
        .map(
            |(parameter_index, (((source_value, parameter), shape), placement))| {
                let ScalarType::Integer(integer_type) = parameter else {
                    return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                };
                let Some((_, actual_type, immediate)) = integer_constants.get(source_value) else {
                    return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                };
                let [
                    ValueLocation::Register {
                        value_byte_offset: 0,
                        byte_size,
                        ..
                    },
                ] = placement.locations.as_slice()
                else {
                    return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                };
                if actual_type != integer_type
                    || placement.shape != *shape
                    || u16::try_from(shape.byte_size) != Ok(*byte_size)
                    || psi_core::ScalarTerm::integer(*integer_type, *immediate).is_err()
                {
                    return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                }
                Ok(omega_target_operations::NormalizedForeignScalarArgument {
                    source_value: *source_value,
                    scalar_type: *integer_type,
                    immediate: *immediate,
                    parameter_index: u32::try_from(parameter_index)
                        .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?,
                    placement: placement.clone(),
                })
            },
        )
        .collect()
}

#[cfg(test)]
mod tests;
