//! Nominal affine cleanups: each parameter handed to a cleanup machine whose
//! body is exactly a run of attachment helper calls.

use super::bounded_nominal_receiver_shape;
use abstract_operations_to_target_operations::target_operations::CallSiteOwner;
use post_allocation_machine_to_selected_form_encoding::machine_code::MachineCodeFunction;
use semantic_vocabulary::MachineId;

use super::CleanupInputs;

/// The cleanup function a nominal cleanup invokes and whether its body is
/// exactly a sequence of attachment-only helper calls with no parameters,
/// results, claims or cleanup of its own.
pub(super) fn exact_nominal_target<'a>(
    functions: &std::collections::BTreeMap<MachineId, &'a MachineCodeFunction>,
    nominal: &terminal_psi::NominalAffineCleanup,
) -> (Option<&'a MachineCodeFunction>, bool) {
    if nominal.cleanup_receiver.is_some() || !nominal.requirement_obligations.is_empty() {
        return (None, false);
    }
    let cleanup_function = functions.get(&nominal.cleanup_machine).copied();
    let cleanup_body_is_exact = cleanup_function.is_some_and(|function| {
        let calls = &function.internal_unit_calls;
        let call_owners = calls
            .iter()
            .map(|call| call.owner)
            .collect::<std::collections::BTreeSet<_>>();
        let call_targets = calls
            .iter()
            .map(|call| call.target)
            .collect::<std::collections::BTreeSet<_>>();
        function.attachment == Some(nominal.structural_type)
            && function.unit_stack.is_some()
            && function.scalar_stack.is_none()
            && function.unit_parameters.is_empty()
            && function.unit_parameter_homes.is_empty()
            && function
                .unit_affine_cleanup
                .as_ref()
                .is_some_and(|return_cleanup| {
                    return_cleanup.locals.is_empty() && return_cleanup.actions.is_empty()
                })
            && call_owners.len() == calls.len()
            && call_targets.len() == calls.len()
            && calls.iter().enumerate().all(|(ordinal, call)| {
                matches!(call.owner, CallSiteOwner::Operation(operation)
                    if function.provenance.operations.get(ordinal) == Some(&operation))
                    && call.operation_ordinal == ordinal
                    && call.result.is_none()
                    && call.arguments.is_empty()
                    && call.claim_transfers.is_empty()
                    && functions.get(&call.target).is_some_and(|helper| {
                        helper.attachment.is_some()
                            && helper.unit_stack.is_some()
                            && helper.scalar_stack.is_none()
                            && helper.unit_parameters.is_empty()
                            && helper.unit_parameter_homes.is_empty()
                            && helper.internal_unit_calls.is_empty()
                            && helper
                                .unit_affine_cleanup
                                .as_ref()
                                .is_some_and(|return_cleanup| {
                                    return_cleanup.locals.is_empty()
                                        && return_cleanup.actions.is_empty()
                                })
                    })
            })
            && calls.windows(2).all(|pair| {
                pair[0]
                    .code_offset
                    .checked_add(pair[0].byte_count)
                    .is_some_and(|end| end <= pair[1].code_offset)
            })
    });
    (cleanup_function, cleanup_body_is_exact)
}

/// Whether the nominal cleanup actions are malformed: each parameter home
/// (last first) must be discarded whole or handed to a nominal cleanup of
/// its own type through a bounded receiver, every invoked cleanup body must
/// be exact, and the cleanup's calls must be exactly one per executable
/// action, in action order and inside the cleanup's bytes.
pub(super) fn nominal_cleanups_are_malformed(inputs: &CleanupInputs<'_>, end: usize) -> bool {
    let CleanupInputs {
        parameter_homes,
        internal_unit_calls,
        attachments,
        functions,
        cleanup,
        allow_mixed_nominal_roots,
        ..
    } = *inputs;
    let nominal = cleanup
        .actions
        .iter()
        .enumerate()
        .filter_map(|(ordinal, action)| match action {
            terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                Some((ordinal, cleanup))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if nominal.is_empty()
        || (!allow_mixed_nominal_roots && nominal.len() != cleanup.actions.len())
        || !cleanup.locals.is_empty()
        || parameter_homes.len() != cleanup.actions.len()
        || parameter_homes
            .iter()
            .rev()
            .zip(&cleanup.actions)
            .any(|(home, action)| match action {
                terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place) => {
                    *place != home.place
                        || home.multiplicity != terminal_psi::StructuralMultiplicity::Affine
                        || home.access != terminal_psi::StructuralAccess::Owned
                }
                terminal_psi::TerminalAffineCleanupAction::InvokeNominal(nominal) => {
                    home.place != nominal.place
                        || home.structural_type != nominal.structural_type
                        || home.multiplicity != terminal_psi::StructuralMultiplicity::Affine
                        || home.access != terminal_psi::StructuralAccess::Owned
                        || !bounded_nominal_receiver_shape(home.shape)
                        || (home.shape.byte_size == 0 && !home.source.locations.is_empty())
                        || (home.shape.byte_size != 0 && home.source.locations.is_empty())
                        || attachments.get(&nominal.cleanup_machine)
                            != Some(&Some(nominal.structural_type))
                }
                terminal_psi::TerminalAffineCleanupAction::DiscardResidual(_) => true,
            })
    {
        true
    } else {
        let targets = nominal
            .iter()
            .map(|(_, nominal)| exact_nominal_target(functions, nominal))
            .collect::<Vec<_>>();
        let executable_ordinals = targets
            .iter()
            .zip(&nominal)
            .filter_map(|((function, _), (action_ordinal, _))| {
                function
                    .is_some_and(|function| !function.internal_unit_calls.is_empty())
                    .then_some(*action_ordinal)
            })
            .collect::<Vec<_>>();
        let cleanup_calls = internal_unit_calls
            .iter()
            .filter(|call| {
                matches!(
                    call.owner,
                    CallSiteOwner::CleanupAction { edge, .. }
                        if edge == cleanup.psi_edge
                )
            })
            .collect::<Vec<_>>();
        let ordered_executable_spans = executable_ordinals
            .iter()
            .map(|ordinal| {
                let action_ordinal = u32::try_from(*ordinal).ok()?;
                let nominal = cleanup
                    .actions
                    .get(*ordinal)
                    .and_then(|action| match action {
                        terminal_psi::TerminalAffineCleanupAction::InvokeNominal(nominal) => {
                            Some(nominal)
                        }
                        _ => None,
                    })?;
                let call = cleanup_calls.iter().find(|call| {
                    call.owner
                        == CallSiteOwner::CleanupAction {
                            edge: cleanup.psi_edge,
                            action_ordinal,
                        }
                        && call.target == nominal.cleanup_machine
                })?;
                Some((
                    call.code_offset,
                    call.code_offset.checked_add(call.byte_count)?,
                ))
            })
            .collect::<Option<Vec<_>>>();
        targets.iter().any(|(_, body_exact)| !body_exact)
            || cleanup_calls.len() != executable_ordinals.len()
            || ordered_executable_spans.is_none_or(|spans| {
                spans
                    .windows(2)
                    .any(|pair| pair[0].0 >= pair[1].0 || pair[0].1 > pair[1].0)
            })
            || executable_ordinals.iter().any(|ordinal| {
                let Ok(action_ordinal) = u32::try_from(*ordinal) else {
                    return true;
                };
                let Some(terminal_psi::TerminalAffineCleanupAction::InvokeNominal(nominal)) =
                    cleanup.actions.get(*ordinal)
                else {
                    return true;
                };
                cleanup_calls
                    .iter()
                    .filter(|call| {
                        call.owner
                            == CallSiteOwner::CleanupAction {
                                edge: cleanup.psi_edge,
                                action_ordinal,
                            }
                            && call.target == nominal.cleanup_machine
                            && call.arguments.is_empty()
                            && call.claim_transfers.is_empty()
                            && call.code_offset >= cleanup.code_offset
                            && call
                                .code_offset
                                .checked_add(call.byte_count)
                                .is_some_and(|call_end| call_end <= end)
                    })
                    .count()
                    != 1
            })
    }
}
