//! The installed function rows: each function's retained facts are canonical
//! and its text span follows the previous function's, then the forwarded
//! descriptor adapters and the compiler-private function continue the text,
//! which must end exactly at the recorded text size.

use super::affine_cleanups::{
    validate_scalar_affine_cleanup_shape, validate_scalar_control_affine_cleanup_shape,
};
use super::stack_facts::{installed_stack_facts_are_canonical, is_partial_cleanup_path};
use crate::installation_record::{
    BoundaryRealization, CallSiteOwner, InstallationError, InstallationRecord, InstalledFunction,
    MachineId, SemanticCodeSite, StructuralMultiplicity, StructuralPathSegment, StructuralTypeId,
    StructuralTypeShape, ValueClass, ValueShape, borrowed_structural, graph_structural,
    incoming_structural, installed_function_scalar_transport_is_canonical,
    installed_unit_scalar_transport,
};
use semantic_vocabulary::PlaceId;

/// Walk the functions in canonical order, then the adapters and the private
/// function, carrying the expected text offset across all of them.
pub(super) fn validate_function_rows(record: &InstallationRecord) -> Result<(), InstallationError> {
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
        validate_function_row(
            record,
            function,
            previous_function,
            expected_text_offset,
            &attachments,
        )?;
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
    Ok(())
}

/// One installed function: continuation and Unit-body custody, canonical
/// text placement, stack facts and parameter homes, then the affine cleanups
/// it retains.
fn validate_function_row(
    record: &InstallationRecord,
    function: &InstalledFunction,
    previous_function: Option<MachineId>,
    expected_text_offset: usize,
    attachments: &std::collections::BTreeMap<MachineId, Option<StructuralTypeId>>,
) -> Result<(), InstallationError> {
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
    if function.byte_count == 0
        || function.text_offset != expected_text_offset
        || previous_function.is_some_and(|previous| previous >= function.machine)
    {
        return Err(InstallationError::NonCanonicalInstalledFunctions);
    }
    let has_scalar_control_cleanup = !function.scalar_control_affine_cleanups.is_empty();
    let has_scalar_cleanup = function.scalar_affine_cleanup.is_some() || has_scalar_control_cleanup;
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
    let structural_call_scalar_result_is_exact =
        function
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
    if !installed_stack_facts_are_canonical(function, attachments)
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
            && (!function.unit_parameters.is_empty() || !function.unit_parameter_homes.is_empty()))
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
        validate_unit_affine_cleanup_shape(
            record,
            function,
            cleanup,
            attachments,
            &function_unit_calls,
            &continuation_discards,
        )?;
    }
    if let Some(cleanup) = &function.scalar_affine_cleanup {
        validate_scalar_affine_cleanup_shape(record, function, cleanup, true)?;
    }
    if has_scalar_control_cleanup {
        validate_scalar_control_affine_cleanup_shape(record, function)?;
    }
    Ok(())
}

/// The function's Unit affine cleanup: an exact construction prefix whose
/// residual cleanups match the parameter homes, projected results and
/// continuation discards the function retains.
fn validate_unit_affine_cleanup_shape(
    record: &InstallationRecord,
    function: &InstalledFunction,
    cleanup: &machine_code::UnitAffineCleanupRecord,
    attachments: &std::collections::BTreeMap<MachineId, Option<StructuralTypeId>>,
    function_unit_calls: &[machine_code::InternalUnitCallRecord],
    continuation_discards: &[PlaceId],
) -> Result<(), InstallationError> {
    let fully_consumed_affine_parameter =
        crate::object_artifact::replay::structural::affine_projected_calls::exact_fully_consumed_affine_parameter(
            &function.unit_parameter_homes,
            function_unit_calls,
            function.unit_affine_cleanup.as_ref(),
        );
    let partially_consumed_affine_parameter =
        crate::object_artifact::replay::structural::affine_projected_calls::exact_partially_consumed_affine_parameter(
            &function.unit_parameter_homes,
            function_unit_calls,
            function.unit_affine_cleanup.as_ref(),
        );
    let projected_affine_result = crate::object_artifact::replay::structural::affine_projected_calls::exact_projected_affine_result(
        &function.unit_parameter_homes,
        function_unit_calls,
        function.unit_affine_cleanup.as_ref(),
    );
    if !crate::object_artifact::replay::unit::affine_cleanup::exact_construction_prefix(cleanup) {
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
                    && result.operation_result.multiplicity == StructuralMultiplicity::Affine
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
            terminal_psi::TerminalAffineCleanupAction::DiscardResidual(discard) => Some(discard),
            _ => None,
        })
        .collect::<Vec<_>>();
    let nominal_cleanups = cleanup
        .actions
        .iter()
        .filter_map(|action| match action {
            terminal_psi::TerminalAffineCleanupAction::InvokeNominal(nominal) => Some(nominal),
            _ => None,
        })
        .collect::<Vec<_>>();
    let exact_nominal_body = |nominal: &terminal_psi::NominalAffineCleanup| {
        if nominal.cleanup_receiver.is_some() || !nominal.requirement_obligations.is_empty() {
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
                            && helper
                                .unit_affine_cleanup
                                .as_ref()
                                .is_some_and(|return_cleanup| {
                                    return_cleanup.locals.is_empty()
                                        && return_cleanup.actions.is_empty()
                                })
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
        || cleanup
            .locals
            .iter()
            .enumerate()
            .any(|(ordinal, (_, place, structural_type))| {
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
            })
        || discards.get(..expected_cleanup_prefix.len()) != Some(expected_cleanup_prefix.as_slice())
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
                    || function.unit_parameter_homes[0].structural_type != nominal.structural_type
                    || function.unit_parameter_homes[0].multiplicity
                        != StructuralMultiplicity::Affine
                    || !bounded_nominal_receiver_shape(function.unit_parameter_homes[0].shape)
                    || (function.unit_parameter_homes[0].shape.byte_size == 0
                        && !function.unit_parameter_homes[0].source.locations.is_empty())
                    || (function.unit_parameter_homes[0].shape.byte_size != 0
                        && function.unit_parameter_homes[0].source.locations.is_empty())
                    || attachments.get(&nominal.cleanup_machine)
                        != Some(&Some(nominal.structural_type))
                    || cleanup_is_executable.is_none()
                    || matching_cleanup_calls != usize::from(cleanup_is_executable == Some(true))
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
                                || (home.shape.byte_size == 0 && !home.source.locations.is_empty())
                                || (home.shape.byte_size != 0 && home.source.locations.is_empty())
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
                                        && call.custody.target == nominal[*ordinal].cleanup_machine
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
    Ok(())
}

fn bounded_nominal_receiver_shape(shape: ValueShape) -> bool {
    shape == ValueShape::integer(0, 1)
        || shape.class == ValueClass::Integer
            && shape.byte_size != 0
            && matches!(shape.alignment, 1 | 2 | 4 | 8)
            && shape.byte_size.is_multiple_of(shape.alignment)
}
