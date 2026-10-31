//! Call requirements discharged by a standing collection-predicate fact.
//!
//! A `where` fact holds at every legal observation of a place whose data is
//! not zero-gated: zero satisfies it, every write re-proves it before the
//! next consumption point, and a call is such a consumption point, so no
//! window is open when the call's requirements are checked. A requirement
//! `P(cells) == true` whose actual argument is `&receiver.board` is
//! therefore discharged when the receiver's data states exactly
//! `P(&board) == true` over the same arguments. This matches the whole fact;
//! it never takes the predicate apart.
//! Readable borrows and authenticated builtin whole views keep that argument
//! identity. Partial views do not inherit an arbitrary collection predicate.
//! Selected fields match their declaration symbols under one receiver, not
//! equal spellings belonging to unrelated declarations.

use super::place_queries::data_definition_for_expression;
use crate::frozen_program::expression_table;
use crate::proof_contracts::collection_predicates::{fact_argument_field, predicate_application};
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;

/// One argument of the instantiated requirement.
#[derive(PartialEq, Eq)]
enum RequiredArgument {
    Field(symbols::SymbolHandle),
    Integer(i64),
}

/// Whether a standing data fact at the call's actual receiver place proves
/// `requirement`, a collection-predicate application over the callee's
/// formals `parameters`, under the positional `actuals`.
pub fn standing_facts_prove_requirement(
    program: &dyn crate::CheckingInput,
    caller: &Machine,
    caller_state: &State,
    callee: symbols::SymbolHandle,
    parameters: &[StateParameter],
    actuals: &[ExpressionHandle],
    requirement: ExpressionHandle,
) -> bool {
    let Some(application) = predicate_application(program, callee, requirement) else {
        return false;
    };
    let mut receiver = None;
    let mut required = Vec::with_capacity(application.arguments.len());
    for argument in application.arguments {
        if let Some(value) = expression_table(program).constant_integer_value(*argument) {
            required.push(RequiredArgument::Integer(value));
            continue;
        }
        let Some(position) = formal_position(program, parameters, *argument) else {
            return false;
        };
        let Some(actual) = actuals.get(position) else {
            return false;
        };
        let Some(place) = collection_observation_place(program, caller, caller_state, *actual)
        else {
            return false;
        };
        let Some(member) = expression_table(program).member_node(place) else {
            return false;
        };
        // Every field argument must come from one receiver place, the one
        // whose data states the fact.
        match receiver {
            None => receiver = Some(member.receiver),
            Some(existing)
                if expression_table(program)
                    .expressions_structurally_equal(existing, member.receiver) => {}
            Some(_) => return false,
        }
        let Some(field) = crate::value_custody::places::declared_member_field_symbol(
            program,
            caller,
            Some(caller_state),
            place,
        ) else {
            return false;
        };
        required.push(RequiredArgument::Field(field));
    }
    let Some(receiver) = receiver else {
        return false;
    };
    let Some(definition) =
        data_definition_for_expression(program, caller, Some(caller_state), receiver)
    else {
        return false;
    };
    if definition.zero_gated {
        return false;
    }
    program
        .conjunctive_proof_facts(definition.where_facts)
        .any(|fact| {
            let Some(fact) = fact.as_expression() else {
                return false;
            };
            let Some(standing) = predicate_application(program, definition.symbol, *fact) else {
                return false;
            };
            standing.machine == application.machine
                && standing.arguments.len() == required.len()
                && standing
                    .arguments
                    .iter()
                    .zip(&required)
                    .all(|(argument, required)| match required {
                        RequiredArgument::Field(symbol) => {
                            fact_argument_field(program, definition, *argument)
                                .is_some_and(|field| field.symbol == *symbol)
                        }
                        RequiredArgument::Integer(value) => {
                            expression_table(program).constant_integer_value(*argument)
                                == Some(*value)
                        }
                    })
        })
}

/// Borrowing and a builtin whole view preserve the collection observation.
/// A partial window does not: an arbitrary predicate on the whole collection
/// need not hold on a subset. This is argument identity, not an elementwise
/// decomposition or permission to read a write-only loan.
fn collection_observation_place(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    mut expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    for _ in 0..128 {
        if let Some(borrow) = expression_table(program).borrow_expression(expression) {
            if borrow.access == language_core::ReferenceAccess::WriteOnly {
                return None;
            }
            expression = borrow.target;
        } else if let Some(source) = crate::value_custody::places::whole_collection_view_source(
            program,
            machine,
            Some(state),
            expression,
        ) {
            expression = source;
        } else {
            return Some(expression);
        }
    }
    None
}

fn formal_position(
    program: &dyn crate::CheckingInput,
    parameters: &[StateParameter],
    argument: ExpressionHandle,
) -> Option<usize> {
    let symbol = crate::expression_traversal::single_member_argument_symbol(program, argument)?;
    typed_trees::signature::state_parameter_position(parameters, symbol)
}
