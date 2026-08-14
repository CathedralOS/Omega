use super::*;
use crate::lookup::{statement_call_receiver_members, statement_call_receiver_path};

pub(crate) fn receiver_place_for_call(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut FactPlan,
    call: &ContractCallFact,
    call_site: &super::CallSite<'_>,
) -> Option<psi_facts::PlaceHandle> {
    match call_site {
        super::CallSite::Statement(statement) => {
            if let Some(members) = statement_call_receiver_members(program, statement) {
                if members
                    .first()
                    .is_some_and(|member| member.as_str() == "self")
                {
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
                        let symbol = super::resolve_place_member_symbol(
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
                        if let Some(variant) = psi_facts::payload_variant_for_field(program, symbol)
                        {
                            place = super::append_place_segment(
                                facts,
                                place,
                                psi_facts::PlaceSegment::Case { variant },
                            );
                        }
                        place = super::append_place_segment(
                            facts,
                            place,
                            psi_facts::PlaceSegment::Field { symbol },
                        );
                    }
                    return Some(place);
                }

                if let Some(path) = statement_call_receiver_path(program, statement) {
                    return Some(super::append_place_from_name_path(facts, &path));
                }
            }
            statement
                .receiver_symbol
                .is_valid()
                .then(|| facts.append_symbol_place(statement.receiver_symbol))
        }
        super::CallSite::Expression {
            call: statement, ..
        } => {
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
        super::CallSite::TransitionNamed { .. } => {
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
