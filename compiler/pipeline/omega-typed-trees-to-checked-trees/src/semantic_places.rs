use super::*;
use crate::lookup::{statement_call_receiver_members, statement_call_receiver_path};
use crate::semantic::contract_fact_place;
use crate::flow::{
    canonical_place_from_expression, effective_member_symbol, expression_type_symbol,
    symbol_type_symbol,
};

pub(crate) fn instantiate_call_contract_place(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut FactPlan,
    call: &ContractCallFact,
    contract: &ContractProofFact,
) -> FactPlace {
    match program.proof_facts.get(contract.fact) {
        omega_typed_trees::domain::ProofFact::Expression(expression) => {
            if let Some(place) =
                instantiate_call_contract_expression_place(program, facts, call, *expression)
            {
                return FactPlace::Place(place);
            }
        }
        omega_typed_trees::domain::ProofFact::Membership(membership) => {
            if let Some(place) = instantiate_call_contract_expression_place(
                program,
                facts,
                call,
                membership.value,
            ) {
                return FactPlace::Place(place);
            }
        }
    }

    let original_place = contract_fact_place(program, facts, contract);
    let FactPlace::Place(original_place_handle) = original_place else {
        return original_place;
    };

    let Some(substitution) =
        call_contract_place_substitution(program, facts, call, original_place_handle)
    else {
        return original_place;
    };

    let original_place = *facts.places.get(original_place_handle);
    let original_segments: Vec<_> = facts
        .place_segments
        .span_or_empty(original_place.segments)
        .iter()
        .copied()
        .collect();

    let mut segments = substitution.segments;
    segments.extend(original_segments);
    FactPlace::Place(append_place_with_segments(
        facts,
        substitution.root,
        &segments,
    ))
}

fn instantiate_call_contract_expression_place(
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
            let segment = omega_facts::PlaceSegment::Field {
                symbol: effective_member_symbol(program, member.receiver, member),
            };
            Some(append_place_segment(facts, receiver, segment))
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
            Some(append_place_segment(facts, receiver, segment))
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
        receiver_place_for_call(program, facts, call, &call_site)?
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
                matched = argument.and_then(|expr| canonical_place_to_fact_place(program, facts, expr));
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
            .or_else(|| resolve_place_member_symbol(program, facts, place, member_name.as_str()))
            .unwrap_or_else(SymbolHandle::invalid);
        place = append_place_segment(
            facts,
            place,
            omega_facts::PlaceSegment::Field { symbol },
        );
    }

    Some(place)
}

fn canonical_place_to_fact_place(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut FactPlan,
    expression: ExpressionHandle,
) -> Option<omega_facts::PlaceHandle> {
    let canonical = canonical_place_from_expression(program, expression)?;
    Some(append_place_with_segments(
        facts,
        canonical.root,
        &canonical.segments,
    ))
}

fn append_place_segment(
    facts: &mut FactPlan,
    base_place: omega_facts::PlaceHandle,
    segment: omega_facts::PlaceSegment,
) -> omega_facts::PlaceHandle {
    let place = *facts.places.get(base_place);
    let mut segments: Vec<_> = facts
        .place_segments
        .span_or_empty(place.segments)
        .iter()
        .copied()
        .collect();
    segments.push(segment);
    append_place_with_segments(facts, place.root, &segments)
}

fn resolve_place_member_symbol(
    program: &omega_typed_trees::TypedTrees,
    facts: &FactPlan,
    place: omega_facts::PlaceHandle,
    member_name: &str,
) -> Option<SymbolHandle> {
    let place = facts.places.get(place);
    let base_symbol = fact_place_type_symbol(program, facts, place)?;

    if let Some(data) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == base_symbol)
    {
        for member in program.data_members(data) {
            match member {
                omega_typed_trees::data::DataMember::Field(field)
                    if field.name.as_str() == member_name =>
                {
                    return Some(field.symbol);
                }
                omega_typed_trees::data::DataMember::Variant(variant)
                    if variant.name.as_str() == member_name =>
                {
                    return Some(variant.symbol);
                }
                _ => {}
            }
        }
    }

    None
}

fn fact_place_type_symbol(
    program: &omega_typed_trees::TypedTrees,
    facts: &FactPlan,
    place: &omega_facts::Place,
) -> Option<SymbolHandle> {
    let mut current = match place.root {
        omega_facts::PlaceRoot::Symbol(symbol) => symbol_type_symbol(program, symbol)?,
        omega_facts::PlaceRoot::Expression(expression) => {
            expression_type_symbol(program, expression)?
        }
        _ => return None,
    };

    for segment in facts.place_segments.span_or_empty(place.segments) {
        match segment {
            omega_facts::PlaceSegment::Field { symbol } => {
                current = symbol_type_symbol(program, *symbol)?;
            }
            omega_facts::PlaceSegment::Index { .. } => {
                return None;
            }
        }
    }

    Some(current)
}

#[derive(Debug, Clone)]
struct ContractPlaceSubstitution {
    root: omega_facts::PlaceRoot,
    segments: Vec<omega_facts::PlaceSegment>,
}

fn call_contract_place_substitution(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut FactPlan,
    call: &ContractCallFact,
    original_place_handle: omega_facts::PlaceHandle,
) -> Option<ContractPlaceSubstitution> {
    let original_place = *facts.places.get(original_place_handle);
    let omega_facts::PlaceRoot::Symbol(parameter_symbol) = original_place.root else {
        return None;
    };
    let call_site = super::find_call_site(
        program,
        call.caller_machine_symbol,
        call.caller_state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let target_state = super::find_state(program, call.target_state_symbol)?;
    let mut argument_index = 0usize;

    for parameter in program.state_parameters(target_state) {
        let parameter_matches = parameter.symbol == parameter_symbol
            || symbol_name(program, parameter_symbol) == parameter.name.as_str();
        let substitution_place = if parameter.is_self {
            if !parameter_matches {
                continue;
            }
            receiver_place_for_call(program, facts, call, &call_site)
        } else {
            let argument = super::call_site_argument_expressions(program, &call_site)
                .get(argument_index)
                .copied();
            argument_index = argument_index.saturating_add(1);
            if !parameter_matches {
                continue;
            }
            argument.map(|expression| facts.append_place_from_expression(program, expression))
        }?;

        let place = *facts.places.get(substitution_place);
        let segments = facts
            .place_segments
            .span_or_empty(place.segments)
            .iter()
            .copied()
            .collect();
        return Some(ContractPlaceSubstitution {
            root: place.root,
            segments,
        });
    }

    if symbol_name(program, parameter_symbol) == "self" {
        let substitution_place = receiver_place_for_call(program, facts, call, &call_site)?;
        let place = *facts.places.get(substitution_place);
        let segments = facts
            .place_segments
            .span_or_empty(place.segments)
            .iter()
            .copied()
            .collect();
        return Some(ContractPlaceSubstitution {
            root: place.root,
            segments,
        });
    }

    None
}

fn append_place_with_segments(
    facts: &mut FactPlan,
    root: omega_facts::PlaceRoot,
    segments: &[omega_facts::PlaceSegment],
) -> omega_facts::PlaceHandle {
    let place = facts.append_place(omega_facts::Place {
        root,
        segments: HandleSpan::empty(),
    });
    for segment in segments {
        facts.push_place_segment(place, *segment);
    }
    place
}

fn receiver_place_for_call(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut FactPlan,
    call: &ContractCallFact,
    call_site: &super::CallSite<'_>,
) -> Option<omega_facts::PlaceHandle> {
    match call_site {
        super::CallSite::Statement(statement) => {
            if let Some(members) = statement_call_receiver_members(program, statement) {
                if members.first().is_some_and(|member| member.as_str() == "self") {
                    let caller_state = super::find_state_in_machine(
                        program,
                        call.caller_machine_symbol,
                        call.caller_state_symbol,
                    )?;
                    let self_parameter = program
                        .state_parameters(caller_state)
                        .iter()
                        .find(|parameter| parameter.is_self)?;
                    let mut place = facts.append_symbol_place(self_parameter.symbol);
                    for member in members.iter().skip(1) {
                        let symbol = resolve_place_member_symbol(
                            program,
                            facts,
                            place,
                            member.as_str(),
                        )
                        .or_else(|| {
                            statement
                                .receiver_symbol
                                .is_valid()
                                .then_some(statement.receiver_symbol)
                        })
                        .unwrap_or_else(SymbolHandle::invalid);
                        place = append_place_segment(
                            facts,
                            place,
                            omega_facts::PlaceSegment::Field { symbol },
                        );
                    }
                    return Some(place);
                }

                if let Some(path) = statement_call_receiver_path(program, statement) {
                    return Some(append_place_from_name_path(facts, &path));
                }
            }
            statement
                .receiver_symbol
                .is_valid()
                .then(|| facts.append_symbol_place(statement.receiver_symbol))
        }
        super::CallSite::Expression(statement) => {
            if statement.receiver.is_valid() {
                return Some(facts.append_place_from_expression(program, statement.receiver));
            }

            let caller_state = super::find_state_in_machine(
                program,
                call.caller_machine_symbol,
                call.caller_state_symbol,
            )?;
            let self_parameter = program
                .state_parameters(caller_state)
                .iter()
                .find(|parameter| parameter.is_self)?;
            Some(facts.append_symbol_place(self_parameter.symbol))
        }
    }
}

fn append_place_from_name_path(
    facts: &mut FactPlan,
    path: &NamePath,
) -> omega_facts::PlaceHandle {
    let place = facts.append_symbol_place(path.head_symbol());
    for symbol in path.member_symbols().iter().skip(1) {
        if symbol.is_valid() {
            facts.push_place_segment(place, omega_facts::PlaceSegment::Field { symbol: *symbol });
        }
    }
    place
}
