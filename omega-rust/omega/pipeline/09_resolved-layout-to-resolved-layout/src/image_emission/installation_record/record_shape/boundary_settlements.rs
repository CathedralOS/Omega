//! The boundary settlement rows: canonical order, one row per operation,
//! exact completion custody, and a realization that matches the declared
//! boundary byte for byte.

use crate::image_emission::installation_record::{
    Architecture, BoundaryRealization, CompletionCustodyError, InstallationError,
    InstallationRecord, InstalledFunction, MachineId, SemanticCodeSite, boundary_result_is_exact,
    hosted_write_byte_custody_is_exact, linux_write_line_custody_is_exact,
    validate_completion_custody,
};

pub(super) fn validate_boundary_settlements(
    record: &InstallationRecord,
    function_by_machine: &std::collections::BTreeMap<MachineId, &InstalledFunction>,
) -> Result<(), InstallationError> {
    let mut previous_machine = None;
    let mut previous_text_offset = 0;
    let mut previous_operation_ordinal = 0;
    let mut operations = std::collections::BTreeSet::new();
    for installed in &record.boundary_settlements {
        if let Some(machine) = previous_machine
            && (installed.machine < machine
                || (installed.machine == machine
                    && (
                        installed.text_offset,
                        installed.settlement.operation_ordinal,
                    ) <= (previous_text_offset, previous_operation_ordinal)))
        {
            return Err(InstallationError::NonCanonicalBoundarySettlementOrder);
        }
        if !operations.insert((installed.machine, installed.settlement.psi_operation)) {
            return Err(InstallationError::DuplicateBoundarySettlementOperation {
                machine: installed.machine,
                operation: installed.settlement.psi_operation,
            });
        }
        let function = function_by_machine
            .get(&installed.machine)
            .ok_or(InstallationError::EffectMachineMissing(installed.machine))?;
        let expected = function
            .text_offset
            .checked_add(installed.settlement.code_offset)
            .ok_or(InstallationError::SettlementOffsetNotRepresentable)?;
        if installed.text_offset != expected
            || installed
                .settlement
                .code_offset
                .checked_add(installed.settlement.byte_count)
                .is_none_or(|end| end > function.byte_count)
        {
            return Err(InstallationError::InvalidBoundarySettlementOffset {
                machine: installed.machine,
                operation: installed.settlement.psi_operation,
            });
        }
        if let Err(error) = validate_completion_custody(&installed.settlement) {
            return Err(match error {
                CompletionCustodyError::ArgumentPath => {
                    InstallationError::InvalidSettlementArgumentField
                }
                CompletionCustodyError::ReceiptArgumentIndex => {
                    InstallationError::InvalidCompletionReceiptArgumentIndex {
                        machine: installed.machine,
                        operation: installed.settlement.psi_operation,
                    }
                }
                CompletionCustodyError::ReceiptCustody => {
                    InstallationError::InvalidCompletionReceiptCustody {
                        machine: installed.machine,
                        operation: installed.settlement.psi_operation,
                    }
                }
                CompletionCustodyError::ProviderCustody => {
                    InstallationError::InvalidCompletionProviderCustody {
                        machine: installed.machine,
                        operation: installed.settlement.psi_operation,
                    }
                }
            });
        }
        let valid_realization = match installed.settlement.realization {
            BoundaryRealization::MetadataOnlyPort(realization) => {
                installed.settlement.scalar_arguments.is_empty()
                    && installed.settlement.runtime_scalar_arguments.is_empty()
                    && installed.settlement.byte_sequence_arguments.is_empty()
                    && installed.settlement.byte_count == 0
                    && record
                        .port_effects
                        .iter()
                        .filter(|effect| {
                            effect.machine == installed.machine
                                && effect.effect.psi_operation == realization.effect_operation
                                && effect.effect.service == realization.service
                                && effect.effect.port == realization.port
                                && effect.effect.value == realization.value
                                && effect.effect.operation_ordinal.checked_add(1)
                                    == Some(installed.settlement.operation_ordinal)
                                && effect
                                    .effect
                                    .code_offset
                                    .checked_add(effect.effect.byte_count)
                                    == Some(installed.settlement.code_offset)
                        })
                        .count()
                        == 1
            }
            BoundaryRealization::ClaimCompletionOnly(_) => {
                installed.settlement.scalar_arguments.is_empty()
                    && installed.settlement.runtime_scalar_arguments.is_empty()
                    && installed.settlement.byte_sequence_arguments.is_empty()
                    && installed.settlement.native_result.is_unit()
                    && installed.settlement.byte_count == 0
            }
            BoundaryRealization::DirectPortReadU8(_) => {
                let exact_return_edge =
                    installed
                        .settlement
                        .native_result
                        .scalar()
                        .is_some_and(|result| {
                            let Some(return_ordinal) =
                                installed.settlement.operation_ordinal.checked_add(1)
                            else {
                                return false;
                            };
                            let Some(return_offset) = installed
                                .settlement
                                .code_offset
                                .checked_add(installed.settlement.byte_count)
                            else {
                                return false;
                            };
                            record
                                .semantic_code_attribution
                                .iter()
                                .filter(|attribution| {
                                    attribution.machine == installed.machine
                                        && attribution.attribution.site
                                            == SemanticCodeSite::Edge(result.return_edge)
                                        && attribution.attribution.operation_ordinal
                                            == return_ordinal
                                        && attribution.attribution.code_offset == return_offset
                                        && attribution.attribution.byte_count == 1
                                })
                                .count()
                                == 1
                        });
                installed.settlement.scalar_arguments.is_empty()
                    && installed.settlement.runtime_scalar_arguments.is_empty()
                    && installed.settlement.byte_sequence_arguments.is_empty()
                    && record.target.architecture == Architecture::X86_64
                    && installed.settlement.byte_count == crate::image_emission::x86_encoding::IMMEDIATE_PORT_READ_U8_WIDTH
                    && function.unit_stack.is_none()
                    && function.scalar_stack.is_some()
                    && exact_return_edge
                    && installed.settlement.arguments.iter().all(|argument| {
                        argument.path.is_empty()
                            && function
                                .scalar_structural_parameters
                                .iter()
                                .any(|parameter| parameter.place == argument.place)
                    })
            }
            BoundaryRealization::LinuxWriteLine(_) => {
                linux_write_line_custody_is_exact(record.target, &installed.settlement, None)
                    && function.unit_body
                    && function.scalar_stack.is_none()
            }
            BoundaryRealization::HostedExitProcessI32(_) => {
                if installed.settlement.runtime_scalar_arguments.iter().any(|argument| matches!(argument.source, post_allocation_machine_to_selected_form_encoding::machine_code::InternalUnitScalarArgumentSourceRecord::SelectedProcessExit { .. })) {
                    crate::image_emission::object_artifact::replay::boundary::runtime_scalar_custody::process_exit::shape_is_exact(record.target, &installed.settlement)
                        && function.scalar_stack.is_none()
                        && installed.settlement.operation_ordinal.checked_add(1).is_some_and(|ordinal| record.semantic_code_attribution.iter().filter(|row| {
                            row.machine == installed.machine
                                && matches!(row.attribution.site, SemanticCodeSite::Edge(_))
                                && row.attribution.operation_ordinal == ordinal
                                && row.attribution.byte_count == 0
                                && installed.settlement.code_offset.checked_add(installed.settlement.byte_count) == Some(row.attribution.code_offset)
                        }).count() == 1)
                } else {
                let [argument] = installed.settlement.scalar_arguments.as_slice() else {
                    return Err(InstallationError::BoundaryRealizationMismatch {
                        machine: installed.machine,
                        operation: installed.settlement.psi_operation,
                    });
                };
                let i32_type = semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Signed,
                    32,
                )
                .expect("i32 is valid");
                let value = match (argument.scalar_type, argument.immediate) {
                    (
                        semantic_vocabulary::ScalarType::Integer(actual),
                        semantic_vocabulary::IntegerValue::Signed(value),
                    ) if actual == i32_type => i32::try_from(value).ok(),
                    _ => None,
                };
                let expected_destination =
                    if !abstract_operations_to_target_operations::target_operations::HostedExitProcessI32Realization::supports_target(
                        record.target,
                    ) {
                        None
                    } else {
                        match record.target.architecture {
                            Architecture::X86_64 => {
                                Some(abstract_operations_to_target_operations::calling_conventions::MachineRegister::X86Rdi)
                            }
                            Architecture::Aarch64 => {
                                Some(abstract_operations_to_target_operations::calling_conventions::MachineRegister::Aarch64X(0))
                            }
                        }
                    };
                let expected_byte_count = value
                    .and_then(|value| match record.target.architecture {
                        Architecture::X86_64 => {
                            Some(target_operations_to_selected_instructions::isa_x86_64::encode_hosted_exit_process_i32(value).len())
                        }
                        Architecture::Aarch64 => {
                            target_operations_to_selected_instructions::isa_aarch64::encode_hosted_exit_process_i32(record.target, value)
                                .ok()
                                .map(|bytes| bytes.len())
                        }
                    })
                    .unwrap_or(0);
                let exact_nominal_tail = installed
                    .settlement
                    .operation_ordinal
                    .checked_add(1)
                    .is_some_and(|tail_ordinal| {
                        record
                            .semantic_code_attribution
                            .iter()
                            .filter(|attribution| {
                                attribution.machine == installed.machine
                                    && matches!(
                                        attribution.attribution.site,
                                        SemanticCodeSite::Edge(_)
                                    )
                                    && attribution.attribution.operation_ordinal == tail_ordinal
                                    && attribution.attribution.code_offset
                                        == installed
                                            .settlement
                                            .code_offset
                                            .saturating_add(installed.settlement.byte_count)
                                    && attribution
                                        .attribution
                                        .code_offset
                                        .checked_add(attribution.attribution.byte_count)
                                        == Some(function.byte_count)
                                    && (function.unit_body
                                        || attribution.attribution.byte_count == 0)
                            })
                            .count()
                            == 1
                    });
                expected_destination == Some(argument.destination)
                    && installed.settlement.runtime_scalar_arguments.is_empty()
                    && installed.settlement.byte_count == expected_byte_count
                    && expected_byte_count != 0
                    && installed.settlement.arguments.is_empty()
                    && installed.settlement.byte_sequence_arguments.is_empty()
                    && installed.settlement.native_result.is_unit()
                    && function.scalar_stack.is_none()
                    && exact_nominal_tail
                }
            }
            BoundaryRealization::HostedWriteByteI32(_) => {
                if installed
                    .settlement
                    .runtime_scalar_arguments
                    .iter()
                    .any(|argument| {
                        matches!(
                    argument.source,
                    post_allocation_machine_to_selected_form_encoding::machine_code::InternalUnitScalarArgumentSourceRecord::SelectedBoundary { .. } | post_allocation_machine_to_selected_form_encoding::machine_code::InternalUnitScalarArgumentSourceRecord::SelectedProcessExit { .. }
                )
                    })
                {
                    crate::image_emission::object_artifact::replay::boundary::runtime_scalar_custody::selected_byte_output_shape_is_exact(
                        record.target,
                        &installed.settlement,
                    ) && function
                        .parameter_abi
                        .as_ref()
                        .is_none_or(|abi| abi.call_plan.result.is_none())
                        && function.unit_stack.is_some()
                        && function.scalar_stack.is_none()
                } else {
                    let machine_settlements = record
                        .boundary_settlements
                        .iter()
                        .filter(|candidate| candidate.machine == installed.machine)
                        .map(|candidate| candidate.settlement.clone())
                        .collect::<Vec<_>>();
                    hosted_write_byte_custody_is_exact(
                        record.target,
                        &installed.settlement,
                        &machine_settlements,
                        &function.unit_integer_constants,
                        &function.unit_scalar_homes,
                        |home, consumer_ordinal, consumer_offset| {
                            record
                                .internal_unit_scalar_calls
                                .iter()
                                .filter(|producer| {
                                    producer.machine == installed.machine
                                        && producer.custody.result.home == home
                                        && producer.custody.operation_ordinal < consumer_ordinal
                                        && producer
                                            .custody
                                            .result
                                            .code_offset
                                            .checked_add(producer.custody.result.byte_count)
                                            .is_some_and(|end| end <= consumer_offset)
                                })
                                .count()
                        },
                        None,
                    ) && function.unit_body
                        && function.scalar_stack.is_none()
                }
            }
            BoundaryRealization::HostedReadByte(_) => {
                installed.settlement.scalar_arguments.is_empty()
                    && installed.settlement.runtime_scalar_arguments.is_empty()
                    && installed.settlement.arguments.is_empty()
                    && installed.settlement.byte_sequence_arguments.is_empty()
                    && installed.settlement.byte_count != 0
                    // The byte result is local to this boundary occurrence,
                    // independent of the enclosing function's return kind.
                    // Retain exactly one frame record; function/image replay
                    // separately checks its role, geometry, and physical home.
                    && (function.unit_stack.is_some() != function.scalar_stack.is_some())
            }
        };
        if !valid_realization
            || !boundary_result_is_exact(
                record.target,
                installed.settlement.realization,
                &installed.settlement.native_result,
            )
        {
            return Err(InstallationError::BoundaryRealizationMismatch {
                machine: installed.machine,
                operation: installed.settlement.psi_operation,
            });
        }
        previous_machine = Some(installed.machine);
        previous_text_offset = installed.text_offset;
        previous_operation_ordinal = installed.settlement.operation_ordinal;
    }
    Ok(())
}
