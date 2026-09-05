//! Proven exclusive-reference origins shared by boundary and method receivers.

use super::caller_aliases::{CallerWriteSite, caller_binding_type, caller_statement_at_site};
use super::isolation::{data_definition_has_only_owned_storage, type_is_caller_isolated_local};
use super::{
    FramePathPrecision, FramePlaceOrigin, frame_place_path, transparent_call_result_origin,
};
use crate::symbols::TopLevelSymbols;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Receiver lookup spelling and storage precision travel together. A proven
/// computed origin is storage evidence, not authority for lookup by name.
pub(super) fn receiver_frame_origin(
    program: &TypedTrees,
    current_machine: &Machine,
    receiver: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
) -> Option<(Vec<String>, Option<FramePlaceOrigin>)> {
    if !receiver.is_valid() {
        return Some((Vec::new(), None));
    }
    let origin = frame_place_path(program, receiver).or_else(|| {
        owned_receiver_origin(program, current_machine, receiver, symbols, active_states)
    })?;
    let members = origin.path.split('.').map(str::to_owned).collect();
    Some((members, Some(origin)))
}

/// Reuse the checked body's result relation, validating its selected input for
/// caller storage. A helper cannot turn an untracked reference-bearing carrier
/// or a foreign binding identity into a proven caller storage origin.
pub(super) fn exclusive_reference_origin(
    program: &TypedTrees,
    current_machine: &Machine,
    argument: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
) -> Option<FramePlaceOrigin> {
    match program.expression_table.expression(argument) {
        ExpressionNode::Borrow(place) if place.access.is_exclusive() => {
            frame_place_path(program, place.target)
        }
        ExpressionNode::Name(_) => Some(FramePlaceOrigin {
            path: exclusive_reference_binding_path(program, current_machine, argument)?,
            precision: FramePathPrecision::Exact,
        }),
        ExpressionNode::Call(call) => transparent_call_result_origin(
            program,
            call,
            symbols,
            active_states,
            |callee_machine, parameter, actual, active_states| {
                let referee = exclusive_reference_referee(program, parameter.type_reference)?;
                let owned = if parameter.is_self {
                    // The typed receiver uses nominal `Self`, whose concrete
                    // declaration belongs to this resolved attached machine.
                    let attached = callee_machine.attached_data.as_ref()?;
                    let mut definitions = program
                        .data_definitions()
                        .iter()
                        .filter(|definition| definition.name == *attached);
                    let definition = definitions.next()?;
                    definitions.next().is_none()
                        && data_definition_has_only_owned_storage(program, definition)
                } else {
                    referent_has_only_owned_storage(program, referee)
                };
                if !owned {
                    return None;
                }
                if parameter.is_self {
                    // Attached methods implicitly borrow their owned receiver;
                    // they cannot bypass the origin fence on a loaded reference.
                    match program.expression_table.expression(actual) {
                        ExpressionNode::Name(_)
                        | ExpressionNode::Member(_)
                        | ExpressionNode::Indexed(_) => {
                            return owned_receiver_origin(
                                program,
                                current_machine,
                                actual,
                                symbols,
                                active_states,
                            );
                        }
                        _ => {}
                    }
                }
                exclusive_reference_origin(program, current_machine, actual, symbols, active_states)
            },
        ),
        _ => None,
    }
}

fn exclusive_reference_has_owned_storage(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> bool {
    exclusive_reference_referee(program, reference)
        .is_some_and(|referee| referent_has_only_owned_storage(program, referee))
}

fn exclusive_reference_referee(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            exclusive_reference_referee(program, *base_type)
        }
        TypeReferenceNode::Reference {
            access, referee, ..
        } => (reference.is_valid() && access.is_exclusive()).then_some(*referee),
        _ => None,
    }
}

fn owned_receiver_origin(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
) -> Option<FramePlaceOrigin> {
    let parent = match program.expression_table.expression(expression) {
        ExpressionNode::Name(_) => {
            let reference = caller_binding_type(program, current_machine, expression)?;
            if exclusive_reference_referee(program, reference).is_none()
                && !type_is_caller_isolated_local(program, reference)
            {
                return None;
            }
            return frame_place_path(program, expression);
        }
        ExpressionNode::Member(member) => member.receiver,
        ExpressionNode::Indexed(indexed) => indexed.collection,
        ExpressionNode::Call(_) => {
            return exclusive_reference_origin(
                program,
                current_machine,
                expression,
                symbols,
                active_states,
            );
        }
        _ => return None,
    };
    let origin = owned_receiver_origin(program, current_machine, parent, symbols, active_states)?;
    let (state, _, _) = caller_statement_at_site(
        program,
        current_machine,
        CallerWriteSite::Expression(expression),
    )?;
    let reference =
        crate::places::declared_place_type_raw(program, current_machine, Some(state), expression)?;
    if !type_is_caller_isolated_local(program, reference) {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Indexed(_) => Some(FramePlaceOrigin {
            path: origin.path,
            precision: FramePathPrecision::CollectionCoarse,
        }),
        ExpressionNode::Member(member) => Some(match origin.precision {
            FramePathPrecision::Exact => FramePlaceOrigin {
                path: format!("{}.{}", origin.path, member.member.as_str()),
                precision: FramePathPrecision::Exact,
            },
            FramePathPrecision::CollectionCoarse => origin,
        }),
        _ => None,
    }
}

/// Forward a reference value, not a borrow of its binding slot. Only an exact
/// caller declaration can supply its type. This returns a raw binding path:
/// the state transfer and public demand closure own local-origin admission.
/// Replaying that prefix here would recursively replay every earlier boundary
/// call. Carrier fields stay opaque.
fn exclusive_reference_binding_path(
    program: &TypedTrees,
    current_machine: &Machine,
    argument: ExpressionHandle,
) -> Option<String> {
    let reference = caller_binding_type(program, current_machine, argument)?;
    exclusive_reference_has_owned_storage(program, reference)
        .then(|| frame_place_path(program, argument).map(|origin| origin.path))
        .flatten()
}

pub(super) fn referent_has_only_owned_storage(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            referent_has_only_owned_storage(program, *base_type)
        }
        TypeReferenceNode::Slice { element_type } => {
            type_is_caller_isolated_local(program, *element_type)
        }
        TypeReferenceNode::Unit => reference.is_valid(),
        _ => type_is_caller_isolated_local(program, reference),
    }
}
