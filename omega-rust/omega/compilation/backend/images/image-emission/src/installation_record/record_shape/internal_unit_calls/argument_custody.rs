//! Each structural argument's custody: continuations, the result home,
//! per-argument sources and spans, projected arguments and claim transfers.

use super::InternalUnitCall;
use crate::installation_record::{InstallationError, SemanticCodeSite, ValueShape};
use machine_code::unit_call_arguments;

impl InternalUnitCall<'_> {
    /// A projected argument under Unit continuations needs a callee whose
    /// parameters and cleanup accept exactly that projection.
    pub(super) fn validate_projected_continuation(&self) -> Result<(), InstallationError> {
        let InternalUnitCall {
            record,
            function,
            installed,
            custody,
            ..
        } = *self;
        if !function.unit_continuations.is_empty()
            && custody
                .arguments
                .iter()
                .any(|argument| !argument.path.is_empty())
            && record.function(custody.target)
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
        Ok(())
    }

    /// A structural result home is stored right after the call's stack release
    /// and matches the retained frame layout exactly.
    pub(super) fn validate_result_home(&self) -> Result<(), InstallationError> {
        let InternalUnitCall {
            record,
            function,
            installed,
            custody,
            parameter_homes,
            projected_result,
            function_unit_calls,
            ..
        } = *self;
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
                    function_unit_calls,
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
        Ok(())
    }

    /// Some structural argument's source, bytes, span or projection path
    /// disagrees with its parameter home, projected result or affine local.
    pub(super) fn arguments_lack_exact_custody(&self) -> bool {
        let InternalUnitCall {
            record,
            installed,
            custody,
            plan,
            end,
            incoming_call,
            parameter_homes,
            affine_cleanup,
            projected_result,
            function_unit_calls,
            ..
        } = *self;
        let scalar_count = custody.scalar_arguments.len();
        custody
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
                                    crate::object_artifact::replay::unit::call_custody::argument_custody::argument_is_established_local_source(
                                        argument,
                                        place,
                                        structural_type,
                                        source_placement,
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
                            unit_call_arguments(function_unit_calls)
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
                            .checked_add(argument.shape.byte_size)
                            .is_none_or(|end| end > source_placement.shape.byte_size)
                        || crate::object_artifact::replay::unit::call_custody::argument_custody::argument_path_is_malformed(
                                argument,
                                source_placement,
                                parameter_homes,
                                affine_cleanup,
                                result_source,
                                self.exact_borrowed_argument(argument_index, argument),
                            )
                })
    }

    /// A projected argument that is neither transferred, borrowed, consumed
    /// nor discarded by a residual cleanup.
    pub(super) fn projected_arguments_are_unsettled(&self) -> bool {
        let InternalUnitCall {
            custody,
            affine_cleanup,
            projected_result,
            fully_consumed_affine_parameter,
            continuation_discards,
            projected_argument_indexes,
            transferred_argument_indexes,
            ..
        } = *self;
        projected_argument_indexes.iter().any(|index| {
            if transferred_argument_indexes.contains(index) {
                return false;
            }
            let Some(argument) = custody.arguments.get(*index) else {
                return true;
            };
            if self.exact_borrowed_argument(*index, argument) {
                return false;
            }
            argument.path.is_empty()
                || (!fully_consumed_affine_parameter
                    && projected_result.is_none()
                    && !continuation_discards.contains(&argument.place)
                    && crate::object_artifact::replay::unit::call_custody::argument_custody::no_disjoint_residual_discard(
                        argument,
                        affine_cleanup,
                    ))
        })
    }
}
