//! Origins of transparent call results and the calls that preserve them.

use crate::declarations::symbols::TopLevelSymbols;
use crate::machine_calls::calls::write_frames::call_targets::machine_state_by_symbol;
use crate::machine_calls::calls::write_frames::inference::FrameInference;
use crate::machine_calls::calls::write_frames::parameter_aliases::ParameterRelativeFrameOrigin;
use crate::machine_calls::calls::write_frames::place_paths::{
    FramePathPrecision, FramePlaceOrigin, append_place_suffix, frame_place_path,
    push_unique_origin, same_place_origin, split_place_root,
};
use crate::machine_calls::calls::write_frames::transparent_effects::expression_is_effectful_for_transparent_result;
use crate::machine_calls::calls::write_frames::type_capabilities::type_reference_is_reference;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;

/// Instantiate every proven result route through its exact actual input.
/// The shared body relation preserves each normal completion's origins and
/// validates intervening writes, rebinding and exposure. This boundary retains
/// its caller-specific input resolver: a body-derived source does not itself
/// authorize borrowing the actual storage. Unresolved alternatives and recursive
/// machine calls remain opaque; a return type or lifetime is never an origin.
pub(crate) fn transparent_call_result_origins(
    program: &dyn crate::CheckingInput,
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
        if !result_origin.parameter_symbol.is_valid() {
            // A parameter-independent route (image storage) names the same
            // backing for every caller; it crosses the boundary unchanged.
            push_unique_origin(&mut origins, result_origin.place);
            continue;
        }
        let result_parameter = parameters
            .iter()
            .find(|parameter| parameter.symbol == result_origin.parameter_symbol)?;
        let (_, result_suffix) = split_place_root(&result_origin.place.path);
        let actual = {
            let (argument_index, _) = parameters
                .iter()
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

/// Every proven place the expression may name, in caller spelling. Direct
/// places keep their single origin; transparent helper results and divergent
/// match arms contribute their candidate union. Any unresolvable route fails
/// the expression rather than dropping a possible referent.
pub(crate) fn transparent_place_expression_origins(
    program: &dyn crate::CheckingInput,
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
        ExpressionNode::Call(call) => transparent_call_result_origins(
            program,
            call,
            symbols,
            inference,
            |_, _, _, actual, inference| {
                transparent_place_expression_origins(program, actual, symbols, inference)
            },
        )?,
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
/// caller-reachable parameter root fails the whole relation closed. Internal state
/// cycles use the finite result solver; recursive machine calls remain opaque.
pub(crate) fn transparent_callee_result_origins(
    program: &dyn crate::CheckingInput,
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
    let include_shared =
        super::reference_origins::exclusive_reference_referee(program, callee_state.return_type)
            .is_none();
    let relative = super::result_origins::state_result_origins(
        program,
        callee_machine,
        callee_state,
        callee_state.return_type,
        symbols,
        inference,
        include_shared,
    )?;
    // This boundary exports one reference value, never an aggregate subtree or
    // unresolved moved carrier. Its source still must name a declared input
    // reference frontier, or parameter-independent image storage.
    if !relative.moves.is_empty() || !relative.cases.is_empty() {
        return None;
    }
    let parameters = program.state_parameters(callee_state);
    let mut returned = Vec::new();
    for leaf in relative.references {
        if !leaf.local_suffix.is_empty() || !leaf.local_segments.is_empty() {
            return None;
        }
        let place = leaf.origin?;
        let parameter_symbol = if place.source.is_static() {
            SymbolHandle::invalid()
        } else {
            let parameter = parameters
                .iter()
                .find(|parameter| place.source.root == parameter.symbol)?;
            if !type_reference_is_reference(program, parameter.type_reference) {
                let declared = super::stored_origins::declared_origins_for_query(
                    program,
                    parameter.symbol,
                    parameter.name.as_str(),
                    parameter.type_reference,
                    include_shared,
                )?;
                if !declared.references.iter().any(|leaf| {
                    super::stored_origins::source_reaches_leaf(
                        &place.source.segments,
                        &leaf.local_segments,
                    )
                }) {
                    return None;
                }
            }
            parameter.symbol
        };
        if !returned.iter().any(|prior: &ParameterRelativeFrameOrigin| {
            prior.parameter_symbol == parameter_symbol && same_place_origin(&prior.place, &place)
        }) {
            returned.push(ParameterRelativeFrameOrigin {
                place,
                parameter_symbol,
            });
        }
    }
    (!returned.is_empty()).then_some(returned)
}
