use crate::flow::effective_member_symbol;
use checked_trees::ContractCallFact;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use facts::FactPlan;
use symbols::SymbolHandle;

pub(crate) fn instantiate_call_contract_expression_place(
    program: &typed_trees::TypedTrees,
    facts: &mut FactPlan,
    call: &ContractCallFact,
    expression: ExpressionHandle,
) -> Option<facts::PlaceHandle> {
    instantiate_contract_expression_place(program, facts, call, None, expression)
}

pub(crate) fn instantiate_outcome_contract_expression_place(
    program: &typed_trees::TypedTrees,
    facts: &mut FactPlan,
    call: &ContractCallFact,
    result_statement_index: usize,
    result_expression: ExpressionHandle,
    expression: ExpressionHandle,
) -> Option<facts::PlaceHandle> {
    instantiate_contract_expression_place(
        program,
        facts,
        call,
        Some((result_statement_index, result_expression)),
        expression,
    )
}

fn instantiate_contract_expression_place(
    program: &typed_trees::TypedTrees,
    facts: &mut FactPlan,
    call: &ContractCallFact,
    result: Option<(usize, ExpressionHandle)>,
    expression: ExpressionHandle,
) -> Option<facts::PlaceHandle> {
    if !expression.is_valid() {
        return None;
    }

    // A constructor actual captures its selected field, not a fictitious
    // storage field below the constructor expression. Keep that same source
    // projection for both contract dependency invalidation and proof use.
    if result.is_none()
        && let Some(site) = super::find_call_site(
            program,
            call.caller_machine_symbol,
            call.caller_state_symbol,
            call.statement_index,
            call.call_ordinal,
        )
        && let Some(parameters) = super::call_target_parameters(program, call.target_state_symbol)
        && let Some((value, remaining)) = call_contract_argument_projection(
            program,
            parameters,
            super::call_site_argument_expressions(program, &site),
            expression,
        )
    {
        let mut place = crate::flow::canonical_place_from_expression_in_state(
            program,
            call.caller_state_symbol,
            call.statement_index,
            value,
        )?;
        place.segments.extend(remaining);
        return Some(super::append_place_with_segments(
            facts,
            place.root,
            &place.segments,
        ));
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            instantiate_contract_expression_place(program, facts, call, result, inner.target)
        }
        ExpressionNode::Name(path) => {
            instantiate_call_contract_name_path_place(program, facts, call, result, path)
        }
        ExpressionNode::Member(member) => {
            let receiver = instantiate_contract_expression_place(
                program,
                facts,
                call,
                result,
                member.receiver,
            )?;
            let symbol = {
                let symbol = effective_member_symbol(program, member.receiver, member);
                if symbol.is_valid() {
                    symbol
                } else {
                    super::resolve_place_member_symbol(
                        program,
                        facts,
                        receiver,
                        member.member.as_str(),
                    )
                    .unwrap_or_else(SymbolHandle::invalid)
                }
            };
            let receiver = if let Some(variant) = facts::payload_variant_for_field(program, symbol)
            {
                super::append_place_segment(facts, receiver, facts::PlaceSegment::Case { variant })
            } else {
                receiver
            };
            Some(super::append_place_segment(
                facts,
                receiver,
                facts::PlaceSegment::Field { symbol },
            ))
        }
        ExpressionNode::Indexed(indexed) => {
            let receiver = instantiate_contract_expression_place(
                program,
                facts,
                call,
                result,
                indexed.collection,
            )?;
            let segment = crate::flow::index_place_segment(program, indexed.index);
            Some(super::append_place_segment(facts, receiver, segment))
        }
        _ => None,
    }
}

/// Select the captured value of an explicit formal projection using exact
/// nominal fields. Non-constructed actuals retain their remaining field path.
pub(crate) fn call_contract_argument_projection(
    program: &typed_trees::TypedTrees,
    parameters: &[typed_trees::signature::StateParameter],
    arguments: &[ExpressionHandle],
    expression: ExpressionHandle,
) -> Option<(ExpressionHandle, Vec<facts::PlaceSegment>)> {
    let formal = crate::flow::canonical_place_from_expression(program, expression)?;
    let facts::PlaceRoot::Symbol(symbol) = formal.root else {
        return None;
    };
    let (position, parameter) = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .enumerate()
        .find(|(_, parameter)| parameter.symbol == symbol)?;
    if formal
        .segments
        .iter()
        .any(|segment| crate::flow::place_segment_has_unresolved_identity(*segment))
    {
        return None;
    }
    let mut projections = crate::flow::literal_value_projections(
        program,
        *arguments.get(position)?,
        parameter.type_reference,
        &formal.segments,
        false,
    )?;
    let projection = projections.pop()?;
    if !projections.is_empty() {
        return None;
    }
    Some((projection.expression, projection.remaining))
}

fn instantiate_call_contract_name_path_place(
    program: &typed_trees::TypedTrees,
    facts: &mut FactPlan,
    call: &ContractCallFact,
    result: Option<(usize, ExpressionHandle)>,
    path: &typed_trees::expression::TableNamePath,
) -> Option<facts::PlaceHandle> {
    let members = program.expression_table.name_path_members(path.members);
    let call_site = super::find_call_site(
        program,
        call.caller_machine_symbol,
        call.caller_state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let target_parameters = super::call_target_parameters(program, call.target_state_symbol)?;
    let first_member = members.first().map(|member| member.as_str());

    let mut place = if first_member == Some("result") {
        if let Some((statement_index, expression)) = result {
            super::canonical_place_to_fact_place_in_state(
                program,
                facts,
                call.caller_state_symbol,
                statement_index,
                expression,
            )?
        } else {
            let super::CallSite::Expression { expression, .. } = call_site else {
                return None;
            };
            facts.append_place_from_expression(program, expression)
        }
    } else if first_member == Some("self")
        || target_parameters
            .iter()
            .find(|parameter| parameter.is_self)
            .is_some_and(|parameter| {
                path.head_symbol == parameter.symbol || path.symbol == parameter.symbol
            })
    {
        super::receiver_place_for_call(program, facts, call, &call_site)?
    } else {
        let mut argument_index = 0usize;
        let mut matched = None;
        for parameter in target_parameters {
            if parameter.is_self {
                continue;
            }

            let argument = super::call_site_argument_expressions(program, &call_site)
                .get(argument_index)
                .copied();
            argument_index = argument_index.saturating_add(1);

            if first_member == Some(parameter.name.as_str())
                || path.head_symbol == parameter.symbol
                || path.symbol == parameter.symbol
            {
                matched = argument.and_then(|expr| call_argument_place(program, facts, call, expr));
                break;
            }
        }
        matched?
    };

    let tail_count = members.len().saturating_sub(1);
    if tail_count == 0 {
        return Some(place);
    }

    let member_symbols = program
        .expression_table
        .name_path_member_symbols(path.member_symbols);
    for (offset, member_name) in members.iter().skip(1).enumerate() {
        let symbol = member_symbols
            .get(offset + 1)
            .copied()
            .filter(|symbol| symbol.is_valid())
            .or_else(|| {
                super::resolve_place_member_symbol(program, facts, place, member_name.as_str())
            })
            .unwrap_or_else(SymbolHandle::invalid);
        if let Some(variant) = facts::payload_variant_for_field(program, symbol) {
            place =
                super::append_place_segment(facts, place, facts::PlaceSegment::Case { variant });
        }
        place = super::append_place_segment(facts, place, facts::PlaceSegment::Field { symbol });
    }

    Some(place)
}

fn call_argument_place(
    program: &typed_trees::TypedTrees,
    facts: &mut FactPlan,
    call: &ContractCallFact,
    expression: ExpressionHandle,
) -> Option<facts::PlaceHandle> {
    super::canonical_place_to_fact_place_in_state(
        program,
        facts,
        call.caller_state_symbol,
        call.statement_index,
        expression,
    )
}
