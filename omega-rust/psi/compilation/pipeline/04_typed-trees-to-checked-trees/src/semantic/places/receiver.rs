use crate::lookup::{statement_call_receiver_members, statement_call_receiver_path};
use checked_trees::ContractCallFact;
use facts::FactPlan;

/// The place a call reads its subject from, or `None` when the call names no
/// subject operand.
///
/// Every call supplies its subject explicitly: a statement call names a
/// receiver path, an expression call names a receiver expression, and a
/// transition supplies the record as the argument at the callee's data-binding
/// position, which `instantiate_call_relative_places` resolves positionally.
/// Nothing arrives implicitly -- a carrier qualification names the semantic
/// owner and contributes no argument or storage -- so a call with no receiver
/// operand has no subject place to report here.
///
/// The three arms below each ended in a `StateParameter::is_self` lookup for
/// the retired implicit receiver, and the statement arm recognized a receiver
/// path headed by the `self` keyword. Stage 01 rejects both spellings, so each
/// was an unconditional refusal standing in front of the explicit operand.
pub(crate) fn receiver_place_for_call(
    program: &typed_trees::TypedTrees,
    facts: &mut FactPlan,
    call: &ContractCallFact,
    call_site: &super::CallSite<'_>,
) -> Option<facts::PlaceHandle> {
    match call_site {
        super::CallSite::Statement(statement) => {
            if statement_call_receiver_members(program, statement).is_some() {
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
            call: expression, ..
        } => expression
            .receiver
            .is_valid()
            .then(|| facts.append_place_from_expression(program, expression.receiver)),
        super::CallSite::TransitionNamed { .. } => None,
    }
}
