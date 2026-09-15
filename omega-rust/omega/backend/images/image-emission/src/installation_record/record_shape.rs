//! Shape validation for a decoded installation record: every retained row
//! must be canonical and self-consistent before the record may be compared
//! against object or image evidence.

use super::{
    Architecture, BoundaryRealization, CallSignature, CallSiteOwner, CallingPolicy,
    CompletionCustodyError, InstallationError, InstallationRecord, InstalledFunction, MachineId,
    ObjectFormat, SemanticCodeSite, StructuralMultiplicity, StructuralPathSegment,
    StructuralTypeId, StructuralTypeShape, ValueClass, ValueShape, borrowed_structural,
    boundary_result_is_exact, can_emit_executable_image, direct_structural_return_placement,
    evaluate_call_plan, fingerprint_initialized_data, graph_structural,
    hosted_write_byte_custody_is_exact, incoming_structural,
    installed_forwarded_dynamic_scalar_result_is_canonical,
    installed_function_scalar_transport_is_canonical, installed_scalar_source_is_exact,
    installed_unit_scalar_transport, linux_write_line_custody_is_exact, semantic_code_attribution,
    validate_completion_custody, validate_installed_unit_dynamic_descriptor_joins,
    validate_installed_unit_scalar_calls, validate_installed_unit_structural_scalar_field_stores,
    validate_installed_unit_write_only_primitive_stores,
};
pub(super) fn validate_record_shape(record: &InstallationRecord) -> Result<(), InstallationError> {
    if !record
        .compiler_text_validation
        .has_valid_derivation_digest()
    {
        return Err(InstallationError::InvalidCompilerTextDerivationDigest);
    }
    if !can_emit_executable_image(record.target) {
        return Err(InstallationError::UnsupportedTarget(record.target));
    }
    validate_installed_dynamic_conformance(record)?;
    validate_installed_unit_dynamic_descriptor_joins(record)?;
    match record.target.object_format {
        ObjectFormat::Coff if record.subsystem.is_none() => {
            return Err(InstallationError::MissingCoffSubsystem);
        }
        ObjectFormat::Elf | ObjectFormat::MachO if record.subsystem.is_some() => {
            return Err(InstallationError::UnexpectedSubsystem);
        }
        ObjectFormat::Coff | ObjectFormat::Elf | ObjectFormat::MachO => {}
    }
    if record
        .selected_provider_plans
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(InstallationError::NonCanonicalProviderPlanOrder);
    }
    let selected = record
        .selected_provider_plans
        .iter()
        .map(|provider| provider.get())
        .collect::<std::collections::BTreeSet<_>>();
    let required = record
        .boundary_settlements
        .iter()
        .filter_map(|settlement| {
            let machine_code::BoundaryExecutionRecord::AdmittedProvider(execution) =
                settlement.settlement.execution
            else {
                return None;
            };
            Some(execution.provider_plan_report_identity)
        })
        .chain(
            record
                .functions
                .iter()
                .flat_map(|function| &function.foreign_call_stacks)
                .map(|call| call.provider_plan_report_identity),
        )
        .collect::<std::collections::BTreeSet<_>>();
    if !required.is_subset(&selected) {
        return Err(InstallationError::ProviderSettlementClosureMismatch);
    }
    if record.functions.is_empty() {
        return Err(InstallationError::NoInstalledFunctions);
    }
    let mut expected_text_offset = 0_usize;
    let mut previous_function = None;
    let attachments = record
        .functions
        .iter()
        .map(|function| (function.machine, function.attachment))
        .collect::<std::collections::BTreeMap<_, _>>();
    for function in &record.functions {
        let function_unit_calls = record
            .internal_unit_calls
            .iter()
            .filter(|call| call.machine == function.machine)
            .map(|call| call.custody.clone())
            .collect::<Vec<_>>();
        let continuation_discards =
            crate::object_artifact::replay::unit::continuations::completed_roots(
                &function.unit_parameter_homes,
                &function_unit_calls,
                &function.unit_continuations,
                function.unit_affine_cleanup.as_ref(),
            )
            .ok_or(InstallationError::InvalidUnitAffineCleanup(
                function.machine,
            ))?;
        if !function.unit_continuations.is_empty()
            && crate::object_artifact::replay::unit::scalar_call_custody::entry_spills::validate_shape(
                record.target,
                &function.unit_parameter_homes,
                function.parameter_abi.as_ref(),
                true,
                function
                    .unit_stack
                    .as_ref()
                    .map_or(0, |stack| stack.frame_bytes),
            )
            .is_none_or(|end| {
                function.unit_stack.as_ref().is_none_or(|stack| {
                    end > stack.frame_bytes
                        || (function_unit_calls
                            .iter()
                            .all(|call| call.structural_result.is_none())
                            && !crate::object_artifact::replay::unit::call_custody::result_home::exact_frame(
                                record.target,
                                end,
                                stack.frame_bytes,
                                None,
                            ))
                })
            })
        {
            return Err(InstallationError::InvalidUnitAffineCleanup(
                function.machine,
            ));
        }
        if function.unit_body
            && !crate::object_artifact::replay::unit::continuations::exact_attribution(
                &function.unit_continuations,
                function.unit_affine_cleanup.as_ref(),
                function_unit_calls.len(),
                &record
                    .semantic_code_attribution
                    .iter()
                    .filter(|row| row.machine == function.machine)
                    .map(|row| row.attribution)
                    .collect::<Vec<_>>(),
            )
        {
            return Err(InstallationError::InvalidUnitAffineCleanup(
                function.machine,
            ));
        }
        if !function.unit_continuations.is_empty()
            && (!function.unit_body
                || function.scalar_stack.is_some()
                || function.scalar_abi.is_some()
                || function.scalar_affine_cleanup.is_some()
                || !function.scalar_control_affine_cleanups.is_empty()
                || function.structural_call_scalar_return.is_some()
                || !crate::object_artifact::replay::unit::continuations::exact_scalar_bindings(
                    function.parameter_abi.as_ref(),
                    &function.unit_continuations,
                )
                || !function.unit_scalar_homes.is_empty()
                || !function.unit_integer_constants.is_empty()
                || !function.unit_affine_scalar_records.is_empty()
                || !function.unit_structural_scalar_field_stores.is_empty()
                || !function.unit_write_only_primitive_stores.is_empty()
                || record
                    .boundary_settlements
                    .iter()
                    .any(|settlement| settlement.machine == function.machine))
        {
            return Err(InstallationError::InvalidUnitAffineCleanup(
                function.machine,
            ));
        }
        if function.unit_continuations.is_empty()
            && function
                .parameter_abi
                .as_ref()
                .is_some_and(|abi| !abi.entry_register_spills.is_empty())
        {
            return Err(InstallationError::InvalidUnitAffineCleanup(
                function.machine,
            ));
        }
        let fully_consumed_affine_parameter =
            crate::object_artifact::replay::structural::affine_projected_calls::exact_fully_consumed_affine_parameter(
                &function.unit_parameter_homes,
                &function_unit_calls,
                function.unit_affine_cleanup.as_ref(),
            );
        let partially_consumed_affine_parameter =
            crate::object_artifact::replay::structural::affine_projected_calls::exact_partially_consumed_affine_parameter(
                &function.unit_parameter_homes,
                &function_unit_calls,
                function.unit_affine_cleanup.as_ref(),
            );
        let projected_affine_result = crate::object_artifact::replay::structural::affine_projected_calls::exact_projected_affine_result(
            &function.unit_parameter_homes,
            &function_unit_calls,
            function.unit_affine_cleanup.as_ref(),
        );
        if function.byte_count == 0
            || function.text_offset != expected_text_offset
            || previous_function.is_some_and(|previous| previous >= function.machine)
        {
            return Err(InstallationError::NonCanonicalInstalledFunctions);
        }
        let has_scalar_control_cleanup = !function.scalar_control_affine_cleanups.is_empty();
        let has_scalar_cleanup =
            function.scalar_affine_cleanup.is_some() || has_scalar_control_cleanup;
        let has_scalar_boundary_custody = record.boundary_settlements.iter().any(|settlement| {
            settlement.machine == function.machine
                && matches!(
                    settlement.settlement.realization,
                    BoundaryRealization::DirectPortReadU8(_)
                        | BoundaryRealization::HostedExitProcessI32(_)
                )
        });
        let has_scalar_custody = has_scalar_cleanup
            || has_scalar_boundary_custody
            || function.mixed_structural_scalar_abi.is_some()
            || !function.scalar_structural_scalar_field_stores.is_empty();
        let structural_call_scalar_result_is_exact = function
            .structural_call_scalar_return
            .is_none_or(|returned| {
                let attributions = record
                    .semantic_code_attribution
                    .iter()
                    .filter(|attribution| attribution.machine == function.machine)
                    .map(|attribution| &attribution.attribution)
                    .collect::<Vec<_>>();
                function.unit_body
                    && function.unit_stack.is_some()
                    && function.scalar_stack.is_none()
                    && matches!(
                        (
                            function_unit_calls.as_slice(),
                            attributions.as_slice(),
                            function.unit_affine_cleanup.as_ref(),
                        ),
                        ([call], [call_attribution, return_attribution], Some(cleanup))
                        if call.owner == CallSiteOwner::Operation(returned.psi_operation)
                            && call.target == returned.callee
                            && call.operation_ordinal == 0
                            && call.result == Some(returned.scalar_type)
                            && call.semantic_result.as_ref().is_some_and(|result| {
                                result.value == returned.source_value
                                    && result.scalar_type == returned.scalar_type
                            })
                            && call_attribution.site
                                == SemanticCodeSite::Operation(returned.psi_operation)
                            && call_attribution.operation_ordinal == call.operation_ordinal
                            && call_attribution.code_offset == call.code_offset
                            && call_attribution.byte_count == call.byte_count
                            && return_attribution.site == SemanticCodeSite::Edge(returned.psi_edge)
                            && return_attribution.operation_ordinal == 1
                            && return_attribution.code_offset == cleanup.code_offset
                            && return_attribution.byte_count == cleanup.byte_count
                            && cleanup.psi_edge == returned.psi_edge
                            && cleanup.locals.is_empty()
                            && cleanup.actions.is_empty()
                    )
            });
        // This is a record-shape check, not erasure authority. The installation
        // is joined field-for-field to the independently admitted image below;
        // image replay requires retained source for every missing home.
        let unmaterialized_owned = function.scalar_structural_parameter_homes.is_empty()
            && function
                .mixed_structural_scalar_abi
                .as_ref()
                .is_some_and(|abi| {
                    !abi.structural_parameters.is_empty()
                        && abi.structural_parameters.iter().all(|parameter| {
                            parameter.access == terminal_psi::StructuralAccess::Owned
                                && matches!(
                                    parameter.multiplicity,
                                    StructuralMultiplicity::Affine
                                        | StructuralMultiplicity::Unrestricted
                                )
                                && parameter.projected_qualifications.is_empty()
                                && parameter.shape.class
                                    != calling_conventions::ValueClass::BorrowedReference
                        })
                });
        let graph_structural_roster = graph_structural::function_is_exact(function, record.target);
        let mixed_structural_roster_is_exact = graph_structural_roster || function
            .mixed_structural_scalar_abi
            .as_ref()
            .is_none_or(|abi| {
                function.scalar_structural_parameters.len() == abi.structural_parameters.len()
                    && (unmaterialized_owned || function.scalar_structural_parameter_homes.len()
                        == abi.structural_parameters.len())
                    && function
                        .scalar_structural_parameters
                        .iter()
                        .zip(&abi.structural_parameters)
                        .all(|(parameter, retained)| {
                            parameter.place == retained.place
                                && parameter.structural_type == retained.structural_type
                                && parameter.multiplicity == retained.multiplicity
                                && parameter.access == retained.access
                                && parameter.shape == retained.shape
                        })
                    && function.scalar_structural_parameter_homes.iter()
                        .zip(&abi.structural_parameters)
                        .all(|(home, retained)| {
                            home.place == retained.place
                                && home.structural_type == retained.structural_type
                                && home.multiplicity == retained.multiplicity
                                && home.access == retained.access
                                && home.shape == retained.shape
                                && home.source == retained.placement
                                && installed_unit_scalar_transport::mixed_structural_home_is_canonical(home, retained)
                        })
            });
        if !installed_stack_facts_are_canonical(function, &attachments)
            || !installed_function_scalar_transport_is_canonical(function, record.target)
            || !structural_call_scalar_result_is_exact
            || !mixed_structural_roster_is_exact
            || (!graph_structural_roster
                && function.unit_parameters.len() != function.unit_parameter_homes.len())
            || function.unit_body != function.unit_affine_cleanup.is_some()
            || (incoming_structural::has_incoming(function)
                && !incoming_structural::function_is_exact(record, function))
            || (!graph_structural_roster
                && borrowed_structural::has_borrowed(function)
                && !borrowed_structural::function_is_exact(record, function))
            || (!graph_structural_roster
                && !incoming_structural::has_incoming(function)
                && !borrowed_structural::has_borrowed(function)
                && !function.unit_body
                && !has_scalar_cleanup
                && (!function.unit_parameters.is_empty()
                    || !function.unit_parameter_homes.is_empty()))
            || (!graph_structural_roster
                && !unmaterialized_owned
                && function.scalar_structural_parameters.len()
                    != function.scalar_structural_parameter_homes.len())
            || (!function.scalar_control_affine_cleanups.is_empty()
                && function.scalar_control_affine_cleanups.len() < 2)
            || (function.scalar_affine_cleanup.is_some() && has_scalar_control_cleanup)
            || (has_scalar_cleanup && function.unit_body)
            || (!graph_structural_roster
                && function
                    .scalar_structural_parameters
                    .iter()
                    .zip(&function.scalar_structural_parameter_homes)
                    .any(|(parameter, home)| {
                        parameter.place != home.place
                            || parameter.structural_type != home.structural_type
                            || parameter.multiplicity != home.multiplicity
                            || parameter.access != home.access
                            || parameter.shape != home.shape
                    }))
            || (!has_scalar_custody
                && (!function.scalar_structural_parameters.is_empty()
                    || !function.scalar_structural_parameter_homes.is_empty()))
            || (!graph_structural_roster
                && function
                    .unit_parameters
                    .iter()
                    .zip(&function.unit_parameter_homes)
                    .any(|(parameter, home)| {
                        parameter.place != home.place
                            || parameter.structural_type != home.structural_type
                            || parameter.multiplicity != home.multiplicity
                            || parameter.access != home.access
                            || parameter.shape != home.shape
                    }))
        {
            return Err(InstallationError::InvalidUnitAffineCleanup(
                function.machine,
            ));
        }
        if let Some(cleanup) = &function.unit_affine_cleanup {
            if !crate::object_artifact::replay::unit::affine_cleanup::exact_construction_prefix(
                cleanup,
            ) {
                return Err(InstallationError::InvalidUnitAffineCleanup(
                    function.machine,
                ));
            }
            let end = cleanup
                .code_offset
                .checked_add(cleanup.byte_count)
                .ok_or(InstallationError::FunctionOffsetNotRepresentable)?;
            let expected_local_prefix = cleanup
                .locals
                .iter()
                .rev()
                .map(|(_, place, _)| place.id)
                .collect::<Vec<_>>();
            let structural_result_prefix = record
                .internal_unit_calls
                .iter()
                .filter(|call| call.machine == function.machine)
                .rev()
                .filter_map(|call| match call.custody.structural_result.as_ref() {
                    Some(result)
                        if projected_affine_result.is_none()
                            && !continuation_discards.contains(&result.operation_result.place)
                            && result.operation_result.multiplicity
                                == StructuralMultiplicity::Affine
                            && result.operation_result.claims.is_empty()
                            && result.returned_claim_transfers.is_empty()
                            && result.returned_claims.is_empty() =>
                    {
                        Some(result.operation_result.place)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let expected_cleanup_prefix = structural_result_prefix
                .iter()
                .copied()
                .chain(expected_local_prefix.iter().copied())
                .collect::<Vec<_>>();
            let transferred_roots = record
                .internal_unit_calls
                .iter()
                .filter(|call| call.machine == function.machine)
                .flat_map(|call| &call.custody.arguments)
                .filter(|argument| argument.path.is_empty())
                .map(|argument| argument.place)
                .collect::<std::collections::BTreeSet<_>>();
            let expected_parameter_discards = function
                .unit_parameter_homes
                .iter()
                .rev()
                .filter(|home| {
                    home.multiplicity == StructuralMultiplicity::Affine
                        && home.access == terminal_psi::StructuralAccess::Owned
                        && !transferred_roots.contains(&home.place)
                        && !fully_consumed_affine_parameter
                        && !continuation_discards.contains(&home.place)
                })
                .map(|home| home.place)
                .collect::<Vec<_>>();
            let discards = cleanup
                .actions
                .iter()
                .filter_map(|action| match action {
                    terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place) => Some(*place),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let residual_discards = cleanup
                .actions
                .iter()
                .filter_map(|action| match action {
                    terminal_psi::TerminalAffineCleanupAction::DiscardResidual(discard) => {
                        Some(discard)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let nominal_cleanups = cleanup
                .actions
                .iter()
                .filter_map(|action| match action {
                    terminal_psi::TerminalAffineCleanupAction::InvokeNominal(nominal) => {
                        Some(nominal)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let exact_nominal_body = |nominal: &terminal_psi::NominalAffineCleanup| {
                if nominal.cleanup_receiver.is_some() || !nominal.requirement_obligations.is_empty()
                {
                    return None;
                }
                let target = record
                    .functions
                    .iter()
                    .find(|candidate| candidate.machine == nominal.cleanup_machine)?;
                let calls = record
                    .internal_unit_calls
                    .iter()
                    .filter(|call| call.machine == nominal.cleanup_machine)
                    .collect::<Vec<_>>();
                let owners = calls
                    .iter()
                    .map(|call| call.custody.owner)
                    .collect::<std::collections::BTreeSet<_>>();
                let targets = calls
                    .iter()
                    .map(|call| call.custody.target)
                    .collect::<std::collections::BTreeSet<_>>();
                (target.attachment == Some(nominal.structural_type)
                    && target.unit_body
                    && target.unit_parameters.is_empty()
                    && target.unit_parameter_homes.is_empty()
                    && target
                        .unit_affine_cleanup
                        .as_ref()
                        .is_some_and(|return_cleanup| {
                            return_cleanup.locals.is_empty() && return_cleanup.actions.is_empty()
                        })
                    && owners.len() == calls.len()
                    && targets.len() == calls.len()
                    && calls.iter().enumerate().all(|(ordinal, call)| {
                        matches!(call.custody.owner, CallSiteOwner::Operation(_))
                            && call.custody.operation_ordinal == ordinal
                            && call.custody.result.is_none()
                            && call.custody.structural_result.is_none()
                            && call.custody.arguments.is_empty()
                            && call.custody.claim_transfers.is_empty()
                            && record.functions.iter().any(|helper| {
                                helper.machine == call.custody.target
                                    && helper.attachment.is_some()
                                    && helper.unit_body
                                    && helper.unit_parameters.is_empty()
                                    && helper.unit_parameter_homes.is_empty()
                                    && helper.unit_affine_cleanup.as_ref().is_some_and(
                                        |return_cleanup| {
                                            return_cleanup.locals.is_empty()
                                                && return_cleanup.actions.is_empty()
                                        },
                                    )
                                    && !record
                                        .internal_unit_calls
                                        .iter()
                                        .any(|helper_call| helper_call.machine == helper.machine)
                            })
                    })
                    && calls.windows(2).all(|pair| {
                        pair[0]
                            .custody
                            .code_offset
                            .checked_add(pair[0].custody.byte_count)
                            .is_some_and(|end| end <= pair[1].custody.code_offset)
                    }))
                .then_some(!calls.is_empty())
            };
            let Some(parameter_discards) = discards.get(expected_cleanup_prefix.len()..) else {
                return Err(InstallationError::InvalidUnitAffineCleanup(
                    function.machine,
                ));
            };
            let local_operations = cleanup
                .locals
                .iter()
                .map(|(operation, _, _)| *operation)
                .collect::<std::collections::BTreeSet<_>>();
            if cleanup.byte_count == 0
                || end != function.byte_count
                || local_operations.len() != cleanup.locals.len()
                || cleanup.locals.iter().enumerate().any(
                    |(ordinal, (_, place, structural_type))| {
                        !matches!(
                            place.kind,
                            semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                                declaration_ordinal,
                                structural_type: local_type,
                                ..
                            } if usize::try_from(declaration_ordinal) == Ok(ordinal)
                                && local_type == structural_type.id
                        ) || !matches!(
                            structural_type.shape,
                            StructuralTypeShape::Record { ref fields } if fields.is_empty()
                        )
                    },
                )
                || discards.get(..expected_cleanup_prefix.len())
                    != Some(expected_cleanup_prefix.as_slice())
                || discards
                    .iter()
                    .copied()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    != discards.len()
                || discards.len() + residual_discards.len() + nominal_cleanups.len()
                    != cleanup.actions.len()
                || match (nominal_cleanups.as_slice(), residual_discards.as_slice()) {
                    _ if projected_affine_result.is_some() => false,
                    ([nominal], []) => {
                        let cleanup_is_executable = exact_nominal_body(nominal);
                        let matching_cleanup_calls = record
                            .internal_unit_calls
                            .iter()
                            .filter(|call| {
                                call.machine == function.machine
                                    && call.custody.owner
                                        == CallSiteOwner::CleanupAction {
                                            edge: cleanup.psi_edge,
                                            action_ordinal: 0,
                                        }
                                    && call.custody.target == nominal.cleanup_machine
                                    && call.custody.arguments.is_empty()
                                    && call.custody.claim_transfers.is_empty()
                                    && call.custody.code_offset == cleanup.code_offset
                            })
                            .count();
                        !cleanup.locals.is_empty()
                            || !discards.is_empty()
                            || function.unit_parameter_homes.len() != 1
                            || function.unit_parameter_homes[0].place != nominal.place
                            || function.unit_parameter_homes[0].structural_type
                                != nominal.structural_type
                            || function.unit_parameter_homes[0].multiplicity
                                != StructuralMultiplicity::Affine
                            || !bounded_nominal_receiver_shape(
                                function.unit_parameter_homes[0].shape,
                            )
                            || (function.unit_parameter_homes[0].shape.byte_size == 0
                                && !function.unit_parameter_homes[0].source.locations.is_empty())
                            || (function.unit_parameter_homes[0].shape.byte_size != 0
                                && function.unit_parameter_homes[0].source.locations.is_empty())
                            || attachments.get(&nominal.cleanup_machine)
                                != Some(&Some(nominal.structural_type))
                            || cleanup_is_executable.is_none()
                            || matching_cleanup_calls
                                != usize::from(cleanup_is_executable == Some(true))
                    }
                    ([], []) => parameter_discards != expected_parameter_discards,
                    ([], residuals @ [_, ..]) => {
                        let residual_root = residuals[0].place;
                        let parameter_type = function
                            .unit_parameters
                            .iter()
                            .find(|parameter| parameter.place == residual_root)
                            .map(|parameter| parameter.structural_type);
                        let moved = record
                            .internal_unit_calls
                            .iter()
                            .filter(|call| call.machine == function.machine)
                            .flat_map(|call| &call.custody.arguments)
                            .filter(|argument| {
                                argument.place == residual_root
                                    && Some(argument.root_structural_type) == parameter_type
                            })
                            .map(|argument| (argument.path.as_slice(), argument.structural_type))
                            .collect::<Vec<_>>();
                        cleanup.actions.get(..discards.len()).is_none_or(|prefix| {
                            !prefix.iter().zip(&discards).all(|(action, place)| {
                                matches!(action,
                                    terminal_psi::TerminalAffineCleanupAction::DiscardRoot(actual)
                                        if actual == place)
                            })
                        })
                            || cleanup.actions.get(discards.len()..).is_none_or(|suffix| {
                                suffix.iter().zip(residuals).any(|(action, residual)| {
                                    !matches!(action,
                                        terminal_psi::TerminalAffineCleanupAction::DiscardResidual(actual)
                                            if actual == *residual)
                                })
                            })
                            || !parameter_discards.is_empty()
                            || expected_parameter_discards.as_slice() != [residual_root]
                            || residuals.iter().any(|residual| {
                                residual.place != residual_root
                                    || residual.path.is_empty()
                                    || !is_partial_cleanup_path(&residual.path)
                                    || parameter_type == Some(residual.structural_type)
                            })
                            || residuals
                                .iter()
                                .enumerate()
                                .any(|(index, residual)| {
                                    residuals[..index].iter().any(|earlier| {
                                        residual.path.starts_with(&earlier.path)
                                            || earlier.path.starts_with(&residual.path)
                                    })
                                })
                            || parameter_type.is_none()
                            || (moved.iter().any(|(path, _)| {
                                path.iter().any(|segment| {
                                    matches!(segment, StructuralPathSegment::FixedIndex(_))
                                })
                            }) && !partially_consumed_affine_parameter)
                            || moved.is_empty()
                            || moved.iter().any(|(path, _)| {
                                path.is_empty()
                                    || !is_partial_cleanup_path(path)
                                    || residuals
                                        .iter()
                                        .any(|residual| {
                                            path.starts_with(&residual.path)
                                                || residual.path.starts_with(path)
                                        })
                            })
                            || moved
                                .iter()
                                .enumerate()
                                .any(|(index, (path, _))| {
                                    moved[..index].iter().any(|(earlier, _)| {
                                        path.starts_with(earlier)
                                            || earlier.starts_with(path)
                                    })
                                })
                            || parameter_type.is_none_or(|root_type| {
                                !crate::object_artifact::replay::structural::partial_cleanup_partition::exact_partial_cleanup_partition(
                                    &cleanup.structural_types,
                                    root_type,
                                    &moved,
                                    residuals,
                                )
                            })
                    }
                    (nominal @ [_, _, ..], []) => {
                        let bodies = nominal
                            .iter()
                            .map(|cleanup| exact_nominal_body(cleanup))
                            .collect::<Vec<_>>();
                        let executable = bodies
                            .iter()
                            .enumerate()
                            .filter_map(|(ordinal, body)| (*body == Some(true)).then_some(ordinal))
                            .collect::<Vec<_>>();
                        let caller_cleanup_calls = record
                            .internal_unit_calls
                            .iter()
                            .filter(|call| {
                                call.machine == function.machine
                                    && matches!(call.custody.owner,
                                        CallSiteOwner::CleanupAction { edge, .. }
                                            if edge == cleanup.psi_edge)
                            })
                            .collect::<Vec<_>>();
                        let ordered_executable_spans = executable
                            .iter()
                            .map(|ordinal| {
                                let action_ordinal = u32::try_from(*ordinal).ok()?;
                                let call = caller_cleanup_calls.iter().find(|call| {
                                    call.custody.owner
                                        == CallSiteOwner::CleanupAction {
                                            edge: cleanup.psi_edge,
                                            action_ordinal,
                                        }
                                        && call.custody.target == nominal[*ordinal].cleanup_machine
                                })?;
                                Some((
                                    call.custody.code_offset,
                                    call.custody
                                        .code_offset
                                        .checked_add(call.custody.byte_count)?,
                                ))
                            })
                            .collect::<Option<Vec<_>>>();
                        !cleanup.locals.is_empty()
                            || !discards.is_empty()
                            || function.unit_parameter_homes.len() != nominal.len()
                            || function.unit_parameter_homes.iter().rev().zip(nominal).any(
                                |(home, nominal)| {
                                    home.place != nominal.place
                                        || home.structural_type != nominal.structural_type
                                        || home.multiplicity != StructuralMultiplicity::Affine
                                        || !bounded_nominal_receiver_shape(home.shape)
                                        || (home.shape.byte_size == 0
                                            && !home.source.locations.is_empty())
                                        || (home.shape.byte_size != 0
                                            && home.source.locations.is_empty())
                                        || attachments.get(&nominal.cleanup_machine)
                                            != Some(&Some(nominal.structural_type))
                                },
                            )
                            || bodies.iter().any(Option::is_none)
                            || caller_cleanup_calls.len() != executable.len()
                            || ordered_executable_spans.is_none_or(|spans| {
                                spans
                                    .windows(2)
                                    .any(|pair| pair[0].0 >= pair[1].0 || pair[0].1 > pair[1].0)
                            })
                            || executable.iter().any(|ordinal| {
                                let action_ordinal = u32::try_from(*ordinal).ok();
                                action_ordinal.is_none_or(|action_ordinal| {
                                    caller_cleanup_calls
                                        .iter()
                                        .filter(|call| {
                                            call.custody.owner
                                                == CallSiteOwner::CleanupAction {
                                                    edge: cleanup.psi_edge,
                                                    action_ordinal,
                                                }
                                                && call.custody.target
                                                    == nominal[*ordinal].cleanup_machine
                                                && call.custody.arguments.is_empty()
                                                && call.custody.claim_transfers.is_empty()
                                                && call.custody.code_offset >= cleanup.code_offset
                                                && call
                                                    .custody
                                                    .code_offset
                                                    .checked_add(call.custody.byte_count)
                                                    .is_some_and(|call_end| call_end <= end)
                                        })
                                        .count()
                                        != 1
                                })
                            })
                    }
                    _ => true,
                }
            {
                return Err(InstallationError::InvalidUnitAffineCleanup(
                    function.machine,
                ));
            }
        }
        if let Some(cleanup) = &function.scalar_affine_cleanup {
            validate_scalar_affine_cleanup_shape(record, function, cleanup, true)?;
        }
        if has_scalar_control_cleanup {
            validate_scalar_control_affine_cleanup_shape(record, function)?;
        }
        expected_text_offset = expected_text_offset
            .checked_add(function.byte_count)
            .ok_or(InstallationError::FunctionOffsetNotRepresentable)?;
        previous_function = Some(function.machine);
    }
    let mut adapter_identities = std::collections::BTreeSet::new();
    for adapter in &record.forwarded_dynamic_descriptor_adapters {
        if adapter.application_commitment.is_zero()
            || adapter.byte_count == 0
            || adapter.text_offset != expected_text_offset
            || !adapter_identities.insert((
                adapter.application_commitment,
                adapter.row_index,
                adapter.realization,
            ))
        {
            return Err(InstallationError::InvalidForwardedDynamicDescriptorAdapter);
        }
        expected_text_offset = expected_text_offset
            .checked_add(adapter.byte_count)
            .ok_or(InstallationError::FunctionOffsetNotRepresentable)?;
    }
    if record.private_functions.len() > 1 {
        return Err(InstallationError::TooManyCompilerPrivateFunctions);
    }
    for private in &record.private_functions {
        if private.identity.callback_thunk_placement_index().is_none()
            || !private.identity.is_valid()
            || private.byte_count == 0
            || private.text_offset != expected_text_offset
            || !installed_unit_scalar_transport::installed_scalar_abi_is_canonical(
                &private.scalar_abi,
                record.target,
            )
        {
            return Err(InstallationError::InvalidCompilerPrivateFunction);
        }
        expected_text_offset = expected_text_offset
            .checked_add(private.byte_count)
            .ok_or(InstallationError::CompilerPrivateFunctionOffsetNotRepresentable)?;
    }
    if expected_text_offset != record.image_sections.text_byte_count {
        return Err(InstallationError::InvalidImageSectionLayout);
    }
    let function_by_machine = record
        .functions
        .iter()
        .map(|function| (function.machine, function))
        .collect::<std::collections::BTreeMap<_, _>>();
    validate_installed_unit_scalar_calls(record, &function_by_machine)?;
    validate_installed_unit_structural_scalar_field_stores(record, &function_by_machine)?;
    validate_installed_unit_write_only_primitive_stores(record, &function_by_machine)?;
    let mut previous_return = None;
    for installed in &record.structural_returns {
        let function = function_by_machine.get(&installed.machine).ok_or(
            InstallationError::StructuralReturnMachineMissing(installed.machine),
        )?;
        let returned = &installed.returned;
        let scalar_shapes = returned
            .scalar_parameters
            .iter()
            .map(|parameter| {
                let semantic_vocabulary::ScalarType::Integer(integer) = parameter.scalar_type
                else {
                    return None;
                };
                if integer.is_address() || !matches!(integer.bits(), 8 | 16 | 32 | 64) {
                    return None;
                }
                let bytes = integer.bits() / 8;
                Some(ValueShape::integer(bytes, bytes))
            })
            .collect::<Option<Vec<_>>>()
            .ok_or(InstallationError::InvalidStructuralReturn(
                installed.machine,
            ))?;
        let expected_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(record.target),
            &CallSignature {
                parameters: scalar_shapes
                    .iter()
                    .copied()
                    .chain(
                        returned
                            .parameter_placements
                            .iter()
                            .map(|placement| placement.shape),
                    )
                    .collect(),
                result: Some(returned.shape),
            },
        )
        .map_err(|_| InstallationError::InvalidStructuralReturn(installed.machine))?;
        let expected_result = expected_plan.result.as_ref();
        let structural_attribution = record
            .semantic_code_attribution
            .iter()
            .filter(|attribution| attribution.machine == installed.machine)
            .collect::<Vec<_>>();
        let exact_claimful_linear = returned.scalar_parameters.is_empty()
            && returned.source.multiplicity == StructuralMultiplicity::Linear
            && returned.result.multiplicity == StructuralMultiplicity::Linear
            && returned.returned_claims.len() == 1;
        let exact_claim_free_affine =
            crate::object_artifact::replay::structural::return_record::has_claim_free_affine_identity_custody(returned);
        if previous_return.is_some_and(|previous| previous >= installed.machine)
            || !returned.result.reference_sources.is_empty()
            || returned.code_offset != 0
            || returned.byte_count != function.byte_count
            || returned.source.position != 0
            || returned.source.is_self
            || (!exact_claimful_linear && !exact_claim_free_affine)
            || returned.source.structural_type != returned.result.structural_type
            || returned.source.qualifications != returned.result.qualifications
            || returned.source.projected_qualifications
                != returned.result.projected_qualifications
            || returned.source.place == returned.result.place
            || returned.shape.class != ValueClass::Integer
            || !((returned.shape.byte_size == 8 && returned.shape.alignment == 8)
                || (9..=16).contains(&returned.shape.byte_size))
            || returned.source_placement.shape != returned.shape
            || returned.result_placement.shape != returned.shape
            || !direct_structural_return_placement(&returned.source_placement)
            || !direct_structural_return_placement(&returned.result_placement)
            || returned.parameters.first() != Some(&returned.source)
            || returned.parameters.iter().skip(1).any(|parameter| {
                parameter.place == returned.source.place
                    || parameter.place == returned.result.place
                    || !parameter.qualifications.is_empty()
            })
            || returned
                .parameters
                .iter()
                .map(|parameter| parameter.place)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != returned.parameters.len()
            || returned.trivial_affine_locals.iter().enumerate().any(|(index, (_, local, local_type))| {
                !matches!(
                    local.kind,
                    semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                        declaration_ordinal,
                        structural_type,
                        construction: None,
                    } if usize::try_from(declaration_ordinal) == Ok(index)
                        && structural_type == local_type.id
                ) || local.id == returned.source.place
                    || local.id == returned.result.place
                    || returned.parameters.iter().any(|parameter| parameter.place == local.id)
                    || local_type.identity.is_empty()
                    || !matches!(
                        local_type.shape,
                        terminal_psi::StructuralTypeShape::Record { ref fields } if fields.is_empty()
                    )
            })
            || returned
                .trivial_affine_locals
                .iter()
                .map(|(_, local, _)| local.id)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != returned.trivial_affine_locals.len()
            || returned.parameter_placements.len() != returned.parameters.len()
            || expected_plan.parameters.len()
                != returned.scalar_parameters.len() + returned.parameter_placements.len()
            || expected_plan.parameters[..returned.scalar_parameters.len()]
                .iter()
                .zip(&returned.scalar_parameters)
                .any(|(placement, parameter)| placement != &parameter.placement)
            || expected_plan.parameters[returned.scalar_parameters.len()..]
                != returned.parameter_placements
            || returned.parameter_placements.first() != Some(&returned.source_placement)
            || returned
                .parameters
                .iter()
                .enumerate()
                .any(|(index, parameter)| {
                    parameter.is_self || usize::try_from(parameter.position) != Ok(index)
                })
            || returned.trivial_affine_discards
                != returned
                    .trivial_affine_locals
                    .iter()
                    .rev()
                    .map(|(_, local, _)| local.id)
                    .chain(
                        returned
                            .parameters
                            .iter()
                            .skip(1)
                            .rev()
                            .map(|parameter| parameter.place),
                    )
                    .collect::<Vec<_>>()
            || returned
                .parameters
                .iter()
                .skip(1)
                .any(|parameter| parameter.multiplicity != StructuralMultiplicity::Affine)
            || expected_result != Some(&returned.result_placement)
            || structural_attribution.len() != returned.trivial_affine_locals.len() + 1
            || returned
                .trivial_affine_locals
                .iter()
                .enumerate()
                .any(|(ordinal, (operation, _, _))| {
                    structural_attribution.get(ordinal).is_none_or(|installed| {
                        installed.attribution.site
                                != SemanticCodeSite::Operation(*operation)
                            || installed.attribution.operation_ordinal != ordinal
                            || installed.attribution.code_offset != 0
                            || installed.attribution.byte_count != 0
                    })
                })
            || structural_attribution.last().is_none_or(|installed| {
                installed.attribution.site
                        != SemanticCodeSite::Edge(returned.psi_edge)
                    || installed.attribution.operation_ordinal
                        != returned.trivial_affine_locals.len()
                    || installed.attribution.code_offset != 0
                    || installed.attribution.byte_count != returned.byte_count
            })
        {
            return Err(InstallationError::InvalidStructuralReturn(
                installed.machine,
            ));
        }
        previous_return = Some(installed.machine);
    }
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
    semantic_code_attribution::validate_order(&record.semantic_code_attribution)?;
    for installed in &record.semantic_code_attribution {
        let function = function_by_machine.get(&installed.machine).ok_or(
            InstallationError::SemanticCodeAttributionMachineMissing(installed.machine),
        )?;
        let expected = function
            .text_offset
            .checked_add(installed.attribution.code_offset)
            .ok_or(InstallationError::SemanticCodeAttributionOffsetNotRepresentable)?;
        let end = installed
            .attribution
            .code_offset
            .checked_add(installed.attribution.byte_count)
            .ok_or(InstallationError::SemanticCodeAttributionOffsetNotRepresentable)?;
        if installed.text_offset != expected || end > function.byte_count {
            return Err(InstallationError::InvalidSemanticCodeAttribution {
                machine: installed.machine,
                site: installed.attribution.site,
            });
        }
    }
    let mut previous_port = None;
    let mut port_operations = std::collections::BTreeSet::new();
    for installed in &record.port_effects {
        let function = function_by_machine
            .get(&installed.machine)
            .ok_or(InstallationError::EffectMachineMissing(installed.machine))?;
        let expected = function
            .text_offset
            .checked_add(installed.effect.code_offset)
            .ok_or(InstallationError::PortEffectOffsetNotRepresentable)?;
        let end = installed
            .effect
            .code_offset
            .checked_add(installed.effect.byte_count)
            .ok_or(InstallationError::PortEffectOffsetNotRepresentable)?;
        if installed.text_offset != expected
            || end > function.byte_count
            || installed.effect.byte_count
                != x86_encoding::encode_immediate_port_write(
                    installed.effect.port,
                    installed.effect.value,
                )
                .len()
        {
            return Err(InstallationError::InvalidPortEffectOffset {
                machine: installed.machine,
                operation: installed.effect.psi_operation,
            });
        }
        let key = (
            installed.machine,
            installed.text_offset,
            installed.effect.operation_ordinal,
        );
        if previous_port.is_some_and(|previous| previous >= key) {
            return Err(InstallationError::NonCanonicalPortEffectOrder);
        }
        if !port_operations.insert((installed.machine, installed.effect.psi_operation)) {
            return Err(InstallationError::DuplicatePortEffectOperation {
                machine: installed.machine,
                operation: installed.effect.psi_operation,
            });
        }
        previous_port = Some(key);
    }
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
                    && installed.settlement.byte_count == x86_encoding::IMMEDIATE_PORT_READ_U8_WIDTH
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
                if installed.settlement.runtime_scalar_arguments.iter().any(|argument| matches!(argument.source, machine_code::InternalUnitScalarArgumentSourceRecord::SelectedProcessExit { .. })) {
                    crate::object_artifact::replay::boundary::runtime_scalar_custody::process_exit::shape_is_exact(record.target, &installed.settlement)
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
                    if !target_operations::HostedExitProcessI32Realization::supports_target(
                        record.target,
                    ) {
                        None
                    } else {
                        match record.target.architecture {
                            Architecture::X86_64 => {
                                Some(calling_conventions::MachineRegister::X86Rdi)
                            }
                            Architecture::Aarch64 => {
                                Some(calling_conventions::MachineRegister::Aarch64X(0))
                            }
                        }
                    };
                let expected_byte_count = value
                    .and_then(|value| match record.target.architecture {
                        Architecture::X86_64 => {
                            Some(isa_x86_64::encode_hosted_exit_process_i32(value).len())
                        }
                        Architecture::Aarch64 => {
                            isa_aarch64::encode_hosted_exit_process_i32(record.target, value)
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
                    machine_code::InternalUnitScalarArgumentSourceRecord::SelectedBoundary { .. } | machine_code::InternalUnitScalarArgumentSourceRecord::SelectedProcessExit { .. }
                )
                    })
                {
                    crate::object_artifact::replay::boundary::runtime_scalar_custody::selected_byte_output_shape_is_exact(
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

fn validate_installed_dynamic_conformance(
    record: &InstallationRecord,
) -> Result<(), InstallationError> {
    let sections = record.image_sections;
    if sections.text_byte_count == 0
        || sections.layout.text_address == 0
        || sections.final_data_fingerprint.as_bytes() == &[0; 32]
        || (sections.data_byte_count == 0
            && sections.final_data_fingerprint != fingerprint_initialized_data(&[]))
        || sections.final_text_byte_count < sections.text_byte_count
        || sections.final_data_byte_count < sections.data_byte_count
        || sections.executable_inventory_digest.as_bytes() == &[0; 32]
        || sections.data_inventory_digest.as_bytes() == &[0; 32]
    {
        return Err(InstallationError::InvalidImageSectionLayout);
    }
    let functions = record
        .functions
        .iter()
        .map(|function| (function.machine, function))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut parameter_call_sites = std::collections::BTreeSet::new();
    for call in &record.dynamic_parameter_calls {
        let function = functions
            .get(&call.machine)
            .ok_or(InstallationError::InvalidDynamicParameterCall(call.machine))?;
        let end = call
            .text_offset
            .checked_add(call.byte_count)
            .ok_or(InstallationError::InvalidDynamicParameterCall(call.machine))?;
        let function_end = function
            .text_offset
            .checked_add(function.byte_count)
            .ok_or(InstallationError::InvalidDynamicParameterCall(call.machine))?;
        if call.byte_count == 0
            || call.text_offset < function.text_offset
            || end > function_end
            || !parameter_call_sites.insert((call.machine, call.operation))
        {
            return Err(InstallationError::InvalidDynamicParameterCall(call.machine));
        }
    }
    let forwarded_parameter_machines = record
        .forwarded_dynamic_parameter_calls
        .iter()
        .map(|call| call.machine)
        .collect::<std::collections::BTreeSet<_>>();
    let dynamic_parameter_machines = record
        .dynamic_parameter_calls
        .iter()
        .map(|call| call.machine)
        .collect::<std::collections::BTreeSet<_>>();
    let mut forwarded_parameter_sites = std::collections::BTreeSet::new();
    for call in &record.forwarded_dynamic_parameter_calls {
        let function = functions.get(&call.machine).ok_or(
            InstallationError::InvalidForwardedDynamicParameterCall(call.machine),
        )?;
        let end = call.text_offset.checked_add(call.byte_count).ok_or(
            InstallationError::InvalidForwardedDynamicParameterCall(call.machine),
        )?;
        let function_end = function
            .text_offset
            .checked_add(function.byte_count)
            .ok_or(InstallationError::InvalidForwardedDynamicParameterCall(
                call.machine,
            ))?;
        if call.byte_count == 0
            || call.text_offset < function.text_offset
            || end > function_end
            || call.source_parameter_ordinal != 0
            || call.target_parameter_ordinal != 0
            || !functions.contains_key(&call.callee)
            || (!forwarded_parameter_machines.contains(&call.callee)
                && !dynamic_parameter_machines.contains(&call.callee))
            || !matches!(
                (call.source_value, call.scalar_type),
                (None, None)
                    | (
                        Some(_),
                        Some(
                            semantic_vocabulary::ScalarType::Boolean
                                | semantic_vocabulary::ScalarType::Integer(_)
                        )
                    )
            )
            || !forwarded_parameter_sites.insert((call.machine, call.operation))
        {
            return Err(InstallationError::InvalidForwardedDynamicParameterCall(
                call.machine,
            ));
        }
    }
    if sections.data_byte_count == 0 {
        if !record.dynamic_conformance_tables.is_empty()
            || !record.dynamic_calls.is_empty()
            || !record.stored_dynamic_calls.is_empty()
            || !record.forwarded_dynamic_descriptor_adapters.is_empty()
            || !record.forwarded_dynamic_descriptor_tables.is_empty()
            || !record.forwarded_dynamic_descriptor_calls.is_empty()
        {
            return Err(InstallationError::InvalidImageSectionLayout);
        }
        return Ok(());
    }
    let text_end = sections
        .layout
        .text_address
        .checked_add(
            u64::try_from(sections.text_byte_count)
                .map_err(|_| InstallationError::InvalidImageSectionLayout)?,
        )
        .ok_or(InstallationError::InvalidImageSectionLayout)?;
    if sections.layout.data_address < text_end
        || !sections.layout.data_address.is_multiple_of(8)
        || (record.dynamic_conformance_tables.is_empty()
            && record.forwarded_dynamic_descriptor_tables.is_empty())
    {
        return Err(InstallationError::InvalidImageSectionLayout);
    }

    let mut commitments = std::collections::BTreeSet::new();
    let mut expected_data_offset = 0usize;
    for table in &record.dynamic_conformance_tables {
        let table_byte_count = table
            .slots
            .len()
            .checked_mul(8)
            .ok_or(InstallationError::InvalidDynamicConformanceTable)?;
        if table.application_commitment.is_zero()
            || table.application_report_fingerprint == 0
            || !commitments.insert(table.application_commitment)
            || table.data_offset != expected_data_offset
            || table.byte_count != table_byte_count
            || table.slots.is_empty()
        {
            return Err(InstallationError::InvalidDynamicConformanceTable);
        }
        for (row_index, slot) in table.slots.iter().enumerate() {
            let slot_offset = table
                .data_offset
                .checked_add(
                    row_index
                        .checked_mul(8)
                        .ok_or(InstallationError::InvalidDynamicConformanceTable)?,
                )
                .ok_or(InstallationError::InvalidDynamicConformanceTable)?;
            if usize::try_from(slot.row_index) != Ok(row_index)
                || slot.data_offset != slot_offset
                || slot
                    .target
                    .is_some_and(|target| !functions.contains_key(&target))
            {
                return Err(InstallationError::InvalidDynamicConformanceTable);
            }
        }
        expected_data_offset = expected_data_offset
            .checked_add(table.byte_count)
            .ok_or(InstallationError::InvalidDynamicConformanceTable)?;
    }
    let adapters = record
        .forwarded_dynamic_descriptor_adapters
        .iter()
        .map(|adapter| {
            (
                (
                    adapter.application_commitment,
                    adapter.row_index,
                    adapter.realization,
                ),
                adapter,
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    if adapters.len() != record.forwarded_dynamic_descriptor_adapters.len() {
        return Err(InstallationError::InvalidForwardedDynamicDescriptorAdapter);
    }
    let mut forwarded_commitments = std::collections::BTreeSet::new();
    let mut table_adapter_identities = std::collections::BTreeSet::new();
    for table in &record.forwarded_dynamic_descriptor_tables {
        let table_byte_count = table
            .slots
            .len()
            .checked_mul(8)
            .ok_or(InstallationError::InvalidForwardedDynamicDescriptorTable)?;
        if table.application_commitment.is_zero()
            || table.application_report_fingerprint == 0
            || !forwarded_commitments.insert(table.application_commitment)
            || table.data_offset != expected_data_offset
            || table.byte_count != table_byte_count
            || table.slots.is_empty()
        {
            return Err(InstallationError::InvalidForwardedDynamicDescriptorTable);
        }
        for (row_index, slot) in table.slots.iter().enumerate() {
            let data_offset = table
                .data_offset
                .checked_add(
                    row_index
                        .checked_mul(8)
                        .ok_or(InstallationError::InvalidForwardedDynamicDescriptorTable)?,
                )
                .ok_or(InstallationError::InvalidForwardedDynamicDescriptorTable)?;
            let adapter = adapters
                .get(&(
                    table.application_commitment,
                    slot.row_index,
                    slot.realization,
                ))
                .ok_or(InstallationError::InvalidForwardedDynamicDescriptorTable)?;
            table_adapter_identities.insert((
                table.application_commitment,
                slot.row_index,
                slot.realization,
            ));
            if usize::try_from(slot.row_index) != Ok(row_index)
                || slot.data_offset != data_offset
                || slot.adapter_text_offset != adapter.text_offset
                || !functions.contains_key(&slot.realization)
            {
                return Err(InstallationError::InvalidForwardedDynamicDescriptorTable);
            }
        }
        expected_data_offset = expected_data_offset
            .checked_add(table.byte_count)
            .ok_or(InstallationError::InvalidForwardedDynamicDescriptorTable)?;
    }
    if table_adapter_identities.len() != adapters.len() {
        return Err(InstallationError::InvalidForwardedDynamicDescriptorTable);
    }
    if expected_data_offset != sections.data_byte_count {
        return Err(InstallationError::InvalidDynamicConformanceTable);
    }

    let mut previous_call = None;
    let mut call_sites = std::collections::BTreeSet::new();
    let mut referenced_commitments = std::collections::BTreeSet::new();
    for call in &record.dynamic_calls {
        let function = functions
            .get(&call.machine)
            .ok_or(InstallationError::InvalidDynamicCall(call.machine))?;
        let call_end = call
            .text_offset
            .checked_add(call.byte_count)
            .ok_or(InstallationError::InvalidDynamicCall(call.machine))?;
        let function_end = function
            .text_offset
            .checked_add(function.byte_count)
            .ok_or(InstallationError::InvalidDynamicCall(call.machine))?;
        let table = record
            .dynamic_conformance_tables
            .iter()
            .find(|table| table.application_commitment == call.application_commitment)
            .ok_or(InstallationError::InvalidDynamicCall(call.machine))?;
        referenced_commitments.insert(call.application_commitment);
        let selected_index = usize::try_from(call.selected_table_byte_offset / 8)
            .map_err(|_| InstallationError::InvalidDynamicCall(call.machine))?;
        let selected = table
            .slots
            .get(selected_index)
            .ok_or(InstallationError::InvalidDynamicCall(call.machine))?;
        let order = (call.text_offset, call.machine, call.operation);
        if call.byte_count == 0
            || call.selected_table_byte_offset % 8 != 0
            || call.text_offset < function.text_offset
            || call_end > function_end
            || selected.target != Some(call.realization)
            || !function
                .unit_parameter_homes
                .iter()
                .any(|home| home.place == call.initial_source)
            || !function
                .unit_parameter_homes
                .iter()
                .any(|home| home.place == call.rebound_source)
            || previous_call.is_some_and(|previous| previous >= order)
            || !call_sites.insert((call.machine, call.operation))
        {
            return Err(InstallationError::InvalidDynamicCall(call.machine));
        }
        previous_call = Some(order);
    }
    let mut previous_stored_call = None;
    for call in &record.stored_dynamic_calls {
        let invalid = || InstallationError::InvalidStoredDynamicCall(call.machine);
        let function = functions.get(&call.machine).ok_or_else(invalid)?;
        let establishment_end = call
            .establishment_text_offset
            .checked_add(call.establishment_byte_count)
            .ok_or_else(invalid)?;
        let call_end = call
            .text_offset
            .checked_add(call.byte_count)
            .ok_or_else(invalid)?;
        let function_end = function
            .text_offset
            .checked_add(function.byte_count)
            .ok_or_else(invalid)?;
        let table = record
            .dynamic_conformance_tables
            .iter()
            .find(|table| table.application_commitment == call.application_commitment)
            .ok_or_else(invalid)?;
        referenced_commitments.insert(call.application_commitment);
        let selected_index =
            usize::try_from(call.selected_table_byte_offset / 8).map_err(|_| invalid())?;
        let selected = table.slots.get(selected_index).ok_or_else(invalid)?;
        let order = (
            call.establishment_text_offset,
            call.text_offset,
            call.machine,
            call.operation,
        );
        if call.establishment_byte_count == 0
            || call.byte_count == 0
            || call.selected_table_byte_offset % 8 != 0
            || call.descriptor_home_byte_offset % 8 != 0
            || call.establishment_text_offset < function.text_offset
            || establishment_end > call.text_offset
            || call_end > function_end
            || selected.target != Some(call.realization)
            || !function
                .unit_parameter_homes
                .iter()
                .any(|home| home.place == call.source)
            || previous_stored_call.is_some_and(|previous| previous >= order)
            || !call_sites.insert((call.machine, call.establishment_operation))
            || !call_sites.insert((call.machine, call.operation))
        {
            return Err(invalid());
        }
        previous_stored_call = Some(order);
    }
    if referenced_commitments != commitments {
        return Err(InstallationError::InvalidDynamicConformanceTable);
    }
    let mut forwarded_references = std::collections::BTreeSet::new();
    let mut forwarded_call_sites = std::collections::BTreeSet::new();
    for call in &record.forwarded_dynamic_descriptor_calls {
        let function = functions.get(&call.machine).ok_or(
            InstallationError::InvalidForwardedDynamicDescriptorCall(call.machine),
        )?;
        let end = call.text_offset.checked_add(call.byte_count).ok_or(
            InstallationError::InvalidForwardedDynamicDescriptorCall(call.machine),
        )?;
        let function_end = function
            .text_offset
            .checked_add(function.byte_count)
            .ok_or(InstallationError::InvalidForwardedDynamicDescriptorCall(
                call.machine,
            ))?;
        if call.byte_count == 0
            || call.text_offset < function.text_offset
            || end > function_end
            || !functions.contains_key(&call.callee)
            || !forwarded_commitments.contains(&call.application_commitment)
            || !installed_forwarded_dynamic_scalar_result_is_canonical(
                call,
                function,
                record.target,
            )
            || !forwarded_call_sites.insert((call.machine, call.operation))
        {
            return Err(InstallationError::InvalidForwardedDynamicDescriptorCall(
                call.machine,
            ));
        }
        forwarded_references.insert(call.application_commitment);
    }
    if forwarded_references != forwarded_commitments {
        return Err(InstallationError::InvalidForwardedDynamicDescriptorTable);
    }
    Ok(())
}

pub(super) fn is_partial_cleanup_path(path: &[terminal_psi::StructuralPathSegment]) -> bool {
    !path.is_empty()
        && path.iter().all(|segment| match segment {
            terminal_psi::StructuralPathSegment::Referent => false,
            terminal_psi::StructuralPathSegment::Field(identity) => !identity.is_empty(),
            terminal_psi::StructuralPathSegment::FixedIndex(_) => true,
        })
}

pub(super) fn installed_stack_facts_are_canonical(
    function: &InstalledFunction,
    functions: &std::collections::BTreeMap<MachineId, Option<StructuralTypeId>>,
) -> bool {
    let valid_alignment = |alignment: u32| alignment != 0 && alignment.is_power_of_two();
    if function.unit_stack.is_some() && function.scalar_stack.is_some()
        || function
            .unit_stack
            .is_some_and(|stack| !valid_alignment(stack.stack_alignment))
        || function
            .scalar_stack
            .is_some_and(|stack| !valid_alignment(stack.stack_alignment))
        || (!function.unit_call_stacks.is_empty() && function.unit_stack.is_none())
        || (!function.scalar_call_stacks.is_empty() && function.scalar_stack.is_none())
        || (!function.foreign_call_stacks.is_empty() && function.unit_stack.is_none())
    {
        return false;
    }
    let call_in_function = |target: MachineId, text_offset: usize| {
        functions.contains_key(&target)
            && text_offset >= function.text_offset
            && text_offset < function.text_offset.saturating_add(function.byte_count)
    };
    let unit_calls_valid = function.unit_call_stacks.iter().all(|call| {
        call_in_function(call.target, call.text_offset)
            && call
                .active_frame_bytes
                .checked_add(call.transient_bytes)
                .is_some_and(|sum| sum == call.caller_live_bytes)
    });
    let scalar_calls_valid = function
        .scalar_call_stacks
        .iter()
        .all(|call| call_in_function(call.target, call.text_offset));
    let foreign_calls_valid = function.foreign_call_stacks.iter().all(|call| {
        call.text_offset >= function.text_offset
            && call.text_offset < function.text_offset.saturating_add(function.byte_count)
            && call.caller_live_bytes != 0
            && call.provider_plan_report_identity != 0
            && call.contribution_report_identity.normalized_identity() != 0
            && !call.contribution_commitment.is_zero()
            && call.contribution_bytes != 0
            && call.contribution_alignment != 0
            && call.contribution_alignment.is_power_of_two()
            && function.unit_stack.is_some_and(|stack| {
                call.contribution_alignment <= u64::from(stack.stack_alignment)
            })
    });
    let unit_ordered = function.unit_call_stacks.windows(2).all(|pair| {
        (pair[0].text_offset, pair[0].owner, pair[0].target)
            < (pair[1].text_offset, pair[1].owner, pair[1].target)
    });
    let scalar_ordered = function.scalar_call_stacks.windows(2).all(|pair| {
        (pair[0].text_offset, pair[0].owner, pair[0].target)
            < (pair[1].text_offset, pair[1].owner, pair[1].target)
    });
    let foreign_ordered = function
        .foreign_call_stacks
        .windows(2)
        .all(|pair| (pair[0].text_offset, pair[0].owner) < (pair[1].text_offset, pair[1].owner));
    unit_calls_valid
        && scalar_calls_valid
        && foreign_calls_valid
        && unit_ordered
        && scalar_ordered
        && foreign_ordered
}

fn validate_scalar_affine_cleanup_shape(
    record: &InstallationRecord,
    function: &InstalledFunction,
    cleanup: &machine_code::UnitAffineCleanupRecord,
    require_function_end: bool,
) -> Result<(), InstallationError> {
    let invalid = || InstallationError::InvalidUnitAffineCleanup(function.machine);
    let end = cleanup
        .code_offset
        .checked_add(cleanup.byte_count)
        .ok_or(InstallationError::FunctionOffsetNotRepresentable)?;
    if cleanup.byte_count == 0
        || end > function.byte_count
        || (require_function_end && end != function.byte_count)
        || !cleanup.locals.is_empty()
        || cleanup.actions.len() != function.scalar_structural_parameter_homes.len()
        || function
            .scalar_structural_parameter_homes
            .iter()
            .rev()
            .zip(&cleanup.actions)
            .any(|(home, action)| match action {
                terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place) => {
                    *place != home.place || home.multiplicity != StructuralMultiplicity::Affine
                }
                terminal_psi::TerminalAffineCleanupAction::InvokeNominal(nominal) => {
                    nominal.place != home.place
                        || nominal.structural_type != home.structural_type
                        || home.multiplicity != StructuralMultiplicity::Affine
                        || nominal.cleanup_receiver.is_some()
                        || !nominal.requirement_obligations.is_empty()
                        || record.functions.iter().all(|target| {
                            target.machine != nominal.cleanup_machine
                                || target.attachment != Some(nominal.structural_type)
                                || !target.unit_body
                        })
                }
                terminal_psi::TerminalAffineCleanupAction::DiscardResidual(_) => true,
            })
    {
        return Err(invalid());
    }
    for (ordinal, action) in cleanup.actions.iter().enumerate() {
        let terminal_psi::TerminalAffineCleanupAction::InvokeNominal(nominal) = action else {
            continue;
        };
        let target = record
            .functions
            .iter()
            .find(|target| target.machine == nominal.cleanup_machine)
            .ok_or_else(invalid)?;
        let executable = record
            .internal_unit_calls
            .iter()
            .any(|call| call.machine == target.machine);
        let action_ordinal = u32::try_from(ordinal).map_err(|_| invalid())?;
        let matching = record
            .internal_unit_calls
            .iter()
            .filter(|call| {
                call.machine == function.machine
                    && call.custody.owner
                        == CallSiteOwner::CleanupAction {
                            edge: cleanup.psi_edge,
                            action_ordinal,
                        }
                    && call.custody.target == nominal.cleanup_machine
                    && call.custody.arguments.is_empty()
                    && call.custody.claim_transfers.is_empty()
                    && call.custody.code_offset >= cleanup.code_offset
                    && call
                        .custody
                        .code_offset
                        .checked_add(call.custody.byte_count)
                        .is_some_and(|call_end| call_end <= end)
            })
            .count();
        if matching != usize::from(executable) {
            return Err(invalid());
        }
    }
    Ok(())
}

fn validate_scalar_control_affine_cleanup_shape(
    record: &InstallationRecord,
    function: &InstalledFunction,
) -> Result<(), InstallationError> {
    let invalid = || InstallationError::InvalidUnitAffineCleanup(function.machine);
    let cleanups = &function.scalar_control_affine_cleanups;
    if cleanups.len() < 2 {
        return Err(InstallationError::InvalidScalarControlAffineCleanupCount(
            cleanups.len(),
        ));
    }
    if !scalar_control_affine_cleanups_are_canonical(cleanups, function.byte_count) {
        return Err(invalid());
    }
    for (leaf_ordinal, cleanup) in cleanups.iter().enumerate() {
        validate_scalar_affine_cleanup_shape(record, function, cleanup, false)?;
        if record
            .semantic_code_attribution
            .iter()
            .filter(|attribution| {
                attribution.machine == function.machine
                    && attribution.attribution.site == SemanticCodeSite::Edge(cleanup.psi_edge)
                    && attribution.attribution.operation_ordinal == leaf_ordinal
                    && attribution.attribution.code_offset == cleanup.code_offset
                    && attribution.attribution.byte_count == cleanup.byte_count
            })
            .count()
            != 1
        {
            return Err(invalid());
        }
    }
    Ok(())
}

pub(super) fn scalar_control_affine_cleanups_are_canonical(
    cleanups: &[machine_code::UnitAffineCleanupRecord],
    function_byte_count: usize,
) -> bool {
    let Some(first) = cleanups.first() else {
        return false;
    };
    let edges = cleanups
        .iter()
        .map(|cleanup| cleanup.psi_edge)
        .collect::<std::collections::BTreeSet<_>>();
    edges.len() == cleanups.len()
        && cleanups.iter().all(|cleanup| {
            cleanup.locals.is_empty()
                && cleanup.structural_types == first.structural_types
                && cleanup.actions == first.actions
                && cleanup.byte_count == first.byte_count
        })
        && cleanups.windows(2).all(|pair| {
            pair[0]
                .code_offset
                .checked_add(pair[0].byte_count)
                .is_some_and(|end| end <= pair[1].code_offset)
        })
        && cleanups.last().is_some_and(|last| {
            last.code_offset.checked_add(last.byte_count) == Some(function_byte_count)
        })
}

fn bounded_nominal_receiver_shape(shape: ValueShape) -> bool {
    shape == ValueShape::integer(0, 1)
        || shape.class == ValueClass::Integer
            && shape.byte_size != 0
            && matches!(shape.alignment, 1 | 2 | 4 | 8)
            && shape.byte_size.is_multiple_of(shape.alignment)
}
