//! Proven exclusive-reference origins shared by boundary and method receivers.

use super::caller_aliases::{CallerWriteSite, caller_binding_type, caller_statement_at_site};
use super::isolation::{
    data_definition_has_only_owned_storage, type_is_caller_isolated_local,
    type_is_caller_isolated_local_in,
};
use super::type_instantiation::{bind_formal_type, substituted_head};
use super::{FramePathPrecision, FramePlaceOrigin, frame_place_path};
use crate::declarations::symbols::TopLevelSymbols;
use crate::machine_calls::calls::write_frames::FrameInference;
use crate::machine_calls::calls::write_frames::place_paths::{
    push_unique_origin, single_place_origin,
};
use crate::machine_calls::calls::write_frames::transparent_results::transparent_call_result_origins;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Receiver lookup spelling and storage precision travel together. A proven
/// computed origin is storage evidence, not authority for lookup by name.
pub(super) fn receiver_frame_origin(
    program: &TypedTrees,
    current_machine: &Machine,
    receiver: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<(Vec<String>, Option<FramePlaceOrigin>)> {
    if !receiver.is_valid() {
        return Some((Vec::new(), None));
    }
    let origin = frame_place_path(program, receiver).or_else(|| {
        owned_receiver_origin(program, current_machine, receiver, symbols, inference)
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
    inference: &mut FrameInference,
) -> Option<FramePlaceOrigin> {
    single_place_origin(exclusive_reference_origins(
        program,
        current_machine,
        argument,
        symbols,
        inference,
    )?)
}

/// Every proven caller-storage origin the argument may lend to a callee's
/// exclusive parameter. Direct borrows and bindings supply one origin; a
/// transparent helper result or a divergent match keeps the exact finite
/// union of its routes. An unproven route fails the whole argument closed.
pub(super) fn exclusive_reference_origins(
    program: &TypedTrees,
    current_machine: &Machine,
    argument: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<FramePlaceOrigin>> {
    let origins = match program.expression_table.expression(argument) {
        ExpressionNode::Borrow(place) if place.access.is_exclusive() => {
            declared_origin_root(program, current_machine, place.target)?;
            vec![frame_place_path(program, place.target)?]
        }
        ExpressionNode::Name(_) => vec![
            exclusive_reference_binding_path(program, current_machine, argument)
                .map(|path| FramePlaceOrigin {
                    path,
                    precision: FramePathPrecision::Exact,
                    source: super::FrameSourcePlace::from_expression(program, argument),
                })
                .or_else(|| carried_reference_origin(program, current_machine, argument))?,
        ],
        ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
            vec![carried_reference_origin(
                program,
                current_machine,
                argument,
            )?]
        }
        ExpressionNode::Match(dispatch) => {
            let mut selected = Vec::new();
            for arm in program.expression_table.match_arms(dispatch.arms) {
                for origin in exclusive_reference_origins(
                    program,
                    current_machine,
                    arm.value,
                    symbols,
                    inference,
                )? {
                    push_unique_origin(&mut selected, origin);
                }
            }
            selected
        }
        ExpressionNode::Call(call) => transparent_call_result_origins(
            program,
            call,
            symbols,
            inference,
            |callee_machine, parameter, result_place, actual, inference| {
                let Some(referee) = exclusive_reference_referee(program, parameter.type_reference)
                else {
                    // The result is rooted in a by-value carrier parameter's
                    // declared reference leaf. The actual must be a caller
                    // place spelling the carrier itself; the shared suffix
                    // composition below then names the leaf inside it.
                    if parameter.is_self || result_place.source.root != parameter.symbol {
                        return None;
                    }
                    declared_origin_root(program, current_machine, actual)?;
                    return frame_place_path(program, actual).map(|origin| vec![origin]);
                };
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
                    // A generic helper's carrier is judged on the storage the
                    // call instantiates, not the formal parameter name: bind
                    // the callee's `Type` parameters from this actual's
                    // declared type, then run the same owned-storage gate.
                    let type_parameters = program.machine_type_parameters(callee_machine);
                    let mut bindings = Vec::new();
                    if !type_parameters.is_empty() {
                        let (state, _, _) = caller_statement_at_site(
                            program,
                            current_machine,
                            CallerWriteSite::Expression(actual),
                        )?;
                        let actual_type = crate::value_custody::places::declared_place_type_raw(
                            program,
                            current_machine,
                            Some(state),
                            actual,
                        )?;
                        if !bind_formal_type(
                            program,
                            parameter.type_reference,
                            actual_type,
                            type_parameters,
                            &mut bindings,
                        ) {
                            return None;
                        }
                    }
                    referent_has_only_owned_storage_in(program, referee, &bindings)
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
                                inference,
                            )
                            .map(|origin| vec![origin]);
                        }
                        _ => {}
                    }
                }
                exclusive_reference_origins(program, current_machine, actual, symbols, inference)
            },
        )?,
        _ => return None,
    };
    (!origins.is_empty()).then_some(origins)
}

/// An owned carrier transports its declared reference leaf as a value. This
/// symbolic source is frozen by state transfer; it is not a loaded carrier
/// behind another reference, nor permission to replace the reference slot.
fn carried_reference_origin(
    program: &TypedTrees,
    machine: &Machine,
    expression: ExpressionHandle,
) -> Option<FramePlaceOrigin> {
    let mut root_expression = expression;
    loop {
        root_expression = match program.expression_table.expression(root_expression) {
            ExpressionNode::Name(_) => break,
            ExpressionNode::Member(member) => member.receiver,
            ExpressionNode::Indexed(indexed) => indexed.collection,
            _ => return None,
        };
    }
    let root_type =
        super::caller_aliases::caller_name_root_type(program, machine, root_expression)?;
    if super::type_reference_is_reference(program, root_type) {
        return None;
    }
    let (state, _, _) =
        caller_statement_at_site(program, machine, CallerWriteSite::Expression(expression))?;
    let reference = crate::value_custody::places::declared_place_type_raw(
        program,
        machine,
        Some(state),
        expression,
    )?;
    if !exclusive_reference_has_owned_storage(program, reference) {
        return None;
    }
    let origin = frame_place_path(program, expression)?;
    let declared = super::stored_origins::declared_origins(
        program,
        origin.source.root,
        program.symbols.name(origin.source.root),
        root_type,
    )?;
    declared
        .references
        .iter()
        .any(|leaf| {
            origin.source.segments.len() == leaf.local_segments.len()
                && super::stored_origins::source_reaches_leaf(
                    &origin.source.segments,
                    &leaf.local_segments,
                )
        })
        .then_some(origin)
}

/// Raw borrowed paths retain a real caller declaration before their string
/// footprint can select a formal origin. In particular, a stale or foreign
/// root with the same spelling cannot be exported through a helper result.
pub(super) fn declared_origin_root(
    program: &TypedTrees,
    machine: &Machine,
    mut expression: ExpressionHandle,
) -> Option<()> {
    loop {
        expression = match program.expression_table.expression(expression) {
            ExpressionNode::Name(name) => {
                let members = program.expression_table.name_path_members(name.members);
                if name.head_symbol == machine.symbol
                    && (members.len() != 1 || name.symbol == machine.symbol)
                    && program.symbols.get(machine.symbol).kind == symbols::SymbolKind::Machine
                    && members
                        .first()
                        .is_some_and(|member| member.as_str() == "self")
                {
                    return Some(());
                }
                return super::caller_aliases::caller_name_root_type(program, machine, expression)
                    .map(|_| ());
            }
            ExpressionNode::Member(member) => member.receiver,
            ExpressionNode::Indexed(indexed) => indexed.collection,
            _ => return None,
        };
    }
}

fn exclusive_reference_has_owned_storage(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> bool {
    exclusive_reference_referee(program, reference)
        .is_some_and(|referee| referent_has_only_owned_storage(program, referee))
}

pub(super) fn exclusive_reference_referee(
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

pub(super) fn owned_receiver_origin(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
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
                inference,
            );
        }
        _ => return None,
    };
    let origin = owned_receiver_origin(program, current_machine, parent, symbols, inference)?;
    let (state, _, _) = caller_statement_at_site(
        program,
        current_machine,
        CallerWriteSite::Expression(expression),
    )?;
    let reference = crate::value_custody::places::declared_place_type_raw(
        program,
        current_machine,
        Some(state),
        expression,
    )?;
    if !type_is_caller_isolated_local(program, reference) {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Indexed(_) => Some(FramePlaceOrigin {
            source: origin.source.projected(program, expression, parent),
            path: origin.path,
            precision: FramePathPrecision::CollectionCoarse,
        }),
        ExpressionNode::Member(member) => {
            let source = origin.source.projected(program, expression, parent);
            Some(match origin.precision {
                FramePathPrecision::Exact => FramePlaceOrigin {
                    path: format!("{}.{}", origin.path, member.member.as_str()),
                    precision: FramePathPrecision::Exact,
                    source,
                },
                FramePathPrecision::CollectionCoarse => FramePlaceOrigin { source, ..origin },
            })
        }
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
    referent_has_only_owned_storage_in(program, reference, &[])
}

/// Under an active substitution a bound parameter head resolves to its
/// caller actual before the owned-storage walk; an unbound parameter keeps
/// the named leaf and stays opaque.
pub(super) fn referent_has_only_owned_storage_in(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    bindings: &[(symbols::SymbolHandle, TypeReferenceHandle)],
) -> bool {
    let reference = substituted_head(program, reference, bindings);
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            referent_has_only_owned_storage_in(program, *base_type, bindings)
        }
        TypeReferenceNode::Slice { element_type } => {
            type_is_caller_isolated_local_in(program, *element_type, bindings)
        }
        TypeReferenceNode::Unit => reference.is_valid(),
        _ => type_is_caller_isolated_local_in(program, reference, bindings),
    }
}
