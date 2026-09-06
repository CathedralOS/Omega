//! Boundary settlement and admitted-provider Unit-call lowering.

use super::super::boundary_settlements::claim_completion_only_boundary_is_exact;
use super::super::scalar_abi::fixed_native_integer_shape;
use super::super::shared::*;
use super::super::structural_layout::{structural_shape, structural_sum_layout};
use super::scalar_call::{KnownUnitInteger, insert_known_unit_integer};

mod installed_provider;
mod normalized_foreign;

#[cfg(test)]
use normalized_foreign::lower_normalized_foreign_scalar_arguments;
use normalized_foreign::{
    lower_normalized_foreign_scalar_arguments_with_result, lower_normalized_foreign_scalar_result,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_boundary_call(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    installed_calls: &BTreeMap<
        (MachineId, OperationId, BoundaryMachineId),
        InstalledProviderUnitCallEvidence,
    >,
    native_callbacks: &BTreeMap<OperationId, target_operations::TargetNativeCallbackArgument>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    established_byte_sequences: &BTreeMap<
        PlaceId,
        (OperationId, StructuralTypeDeclaration, Vec<u8>),
    >,
    integer_constants: &BTreeMap<ValueId, (OperationId, IntegerType, IntegerValue)>,
    scalar_values: &mut BTreeMap<ValueId, KnownUnitInteger>,
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
            let native_callback = native_callbacks.get(psi_operation);
            if installed_provider::try_lower(
                operation,
                function,
                target,
                functions,
                structural_types,
                boundary_machines,
                installed_calls,
                parameters_by_place,
                shape_cache,
                active,
                scalar_values,
                operations,
                provenance,
            )? {
                return Ok(());
            }
            let binding = settlements
                .get(boundary)
                .cloned()
                .ok_or(LoweringError::MissingBoundarySettlement(*boundary))?;
            let declaration = boundary_machines
                .get(boundary)
                .copied()
                .ok_or(LoweringError::UnknownBoundarySettlement(*boundary))?;
            if let target_operations::BoundarySettlementRealization::NormalizedForeignCall(
                foreign,
            ) = &binding.realization
            {
                let result_home = lower_normalized_foreign_scalar_result(
                    *boundary,
                    declaration,
                    *psi_operation,
                    result.scalar(),
                    &foreign.boundary_entry_plan,
                )?;
                let scalar_arguments = lower_normalized_foreign_scalar_arguments_with_result(
                    *boundary,
                    declaration,
                    arguments,
                    &foreign.boundary_entry_plan,
                    scalar_values,
                    result_home.map(|home| home.shape),
                    native_callback,
                )?;
                if arguments.len() != declaration.scalar_parameters.len()
                    || !structural_arguments.is_empty()
                    || !completion_claim_sources.is_empty()
                    || !completion_receipts.is_empty()
                    || !declaration.structural_parameters.is_empty()
                    || (result_home.is_some() && function.attachment.is_none())
                    || foreign.boundary_entry_plan.call.policy
                        != calling_conventions::CallingPolicy::native_for_target(target)
                    || foreign.boundary_entry_plan.call.entry_control
                        != calling_conventions::EntryControl::CallReturn
                    || foreign.locator.target().native_target() != target
                    || native_callback.is_some_and(|callback| {
                        callback.terminal_operation != *psi_operation
                            || callback.registrar_boundary_entry_plan != foreign.boundary_entry_plan
                    })
                {
                    return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                }
                let target_operations::BoundaryExecutionBinding::AdmittedProvider(
                    provider_execution,
                ) = binding.execution
                else {
                    return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                };
                if let Some(home) = result_home {
                    insert_known_unit_integer(
                        scalar_values,
                        home.source_value,
                        KnownUnitInteger::Home(home),
                    )?;
                }
                operations.push(TargetUnitOperation::NormalizedForeignCall {
                    psi_operation: *psi_operation,
                    boundary: *boundary,
                    provider_execution,
                    binding: foreign.clone(),
                    scalar_arguments,
                    result_home,
                });
                provenance.operations.push(*psi_operation);
                return Ok(());
            }
            let target_operations::BoundarySettlementRealization::Builtin(realization) =
                binding.realization
            else {
                unreachable!("normalized foreign settlement returns above")
            };
            let target_result = match result {
                abstract_operations::AbstractBoundaryResult::Unit => {
                    if !declaration.result.is_unit() {
                        return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                    }
                    target_operations::TargetBoundaryResult::Unit
                }
                abstract_operations::AbstractBoundaryResult::Structural(result) => {
                    let terminal_psi::BoundaryMachineResult::Structural(expected) =
                        &declaration.result
                    else {
                        return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                    };
                    if result.structural_type != expected.structural_type
                        || result.multiplicity != expected.multiplicity
                        || result.qualifications != expected.qualifications
                        || !result.projected_qualifications.is_empty()
                        || !result.claims.is_empty()
                    {
                        return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                    }
                    let layout = structural_sum_layout(
                        result.structural_type,
                        structural_types,
                        shape_cache,
                        active,
                    )?;
                    target_operations::TargetBoundaryResult::Structural(
                        target_operations::TargetStructuralHomeRequirement {
                            defining_operation: *psi_operation,
                            result: result.clone(),
                            layout: target_operations::TargetStructuralHomeLayout::Sum(layout),
                        },
                    )
                }
                abstract_operations::AbstractBoundaryResult::Scalar(_) => {
                    return Err(
                        LoweringError::ResultBearingBoundarySettlementRequiresNativeRealization {
                            machine: function.machine,
                            operation: *psi_operation,
                            boundary: *boundary,
                        },
                    );
                }
            };
            let mut scalar_arguments = Vec::new();
            let mut runtime_scalar_arguments = Vec::new();
            let mut byte_sequence_arguments = Vec::new();
            if !matches!(realization, BoundaryRealization::LinuxReadByte(_))
                && !matches!(
                    &target_result,
                    target_operations::TargetBoundaryResult::Unit
                )
            {
                return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
            }
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
                        parameters_by_place,
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
                        || !declaration.result.is_unit()
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
                            != terminal_psi::StructuralMultiplicity::Unrestricted
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
                        || !declaration.result.is_unit()
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
                BoundaryRealization::LinuxReadByte(_) => {
                    if target.object_format != ObjectFormat::Elf
                        || !matches!(
                            target.architecture,
                            Architecture::X86_64 | Architecture::Aarch64
                        )
                        || !arguments.is_empty()
                        || !structural_arguments.is_empty()
                        || !declaration.scalar_parameters.is_empty()
                        || !declaration.structural_parameters.is_empty()
                        || !completion_claim_sources.is_empty()
                        || !completion_receipts.is_empty()
                        || !matches!(
                            &target_result,
                            target_operations::TargetBoundaryResult::Structural(home)
                                if home.layout.sum().is_some_and(|layout| layout.tag_byte_offset == 0
                                    && layout.tag_shape == ValueShape::integer(4, 4))
                        )
                    {
                        return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                    }
                }
                BoundaryRealization::LinuxWriteByteI32(_) => {
                    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32 is valid");
                    let [source_value] = arguments.as_slice() else {
                        return Err(LoweringError::InvalidLinuxExitGroupShape(function.machine));
                    };
                    let Some(known) = scalar_values.get(source_value).copied() else {
                        return Err(LoweringError::UnknownValue(*source_value));
                    };
                    let shape = fixed_native_integer_shape(i32_type)
                        .ok_or(LoweringError::InvalidLinuxExitGroupShape(function.machine))?;
                    let call_plan = evaluate_call_plan(
                        CallingPolicy::native_for_target(target),
                        &CallSignature {
                            parameters: vec![shape],
                            result: None,
                        },
                    )
                    .map_err(LoweringError::AbiPlan)?;
                    let [placement] = call_plan.parameters.as_slice() else {
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
                        || !declaration.result.is_unit()
                        || known.scalar_type() != i32_type
                        || !structural_arguments.is_empty()
                        || placement.shape != shape
                    {
                        return Err(LoweringError::InvalidLinuxExitGroupShape(function.machine));
                    }
                    runtime_scalar_arguments.push(TargetUnitScalarCallArgument {
                        parameter_index: 0,
                        source: known.into_target_source(*source_value),
                        placement: placement.clone(),
                    });
                }
                BoundaryRealization::DirectPortReadU8(_) => {
                    return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                }
            }
            operations.push(TargetUnitOperation::BoundarySettlement {
                psi_operation: *psi_operation,
                boundary: *boundary,
                result: target_result,
                execution: binding.execution,
                realization,
                scalar_arguments,
                runtime_scalar_arguments,
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

#[cfg(test)]
mod tests;
