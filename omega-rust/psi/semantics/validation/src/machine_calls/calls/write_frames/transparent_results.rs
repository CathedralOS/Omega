//! Origins of transparent call results and the calls that preserve them.

use crate::declarations::symbols::{MachineSymbols, TopLevelSymbols};
use crate::machine_calls::calls::write_frames::alias_origins::stable_alias_initializer_origins;
use crate::machine_calls::calls::write_frames::assignment_targets::{
    assignment_target_type, expression_is_effectful_indexed_place,
    transparent_assignment_target_effect_is_structural,
};
use crate::machine_calls::calls::write_frames::boundary_calls::{
    known_boundary_call_written_paths_for_parts, known_requirement_call_written_paths_for_parts,
};
use crate::machine_calls::calls::write_frames::call_targets;
use crate::machine_calls::calls::write_frames::call_targets::{
    discarded_primitive_internal_call_is_relationally_neutral, machine_state_by_symbol,
};
use crate::machine_calls::calls::write_frames::call_trees::parameter_relative_expression_preserves_transparent_result;
use crate::machine_calls::calls::write_frames::caller_aliases::CallerWriteSite;
use crate::machine_calls::calls::write_frames::demand::{
    collect_expression_call_written_paths, statement_value_expression_roots,
};
use crate::machine_calls::calls::write_frames::inference::FrameInference;
use crate::machine_calls::calls::write_frames::isolated_initializers::isolated_local_initializer_preserves_transparent_result;
use crate::machine_calls::calls::write_frames::isolation::type_is_caller_isolated_local;
use crate::machine_calls::calls::write_frames::known_call_written_paths_for_parts_with_origins;
use crate::machine_calls::calls::write_frames::local_aliases::{
    expression_has_exclusive_borrow, expression_may_rebind_mutable_alias,
};
use crate::machine_calls::calls::write_frames::parameter_aliases::{
    ParameterRelativeFrameOrigin, parameter_relative_alias_position,
};
use crate::machine_calls::calls::write_frames::parameter_relative_origins::parameter_relative_place_origin;
use crate::machine_calls::calls::write_frames::parameter_relative_origins::parameter_relative_place_origins;
use crate::machine_calls::calls::write_frames::parameter_relative_origins::single_parameter_relative_origin;
use crate::machine_calls::calls::write_frames::place_paths::{
    FramePathPrecision, FramePlaceOrigin, append_place_suffix, frame_place_path,
    push_unique_origin, same_place_origin, single_place_origin, split_place_root,
};
use crate::machine_calls::calls::write_frames::transparent_effects::{
    call_is_transparent_mutable_slice_view, expression_is_effectful_for_transparent_result,
    frame_place_root_symbol,
};
use crate::machine_calls::calls::write_frames::type_capabilities::type_reference_is_reference;
use crate::machine_calls::calls::write_frames::value_expressions::{
    ValuePosition, value_expression_preserves_transparent_result,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableCall};

/// Every proven caller route one deliberately structural value-call result
/// may alias. The helper may be
/// free or attached, but must be acyclic at the result surface, return a reference,
/// and have terminal result expressions rooted in one reference
/// parameter -- either a trailing expression or the lone ordinary `Always`
/// value transition an authored single-arm return desugars to -- or rooted in
/// a declared exclusive reference leaf of one by-value carrier parameter whose
/// binding stayed frozen across the prefix. A prefix may
/// contain caller-isolated scratch locals and local
/// reference bindings that forward direct places from that parameter, an
/// earlier such local, or another structurally transparent helper. Value-shaped
/// assignments with effect-free right-hand sides may write through those
/// places, scratch locals, validated mutable recast aliases with effect-free
/// sources, or exact transparent call-produced targets without changing their
/// origins; the ordinary frame summary still publishes caller-visible writes.
/// Effect-free discarded expressions are also neutral.
/// A direct stable alias rebind updates only that local; prior reborrows retain
/// their established origins. Explicit arguments and an attached helper's
/// actual receiver both supply exact caller origins. This is body evidence, not
/// lifetime elision: a reference-bearing scratch local, opaque computed rebind,
/// unsupported discarded/statement call, recursive helper relation, named-state route, or
/// alternate result fails closed.
///
/// Conditional result arms each contribute the candidate origins of their
/// selected actual, so a divergent helper result keeps the exact finite union
/// rather than selecting one arm. A candidate whose own actual cannot be
/// resolved leaves the whole result opaque: the union must cover every route
/// the callee admits. Consumers that need one binding collapse the set
/// through `single_place_origin`.
pub(crate) fn transparent_call_result_origins(
    program: &TypedTrees,
    call: &TableCallExpression,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    resolve_actual_origins: impl Fn(
        &Machine,
        &StateParameter,
        &FramePlaceOrigin,
        ExpressionHandle,
        &mut FrameInference,
    ) -> Option<Vec<FramePlaceOrigin>>,
) -> Option<Vec<FramePlaceOrigin>> {
    let (callee_machine, callee_state) = machine_state_by_symbol(program, call.target_symbol)?;
    if call.receiver.is_valid() != callee_machine.attached_data.is_some() {
        return None;
    }

    let result_origins = transparent_callee_result_origins(
        program,
        callee_machine,
        callee_state,
        symbols,
        inference,
    )?;
    let parameters = program.state_parameters(callee_state);
    let mut origins = Vec::new();
    for result_origin in result_origins {
        let result_parameter = parameters
            .iter()
            .find(|parameter| parameter.symbol == result_origin.parameter_symbol)?;
        let (_, result_suffix) = split_place_root(&result_origin.place.path);
        let actual = if result_parameter.is_self {
            if callee_machine.attached_data.is_none() || !call.receiver.is_valid() {
                return None;
            }
            call.receiver
        } else {
            let (argument_index, _) = parameters
                .iter()
                .filter(|parameter| !parameter.is_self)
                .enumerate()
                .find(|(_, parameter)| parameter.symbol == result_parameter.symbol)?;
            *program
                .expression_table
                .expression_handles(call.arguments)
                .get(argument_index)?
        };
        for argument_origin in resolve_actual_origins(
            callee_machine,
            result_parameter,
            &result_origin.place,
            actual,
            inference,
        )? {
            let source = argument_origin
                .source
                .append_relative(&result_origin.place.source);
            push_unique_origin(
                &mut origins,
                match argument_origin.precision {
                    FramePathPrecision::Exact => FramePlaceOrigin {
                        path: append_place_suffix(&argument_origin.path, result_suffix),
                        precision: result_origin.place.precision,
                        source,
                    },
                    FramePathPrecision::CollectionCoarse => FramePlaceOrigin {
                        source,
                        ..argument_origin
                    },
                },
            );
        }
    }
    (!origins.is_empty()).then_some(origins)
}

/// Recover a transparent returned-place origin without imposing a caller
/// namespace. This is used while instantiating a callee frame through a nested
/// statement-call argument: direct places keep their existing spelling, while
/// the compiler-owned `as_mut_slice` view preserves its receiver and a
/// structurally transparent helper selects and composes one of its actual
/// mutable-reference origins. Opaque or recursive helpers fail closed.
pub(crate) fn transparent_place_expression_origin(
    program: &TypedTrees,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<FramePlaceOrigin> {
    single_place_origin(transparent_place_expression_origins(
        program, expression, symbols, inference,
    )?)
}

/// Every proven place the expression may name, in caller spelling. Direct
/// places keep their single origin; transparent helper results and divergent
/// match arms contribute their candidate union. Any unresolvable route fails
/// the expression rather than dropping a possible referent.
pub(crate) fn transparent_place_expression_origins(
    program: &TypedTrees,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<FramePlaceOrigin>> {
    let origins = match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            transparent_place_expression_origins(program, inner.target, symbols, inference)?
        }
        ExpressionNode::Indexed(indexed) => {
            if expression_is_effectful_for_transparent_result(program, indexed.index) {
                return None;
            }
            transparent_place_expression_origins(program, indexed.collection, symbols, inference)?
                .into_iter()
                .map(|mut origin| {
                    origin.source =
                        origin
                            .source
                            .projected(program, expression, indexed.collection);
                    origin.precision = FramePathPrecision::CollectionCoarse;
                    origin
                })
                .collect()
        }
        ExpressionNode::Member(member) => {
            transparent_place_expression_origins(program, member.receiver, symbols, inference)?
                .into_iter()
                .map(|origin| {
                    let source = origin
                        .source
                        .projected(program, expression, member.receiver);
                    match origin.precision {
                        FramePathPrecision::Exact => FramePlaceOrigin {
                            path: format!("{}.{}", origin.path, member.member.as_str()),
                            precision: FramePathPrecision::Exact,
                            source,
                        },
                        FramePathPrecision::CollectionCoarse => {
                            FramePlaceOrigin { source, ..origin }
                        }
                    }
                })
                .collect()
        }
        ExpressionNode::Call(call) => {
            if call_is_transparent_mutable_slice_view(program, call) {
                transparent_place_expression_origins(program, call.receiver, symbols, inference)?
            } else {
                transparent_call_result_origins(
                    program,
                    call,
                    symbols,
                    inference,
                    |_, _, _, actual, inference| {
                        transparent_place_expression_origins(program, actual, symbols, inference)
                    },
                )?
            }
        }
        ExpressionNode::Match(dispatch) => {
            let mut selected = Vec::new();
            for arm in program.expression_table.match_arms(dispatch.arms) {
                for origin in
                    transparent_place_expression_origins(program, arm.value, symbols, inference)?
                {
                    push_unique_origin(&mut selected, origin);
                }
            }
            selected
        }
        _ => vec![frame_place_path(program, expression)?],
    };
    (!origins.is_empty()).then_some(origins)
}

/// Every proven parameter-relative route the callee's result may alias.
/// Conditional arms keep their exact finite union; an arm that cannot name a
/// caller-reachable parameter root fails the whole relation closed, and a
/// re-entered state keeps recursive helper results explicitly opaque.
pub(crate) fn transparent_callee_result_origins(
    program: &TypedTrees,
    callee_machine: &Machine,
    callee_state: &State,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<ParameterRelativeFrameOrigin>> {
    if inference.active_states.contains(&callee_state.symbol)
        || !type_reference_is_reference(program, callee_state.return_type)
    {
        return None;
    }
    inference.active_states.push(callee_state.symbol);
    let result = (|| {
        let statements = program
            .statement_table
            .statements(callee_state.statement_nodes);
        let (tail, prefix) = statements.split_last()?;
        let result = match tail {
            StatementNode::Expression(result) => *result,
            // A lone ordinary value transition is the desugared authored
            // return tail. A guarded arm chain keeps its earlier sibling
            // transitions in the prefix, which the prefix walk still
            // rejects, so admitting this tail cannot select one arm of a
            // multi-result body.
            StatementNode::Transition(transition)
                if transition.exit == typed_trees::statement::TransitionExit::Ordinary
                    && !transition.continuation.is_valid()
                    && transition.target.is_valid()
                    && matches!(
                        transition.guard,
                        typed_trees::statement::TransitionGuardNode::Always
                    ) =>
            {
                let typed_trees::statement::TransitionTargetNode::Value(result) =
                    program.statement_table.transition_target(transition.target)
                else {
                    return None;
                };
                *result
            }
            _ => return None,
        };

        let parameters = program.state_parameters(callee_state);
        let mut local_aliases = Vec::new();
        let mut isolated_local_roots = Vec::new();
        for statement in prefix {
            match statement {
                StatementNode::LocalData(local) => {
                    // The isolated-initializer write fence rebases a written
                    // path through a binding's single proven place. A
                    // divergent binding has none, so its name stays opaque:
                    // any write through it could land on a non-isolated
                    // candidate and fails closed rather than picking a route.
                    let stable_aliases = local_aliases
                        .iter()
                        .filter_map(
                            |(name, _, origins): &(
                                String,
                                SymbolHandle,
                                Vec<ParameterRelativeFrameOrigin>,
                            )| {
                                single_parameter_relative_origin(origins.clone())
                                    .map(|origin| (name.clone(), origin.place))
                            },
                        )
                        .collect::<Vec<_>>();
                    if type_is_caller_isolated_local(program, local.type_reference)
                        && !type_reference_is_reference(program, local.type_reference)
                    {
                        if expression_is_effectful_for_transparent_result(
                            program,
                            local.initial_value,
                        ) && !value_expression_preserves_transparent_result(
                            program,
                            callee_machine,
                            local.initial_value,
                            Some(local.type_reference),
                            symbols,
                            inference,
                            parameters,
                            &local_aliases,
                        ) {
                            return None;
                        }
                        if !isolated_local_initializer_preserves_transparent_result(
                            program,
                            callee_machine,
                            local.initial_value,
                            &isolated_local_roots,
                            &stable_aliases,
                            |machine_symbols, written| {
                                collect_expression_call_written_paths(
                                    program,
                                    local.initial_value,
                                    callee_machine,
                                    machine_symbols,
                                    symbols,
                                    inference,
                                    written,
                                    &mut Vec::new(),
                                )
                            },
                        ) {
                            return None;
                        }
                        isolated_local_roots.push(local.name.as_str().to_owned());
                        continue;
                    }
                    if !type_reference_is_reference(program, local.type_reference) {
                        return None;
                    }
                    // A reference binding keeps every proven route its
                    // initializer admits: a transparent helper result or
                    // divergent match arms bind the exact finite union so a
                    // returned alias re-exports all of them.
                    let origins = parameter_relative_place_origins(
                        program,
                        callee_machine,
                        local.initial_value,
                        parameters,
                        &local_aliases,
                        symbols,
                        inference,
                    )
                    .or_else(|| {
                        let mut diagnostics = Vec::new();
                        let machine_symbols =
                            MachineSymbols::build(program, callee_machine, &mut diagnostics);
                        if !diagnostics.is_empty() {
                            return None;
                        }
                        let places = stable_alias_initializer_origins(
                            program,
                            callee_machine,
                            &machine_symbols,
                            inference,
                            local.initial_value,
                            parameters,
                            &isolated_local_roots,
                            &stable_aliases,
                            &[],
                            symbols,
                            true,
                            &[],
                        )?;
                        // A caller-isolated route composes inside the helper
                        // but cannot be exported; every admitted candidate
                        // must root in one, or the binding mixes private and
                        // caller-visible storage this relation cannot name.
                        places
                            .iter()
                            .all(|place| {
                                let (root, _) = split_place_root(&place.path);
                                isolated_local_roots.iter().any(|local| local == root)
                            })
                            .then(|| {
                                places
                                    .into_iter()
                                    .map(|place| ParameterRelativeFrameOrigin {
                                        place,
                                        parameter_symbol: SymbolHandle::invalid(),
                                    })
                                    .collect::<Vec<_>>()
                            })
                    })?;
                    local_aliases.push((local.name.as_str().to_owned(), local.symbol, origins));
                }
                StatementNode::Assignment(assignment) => {
                    if expression_is_effectful_for_transparent_result(program, assignment.target)
                        && (!transparent_assignment_target_effect_is_structural(
                            program,
                            assignment.target,
                        ) || parameter_relative_place_origin(
                            program,
                            callee_machine,
                            assignment.target,
                            parameters,
                            &local_aliases,
                            symbols,
                            inference,
                        )
                        .is_none())
                    {
                        return None;
                    }
                    if value_expression_preserves_transparent_result(
                        program,
                        callee_machine,
                        assignment.value,
                        assignment_target_type(
                            program,
                            callee_machine,
                            callee_state,
                            assignment.target,
                        ),
                        symbols,
                        inference,
                        parameters,
                        &local_aliases,
                    ) {
                        // A non-reference call result can change only the
                        // target's value, not an established alias origin.
                    } else if expression_may_rebind_mutable_alias(
                        program,
                        callee_machine,
                        callee_state,
                        assignment.value,
                    ) {
                        let position = parameter_relative_alias_position(
                            program,
                            assignment.target,
                            &local_aliases,
                        )?;
                        let replacement = parameter_relative_place_origins(
                            program,
                            callee_machine,
                            assignment.value,
                            parameters,
                            &local_aliases,
                            symbols,
                            inference,
                        )?;
                        local_aliases[position].2 = replacement;
                    } else if expression_is_effectful_for_transparent_result(
                        program,
                        assignment.value,
                    ) {
                        return None;
                    }
                }
                StatementNode::Expression(expression)
                    if !expression_is_effectful_for_transparent_result(program, *expression) => {}
                StatementNode::Call(call)
                    if statement_call_preserves_transparent_result(
                        program,
                        callee_machine,
                        call,
                        symbols,
                        inference,
                        parameters,
                        &local_aliases,
                    ) => {}
                _ => return None,
            }
        }
        parameter_relative_place_origins(
            program,
            callee_machine,
            result,
            parameters,
            &local_aliases,
            symbols,
            inference,
        )
        // A returned place may also be rooted in a by-value carrier
        // parameter's declared exclusive reference leaf.
        .or_else(|| carrier_leaf_result_origins(program, result, parameters, prefix))
        // Every admitted route must export an actual parameter root; a
        // candidate that reaches only helper-private storage means the result
        // may escape to a referent this relation cannot name.
        .filter(|origins| {
            origins.iter().all(|origin| {
                origin.parameter_symbol.is_valid()
                    && parameters.iter().any(|parameter| {
                        parameter.symbol == origin.parameter_symbol
                            && (origin.place.source.root == parameter.symbol
                                || (parameter.is_self
                                    && origin.place.source.root == callee_machine.symbol))
                    })
            })
        })
    })();
    inference.active_states.pop();
    result
}

/// One direct Unit statement call is neutral to a returned-place relation when
/// its inferred frame is complete and no argument exposes a mutable-reference
/// binding for rebinding. The same applies to an explicitly discarded concrete
/// primitive result from a nongeneric checked-body call: the value cannot carry
/// an alias, while the call's complete frame still publishes its writes. Writes
/// through references passed by value may change their contents, but cannot
/// redirect the established origin. One direct value-call argument is admitted
/// under the same complete-frame rule. Other discarded results, deeper computed
/// arguments, binding reborrows, and opaque frames remain fences.
fn statement_call_preserves_transparent_result(
    program: &TypedTrees,
    current_machine: &Machine,
    call: &TableCall,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, Vec<ParameterRelativeFrameOrigin>)],
) -> bool {
    if call.discards_result
        && !discarded_primitive_internal_call_is_relationally_neutral(program, call, symbols)
    {
        return false;
    }
    let mut diagnostics = Vec::new();
    let machine_symbols = MachineSymbols::build(program, current_machine, &mut diagnostics);
    if !diagnostics.is_empty() {
        return false;
    }
    let arguments = program.statement_table.expression_handles(call.arguments);
    let receiver_members = program
        .statement_table
        .name_path_members(call.receiver)
        .iter()
        .map(|member| member.as_str().to_owned())
        .collect::<Vec<_>>();
    let argument_types = call_targets::call_argument_types(
        program,
        current_machine,
        call.target_symbol,
        call.target.as_str(),
        &receiver_members,
        CallerWriteSite::Call(call),
        &machine_symbols,
        symbols,
    );
    // Every sibling must independently preserve the returned-place origin.
    if arguments.iter().enumerate().any(|(index, argument)| {
        !parameter_relative_expression_preserves_transparent_result(
            program,
            current_machine,
            *argument,
            ValuePosition::CallArgument(argument_types.get(index).copied().unwrap_or_default()),
            &machine_symbols,
            symbols,
            inference,
            parameters,
            aliases,
        )
    }) {
        return false;
    }

    let argument_origins = arguments
        .iter()
        .map(|argument| {
            parameter_relative_place_origins(
                program,
                current_machine,
                *argument,
                parameters,
                aliases,
                symbols,
                inference,
            )
            .map(|origins| {
                origins
                    .into_iter()
                    .map(|origin| origin.place)
                    .collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();
    known_call_written_paths_for_parts_with_origins(
        program,
        call.target_symbol,
        call.target.as_str(),
        &receiver_members,
        None,
        arguments,
        current_machine,
        &machine_symbols,
        symbols,
        inference,
        Some(&argument_origins),
        &mut Vec::new(),
    )
    .or_else(|| {
        (!arguments
            .iter()
            .any(|argument| expression_is_effectful_indexed_place(program, *argument)))
        .then(|| {
            known_boundary_call_written_paths_for_parts(
                program,
                current_machine,
                &machine_symbols,
                symbols,
                &receiver_members,
                call.target.as_str(),
                CallerWriteSite::Call(call),
                arguments,
                inference,
            )
        })
        .flatten()
    })
    .or_else(|| {
        (!arguments
            .iter()
            .any(|argument| expression_is_effectful_indexed_place(program, *argument)))
        .then(|| {
            known_requirement_call_written_paths_for_parts(
                program,
                current_machine,
                &machine_symbols,
                symbols,
                &receiver_members,
                call.target.as_str(),
                None,
                CallerWriteSite::Call(call),
                arguments,
                inference,
            )
        })
        .flatten()
    })
    .is_some()
}

/// A returned place can be rooted in a by-value carrier parameter's declared
/// exclusive reference leaf: `machine pf(a: View) -> &mut u64 { a.body }`
/// returns the reference stored in `a.body`, which instantiates through the
/// carrier actual exactly like a parameter-rooted place. The projected path
/// must land on a declared exclusive leaf of the parameter's own type; the
/// declaration walk has already enforced owned referent storage, so a private
/// or doubly-loaded slot cannot be named here. Conditional result arms each
/// contribute their own carrier leaf, keeping the exact finite union; an arm
/// that names no declared leaf fails the whole route closed.
///
/// The carrier binding must also stay frozen across the prefix: reassigning
/// or rebinding the parameter, or lending any place rooted in it through an
/// exclusive borrow, would leave the exported leaf describing a binding the
/// helper no longer holds.
fn carrier_leaf_result_origins(
    program: &TypedTrees,
    result: ExpressionHandle,
    parameters: &[StateParameter],
    prefix: &[StatementNode],
) -> Option<Vec<ParameterRelativeFrameOrigin>> {
    let leaves = carrier_leaf_places(program, result, parameters)?;
    // Every selected carrier's binding must stay frozen across the prefix:
    // rebasing any one of them leaves that route's exported leaf describing
    // storage the helper no longer holds.
    if leaves.iter().any(|(parameter_symbol, _)| {
        prefix
            .iter()
            .any(|statement| statement_rebases_carrier_root(program, statement, *parameter_symbol))
    }) {
        return None;
    }
    Some(
        leaves
            .into_iter()
            .map(|(parameter_symbol, place)| ParameterRelativeFrameOrigin {
                place,
                parameter_symbol,
            })
            .collect(),
    )
}

/// Resolve a result expression to its `(carrier parameter, place)` routes
/// when every producing arm spells a declared exclusive reference leaf
/// beneath a by-value parameter. Divergent arms keep the exact finite union;
/// an arm that names no declared leaf fails the whole route closed.
fn carrier_leaf_places(
    program: &TypedTrees,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
) -> Option<Vec<(SymbolHandle, FramePlaceOrigin)>> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => carrier_leaf_places(program, inner.target, parameters),
        ExpressionNode::Match(dispatch) => {
            let mut selected = Vec::new();
            for arm in program.expression_table.match_arms(dispatch.arms) {
                for leaf in carrier_leaf_places(program, arm.value, parameters)? {
                    if !selected.iter().any(|(parameter_symbol, place)| {
                        *parameter_symbol == leaf.0 && same_place_origin(place, &leaf.1)
                    }) {
                        selected.push(leaf);
                    }
                }
            }
            Some(selected)
        }
        ExpressionNode::Cast(cast)
            if cast.form.is_recast()
                && !expression_is_effectful_for_transparent_result(program, cast.value) =>
        {
            carrier_leaf_places(program, cast.value, parameters)
        }
        _ => {
            let place = frame_place_path(program, expression)?;
            let parameter = parameters.iter().find(|parameter| {
                !parameter.is_self
                    && parameter.symbol == place.source.root
                    && !type_reference_is_reference(program, parameter.type_reference)
            })?;
            let declared = super::stored_origins::declared_origins_for_query(
                program,
                parameter.symbol,
                parameter.name.as_str(),
                parameter.type_reference,
                false,
            )?;
            declared
                .references
                .iter()
                .any(|leaf| {
                    super::stored_origins::source_reaches_leaf(
                        &place.source.segments,
                        &leaf.local_segments,
                    )
                })
                .then_some(vec![(parameter.symbol, place)])
        }
    }
}

/// Whether one prefix statement can redirect the exported carrier leaf: a
/// direct assignment target rooted in the parameter replaces or reshapes the
/// binding, and an exclusive borrow of a rooted place hands the slot to a
/// callee that may rebind it. Either leaves the parameter-relative result
/// describing storage the helper no longer holds.
fn statement_rebases_carrier_root(
    program: &TypedTrees,
    statement: &StatementNode,
    parameter_symbol: SymbolHandle,
) -> bool {
    let retargeted = matches!(statement, StatementNode::Assignment(assignment)
        if frame_place_root_symbol(program, assignment.target) == Some(parameter_symbol));
    retargeted
        || statement_value_expression_roots(program, statement)
            .into_iter()
            .any(|expression| {
                expression_has_exclusive_borrow(program, expression, &|target| {
                    frame_place_root_symbol(program, target) == Some(parameter_symbol)
                })
            })
}
