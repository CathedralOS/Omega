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
use crate::machine_calls::calls::write_frames::transparent_results::transparent_callee_result_origins;
use crate::machine_calls::calls::write_frames::type_capabilities::type_reference_is_reference;
use crate::machine_calls::calls::write_frames::value_expressions::ValuePosition;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;

/// The single proven origin of a place expression. Conditional result routes
/// keep a finite candidate set internally; consumers holding a single
/// parameter-relative binding still require every route to name the same
/// place.
pub(crate) fn parameter_relative_place_origin(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, Vec<ParameterRelativeFrameOrigin>)],
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<ParameterRelativeFrameOrigin> {
    single_parameter_relative_origin(parameter_relative_place_origins(
        program,
        current_machine,
        expression,
        parameters,
        aliases,
        symbols,
        inference,
    )?)
}

/// Every proven route a place expression may name, in the callee's
/// parameter-relative namespace. Each producing match arm contributes its own
/// candidate set, and a transparent helper result routes each arm through its
/// selected actual; divergent arms keep the exact finite union rather than
/// selecting one side. A route that cannot be named fails the whole
/// expression closed.
pub(crate) fn parameter_relative_place_origins(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, Vec<ParameterRelativeFrameOrigin>)],
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<ParameterRelativeFrameOrigin>> {
    let origins = match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => parameter_relative_place_origins(
            program,
            current_machine,
            inner.target,
            parameters,
            aliases,
            symbols,
            inference,
        )?,
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
            parameter_relative_place_origins(
                program,
                current_machine,
                indexed.collection,
                parameters,
                aliases,
                symbols,
                inference,
            )?
            .into_iter()
            .map(|mut origin| {
                origin.place.source =
                    origin
                        .place
                        .source
                        .projected(program, expression, indexed.collection);
                origin.place.precision = FramePathPrecision::CollectionCoarse;
                origin
            })
            .collect()
        }
        ExpressionNode::Member(member) => parameter_relative_place_origins(
            program,
            current_machine,
            member.receiver,
            parameters,
            aliases,
            symbols,
            inference,
        )?
        .into_iter()
        .map(|mut origin| {
            if origin.place.precision == FramePathPrecision::Exact {
                origin.place.path = format!("{}.{}", origin.place.path, member.member.as_str());
            }
            origin.place.source =
                origin
                    .place
                    .source
                    .projected(program, expression, member.receiver);
            origin
        })
        .collect(),
        ExpressionNode::Name(_) => parameter_relative_name_origins(
            program,
            current_machine,
            expression,
            parameters,
            aliases,
        )?,
        ExpressionNode::Call(call) => {
            if call_is_transparent_mutable_slice_view(program, call) {
                // An unresolved view builtin supplies a conservative
                // collection footprint, not a nominal helper-result identity.
                // A same-name unresolved user method cannot establish an
                // exact subject.
                parameter_relative_place_origins(
                    program,
                    current_machine,
                    call.receiver,
                    parameters,
                    aliases,
                    symbols,
                    inference,
                )?
                .into_iter()
                .map(|mut origin| {
                    origin.place.precision = FramePathPrecision::CollectionCoarse;
                    origin
                })
                .collect()
            } else {
                parameter_relative_call_result_origins(
                    program,
                    current_machine,
                    call,
                    parameters,
                    aliases,
                    symbols,
                    inference,
                )?
            }
        }
        ExpressionNode::Match(dispatch) => {
            // Every producing arm contributes its own candidates. Divergent
            // routes keep the exact finite union: the result may reach any
            // proven arm, and an arm that cannot resolve leaves the whole
            // expression opaque rather than selecting the provable side.
            let mut selected = Vec::new();
            for arm in program.expression_table.match_arms(dispatch.arms) {
                for origin in parameter_relative_place_origins(
                    program,
                    current_machine,
                    arm.value,
                    parameters,
                    aliases,
                    symbols,
                    inference,
                )? {
                    push_unique_parameter_relative(&mut selected, origin);
                }
            }
            selected
        }
        ExpressionNode::Cast(cast)
            if cast.form.is_recast()
                && !expression_is_effectful_for_transparent_result(program, cast.value) =>
        {
            parameter_relative_place_origins(
                program,
                current_machine,
                cast.value,
                parameters,
                aliases,
                symbols,
                inference,
            )?
        }
        _ => return None,
    };
    (!origins.is_empty()).then_some(origins)
}

/// A bare name resolves to the parameter place it spells, or to every
/// candidate the earlier local reference alias still admits. A divergent
/// binding keeps the exact finite union rather than selecting one route.
fn parameter_relative_name_origins(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, Vec<ParameterRelativeFrameOrigin>)],
) -> Option<Vec<ParameterRelativeFrameOrigin>> {
    reference_origins::declared_origin_root(program, current_machine, expression)?;
    let place = frame_place_path(program, expression)?;
    let root_symbol = frame_place_root_symbol(program, expression);
    let (root, suffix) = split_place_root(&place.path);
    if let Some(parameter) = parameters.iter().find(|parameter| {
        (root_symbol == Some(parameter.symbol) || (parameter.is_self && root == "self"))
            && type_reference_is_reference(program, parameter.type_reference)
    }) {
        return Some(vec![ParameterRelativeFrameOrigin {
            place,
            parameter_symbol: parameter.symbol,
        }]);
    }
    let parents = aliases.iter().find_map(|(_, symbol, origins)| {
        let exact_symbol =
            root_symbol.is_some_and(|root| root.is_valid() && symbol.is_valid() && root == *symbol);
        exact_symbol.then_some(origins)
    })?;
    Some(
        parents
            .iter()
            .map(|parent| {
                let source = parent.place.source.append_segments(&place.source.segments);
                ParameterRelativeFrameOrigin {
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
                }
            })
            .collect(),
    )
}

/// One candidate per proven callee-result route. Each candidate instantiates
/// through its own selected actual; an actual that cannot be named keeps the
/// whole call opaque because the result could reach untracked storage.
fn parameter_relative_call_result_origins(
    program: &TypedTrees,
    current_machine: &Machine,
    call: &TableCallExpression,
    caller_parameters: &[StateParameter],
    caller_aliases: &[(String, SymbolHandle, Vec<ParameterRelativeFrameOrigin>)],
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<ParameterRelativeFrameOrigin>> {
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
    let callee_origins = transparent_callee_result_origins(
        program,
        callee_machine,
        callee_state,
        symbols,
        inference,
    )?;
    let callee_parameters = program.state_parameters(callee_state);
    let mut origins = Vec::new();
    for callee_origin in callee_origins {
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
        for actual_origin in parameter_relative_place_origins(
            program,
            current_machine,
            actual,
            caller_parameters,
            caller_aliases,
            symbols,
            inference,
        )? {
            let (_, suffix) = split_place_root(&callee_origin.place.path);
            let source = actual_origin
                .place
                .source
                .append_relative(&callee_origin.place.source);
            push_unique_parameter_relative(
                &mut origins,
                match actual_origin.place.precision {
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
                },
            );
        }
    }
    Some(origins)
}

fn push_unique_parameter_relative(
    origins: &mut Vec<ParameterRelativeFrameOrigin>,
    origin: ParameterRelativeFrameOrigin,
) {
    if !origins.iter().any(|existing| {
        existing.parameter_symbol == origin.parameter_symbol
            && same_place_origin(&existing.place, &origin.place)
    }) {
        origins.push(origin);
    }
}

/// Collapse a candidate set to one binding origin. Routes agreeing on
/// parameter identity and storage path merge, coarsening when their referent
/// position or structural source differs; anything else has no single origin.
pub(super) fn single_parameter_relative_origin(
    candidates: Vec<ParameterRelativeFrameOrigin>,
) -> Option<ParameterRelativeFrameOrigin> {
    let mut merged: Option<ParameterRelativeFrameOrigin> = None;
    for origin in candidates {
        match &mut merged {
            None => merged = Some(origin),
            Some(existing)
                if existing.parameter_symbol == origin.parameter_symbol
                    && existing.place.path == origin.place.path =>
            {
                if origin.place.precision == FramePathPrecision::CollectionCoarse
                    || origin.place.source != existing.place.source
                {
                    existing.place.precision = FramePathPrecision::CollectionCoarse;
                }
            }
            Some(_) => return None,
        }
    }
    merged
}
