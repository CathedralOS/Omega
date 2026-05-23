use super::*;

#[derive(Debug, Clone)]
pub(crate) struct ContractPlaceSubstitution {
    pub(crate) root: omega_facts::PlaceRoot,
    pub(crate) segments: Vec<omega_facts::PlaceSegment>,
}

pub(crate) fn call_contract_place_substitution(
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
            super::receiver_place_for_call(program, facts, call, &call_site)
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
        let substitution_place = super::receiver_place_for_call(program, facts, call, &call_site)?;
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
