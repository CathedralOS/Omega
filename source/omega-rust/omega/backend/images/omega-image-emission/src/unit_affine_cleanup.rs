//! Exact Unit-affine cleanup evidence replay.
//!
//! This module validates root, residual, and nominal cleanup actions against
//! retained places, structural paths, provenance, code attribution, and cleanup targets.
//! It does not choose cleanup actions, infer layouts, or emit instructions.

use omega_machine_code::{
    InternalUnitCallRecord, MachineCodeFunction, SemanticCodeAttribution, SemanticCodeSite,
    UnitAffineCleanupRecord, UnitParameterHomeRecord,
};
use omega_target_operations::{CallSiteOwner, TerminalPsiProvenance};
use psi_core::{MachineId, StructuralTypeId};

use super::{ObjectError, exact_partial_cleanup_partition};

pub(super) fn exact_construction_prefix(cleanup: &UnitAffineCleanupRecord) -> bool {
    let construction_locals = cleanup
        .locals
        .iter()
        .filter_map(|(_, place, element_type)| match place.kind {
            psi_core::StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                structural_type,
                construction: Some(construction),
            } => Some((
                declaration_ordinal,
                structural_type,
                construction,
                element_type.id,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    if construction_locals.is_empty() {
        return true;
    }
    let expected_root_length = match construction_locals.len() {
        2 => 3,
        3 => 4,
        4 => 5,
        5 => 6,
        6 => 7,
        7 => 8,
        8 => 9,
        9 => 10,
        10 => 11,
        11 => 12,
        12 => 13,
        13 => 14,
        14 => 15,
        15 => 16,
        16 => 17,
        17 => 18,
        18 => 19,
        19 => 20,
        20 => 21,
        21 => 22,
        22 => 23,
        _ => return false,
    };
    construction_locals.len() == cleanup.locals.len()
        && construction_locals.iter().enumerate().all(
            |(index, (ordinal, structural_type, construction, declared_type))| {
                usize::try_from(*ordinal) == Ok(index)
                    && usize::try_from(construction.index) == Ok(index)
                    && structural_type == declared_type
            },
        )
        && construction_locals
            .first()
            .is_some_and(|(_, element_type, first, _)| {
                construction_locals
                    .iter()
                    .all(|(_, candidate_type, candidate, _)| {
                        candidate_type == element_type
                            && candidate.root_structural_type == first.root_structural_type
                    })
                    && cleanup.structural_types.iter().any(|root| {
                        root.id == first.root_structural_type
                            && matches!(
                                root.shape,
                                psi_terminal::StructuralTypeShape::FixedArray { element, length }
                                    if element == *element_type
                                        && length == expected_root_length
                            )
                    })
            })
}

pub(super) fn validate_unit_affine_cleanup(
    machine: MachineId,
    provenance: &TerminalPsiProvenance,
    bytes: &[u8],
    attribution: &[SemanticCodeAttribution],
    parameter_homes: &[UnitParameterHomeRecord],
    internal_unit_calls: &[InternalUnitCallRecord],
    borrowed_roots: &std::collections::BTreeSet<psi_core::PlaceId>,
    attachments: &std::collections::BTreeMap<MachineId, Option<StructuralTypeId>>,
    functions: &std::collections::BTreeMap<MachineId, &MachineCodeFunction>,
    cleanup: &UnitAffineCleanupRecord,
    allow_mixed_nominal_roots: bool,
    fully_consumed_affine_pair: bool,
    partially_consumed_affine_array: bool,
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidUnitAffineCleanupEvidence(machine);
    if !exact_construction_prefix(cleanup) {
        return Err(invalid());
    }
    let end = cleanup
        .code_offset
        .checked_add(cleanup.byte_count)
        .ok_or_else(invalid)?;
    let local_places = cleanup
        .locals
        .iter()
        .map(|(_, place, _)| place.id)
        .collect::<Vec<_>>();
    let transferred_roots = internal_unit_calls
        .iter()
        .flat_map(|call| &call.arguments)
        .filter(|argument| argument.path.is_empty())
        .map(|argument| argument.place)
        .collect::<std::collections::BTreeSet<_>>();
    let expected_local_prefix = local_places
        .iter()
        .rev()
        .filter(|place| !transferred_roots.contains(place))
        .copied()
        .collect::<Vec<_>>();
    let expected_parameter_suffix = parameter_homes
        .iter()
        .rev()
        .filter(|home| {
            home.multiplicity == psi_terminal::StructuralMultiplicity::Affine
                && !transferred_roots.contains(&home.place)
                && !borrowed_roots.contains(&home.place)
                && !fully_consumed_affine_pair
        })
        .map(|home| home.place)
        .collect::<Vec<_>>();
    let local_operations = cleanup
        .locals
        .iter()
        .map(|(operation, _, _)| *operation)
        .collect::<std::collections::BTreeSet<_>>();
    let expected_root_actions = expected_local_prefix
        .iter()
        .copied()
        .chain(expected_parameter_suffix.iter().copied())
        .map(psi_terminal::TerminalAffineCleanupAction::DiscardRoot)
        .collect::<Vec<_>>();
    let expected_local_actions = expected_local_prefix
        .iter()
        .copied()
        .map(psi_terminal::TerminalAffineCleanupAction::DiscardRoot)
        .collect::<Vec<_>>();
    let exact_nominal_target = |nominal: &psi_terminal::NominalAffineCleanup| {
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
                                && helper.unit_affine_cleanup.as_ref().is_some_and(
                                    |return_cleanup| {
                                        return_cleanup.locals.is_empty()
                                            && return_cleanup.actions.is_empty()
                                    },
                                )
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
    };
    let action_shape_invalid = if cleanup.actions == expected_root_actions {
        cleanup
            .actions
            .iter()
            .filter_map(|action| match action {
                psi_terminal::TerminalAffineCleanupAction::DiscardRoot(place) => Some(*place),
                _ => None,
            })
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != cleanup.actions.len()
    } else if matches!(
        cleanup.actions.get(expected_local_actions.len()),
        Some(psi_terminal::TerminalAffineCleanupAction::DiscardResidual(
            _
        ))
    ) {
        let residual_actions = &cleanup.actions[expected_local_actions.len()..];
        let residuals = residual_actions
            .iter()
            .filter_map(|action| match action {
                psi_terminal::TerminalAffineCleanupAction::DiscardResidual(residual) => {
                    Some(residual)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let residual_root = residuals.first().map(|residual| residual.place);
        let parameter_type = residual_root.and_then(|place| {
            parameter_homes
                .iter()
                .find(|parameter| parameter.place == place)
                .map(|parameter| parameter.structural_type)
        });
        let moved = internal_unit_calls
            .iter()
            .flat_map(|call| &call.arguments)
            .filter(|argument| {
                Some(argument.place) == residual_root
                    && Some(argument.root_structural_type) == parameter_type
            })
            .map(|argument| (argument.path.as_slice(), argument.structural_type))
            .collect::<Vec<_>>();
        let parameter_is_bounded_affine_array = parameter_type.is_some_and(|root_type| {
            cleanup.structural_types.iter().any(|declaration| {
                declaration.id == root_type
                    && matches!(
                        declaration.shape,
                        psi_terminal::StructuralTypeShape::FixedArray { length: 3 | 4, .. }
                    )
            })
        });
        cleanup.actions[..expected_local_actions.len()] != expected_local_actions
            || residuals.len() != residual_actions.len()
            || residuals.is_empty()
            || residual_root.is_none_or(|root| expected_parameter_suffix.as_slice() != [root])
            || parameter_type.is_none()
            || (parameter_is_bounded_affine_array && !partially_consumed_affine_array)
            || residuals.iter().any(|residual| {
                Some(residual.place) != residual_root
                    || residual.path.is_empty()
                    || !is_partial_cleanup_path(&residual.path)
                    || parameter_type == Some(residual.structural_type)
            })
            || residuals.iter().enumerate().any(|(index, residual)| {
                residuals[..index].iter().any(|earlier| {
                    residual.path.starts_with(&earlier.path)
                        || earlier.path.starts_with(&residual.path)
                })
            })
            || moved.is_empty()
            || moved.iter().any(|(path, _)| {
                path.is_empty()
                    || !is_partial_cleanup_path(path)
                    || residuals.iter().any(|residual| {
                        path.starts_with(&residual.path) || residual.path.starts_with(path)
                    })
            })
            || moved.iter().enumerate().any(|(index, (path, _))| {
                moved[..index]
                    .iter()
                    .any(|(earlier, _)| path.starts_with(earlier) || earlier.starts_with(path))
            })
            || parameter_type.is_none_or(|root_type| {
                !exact_partial_cleanup_partition(
                    &cleanup.structural_types,
                    root_type,
                    &moved,
                    &residuals,
                )
            })
    } else {
        let nominal = cleanup
            .actions
            .iter()
            .enumerate()
            .filter_map(|(ordinal, action)| match action {
                psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
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
                    psi_terminal::TerminalAffineCleanupAction::DiscardRoot(place) => {
                        *place != home.place
                            || home.multiplicity != psi_terminal::StructuralMultiplicity::Affine
                    }
                    psi_terminal::TerminalAffineCleanupAction::InvokeNominal(nominal) => {
                        home.place != nominal.place
                            || home.structural_type != nominal.structural_type
                            || home.multiplicity != psi_terminal::StructuralMultiplicity::Affine
                            || !bounded_nominal_receiver_shape(home.shape)
                            || (home.shape.byte_size == 0 && !home.source.locations.is_empty())
                            || (home.shape.byte_size != 0 && home.source.locations.is_empty())
                            || attachments.get(&nominal.cleanup_machine)
                                != Some(&Some(nominal.structural_type))
                    }
                    psi_terminal::TerminalAffineCleanupAction::DiscardResidual(_) => true,
                })
        {
            true
        } else {
            let targets = nominal
                .iter()
                .map(|(_, nominal)| exact_nominal_target(nominal))
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
                    let nominal =
                        cleanup
                            .actions
                            .get(*ordinal)
                            .and_then(|action| match action {
                                psi_terminal::TerminalAffineCleanupAction::InvokeNominal(
                                    nominal,
                                ) => Some(nominal),
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
                    let Some(psi_terminal::TerminalAffineCleanupAction::InvokeNominal(nominal)) =
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
    };
    if cleanup.byte_count == 0
        || end != bytes.len()
        || !provenance.edges.contains(&cleanup.psi_edge)
        || local_operations.len() != cleanup.locals.len()
        || cleanup.locals.iter().enumerate().any(
            |(ordinal, (operation, place, structural_type))| {
                !provenance.operations.contains(operation)
                    || !matches!(
                        place.kind,
                        psi_core::StructuralPlaceKind::TrivialAffineLocal {
                            declaration_ordinal,
                            structural_type: local_type,
                            ..
                        } if usize::try_from(declaration_ordinal) == Ok(ordinal)
                            && local_type == structural_type.id
                    )
                    || !matches!(
                        structural_type.shape,
                        psi_terminal::StructuralTypeShape::Record { ref fields }
                            if fields.is_empty()
                    )
                    || attribution
                        .iter()
                        .filter(|attribution| {
                            attribution.site == SemanticCodeSite::Operation(*operation)
                                && attribution.byte_count == 0
                        })
                        .count()
                        != 1
            },
        )
        || action_shape_invalid
        || attribution
            .iter()
            .filter(|attribution| {
                attribution.site == SemanticCodeSite::Edge(cleanup.psi_edge)
                    && attribution.code_offset == cleanup.code_offset
                    && attribution.byte_count == cleanup.byte_count
            })
            .count()
            != 1
    {
        return Err(invalid());
    }
    Ok(())
}

fn is_partial_cleanup_path(path: &[psi_terminal::StructuralPathSegment]) -> bool {
    (!path.is_empty()
        && path.iter().all(|segment| {
            matches!(segment,
                psi_terminal::StructuralPathSegment::Field(identity) if !identity.is_empty())
        }))
        || matches!(
            path,
            [psi_terminal::StructuralPathSegment::FixedIndex(
                0 | 1 | 2 | 3
            )] | [
                psi_terminal::StructuralPathSegment::FixedIndex(0 | 1),
                psi_terminal::StructuralPathSegment::FixedIndex(0 | 1 | 2 | 3 | 4 | 5 | 6 | 7),
            ]
        )
}

fn bounded_nominal_receiver_shape(shape: omega_calling_conventions::ValueShape) -> bool {
    shape == omega_calling_conventions::ValueShape::integer(0, 1)
        || shape.class == omega_calling_conventions::ValueClass::Integer
            && shape.byte_size != 0
            && matches!(shape.alignment, 1 | 2 | 4 | 8)
            && shape.byte_size % shape.alignment == 0
}
