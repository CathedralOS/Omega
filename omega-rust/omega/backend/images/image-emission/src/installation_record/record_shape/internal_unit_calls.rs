//! The internal Unit call rows: each call's retained custody replays
//! exactly against the caller's parameter homes, the callee's ABI and the
//! call plan, in physical call order.

use crate::installation_record::record_validation::installed_scalar_source_is_exact;
use crate::installation_record::{
    CallSignature, CallSiteOwner, CallingPolicy, InstallationError, InstallationRecord,
    InstalledFunction, MachineId, SemanticCodeSite, StructuralPathSegment, ValueShape,
    borrowed_structural, evaluate_call_plan, incoming_structural,
};

pub(super) fn validate_internal_unit_calls(
    record: &InstallationRecord,
    function_by_machine: &std::collections::BTreeMap<MachineId, &InstalledFunction>,
) -> Result<(), InstallationError> {
    let mut previous_call = None;
    for installed in &record.internal_unit_calls {
        let function = function_by_machine.get(&installed.machine).ok_or(
            InstallationError::InvalidInternalUnitCall(installed.machine),
        )?;
        let custody = &installed.custody;
        if borrowed_structural::has_borrowed(function) || custody.arguments.iter().any(|argument| {
            matches!(
                argument.source,
                machine_code::InternalUnitStructuralArgumentSourceRecord::EstablishedByteView { .. }
                    | machine_code::InternalUnitStructuralArgumentSourceRecord::EstablishedPrimitiveLocal { .. }
                    | machine_code::InternalUnitStructuralArgumentSourceRecord::BlockParameter { .. }
            )
        }) {
            let key = (
                installed.machine,
                custody.code_offset,
                custody.operation_ordinal,
            );
            if previous_call.is_some_and(|previous| previous >= key)
                || !borrowed_structural::call_is_exact(record, function, installed)
            {
                return Err(InstallationError::InvalidInternalUnitCall(
                    installed.machine,
                ));
            }
            previous_call = Some(key);
            continue;
        }
        if custody
            .arguments
            .iter()
            .any(|argument| argument.source.placement().is_none())
        {
            return Err(InstallationError::InvalidInternalUnitCall(
                installed.machine,
            ));
        }
        // Incoming pointer homes describe the caller's parameters, not every
        // call it makes. Only arguments transported from those homes use the
        // legacy incoming-copy checks; a parameterless call still has its
        // ordinary call ABI and attribution even inside an owned-value caller.
        let incoming_call = custody.arguments.iter().any(|argument| {
            matches!(
                argument.source_location,
                machine_code::StructuralSourceLocation::IncomingIndirectPointer { .. }
                    | machine_code::StructuralSourceLocation::IncomingIndirectStackPointer { .. }
            )
        });
        if (incoming_call && !incoming_structural::call_is_exact(record, function, installed))
            || (!incoming_call
                && !matches!(
                    custody.source,
                    machine_code::InternalUnitCallSource::Authored
                ))
        {
            return Err(InstallationError::InvalidInternalUnitCall(
                installed.machine,
            ));
        }
        let callee_parameter_abi = function_by_machine
            .get(&custody.target)
            .and_then(|target| target.parameter_abi.as_ref());
        let callee_unit_parameters = function_by_machine
            .get(&custody.target)
            .map_or(&[][..], |target| target.unit_parameters.as_slice());
        let callee_mixed_abi = function_by_machine
            .get(&custody.target)
            .and_then(|target| target.mixed_structural_scalar_abi.as_ref());
        let target_returns_scalar =
            function_by_machine
                .get(&custody.target)
                .is_some_and(|target| {
                    target.scalar_stack.is_some() || target.structural_call_scalar_return.is_some()
                });
        let target_structural_return = record
            .structural_returns
            .iter()
            .find(|target| target.machine == custody.target)
            .map(|target| &target.returned);
        let structural_result_valid = match (&custody.structural_result, target_structural_return) {
            (None, None) => true,
            (Some(result), Some(target)) => {
                custody.result.is_none()
                    && custody
                        .arguments
                        .iter()
                        .all(|argument| argument.place != result.operation_result.place)
                    && crate::object_artifact::replay::unit::call_custody::structural_result_matches_return(result, target)
            }
            _ => false,
        };
        let callee_mixed_structural_return = target_structural_return.filter(|returned| {
            !returned.scalar_parameters.is_empty()
                || crate::object_artifact::replay::structural::return_record::has_claim_free_affine_identity_custody(returned)
        });
        let expected_text_offset = function
            .text_offset
            .checked_add(custody.code_offset)
            .ok_or(InstallationError::InternalUnitCallOffsetNotRepresentable)?;
        let end = custody
            .code_offset
            .checked_add(custody.byte_count)
            .ok_or(InstallationError::InternalUnitCallOffsetNotRepresentable)?;
        let plan = evaluate_call_plan(
            CallingPolicy::native_for_target(record.target),
            &CallSignature {
                parameters: if let Some(abi) = callee_parameter_abi {
                    abi.parameters
                        .iter()
                        .map(|parameter| {
                            crate::object_artifact::replay::unit::call_custody::unit_scalar_shape(
                                parameter.scalar_type,
                            )
                            .ok_or(
                                InstallationError::InvalidInternalUnitCall(installed.machine),
                            )
                        })
                        .chain(
                            callee_unit_parameters
                                .iter()
                                .map(|parameter| Ok(parameter.shape)),
                        )
                        .collect::<Result<Vec<_>, _>>()?
                } else if let Some(abi) = callee_mixed_abi {
                    abi.scalar_parameters
                        .iter()
                        .map(|parameter| {
                            let semantic_vocabulary::ScalarType::Integer(integer) =
                                parameter.scalar_type
                            else {
                                return Err(InstallationError::InvalidInternalUnitCall(
                                    installed.machine,
                                ));
                            };
                            if integer.is_address() || !matches!(integer.bits(), 8 | 16 | 32 | 64) {
                                return Err(InstallationError::InvalidInternalUnitCall(
                                    installed.machine,
                                ));
                            }
                            let bytes = integer.bits() / 8;
                            Ok(ValueShape::integer(bytes, bytes))
                        })
                        .chain(
                            abi.structural_parameters
                                .iter()
                                .map(|parameter| Ok(parameter.shape)),
                        )
                        .collect::<Result<Vec<_>, _>>()?
                } else if let Some(returned) = callee_mixed_structural_return {
                    returned
                        .scalar_parameters
                        .iter()
                        .map(|parameter| {
                            let semantic_vocabulary::ScalarType::Integer(integer) =
                                parameter.scalar_type
                            else {
                                return Err(InstallationError::InvalidInternalUnitCall(
                                    installed.machine,
                                ));
                            };
                            if integer.is_address() || !matches!(integer.bits(), 8 | 16 | 32 | 64) {
                                return Err(InstallationError::InvalidInternalUnitCall(
                                    installed.machine,
                                ));
                            }
                            let bytes = integer.bits() / 8;
                            Ok(ValueShape::integer(bytes, bytes))
                        })
                        .chain(
                            returned
                                .parameter_placements
                                .iter()
                                .map(|placement| Ok(placement.shape)),
                        )
                        .collect::<Result<Vec<_>, _>>()?
                } else {
                    custody
                        .arguments
                        .iter()
                        .map(|argument| argument.shape)
                        .collect()
                },
                result: if let Some(result) = custody.result {
                    let bytes = match result {
                        semantic_vocabulary::ScalarType::Boolean => 1,
                        semantic_vocabulary::ScalarType::Integer(integer) => {
                            integer.bits().div_ceil(8)
                        }
                        semantic_vocabulary::ScalarType::IeeeFloat(
                            semantic_vocabulary::IeeeFloatFormat::Binary32,
                        ) => 4,
                        semantic_vocabulary::ScalarType::IeeeFloat(
                            semantic_vocabulary::IeeeFloatFormat::Binary64,
                        ) => 8,
                    };
                    Some(match result {
                        semantic_vocabulary::ScalarType::IeeeFloat(_) => ValueShape::float(bytes),
                        _ => ValueShape::integer(bytes, bytes.next_power_of_two().min(8)),
                    })
                } else if custody.structural_result.is_some() {
                    target_structural_return.map(|returned| returned.shape)
                } else {
                    None
                },
            },
        )
        .map_err(|_| InstallationError::InvalidInternalUnitCall(installed.machine))?;
        // The image retains physical call order. Semantic operation ordinals
        // follow the source block roster and need not increase in that order;
        // owner_valid separately binds each ordinal to its exact source span.
        let key = (
            installed.machine,
            custody.code_offset,
            custody.operation_ordinal,
        );
        let projected_argument_indexes = custody
            .arguments
            .iter()
            .enumerate()
            .filter_map(|(index, argument)| (!argument.path.is_empty()).then_some(index))
            .collect::<std::collections::BTreeSet<_>>();
        let transferred_argument_indexes = custody
            .claim_transfers
            .iter()
            .filter_map(|transfer| usize::try_from(transfer.argument_index).ok())
            .collect::<std::collections::BTreeSet<_>>();
        let control_cleanup = match custody.owner {
            CallSiteOwner::CleanupAction { edge, .. } => function
                .scalar_control_affine_cleanups
                .iter()
                .find(|cleanup| cleanup.psi_edge == edge),
            CallSiteOwner::Operation(_) => None,
        };
        let affine_cleanup = function
            .scalar_affine_cleanup
            .as_ref()
            .or_else(|| {
                crate::object_artifact::replay::unit::continuations::cleanup_for_call(
                    &function.unit_continuations,
                    custody.operation_ordinal,
                )
            })
            .or(function.unit_affine_cleanup.as_ref())
            .or(control_cleanup);
        let parameter_homes = if function.scalar_structural_parameter_homes.is_empty() {
            function.unit_parameter_homes.as_slice()
        } else {
            function.scalar_structural_parameter_homes.as_slice()
        };
        let exact_borrowed_argument =
            |index: usize, argument: &machine_code::InternalUnitCallArgumentRecord| {
                parameter_homes
                    .iter()
                    .find(|home| home.place == argument.place)
                    .zip(callee_unit_parameters.get(index))
                    .zip(affine_cleanup)
                    .is_some_and(|((source, destination), cleanup)| {
                        crate::object_artifact::replay::unit::call_custody::exact_borrowed_projection(
                            argument,
                            source,
                            destination,
                            &cleanup.structural_types,
                        )
                    })
            };
        let function_unit_calls = record
            .internal_unit_calls
            .iter()
            .filter(|call| call.machine == installed.machine)
            .map(|call| call.custody.clone())
            .collect::<Vec<_>>();
        let fully_consumed_affine_parameter =
            crate::object_artifact::replay::structural::affine_projected_calls::exact_fully_consumed_affine_parameter(
                &function.unit_parameter_homes,
                &function_unit_calls,
                function.unit_affine_cleanup.as_ref(),
            );
        let projected_result = crate::object_artifact::replay::structural::affine_projected_calls::exact_projected_affine_result(
            parameter_homes,
            &function_unit_calls,
            affine_cleanup,
        )
        .or_else(|| {
            let disposed = crate::object_artifact::replay::unit::continuations::completed_roots(
                parameter_homes,
                &function_unit_calls,
                &function.unit_continuations,
                function.unit_affine_cleanup.as_ref(),
            )?;
            crate::object_artifact::replay::unit::continuations::result_for_call(&function_unit_calls, &disposed, custody)
        });
        let continuation_discards =
            crate::object_artifact::replay::unit::continuations::completed_roots(
                parameter_homes,
                &function_unit_calls,
                &function.unit_continuations,
                function.unit_affine_cleanup.as_ref(),
            )
            .ok_or(InstallationError::InvalidInternalUnitCall(
                installed.machine,
            ))?;
        if !function.unit_continuations.is_empty()
            && custody
                .arguments
                .iter()
                .any(|argument| !argument.path.is_empty())
            && record
                .functions
                .iter()
                .find(|callee| callee.machine == custody.target)
                .is_none_or(|callee| {
                    callee.scalar_abi.is_some()
                        || function.unit_affine_cleanup.as_ref().is_none_or(|cleanup| {
                            !crate::object_artifact::replay::unit::continuations::exact_projected_callee(
                                custody,
                                &callee.unit_parameters,
                                callee.unit_affine_cleanup.as_ref(),
                                cleanup,
                                &record
                                    .semantic_code_attribution
                                    .iter()
                                    .filter(|row| row.machine == callee.machine)
                                    .map(|row| row.attribution)
                                    .collect::<Vec<_>>(),
                            )
                        })
                })
        {
            return Err(InstallationError::InvalidInternalUnitCall(
                installed.machine,
            ));
        }
        if let Some(home) = custody
            .structural_result
            .as_ref()
            .and_then(|result| result.result_home.as_ref())
        {
            let stack =
                function
                    .unit_stack
                    .as_ref()
                    .ok_or(InstallationError::InvalidInternalUnitCall(
                        installed.machine,
                    ))?;
            let call_stack = function
                .unit_call_stacks
                .iter()
                .find(|call| call.owner == custody.owner && call.target == custody.target)
                .ok_or(InstallationError::InvalidInternalUnitCall(
                    installed.machine,
                ))?;
            let linkage = if record.target.architecture == target::Architecture::X86_64 {
                8
            } else {
                0
            };
            let outbound = call_stack.transient_bytes.checked_sub(linkage).ok_or(
                InstallationError::InvalidInternalUnitCall(installed.machine),
            )?;
            let release_bytes = if outbound == 0 {
                0
            } else {
                match record.target.architecture {
                    target::Architecture::X86_64 => {
                        crate::object_artifact::replay::unit::stack::x86_64_stack_adjustment(
                            outbound, true,
                        )
                        .len()
                    }
                    target::Architecture::Aarch64 => 4,
                }
            };
            let store_start = call_stack
                .text_offset
                .checked_sub(function.text_offset)
                .and_then(|offset| offset.checked_add(4))
                .and_then(|offset| offset.checked_add(release_bytes));
            if projected_result.is_none()
                || store_start != Some(home.code_offset)
                || !crate::object_artifact::replay::unit::call_custody::result_home::exact_storage(
                    record.target,
                    custody,
                    &function_unit_calls,
                    stack.frame_bytes,
                    parameter_homes,
                    &function.unit_scalar_homes,
                    function.parameter_abi.as_ref(),
                    None,
                    !function.unit_continuations.is_empty(),
                )
            {
                return Err(InstallationError::InvalidInternalUnitCall(
                    installed.machine,
                ));
            }
        }
        let owner_valid = if incoming_call {
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
        };
        let scalar_count = custody.scalar_arguments.len();
        let selected_scalar_call =
            custody
                .scalar_arguments
                .iter()
                .find_map(|argument| match argument.source {
                    machine_code::InternalUnitScalarArgumentSourceRecord::SelectedCall {
                        instruction,
                        ..
                    } => Some(instruction),
                    _ => None,
                });
        // Selected transport is proved by retained physical replay and compared
        // against the admitted image by validate_installation_record. Its span
        // is the call instruction, not a legacy contiguous materialization.
        let mixed_roster_is_exact = if let Some(call_instruction) = selected_scalar_call {
            callee_parameter_abi.is_some_and(|abi| {
                callee_mixed_abi.is_none()
                    && callee_mixed_structural_return.is_none()
                    && custody.result.is_none()
                    && custody.semantic_result.is_none()
                    && custody.structural_result.is_none()
                    && custody.arguments.is_empty()
                    && callee_unit_parameters.is_empty()
                    && custody.claim_transfers.is_empty()
                    && plan == abi.call_plan
                    && scalar_count == abi.parameters.len()
                    && custody.scalar_arguments.iter().zip(&abi.parameters).enumerate().all(|(parameter_index, (argument, parameter))| {
                        let machine_code::InternalUnitScalarArgumentSourceRecord::SelectedCall { scalar_type, instruction, .. } = argument.source else {
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
                && plan == abi.call_plan
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
                                crate::object_artifact::replay::unit::scalar_call_custody::expected_argument_bytes(
                                    record.target,
                                    &plan,
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
                        exact_borrowed_argument(index, argument)
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
                        && plan == abi.call_plan
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
                                        crate::object_artifact::replay::unit::scalar_call_custody::expected_argument_bytes(
                                            record.target,
                                            &plan,
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
                                        crate::object_artifact::replay::unit::scalar_call_custody::expected_argument_bytes(
                                            record.target,
                                            &plan,
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
        };
        if previous_call.is_some_and(|previous| previous >= key)
            || installed.text_offset != expected_text_offset
            || end > function.byte_count
            || !function_by_machine.contains_key(&custody.target)
            || custody.result.is_some() != target_returns_scalar
            || custody
                .semantic_result
                .as_ref()
                .map(|result| result.scalar_type)
                != custody.result
            || function_by_machine
                .get(&custody.target)
                .is_some_and(|target| {
                    target
                        .structural_call_scalar_return
                        .is_some_and(|returned| custody.result != Some(returned.scalar_type))
                })
            || !structural_result_valid
            || (custody.structural_result.is_some() && target_returns_scalar)
            || !owner_valid
            || !mixed_roster_is_exact
            || plan.parameters.len() != scalar_count + custody.arguments.len()
            || custody.arguments.windows(2).any(|pair| {
                pair[0]
                    .code_offset
                    .checked_add(pair[0].byte_count)
                    .is_none_or(|end| end > pair[1].code_offset)
            })
            || custody
                .arguments
                .iter()
                .zip(&plan.parameters[scalar_count..])
                .enumerate()
                .any(|(argument_index, (argument, destination))| {
                    let Some(source_placement) = argument.source.placement() else { return true; };
                    let parameter_source = parameter_homes
                        .iter()
                        .find(|home| home.place == argument.place)
                        .is_some_and(|home| {
                            argument.root_structural_type == home.structural_type
                                && *source_placement == home.source
                                && source_placement.shape == home.shape
                                && argument.source_location == home.location
                                && (incoming_call || home.location.stack_byte_offset().is_some())
                        });
                    let result_source = projected_result.zip(affine_cleanup).is_some_and(|(result, cleanup)| {
                        crate::object_artifact::replay::structural::affine_projected_calls::exact_owned_result_projection(
                            argument, result, &cleanup.structural_types)
                    });
                    let local_source = affine_cleanup
                        .and_then(|cleanup| {
                            cleanup
                                .locals
                                .iter()
                                .find(|(establishment, place, structural_type)| {
                                    place.id == argument.place
                                        && argument.path.is_empty()
                                        && argument.access == terminal_psi::StructuralAccess::Owned
                                        && argument.root_structural_type == structural_type.id
                                        && argument.structural_type == structural_type.id
                                        && argument.shape == ValueShape::integer(0, 1)
                                        && source_placement.shape == argument.shape
                                        && source_placement.locations.is_empty()
                                        && argument.destination.shape == argument.shape
                                        && argument.destination.locations.is_empty()
                                        && argument.source_location.stack_byte_offset() == Some(0)
                                        && matches!(
                                            place.kind,
                                            semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                                                structural_type: local_type,
                                                construction: None,
                                                ..
                                            } if local_type == structural_type.id
                                        )
                                        && matches!(
                                            structural_type.shape,
                                            terminal_psi::StructuralTypeShape::Record { ref fields }
                                                if fields.is_empty()
                                        )
                                        && record.semantic_code_attribution.iter().any(|row| {
                                            row.machine == installed.machine
                                                && row.attribution.site
                                                    == SemanticCodeSite::Operation(*establishment)
                                                && row.attribution.operation_ordinal
                                                    < custody.operation_ordinal
                                                && row.attribution.byte_count == 0
                                        })
                                })
                        })
                        .is_some_and(|_| {
                            function_unit_calls
                                .iter()
                                .flat_map(|call| &call.arguments)
                                .filter(|candidate| {
                                    candidate.place == argument.place && candidate.path.is_empty()
                                })
                                .count()
                                == 1
                        });
                    let zero_byte_argument = (parameter_source || local_source)
                        && argument.path.is_empty()
                        && argument.byte_count == 0
                        && argument.bytes.is_empty()
                        && argument.shape == ValueShape::integer(0, 1)
                        && source_placement.locations.is_empty()
                        && argument.destination.locations.is_empty();
                    argument.destination != *destination
                        || (!parameter_source && !result_source && !local_source)
                        || (argument.byte_count == 0 && !zero_byte_argument)
                        || argument.bytes.len() != argument.byte_count
                        || (!argument.path.is_empty()
                            && crate::object_artifact::replay::unit::call_custody::expected_projected_copy_bytes(record.target, argument)
                                .as_deref()
                                != Some(argument.bytes.as_slice()))
                        || (!incoming_call && argument.code_offset < custody.code_offset)
                        || argument
                            .code_offset
                            .checked_add(argument.byte_count)
                            .is_none_or(|argument_end| argument_end > if incoming_call { custody.code_offset } else { end })
                        || argument
                            .source_byte_offset
                            .checked_add(u32::from(argument.shape.byte_size))
                            .is_none_or(|end| end > u32::from(source_placement.shape.byte_size))
                        || match argument.path.as_slice() {
                            [] => {
                                argument.source_byte_offset != 0
                                    || source_placement.shape != argument.shape
                                    || argument.root_structural_type != argument.structural_type
                                    || argument.fixed_array_length.is_some()
                                    || argument.element_stride.is_some()
                            }
                            _ if exact_borrowed_argument(argument_index, argument) => false,
                            _ if result_source => false,
                            _ if argument.access == terminal_psi::StructuralAccess::Owned
                                && parameter_homes.iter().any(|home| {
                                    home.place == argument.place
                                        && home.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                                }) =>
                            {
                                parameter_homes.iter().find(|home| home.place == argument.place)
                                    .zip(affine_cleanup)
                                    .is_none_or(|(home, cleanup)| {
                                        !crate::object_artifact::replay::structural::affine_projected_calls::exact_owned_projection(
                                            argument, home, &cleanup.structural_types,
                                        )
                                    })
                            }
                            [StructuralPathSegment::FixedIndex(index)] => {
                                let expected_stride = u32::from(argument.shape.byte_size)
                                    .next_multiple_of(u32::from(argument.shape.alignment));
                                let Some(length) = argument.fixed_array_length else {
                                    return true;
                                };
                                let Some(stride) = argument.element_stride else {
                                    return true;
                                };
                                argument.root_structural_type == argument.structural_type
                                    || *index >= length
                                    || stride != expected_stride
                                    || u64::from(stride).checked_mul(*index)
                                        != Some(u64::from(argument.source_byte_offset))
                                    || u64::from(stride).checked_mul(length)
                                        != Some(u64::from(source_placement.shape.byte_size))
                                    || source_placement.shape.alignment != argument.shape.alignment
                            }
                            [
                                StructuralPathSegment::FixedIndex(outer @ (0 | 1)),
                                StructuralPathSegment::FixedIndex(inner @ (0..=15)),
                            ] => {
                                let leaf_stride = u32::from(argument.shape.byte_size)
                                    .next_multiple_of(u32::from(argument.shape.alignment));
                                let Some(outer_stride) = argument.element_stride else {
                                    return true;
                                };
                                let Some(inner_length) = [
                                    3_u32, 4_u32, 5_u32, 6_u32, 7_u32, 8_u32, 9_u32, 10_u32,
                                    11_u32, 12_u32, 13_u32, 14_u32, 15_u32, 16_u32,
                                ]
                                .into_iter()
                                .find(|length| {
                                    leaf_stride.checked_mul(*length) == Some(outer_stride)
                                }) else {
                                    return true;
                                };
                                let expected_offset = outer_stride
                                    .checked_mul(u32::try_from(*outer).unwrap_or(u32::MAX))
                                    .and_then(|offset| {
                                        leaf_stride
                                            .checked_mul(u32::try_from(*inner).unwrap_or(u32::MAX))
                                            .and_then(|inner| offset.checked_add(inner))
                                    });
                                argument.root_structural_type == argument.structural_type
                                    || argument.fixed_array_length != Some(2)
                                    || *inner >= u64::from(inner_length)
                                    || Some(argument.source_byte_offset) != expected_offset
                                    || outer_stride.checked_mul(2)
                                        != Some(u32::from(source_placement.shape.byte_size))
                                    || source_placement.shape.alignment != argument.shape.alignment
                            }
                            path @ [StructuralPathSegment::Field(_), ..]
                                if path.iter().all(|segment| {
                                    matches!(segment,
                                        StructuralPathSegment::Field(identity)
                                            if !identity.is_empty())
                                }) =>
                            {
                                path.is_empty()
                                    || argument.root_structural_type == argument.structural_type
                                    || argument.fixed_array_length.is_some()
                                    || argument.element_stride.is_some()
                                    || !argument
                                        .source_byte_offset
                                        .is_multiple_of(u32::from(argument.shape.alignment))
                            }
                            _ => true,
                        }
                })
            || projected_argument_indexes.iter().any(|index| {
                if transferred_argument_indexes.contains(index) {
                    return false;
                }
                let Some(argument) = custody.arguments.get(*index) else {
                    return true;
                };
                if exact_borrowed_argument(*index, argument) {
                    return false;
                }
                argument.path.is_empty()
                    || (!fully_consumed_affine_parameter && projected_result.is_none()
                        && !continuation_discards.contains(&argument.place)
                        && affine_cleanup.is_none_or(|cleanup| {
                            !cleanup.actions.iter().any(|action| {
                                matches!(action,
                                terminal_psi::TerminalAffineCleanupAction::DiscardResidual(residual)
                                    if residual.place == argument.place
                                        && !residual.path.is_empty()
                                        && !residual.path.starts_with(&argument.path)
                                        && !argument.path.starts_with(&residual.path)
                                        && residual.structural_type
                                            != argument.root_structural_type)
                            })
                        }))
            })
            || custody.claim_transfers.iter().any(|transfer| {
                usize::try_from(transfer.argument_index)
                    .map_or(true, |index| index >= custody.arguments.len())
            })
            || custody
                .claim_transfers
                .iter()
                .map(|transfer| transfer.claim)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != custody.claim_transfers.len()
        {
            return Err(InstallationError::InvalidInternalUnitCall(
                installed.machine,
            ));
        }
        previous_call = Some(key);
    }
    Ok(())
}
