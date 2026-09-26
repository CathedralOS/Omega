//! Who owns the call and whether its argument roster matches the callee.

use super::InternalUnitCall;
use crate::image_emission::installation_record::record_validation::installed_scalar_source_is_exact;
use crate::image_emission::installation_record::{
    CallSiteOwner, SemanticCodeSite, incoming_structural,
};

impl InternalUnitCall<'_> {
    /// A projected argument that is exactly a borrowed projection of the
    /// caller's parameter home into the callee's parameter.
    pub(super) fn exact_borrowed_argument(
        &self,
        index: usize,
        argument: &post_allocation_machine_to_selected_form_encoding::machine_code::InternalUnitCallArgumentRecord,
    ) -> bool {
        let InternalUnitCall {
            callee_unit_parameters,
            parameter_homes,
            affine_cleanup,
            ..
        } = *self;
        parameter_homes
            .iter()
            .find(|home| home.place == argument.place)
            .zip(callee_unit_parameters.get(index))
            .zip(affine_cleanup)
            .is_some_and(|((source, destination), cleanup)| {
                crate::image_emission::object_artifact::replay::unit::call_custody::exact_borrowed_projection(
                    argument,
                    source,
                    destination,
                    &cleanup.structural_types,
                )
            })
    }

    /// The call's owner binds it to its exact source span: an attributed
    /// operation, or a cleanup action whose nominal invocation this call is.
    pub(super) fn owner_is_valid(&self) -> bool {
        let InternalUnitCall {
            record,
            function,
            installed,
            custody,
            incoming_call,
            affine_cleanup,
            ..
        } = *self;
        if incoming_call {
            incoming_structural::call_attribution_is_exact(record, function, installed)
        } else {
            match custody.owner {
            CallSiteOwner::Operation(operation) => {
                record.semantic_code_attribution.iter().any(|attribution| {
                    attribution.machine == installed.machine
                        && attribution.attribution.site
                            == SemanticCodeSite::Operation(operation)
                        && attribution.attribution.operation_ordinal == custody.operation_ordinal
                        && attribution.attribution.code_offset == custody.code_offset
                        && attribution.attribution.byte_count == custody.byte_count
                })
            }
            CallSiteOwner::CleanupAction {
                edge,
                action_ordinal,
            } => {
                custody.result.is_none()
                    && custody.structural_result.is_none()
                    && custody.scalar_arguments.is_empty()
                    && custody.arguments.is_empty()
                    && custody.claim_transfers.is_empty()
                    && affine_cleanup
                        .is_some_and(|cleanup| {
                            cleanup.psi_edge == edge
                                && usize::try_from(action_ordinal)
                                    .ok()
                                    .and_then(|ordinal| cleanup.actions.get(ordinal))
                                    .is_some_and(|action| matches!(action,
                                        terminal_psi::TerminalAffineCleanupAction::InvokeNominal(nominal)
                                            if nominal.cleanup_machine == custody.target))
                                && cleanup.code_offset <= custody.code_offset
                                && custody
                                    .code_offset
                                    .checked_add(custody.byte_count)
                                    .is_some_and(|call_end| {
                                        cleanup
                                            .code_offset
                                            .checked_add(cleanup.byte_count)
                                            .is_some_and(|cleanup_end| call_end <= cleanup_end)
                                    })
                                && record.semantic_code_attribution.iter().any(|attribution| {
                                    attribution.machine == installed.machine
                                        && attribution.attribution.site
                                            == SemanticCodeSite::Edge(edge)
                                        && attribution.attribution.operation_ordinal
                                            == custody.operation_ordinal
                                        && attribution.attribution.code_offset
                                            == cleanup.code_offset
                                        && attribution.attribution.byte_count == cleanup.byte_count
                                })
                        })
            }
        }
        }
    }

    /// The scalar and structural argument roster agrees with the callee's
    /// ABI or mixed return contract: placements, types, sources, byte spans
    /// and their contiguous layout.
    pub(super) fn mixed_roster_is_exact(&self) -> bool {
        let InternalUnitCall {
            record,
            function,
            installed,
            custody,
            plan,
            end,
            callee_parameter_abi,
            callee_unit_parameters,
            callee_mixed_abi,
            callee_mixed_structural_return,
            ..
        } = *self;
        let scalar_count = custody.scalar_arguments.len();
        let selected_scalar_call =
            custody
                .scalar_arguments
                .iter()
                .find_map(|argument| match argument.source {
                    post_allocation_machine_to_selected_form_encoding::machine_code::InternalUnitScalarArgumentSourceRecord::SelectedCall {
                        instruction,
                        ..
                    } => Some(instruction),
                    _ => None,
                });
        // Selected transport is proved by retained physical replay and compared
        // against the admitted image by validate_installation_record. Its span
        // is the call instruction, not a legacy contiguous materialization.
        if let Some(call_instruction) = selected_scalar_call {
            callee_parameter_abi.is_some_and(|abi| {
                callee_mixed_abi.is_none()
                    && callee_mixed_structural_return.is_none()
                    && custody.result.is_none()
                    && custody.semantic_result.is_none()
                    && custody.structural_result.is_none()
                    && custody.arguments.is_empty()
                    && callee_unit_parameters.is_empty()
                    && custody.claim_transfers.is_empty()
                    && *plan == abi.call_plan
                    && scalar_count == abi.parameters.len()
                    && custody.scalar_arguments.iter().zip(&abi.parameters).enumerate().all(|(parameter_index, (argument, parameter))| {
                        let post_allocation_machine_to_selected_form_encoding::machine_code::InternalUnitScalarArgumentSourceRecord::SelectedCall { scalar_type, instruction, .. } = argument.source else {
                            return false;
                        };
                        instruction == call_instruction
                            && scalar_type == parameter.scalar_type
                            && argument.destination == parameter.placement
                            && usize::try_from(argument.parameter_index) == Ok(parameter_index)
                            && argument.code_offset == custody.code_offset
                            && argument.byte_count == custody.byte_count
                    })
            })
        } else if let Some(abi) = callee_parameter_abi {
            callee_mixed_abi.is_none()
                && callee_mixed_structural_return.is_none()
                && custody.result.is_none()
                && custody.structural_result.is_none()
                && *plan == abi.call_plan
                && scalar_count == abi.parameters.len()
                && custody.arguments.len() == callee_unit_parameters.len()
                && custody
                    .scalar_arguments
                    .iter()
                    .zip(&abi.parameters)
                    .enumerate()
                    .all(|(index, (argument, parameter))| {
                        let expected_argument_bytes = function
                            .unit_call_stacks
                            .iter()
                            .find(|call| {
                                call.owner == custody.owner && call.target == custody.target
                            })
                            .and_then(|call| {
                                let linkage_bytes = match record.target.architecture {
                                    target::Architecture::X86_64 => 8,
                                    target::Architecture::Aarch64 => 0,
                                };
                                crate::image_emission::object_artifact::replay::unit::scalar_call_custody::expected_argument_bytes(
                                    record.target,
                                    plan,
                                    &custody.scalar_arguments,
                                    index,
                                    function.unit_stack.as_ref()?.frame_bytes,
                                    call.transient_bytes.checked_sub(linkage_bytes)?,
                                )
                            });
                        usize::try_from(argument.parameter_index) == Ok(index)
                            && argument.destination == parameter.placement
                            && argument.source.scalar_type() == parameter.scalar_type
                            && installed_scalar_source_is_exact(
                                record,
                                function,
                                installed.machine,
                                custody,
                                argument.source,
                            )
                            && expected_argument_bytes
                                .as_ref()
                                .is_some_and(|bytes| bytes.len() == argument.byte_count)
                            && argument.code_offset >= custody.code_offset
                            && argument
                                .code_offset
                                .checked_add(argument.byte_count)
                                .is_some_and(|argument_end| argument_end <= end)
                    })
                && custody
                    .arguments
                    .iter()
                    .zip(callee_unit_parameters)
                    .zip(&abi.call_plan.parameters[abi.parameters.len()..])
                    .enumerate()
                    .all(|(index, ((argument, parameter), placement))| {
                        self.exact_borrowed_argument(index, argument)
                            || (argument.root_structural_type == parameter.structural_type
                                && argument.structural_type == parameter.structural_type
                                && argument.access == parameter.access
                                && argument.shape == parameter.shape
                                && argument.destination == *placement)
                    })
                && custody.scalar_arguments.windows(2).all(|pair| {
                    pair[0]
                        .code_offset
                        .checked_add(pair[0].byte_count)
                        .is_some_and(|prior_end| prior_end == pair[1].code_offset)
                })
                && custody.scalar_arguments.last().is_none_or(|last| {
                    last.code_offset
                        .checked_add(last.byte_count)
                        .is_some_and(|scalar_end| {
                            custody
                                .arguments
                                .first()
                                .map_or(scalar_end <= end, |argument| {
                                    scalar_end == argument.code_offset
                                })
                        })
                })
                && custody.arguments.windows(2).all(|pair| {
                    pair[0].code_offset.checked_add(pair[0].byte_count) == Some(pair[1].code_offset)
                })
        } else {
            match (callee_mixed_abi, callee_mixed_structural_return) {
                (None, None) => custody.scalar_arguments.is_empty(),
                (Some(_), Some(_)) => false,
                (Some(abi), None) => {
                    custody.result == Some(abi.result.scalar_type)
                        && *plan == abi.call_plan
                        && scalar_count == abi.scalar_parameters.len()
                        && custody.arguments.len() == abi.structural_parameters.len()
                        && custody
                            .scalar_arguments
                            .iter()
                            .zip(&abi.scalar_parameters)
                            .enumerate()
                            .all(|(index, (argument, parameter))| {
                                let expected_argument_bytes =
                                    custody.arguments.first().and_then(|structural| {
                                        crate::image_emission::object_artifact::replay::unit::scalar_call_custody::expected_argument_bytes(
                                            record.target,
                                            plan,
                                            &custody.scalar_arguments,
                                            index,
                                            function.unit_stack.as_ref()?.frame_bytes,
                                            structural.call_stack_bytes,
                                        )
                                    });
                                usize::try_from(argument.parameter_index) == Ok(index)
                                    && argument.destination == parameter.placement
                                    && argument.source.scalar_type() == parameter.scalar_type
                                    && installed_scalar_source_is_exact(
                                        record,
                                        function,
                                        installed.machine,
                                        custody,
                                        argument.source,
                                    )
                                    && expected_argument_bytes
                                        .as_ref()
                                        .is_some_and(|bytes| bytes.len() == argument.byte_count)
                                    && argument.code_offset >= custody.code_offset
                                    && argument
                                        .code_offset
                                        .checked_add(argument.byte_count)
                                        .is_some_and(|argument_end| argument_end <= end)
                            })
                        && custody
                            .arguments
                            .iter()
                            .zip(&abi.structural_parameters)
                            .all(|(argument, parameter)| {
                                argument.path.is_empty()
                                    && argument.root_structural_type == parameter.structural_type
                                    && argument.structural_type == parameter.structural_type
                                    && argument.access == parameter.access
                                    && argument.shape == parameter.shape
                                    && argument.destination == parameter.placement
                            })
                        && custody.scalar_arguments.windows(2).all(|pair| {
                            pair[0]
                                .code_offset
                                .checked_add(pair[0].byte_count)
                                .is_some_and(|prior_end| prior_end == pair[1].code_offset)
                        })
                        && custody.scalar_arguments.last().is_none_or(|last| {
                            last.code_offset.checked_add(last.byte_count).is_some_and(
                                |scalar_end| {
                                    custody
                                        .arguments
                                        .first()
                                        .map_or(scalar_end <= end, |argument| {
                                            scalar_end == argument.code_offset
                                        })
                                },
                            )
                        })
                        && custody.arguments.windows(2).all(|pair| {
                            pair[0].code_offset.checked_add(pair[0].byte_count)
                                == Some(pair[1].code_offset)
                        })
                }
                (None, Some(returned)) => {
                    custody.result.is_none()
                        && custody.structural_result.is_some()
                        && scalar_count == returned.scalar_parameters.len()
                        && custody.arguments.len() == returned.parameters.len()
                        && plan.parameters.len()
                            == returned.scalar_parameters.len() + returned.parameters.len()
                        && plan.parameters[..returned.scalar_parameters.len()]
                            == returned
                                .scalar_parameters
                                .iter()
                                .map(|parameter| parameter.placement.clone())
                                .collect::<Vec<_>>()
                        && plan.parameters[returned.scalar_parameters.len()..]
                            == returned.parameter_placements
                        && plan.result.as_ref() == Some(&returned.result_placement)
                        && custody
                            .scalar_arguments
                            .iter()
                            .zip(&returned.scalar_parameters)
                            .enumerate()
                            .all(|(index, (argument, parameter))| {
                                let expected_argument_bytes =
                                    custody.arguments.first().and_then(|structural| {
                                        crate::image_emission::object_artifact::replay::unit::scalar_call_custody::expected_argument_bytes(
                                            record.target,
                                            plan,
                                            &custody.scalar_arguments,
                                            index,
                                            function.unit_stack.as_ref()?.frame_bytes,
                                            structural.call_stack_bytes,
                                        )
                                    });
                                usize::try_from(argument.parameter_index) == Ok(index)
                                    && argument.destination == parameter.placement
                                    && argument.source.scalar_type() == parameter.scalar_type
                                    && installed_scalar_source_is_exact(
                                        record,
                                        function,
                                        installed.machine,
                                        custody,
                                        argument.source,
                                    )
                                    && expected_argument_bytes
                                        .as_ref()
                                        .is_some_and(|bytes| bytes.len() == argument.byte_count)
                                    && argument.code_offset >= custody.code_offset
                                    && argument
                                        .code_offset
                                        .checked_add(argument.byte_count)
                                        .is_some_and(|argument_end| argument_end <= end)
                            })
                        && custody
                            .arguments
                            .iter()
                            .zip(&returned.parameters)
                            .zip(&returned.parameter_placements)
                            .all(|((argument, parameter), placement)| {
                                argument.path.is_empty()
                                    && argument.root_structural_type == parameter.structural_type
                                    && argument.structural_type == parameter.structural_type
                                    && argument.access == parameter.access
                                    && argument.shape == placement.shape
                                    && argument.destination == *placement
                            })
                        && custody.scalar_arguments.windows(2).all(|pair| {
                            pair[0]
                                .code_offset
                                .checked_add(pair[0].byte_count)
                                .is_some_and(|prior_end| prior_end == pair[1].code_offset)
                        })
                        && custody.scalar_arguments.last().is_none_or(|last| {
                            last.code_offset.checked_add(last.byte_count).is_some_and(
                                |scalar_end| {
                                    custody
                                        .arguments
                                        .first()
                                        .map_or(scalar_end <= end, |argument| {
                                            scalar_end == argument.code_offset
                                        })
                                },
                            )
                        })
                        && custody.arguments.windows(2).all(|pair| {
                            pair[0].code_offset.checked_add(pair[0].byte_count)
                                == Some(pair[1].code_offset)
                        })
                }
            }
        }
    }
}
