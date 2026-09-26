//! The shape of one installed function's Unit affine cleanup: an exact
//! construction prefix whose actions discard the function's affine results,
//! locals and owned parameters, or hand those parameters to nominal
//! cleanups, exactly as the record's calls and homes require.

use super::super::stack_facts::is_partial_cleanup_path;
use crate::image_emission::installation_record::{
    CallSiteOwner, InstallationError, InstallationRecord, InstalledFunction, MachineId,
    StructuralMultiplicity, StructuralPathSegment, StructuralTypeId, StructuralTypeShape,
    ValueClass, ValueShape,
};
use semantic_vocabulary::PlaceId;

/// The facts a cleanup shape is checked against.
struct CleanupShapeFacts<'a> {
    /// The single affine parameter was partially consumed by projected calls.
    partially_consumed_affine_parameter: bool,
    /// The exact projected affine result the function's calls settle into.
    projected_affine_result: Option<&'a post_allocation_machine_to_selected_form_encoding::machine_code::InternalStructuralCallResult>,
    /// The end of the cleanup's bytes.
    end: usize,
    /// The affine results (latest first) then locals the cleanup discards first.
    expected_cleanup_prefix: Vec<PlaceId>,
    /// The owned affine parameters (last first) the cleanup discards last.
    expected_parameter_discards: Vec<PlaceId>,
    /// The root discards the cleanup declares, in action order.
    discards: Vec<PlaceId>,
    /// The residual discards the cleanup declares, in action order.
    residual_discards: Vec<&'a terminal_psi::StructuralAffineDiscard>,
    /// The nominal cleanups the cleanup declares, in action order.
    nominal_cleanups: Vec<&'a terminal_psi::NominalAffineCleanup>,
    /// The operations establishing the cleanup's locals.
    local_operations: std::collections::BTreeSet<semantic_vocabulary::OperationId>,
}

/// The function's Unit affine cleanup: an exact construction prefix whose
/// residual cleanups match the parameter homes, projected results and
/// continuation discards the function retains.
pub(super) fn validate_unit_affine_cleanup_shape(
    record: &InstallationRecord,
    function: &InstalledFunction,
    cleanup: &post_allocation_machine_to_selected_form_encoding::machine_code::UnitAffineCleanupRecord,
    attachments: &std::collections::BTreeMap<MachineId, Option<StructuralTypeId>>,
    function_unit_calls: &[post_allocation_machine_to_selected_form_encoding::machine_code::InternalUnitCallRecord],
    continuation_discards: &[PlaceId],
) -> Result<(), InstallationError> {
    let facts = cleanup_shape_facts(
        record,
        function,
        cleanup,
        function_unit_calls,
        continuation_discards,
    )?;
    let Some(parameter_discards) = facts.discards.get(facts.expected_cleanup_prefix.len()..) else {
        return Err(InstallationError::InvalidUnitAffineCleanup(
            function.machine,
        ));
    };
    if cleanup.byte_count == 0
        || facts.end != function.byte_count
        || locals_are_malformed(cleanup, &facts.local_operations)
        || discards_are_malformed(cleanup, &facts)
        || match (
            facts.nominal_cleanups.as_slice(),
            facts.residual_discards.as_slice(),
        ) {
            _ if facts.projected_affine_result.is_some() => false,
            ([nominal], []) => single_nominal_cleanup_is_malformed(
                record,
                function,
                cleanup,
                attachments,
                nominal,
                &facts,
            ),
            ([], []) => parameter_discards != facts.expected_parameter_discards,
            ([], residuals @ [_, ..]) => residual_discards_are_malformed(
                record,
                function,
                cleanup,
                parameter_discards,
                residuals,
                &facts,
            ),
            (nominal @ [_, _, ..], []) => nominal_cleanups_are_malformed(
                record,
                function,
                cleanup,
                attachments,
                nominal,
                &facts,
            ),
            _ => true,
        }
    {
        return Err(InstallationError::InvalidUnitAffineCleanup(
            function.machine,
        ));
    }
    Ok(())
}

/// The facts the cleanup shape is checked against: the affine results and
/// locals it must discard first, the owned affine parameters it must discard
/// last, the discards, residual discards and nominal cleanups it declares,
/// and the operations establishing its locals.
fn cleanup_shape_facts<'a>(
    record: &'a InstallationRecord,
    function: &'a InstalledFunction,
    cleanup: &'a post_allocation_machine_to_selected_form_encoding::machine_code::UnitAffineCleanupRecord,
    function_unit_calls: &'a [post_allocation_machine_to_selected_form_encoding::machine_code::InternalUnitCallRecord],
    continuation_discards: &[PlaceId],
) -> Result<CleanupShapeFacts<'a>, InstallationError> {
    let fully_consumed_affine_parameter =
        crate::image_emission::object_artifact::replay::structural::affine_projected_calls::exact_fully_consumed_affine_parameter(
            &function.unit_parameter_homes,
            function_unit_calls,
            function.unit_affine_cleanup.as_ref(),
        );
    let partially_consumed_affine_parameter =
        crate::image_emission::object_artifact::replay::structural::affine_projected_calls::exact_partially_consumed_affine_parameter(
            &function.unit_parameter_homes,
            function_unit_calls,
            function.unit_affine_cleanup.as_ref(),
        );
    let projected_affine_result = crate::image_emission::object_artifact::replay::structural::affine_projected_calls::exact_projected_affine_result(
        &function.unit_parameter_homes,
        function_unit_calls,
        function.unit_affine_cleanup.as_ref(),
    );
    if !crate::image_emission::object_artifact::replay::unit::affine_cleanup::exact_construction_prefix(cleanup) {
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
    let local_operations = cleanup
        .locals
        .iter()
        .map(|(operation, _, _)| *operation)
        .collect::<std::collections::BTreeSet<_>>();
    Ok(CleanupShapeFacts {
        partially_consumed_affine_parameter,
        projected_affine_result,
        end,
        expected_cleanup_prefix,
        expected_parameter_discards,
        discards,
        residual_discards,
        nominal_cleanups,
        local_operations,
    })
}

/// Whether the cleanup's locals are malformed: each must occupy a trivial
/// affine local place of its ordinal and type and have an empty record type.
fn locals_are_malformed(
    cleanup: &post_allocation_machine_to_selected_form_encoding::machine_code::UnitAffineCleanupRecord,
    local_operations: &std::collections::BTreeSet<semantic_vocabulary::OperationId>,
) -> bool {
    local_operations.len() != cleanup.locals.len()
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
}

/// Whether the declared discards are malformed: they must start with the
/// expected results-then-locals prefix, be distinct, and with the residual
/// discards and nominal cleanups account for every action.
fn discards_are_malformed(
    cleanup: &post_allocation_machine_to_selected_form_encoding::machine_code::UnitAffineCleanupRecord,
    facts: &CleanupShapeFacts<'_>,
) -> bool {
    let CleanupShapeFacts {
        ref expected_cleanup_prefix,
        ref discards,
        ref residual_discards,
        ref nominal_cleanups,
        ..
    } = *facts;
    discards.get(..expected_cleanup_prefix.len()) != Some(expected_cleanup_prefix.as_slice())
        || discards
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != discards.len()
        || discards.len() + residual_discards.len() + nominal_cleanups.len()
            != cleanup.actions.len()
}

/// Whether a single nominal cleanup is malformed: it must be the only
/// action, hand the function's single affine parameter home to a bounded
/// receiver of the cleanup machine's attachment type, and be called exactly
/// once at the cleanup's offset when its body executes anything.
fn single_nominal_cleanup_is_malformed(
    record: &InstallationRecord,
    function: &InstalledFunction,
    cleanup: &post_allocation_machine_to_selected_form_encoding::machine_code::UnitAffineCleanupRecord,
    attachments: &std::collections::BTreeMap<MachineId, Option<StructuralTypeId>>,
    nominal: &terminal_psi::NominalAffineCleanup,
    facts: &CleanupShapeFacts<'_>,
) -> bool {
    let CleanupShapeFacts { ref discards, .. } = *facts;
    let cleanup_is_executable = exact_nominal_body(record, nominal);
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
        || function.unit_parameter_homes[0].multiplicity != StructuralMultiplicity::Affine
        || !bounded_nominal_receiver_shape(function.unit_parameter_homes[0].shape)
        || (function.unit_parameter_homes[0].shape.byte_size == 0
            && !function.unit_parameter_homes[0].source.locations.is_empty())
        || (function.unit_parameter_homes[0].shape.byte_size != 0
            && function.unit_parameter_homes[0].source.locations.is_empty())
        || attachments.get(&nominal.cleanup_machine) != Some(&Some(nominal.structural_type))
        || cleanup_is_executable.is_none()
        || matching_cleanup_calls != usize::from(cleanup_is_executable == Some(true))
}

/// Whether the residual discards are malformed: the root discards must
/// come first, the residuals must all target the single remaining affine
/// parameter along distinct partial paths that, with the paths moved into
/// callees, exactly partition the parameter's type.
fn residual_discards_are_malformed(
    record: &InstallationRecord,
    function: &InstalledFunction,
    cleanup: &post_allocation_machine_to_selected_form_encoding::machine_code::UnitAffineCleanupRecord,
    parameter_discards: &[PlaceId],
    residuals: &[&terminal_psi::StructuralAffineDiscard],
    facts: &CleanupShapeFacts<'_>,
) -> bool {
    let CleanupShapeFacts {
        partially_consumed_affine_parameter,
        ref expected_parameter_discards,
        ref discards,
        ..
    } = *facts;
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
            argument.place == residual_root && Some(argument.root_structural_type) == parameter_type
        })
        .map(|argument| (argument.path.as_slice(), argument.structural_type))
        .collect::<Vec<_>>();
    cleanup.actions.get(..discards.len()).is_none_or(|prefix| {
        !prefix.iter().zip(discards).all(|(action, place)| {
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
            !crate::image_emission::object_artifact::replay::structural::partial_cleanup_partition::exact_partial_cleanup_partition(
                &cleanup.structural_types,
                root_type,
                &moved,
                residuals,
            )
        })
}

/// Whether several nominal cleanups are malformed: they must be the only
/// actions, pair the parameter homes (last first) with bounded receivers of
/// each cleanup machine's attachment type, have exact bodies, and be called
/// exactly once each, in action order, inside the cleanup's bytes.
fn nominal_cleanups_are_malformed(
    record: &InstallationRecord,
    function: &InstalledFunction,
    cleanup: &post_allocation_machine_to_selected_form_encoding::machine_code::UnitAffineCleanupRecord,
    attachments: &std::collections::BTreeMap<MachineId, Option<StructuralTypeId>>,
    nominal: &[&terminal_psi::NominalAffineCleanup],
    facts: &CleanupShapeFacts<'_>,
) -> bool {
    let CleanupShapeFacts {
        end, ref discards, ..
    } = *facts;
    let bodies = nominal
        .iter()
        .map(|cleanup| exact_nominal_body(record, cleanup))
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
        || function
            .unit_parameter_homes
            .iter()
            .rev()
            .zip(nominal)
            .any(|(home, nominal)| {
                home.place != nominal.place
                    || home.structural_type != nominal.structural_type
                    || home.multiplicity != StructuralMultiplicity::Affine
                    || !bounded_nominal_receiver_shape(home.shape)
                    || (home.shape.byte_size == 0 && !home.source.locations.is_empty())
                    || (home.shape.byte_size != 0 && home.source.locations.is_empty())
                    || attachments.get(&nominal.cleanup_machine)
                        != Some(&Some(nominal.structural_type))
            })
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

/// Whether a nominal cleanup's machine is exactly an attachment-only Unit
/// body whose calls are a run of attachment helper calls in code order:
/// `None` when it is not, otherwise whether that body executes any call.
fn exact_nominal_body(
    record: &InstallationRecord,
    nominal: &terminal_psi::NominalAffineCleanup,
) -> Option<bool> {
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
}

fn bounded_nominal_receiver_shape(shape: ValueShape) -> bool {
    shape == ValueShape::integer(0, 1)
        || shape.class == ValueClass::Integer
            && shape.byte_size != 0
            && matches!(shape.alignment, 1 | 2 | 4 | 8)
            && shape.byte_size.is_multiple_of(shape.alignment)
}
