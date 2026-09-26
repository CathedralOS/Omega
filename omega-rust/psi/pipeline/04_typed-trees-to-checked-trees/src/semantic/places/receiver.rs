use crate::checked_trees::ContractCallFact;
use crate::fact_plan::FactPlan;
use crate::lookup::{statement_call_receiver_members, statement_call_receiver_path};
use symbols::SymbolHandle;

pub(crate) fn receiver_place_for_call(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    facts: &mut FactPlan,
    call: &ContractCallFact,
    call_site: &super::CallSite<'_>,
) -> Option<crate::fact_plan::PlaceHandle> {
    match call_site {
        super::CallSite::Statement(statement) => {
            if let Some(members) = statement_call_receiver_members(program, statement) {
                if members
                    .first()
                    .is_some_and(|member| member.is_self_receiver())
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
                        if let Some(variant) =
                            crate::fact_plan::payload_variant_for_field(program, symbol)
                        {
                            place = super::append_place_segment(
                                facts,
                                place,
                                crate::fact_plan::PlaceSegment::Case { variant },
                            );
                        }
                        place = super::append_place_segment(
                            facts,
                            place,
                            crate::fact_plan::PlaceSegment::Field { symbol },
                        );
                    }
                    return Some(place);
                }

                let state = super::find_state_in_machine(
                    program,
                    call.caller_machine_symbol,
                    call.caller_state_symbol,
                )?;
                if let Some(path) =
                    statement_call_receiver_path(program, state, call.statement_index, statement)
                {
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
