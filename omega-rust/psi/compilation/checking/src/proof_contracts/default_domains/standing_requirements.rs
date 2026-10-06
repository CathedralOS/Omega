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

use super::place_queries::data_definition_for_expression;
use crate::proof_contracts::collection_predicates::{fact_argument_field, predicate_application};
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;

/// One argument of the instantiated requirement.
#[derive(PartialEq, Eq)]
enum RequiredArgument<'program> {
    Field(&'program str),
    Integer(i64),
}

/// Whether a standing data fact at the call's actual receiver place proves
/// `requirement`, a collection-predicate application over the callee's
/// formals `parameters`, under the positional `actuals`.
pub fn standing_facts_prove_requirement(
    program: &TypedTrees,
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
        if let Some(value) = program.expression_table.constant_integer_value(*argument) {
            required.push(RequiredArgument::Integer(value));
            continue;
        }
        let Some(position) = formal_position(program, parameters, *argument) else {
            return false;
        };
        let Some(actual) = actuals.get(position) else {
            return false;
        };
        let place = match program.expression_table.expression(*actual) {
            ExpressionNode::Borrow(borrow)
                if borrow.access == language_core::ReferenceAccess::Shared =>
            {
                borrow.target
            }
            _ => *actual,
        };
        let ExpressionNode::Member(member) = program.expression_table.expression(place) else {
            return false;
        };
        // Every field argument must come from one receiver place, the one
        // whose data states the fact.
        match receiver {
            None => receiver = Some(member.receiver),
            Some(existing) if existing == member.receiver => {}
            Some(_) => return false,
        }
        required.push(RequiredArgument::Field(member.member.as_str()));
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
        .proof_facts
        .span_or_empty(definition.where_facts)
        .iter()
        .any(|fact| {
            let ProofFact::Expression(fact) = fact else {
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
                        RequiredArgument::Field(name) => {
                            fact_argument_field(program, definition, *argument)
                                .is_some_and(|field| field.name.as_str() == *name)
                        }
                        RequiredArgument::Integer(value) => {
                            program.expression_table.constant_integer_value(*argument)
                                == Some(*value)
                        }
                    })
        })
}

fn formal_position(
    program: &TypedTrees,
    parameters: &[StateParameter],
    argument: ExpressionHandle,
) -> Option<usize> {
    let symbol = crate::expression_traversal::single_member_argument_symbol(program, argument)?;
    parameters
        .iter()
        .position(|parameter| parameter.symbol == symbol)
}
