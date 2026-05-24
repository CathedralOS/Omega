use super::*;
use crate::flow::effective_member_symbol;

pub(crate) fn instantiate_call_contract_expression_place(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut FactPlan,
    call: &ContractCallFact,
    expression: ExpressionHandle,
) -> Option<omega_facts::PlaceHandle> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => {
            instantiate_call_contract_expression_place(program, facts, call, *inner)
        }
        ExpressionNode::Name(path) => {
            instantiate_call_contract_name_path_place(program, facts, call, path)
        }
        ExpressionNode::Member(member) => {
            let receiver = instantiate_call_contract_expression_place(
                program,
                facts,
                call,
                member.receiver,
            )?;
            let symbol = {
                let symbol = effective_member_symbol(program, member.receiver, member);
                if symbol.is_valid() {
                    symbol
                } else {
                    super::resolve_place_member_symbol(program, facts, receiver, member.member.as_str())
                        .unwrap_or_else(SymbolHandle::invalid)
                }
            };
            let segment = omega_facts::PlaceSegment::Field {
                symbol,
            };
            Some(super::append_place_segment(facts, receiver, segment))
        }
        ExpressionNode::Indexed(indexed) => {
            let receiver = instantiate_call_contract_expression_place(
                program,
                facts,
                call,
                indexed.collection,
            )?;
            let segment = omega_facts::PlaceSegment::Index {
                expression: indexed.index,
            };
            Some(super::append_place_segment(facts, receiver, segment))
        }
        _ => None,
    }
}

fn instantiate_call_contract_name_path_place(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut FactPlan,
    call: &ContractCallFact,
    path: &omega_typed_trees::expression::TableNamePath,
) -> Option<omega_facts::PlaceHandle> {
    let members = program.expression_table.name_path_members(path.members);
    let head = members.first()?.as_str();
    let call_site = super::find_call_site(
        program,
        call.caller_machine_symbol,
        call.caller_state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let target_state = super::find_state(program, call.target_state_symbol)?;

    let mut place = if head == "self" {
        super::receiver_place_for_call(program, facts, call, &call_site)?
    } else {
        let mut argument_index = 0usize;
        let mut matched = None;
        for parameter in program.state_parameters(target_state) {
            if parameter.is_self {
                continue;
            }

            let argument = super::call_site_argument_expressions(program, &call_site)
                .get(argument_index)
                .copied();
            argument_index = argument_index.saturating_add(1);

            if parameter.name.as_str() == head {
                matched = argument
                    .and_then(|expr| super::canonical_place_to_fact_place(program, facts, expr));
                break;
            }
        }
        matched?
    };

    let tail_count = members.len().saturating_sub(1);
    if tail_count == 0 {
        return Some(place);
    }

    let member_symbols = program.expression_table.name_path_member_symbols(path.member_symbols);
    for (offset, member_name) in members.iter().skip(1).enumerate() {
        let symbol = member_symbols
            .get(offset + 1)
            .copied()
            .filter(|symbol| symbol.is_valid())
            .or_else(|| super::resolve_place_member_symbol(program, facts, place, member_name.as_str()))
            .unwrap_or_else(SymbolHandle::invalid);
        place = super::append_place_segment(
            facts,
            place,
            omega_facts::PlaceSegment::Field { symbol },
        );
    }

    Some(place)
}
