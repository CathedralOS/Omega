//! Parameter-relative origins of places and call results.

use crate::declarations::symbols::{MachineSymbols, TopLevelSymbols};
use crate::machine_calls::calls::write_frames::call_targets::machine_state_by_symbol;
use crate::machine_calls::calls::write_frames::call_trees::parameter_relative_expression_preserves_transparent_result;
use crate::machine_calls::calls::write_frames::inference::FrameInference;
use crate::machine_calls::calls::write_frames::parameter_aliases::{
    ParameterRelativeFrameOrigin, expression_reborrows_transparent_alias_binding,
};
use crate::machine_calls::calls::write_frames::place_paths::{
    FramePathPrecision, FramePlaceOrigin, append_place_suffix, frame_place_path, same_place_origin,
    split_place_root,
};
use crate::machine_calls::calls::write_frames::reference_origins;
use crate::machine_calls::calls::write_frames::transparent_effects::{
    call_is_transparent_mutable_slice_view, expression_is_effectful_for_transparent_result,
    frame_place_root_symbol,
};
use crate::machine_calls::calls::write_frames::transparent_results::transparent_callee_result_origin;
use crate::machine_calls::calls::write_frames::type_capabilities::type_reference_is_reference;
use crate::machine_calls::calls::write_frames::value_expressions::ValuePosition;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;

pub(crate) fn parameter_relative_place_origin(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<ParameterRelativeFrameOrigin> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => parameter_relative_place_origin(
            program,
            current_machine,
            inner.target,
            parameters,
            aliases,
            symbols,
            inference,
        ),
        ExpressionNode::Indexed(indexed) => {
            if expression_is_effectful_for_transparent_result(program, indexed.index) {
                let mut diagnostics = Vec::new();
                let machine_symbols =
                    MachineSymbols::build(program, current_machine, &mut diagnostics);
                if !diagnostics.is_empty()
                    || !parameter_relative_expression_preserves_transparent_result(
                        program,
                        current_machine,
                        indexed.index,
                        ValuePosition::IndexOperand,
                        &machine_symbols,
                        symbols,
                        inference,
                        parameters,
                        aliases,
                    )
                {
                    return None;
                }
            }
            let mut origin = parameter_relative_place_origin(
                program,
                current_machine,
                indexed.collection,
                parameters,
                aliases,
                symbols,
                inference,
            )?;
            origin.place.source =
                origin
                    .place
                    .source
                    .projected(program, expression, indexed.collection);
            origin.place.precision = FramePathPrecision::CollectionCoarse;
            Some(origin)
        }
        ExpressionNode::Member(member) => {
            let mut origin = parameter_relative_place_origin(
                program,
                current_machine,
                member.receiver,
                parameters,
                aliases,
                symbols,
                inference,
            )?;
            if origin.place.precision == FramePathPrecision::Exact {
                origin.place.path = format!("{}.{}", origin.place.path, member.member.as_str());
            }
            origin.place.source =
                origin
                    .place
                    .source
                    .projected(program, expression, member.receiver);
            Some(origin)
        }
        ExpressionNode::Name(_) => {
            reference_origins::declared_origin_root(program, current_machine, expression)?;
            let place = frame_place_path(program, expression)?;
            let root_symbol = frame_place_root_symbol(program, expression);
            let (root, suffix) = split_place_root(&place.path);
            if let Some(parameter) = parameters.iter().find(|parameter| {
                (root_symbol == Some(parameter.symbol) || (parameter.is_self && root == "self"))
                    && type_reference_is_reference(program, parameter.type_reference)
            }) {
                return Some(ParameterRelativeFrameOrigin {
                    place,
                    parameter_symbol: parameter.symbol,
                });
            }
            let parent = aliases.iter().find_map(|(_, symbol, origin)| {
                let exact_symbol = root_symbol
                    .is_some_and(|root| root.is_valid() && symbol.is_valid() && root == *symbol);
                exact_symbol.then_some(origin)
            })?;
            let source = parent.place.source.append_segments(&place.source.segments);
            Some(ParameterRelativeFrameOrigin {
                place: match parent.place.precision {
                    FramePathPrecision::Exact => FramePlaceOrigin {
                        path: append_place_suffix(&parent.place.path, suffix),
                        precision: place.precision,
                        source,
                    },
                    FramePathPrecision::CollectionCoarse => FramePlaceOrigin {
                        source,
                        ..parent.place.clone()
                    },
                },
                parameter_symbol: parent.parameter_symbol,
            })
        }
        ExpressionNode::Call(call) => {
            if call_is_transparent_mutable_slice_view(program, call) {
                let mut origin = parameter_relative_place_origin(
                    program,
                    current_machine,
                    call.receiver,
                    parameters,
                    aliases,
                    symbols,
                    inference,
                )?;
                // An unresolved view builtin supplies a conservative collection
                // footprint, not a nominal helper-result identity. A same-name
                // unresolved user method cannot establish an exact subject.
                origin.place.precision = FramePathPrecision::CollectionCoarse;
                return Some(origin);
            }
            parameter_relative_call_result_origin(
                program,
                current_machine,
                call,
                parameters,
                aliases,
                symbols,
                inference,
            )
        }
        ExpressionNode::Match(dispatch) => {
            // A conditional result contributes one origin only when every
            // producing arm resolves to that same place; divergent routes
            // stay opaque rather than selecting one arm of the case
            // analysis. Arm pattern and subject effects are not result
            // routes and remain with the body walk.
            let mut selected = None;
            for arm in program.expression_table.match_arms(dispatch.arms) {
                let origin = parameter_relative_place_origin(
                    program,
                    current_machine,
                    arm.value,
                    parameters,
                    aliases,
                    symbols,
                    inference,
                )?;
                match &selected {
                    None => selected = Some(origin),
                    Some(existing)
                        if existing.parameter_symbol == origin.parameter_symbol
                            && same_place_origin(&existing.place, &origin.place) => {}
                    Some(_) => return None,
                }
            }
            selected
        }
        ExpressionNode::Cast(cast)
            if cast.form.is_recast()
                && !expression_is_effectful_for_transparent_result(program, cast.value) =>
        {
            parameter_relative_place_origin(
                program,
                current_machine,
                cast.value,
                parameters,
                aliases,
                symbols,
                inference,
            )
        }
        _ => None,
    }
}

fn parameter_relative_call_result_origin(
    program: &TypedTrees,
    current_machine: &Machine,
    call: &TableCallExpression,
    caller_parameters: &[StateParameter],
    caller_aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<ParameterRelativeFrameOrigin> {
    if std::iter::once(call.receiver)
        .chain(
            program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .copied(),
        )
        .any(|expression| {
            expression_reborrows_transparent_alias_binding(
                program,
                expression,
                caller_parameters,
                caller_aliases,
            )
        })
    {
        return None;
    }
    let (callee_machine, callee_state) = machine_state_by_symbol(program, call.target_symbol)?;
    if call.receiver.is_valid() != callee_machine.attached_data.is_some() {
        return None;
    }
    let callee_origin = transparent_callee_result_origin(
        program,
        callee_machine,
        callee_state,
        symbols,
        inference,
    )?;
    let callee_parameters = program.state_parameters(callee_state);
    let callee_parameter = callee_parameters
        .iter()
        .find(|parameter| parameter.symbol == callee_origin.parameter_symbol)?;
    let actual = if callee_parameter.is_self {
        if callee_machine.attached_data.is_none() || !call.receiver.is_valid() {
            return None;
        }
        call.receiver
    } else {
        let (argument_index, _) = callee_parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == callee_parameter.symbol)?;
        *program
            .expression_table
            .expression_handles(call.arguments)
            .get(argument_index)?
    };
    let actual_origin = parameter_relative_place_origin(
        program,
        current_machine,
        actual,
        caller_parameters,
        caller_aliases,
        symbols,
        inference,
    )?;
    let (_, suffix) = split_place_root(&callee_origin.place.path);
    let source = actual_origin
        .place
        .source
        .append_relative(&callee_origin.place.source);
    Some(match actual_origin.place.precision {
        FramePathPrecision::Exact => ParameterRelativeFrameOrigin {
            place: FramePlaceOrigin {
                path: append_place_suffix(&actual_origin.place.path, suffix),
                precision: callee_origin.place.precision,
                source,
            },
            parameter_symbol: actual_origin.parameter_symbol,
        },
        FramePathPrecision::CollectionCoarse => ParameterRelativeFrameOrigin {
            place: FramePlaceOrigin {
                source,
                ..actual_origin.place
            },
            parameter_symbol: actual_origin.parameter_symbol,
        },
    })
}
