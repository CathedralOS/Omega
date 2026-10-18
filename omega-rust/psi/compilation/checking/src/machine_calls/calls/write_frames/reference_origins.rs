//! Proven exclusive-reference origins shared by boundary and method receivers.

use super::caller_aliases::{CallerWriteSite, caller_binding_type, caller_statement_owner};
use super::isolation::{type_is_caller_isolated_local, type_is_caller_isolated_local_in};
use super::type_instantiation::{bind_formal_type, substituted_head};
use super::{FramePathPrecision, FramePlaceOrigin, frame_place_path};
use crate::declarations::symbols::TopLevelSymbols;
use crate::machine_calls::calls::write_frames::FrameInference;
use crate::machine_calls::calls::write_frames::place_paths::{
    push_unique_origin, single_place_origin,
};
use crate::machine_calls::calls::write_frames::transparent_results::transparent_call_result_origins;
use facts::PlaceSegment;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Resolve retained receiver members against their actual declarations. The
/// caller authenticates the root declaration; this owner authenticates every
/// field and the final retained symbol, sharing one path with boundary lookup.
/// Resolving a type does not prove that a stored reference still denotes the
/// same storage; origin consumers separately validate those crossings.
pub(super) fn retained_receiver_projection<'name>(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
    root: symbols::SymbolHandle,
    leaf: symbols::SymbolHandle,
    fields: impl IntoIterator<Item = &'name str>,
) -> Option<(Vec<PlaceSegment>, TypeReferenceHandle)> {
    if !root.is_valid() || !leaf.is_valid() {
        return None;
    }
    let mut selected = root;
    let mut segments = Vec::new();
    for name in fields {
        let definition =
            crate::value_custody::places::data_definition_for_type(program, reference)?;
        let mut candidates = program
            .data_members(definition)
            .iter()
            .filter_map(|member| match member {
                typed_trees::data::DataMember::Field(field) if field.name.as_str() == name => {
                    Some(field)
                }
                _ => None,
            });
        let field = candidates.next()?;
        if candidates.next().is_some() {
            return None;
        }
        selected = field.symbol;
        reference = field.type_reference;
        segments.push(PlaceSegment::Field {
            symbol: field.symbol,
        });
    }
    (selected == leaf).then_some((segments, reference))
}

/// Receiver lookup spelling and storage precision travel together. A proven
/// computed origin is storage evidence, not authority for lookup by name.
///
/// A receiver carrying a finite divergent candidate set keeps every proven
/// route: the callee's receiver-relative writes instantiate under each base and
/// union, and one unproven route fails the whole frame closed. A divergent
/// receiver has no single member-chain spelling, so its member path is empty;
/// its referent data name is the only remaining callee selector, so it is
/// returned beside the candidates — a resolved attached machine still names
/// the one execution the call can take.
/// Boundary authority is not provider-object storage. Its operand is still
/// traversed for effects, but contributes no receiver-relative write base.
pub(super) fn receiver_frame_origins(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<(Vec<String>, Vec<FramePlaceOrigin>, Option<String>, bool)> {
    let receiver = program.expression_table.call_node(expression)?.receiver;
    if !receiver.is_valid() {
        return Some((Vec::new(), Vec::new(), None, false));
    }
    if super::boundary_calls::call_has_authority_receiver(
        program,
        current_machine,
        CallerWriteSite::Expression(expression),
    ) {
        return Some((
            super::receiver_member_chain(program, receiver).unwrap_or_default(),
            Vec::new(),
            None,
            true,
        ));
    }
    if let Some(origin) = frame_place_path(program, receiver)
        .or_else(|| owned_receiver_origin(program, current_machine, receiver, symbols, inference))
    {
        let members = origin.path.split('.').map(str::to_owned).collect();
        return Some((members, vec![origin], None, false));
    }
    match program.expression_table.expression(receiver) {
        ExpressionNode::Call(_) | ExpressionNode::Match(_) => {
            let origins = exclusive_reference_origins(
                program,
                current_machine,
                receiver,
                symbols,
                inference,
            )?;
            if origins.is_empty() {
                return None;
            }
            Some((
                Vec::new(),
                origins,
                receiver_referee_name(program, current_machine, receiver),
                false,
            ))
        }
        _ => None,
    }
}

/// The named data a computed receiver borrows into — the callee selector a
/// divergent candidate set still admits when the call's own target symbol is
/// unresolved. A match receiver names it only when every arm agrees on the
/// same referent data; an ambiguous or untyped arm leaves it unknown.
fn receiver_referee_name(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    receiver: ExpressionHandle,
) -> Option<String> {
    let (state, _, _) = caller_statement_owner(
        program,
        current_machine,
        CallerWriteSite::Expression(receiver),
    )?;
    // `self`-arm borrows may not carry a declared type of their own: peel to
    // the borrowed place, whose declared type is the referent itself.
    let referent_name = |mut expression: ExpressionHandle| {
        let type_reference = loop {
            match crate::value_custody::places::declared_place_type_raw(
                program,
                current_machine,
                Some(state),
                expression,
            ) {
                Some(type_reference) => break type_reference,
                None => match program.expression_table.expression(expression) {
                    ExpressionNode::Borrow(borrow) => expression = borrow.target,
                    _ => return None,
                },
            }
        };
        let mut type_reference = type_reference;
        loop {
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Named { name, .. } => break Some(name.as_str().to_owned()),
                TypeReferenceNode::Reference { .. } | TypeReferenceNode::Constrained { .. } => {
                    type_reference = exclusive_reference_referee(program, type_reference)?;
                }
                _ => break None,
            }
        }
    };
    match program.expression_table.expression(receiver) {
        ExpressionNode::Match(dispatch) => {
            let mut selected = None;
            for arm in program.expression_table.match_arms(dispatch.arms) {
                let name = referent_name(arm.value)?;
                match &selected {
                    None => selected = Some(name),
                    Some(selected) if *selected == name => {}
                    Some(_) => return None,
                }
            }
            selected
        }
        _ => referent_name(receiver),
    }
}

/// Reuse the checked body's result relation, validating its selected input for
/// caller storage. A helper cannot turn an untracked reference-bearing carrier
/// or a foreign binding identity into a proven caller storage origin.
pub(super) fn exclusive_reference_origin(
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    argument: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<FramePlaceOrigin>> {
    // Formation owns the borrow/re-view relation, including the outer borrow
    // around a postfix cast. Its target is the logical referee, not another
    // reference shell. Retain that interpretation beside original storage;
    // neither frame inference nor this address query admits access or casts.
    if let Some(formation) = crate::reference_formation_source(program, argument) {
        if !formation.access.is_exclusive()
            || (formation.view_type.is_valid()
                && super::expression_is_effectful_for_transparent_result(program, formation.source))
        {
            return None;
        }
        declared_origin_root(program, current_machine, formation.source)?;
        let mut origin = frame_place_path(program, formation.source)?;
        if formation.view_type.is_valid() {
            origin.source.recast(formation.view_type, argument);
        }
        return Some(vec![origin]);
    }
    let origins = match program.expression_table.expression(argument) {
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
            match carried_reference_origin(program, current_machine, argument) {
                Some(origin) => vec![origin],
                None => projected_carrier_reference_origins(
                    program,
                    current_machine,
                    argument,
                    symbols,
                    inference,
                )?,
            }
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
                    if result_place.source.root != parameter.symbol {
                        return None;
                    }
                    declared_origin_root(program, current_machine, actual)?;
                    return frame_place_path(program, actual).map(|origin| vec![origin]);
                };
                let owned = {
                    // A generic helper's carrier is judged on the storage the
                    // call instantiates, not the formal parameter name: bind
                    // the callee's `Type` parameters from this actual's
                    // declared type, then run the same owned-storage gate.
                    let type_parameters = program.machine_type_parameters(callee_machine);
                    let mut bindings = Vec::new();
                    if !type_parameters.is_empty() {
                        let (state, _, _) = caller_statement_owner(
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

                exclusive_reference_origins(program, current_machine, actual, symbols, inference)
            },
        )?,
        _ => return None,
    };
    (!origins.is_empty()).then_some(origins)
}

/// A member projection off a transparent helper's aggregate result moves the
/// selected reference leaf with its proven referents: `helper(..).slot` lends
/// exactly the leaf's resolved caller origins, never the carrier's own path.
/// The projection must select a plain declared field all the way down — a
/// case-payload field, an indexed element, or a result that is itself a
/// reference keeps the argument opaque because those leaf positions either
/// never reach the declared frontier or belong to the direct-result relation.
pub(super) fn projected_carrier_reference_origins(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    argument: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<FramePlaceOrigin>> {
    let mut member_names = Vec::new();
    let mut root = argument;
    let call = loop {
        match program.expression_table.expression(root) {
            ExpressionNode::Member(member) => {
                if member.case_variant.is_some() {
                    return None;
                }
                member_names.push(member.member.as_str());
                root = member.receiver;
            }
            ExpressionNode::Call(call) => break call,
            _ => return None,
        }
    };
    member_names.reverse();
    let (_, callee_state) =
        super::call_targets::machine_state_by_symbol(program, call.target_symbol)?;
    if super::type_reference_is_reference(program, callee_state.return_type) {
        return None;
    }
    let mut carrier_symbol =
        super::boundary_calls::receiver_type_symbol(program, callee_state.return_type);
    let mut segments = Vec::with_capacity(member_names.len());
    for member_name in member_names {
        let data =
            crate::declarations::data_queries::data_definition_by_symbol(program, carrier_symbol)?;
        let field = program
            .data_members(data)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field)
                    if field.name.as_str() == member_name =>
                {
                    Some(field)
                }
                _ => None,
            })?;
        segments.push(PlaceSegment::Field {
            symbol: field.symbol,
        });
        carrier_symbol = super::boundary_calls::receiver_type_symbol(program, field.type_reference);
    }
    let returned = super::result_origins::call_result_origins(
        program,
        current_machine,
        call,
        callee_state.return_type,
        symbols,
        inference,
        false,
        &|actual, _, _, inference| {
            exclusive_reference_origins(program, current_machine, actual, symbols, inference)
        },
        &|actual, reference, inference| {
            super::stored_origins::symbolic_reference_leaves(
                program,
                current_machine,
                actual,
                reference,
                inference,
            )
        },
    )?;
    let mut origins = Vec::new();
    for leaf in &returned.references {
        if leaf.local_segments == segments {
            push_unique_origin(&mut origins, leaf.origin.clone()?);
        }
    }
    (!origins.is_empty()).then_some(origins)
}

/// An owned carrier transports its declared reference leaf as a value. This
/// symbolic source is frozen by state transfer; it is not a loaded carrier
/// behind another reference, nor permission to replace the reference slot.
fn carried_reference_origin(
    program: &dyn crate::CheckingInput,
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
        caller_statement_owner(program, machine, CallerWriteSite::Expression(expression))?;
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
    program: &dyn crate::CheckingInput,
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
                        .is_some_and(|member| member.is_self_receiver())
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
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
    exclusive_reference_referee(program, reference)
        .is_some_and(|referee| referent_has_only_owned_storage(program, referee))
}

pub(super) fn exclusive_reference_referee(
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<FramePlaceOrigin> {
    let parent = match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow) if borrow.access.is_exclusive() => {
            // Explicitly borrowing an owned receiver preserves its storage
            // origin, just like the implicit borrow of an attached call.
            // Borrowing a reference binding adds a boundary instead: do not
            // unwrap that slot into the referent's origin.
            if !matches!(
                program.expression_table.expression(borrow.target),
                ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
            ) {
                return None;
            }
            let (state, _, _) = caller_statement_owner(
                program,
                current_machine,
                CallerWriteSite::Expression(expression),
            )?;
            let reference = crate::value_custody::places::declared_place_type_raw(
                program,
                current_machine,
                Some(state),
                borrow.target,
            )?;
            if !type_is_caller_isolated_local(program, reference) {
                return None;
            }
            return owned_receiver_origin(
                program,
                current_machine,
                borrow.target,
                symbols,
                inference,
            );
        }
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
    let (state, _, _) = caller_statement_owner(
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
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    argument: ExpressionHandle,
) -> Option<String> {
    let reference = caller_binding_type(program, current_machine, argument)?;
    exclusive_reference_referee(program, reference)
        .is_some()
        .then(|| frame_place_path(program, argument).map(|origin| origin.path))
        .flatten()
}

pub(super) fn referent_has_only_owned_storage(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
    referent_has_only_owned_storage_in(program, reference, &[])
}

/// Under an active substitution a bound parameter head resolves to its
/// caller actual before the owned-storage walk; an unbound parameter keeps
/// the named leaf and stays opaque.
pub(super) fn referent_has_only_owned_storage_in(
    program: &dyn crate::CheckingInput,
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
