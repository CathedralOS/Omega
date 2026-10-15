//! Normal returned views must denote storage that outlives this invocation.
//! The borrow builder captures their leaves against the current descriptor
//! roster, shared with local and store formation. An incoming referent is not
//! its owned carrier's storage; a declared lifetime alone is not an origin.
//! Locals and by-value inputs are both invocation-owned. Returning their stored
//! external references is valid; returning a fresh borrow of their storage is not.

use checked_trees::CheckFacts;
use diagnostics::Diagnostic;

pub(super) fn check_view_return_escape(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (_, borrow_state) in facts.borrow.states.iter() {
        let Some((machine, state)) = validation::machine_state_by_symbols(
            program,
            borrow_state.machine_symbol,
            borrow_state.state_symbol,
        ) else {
            continue;
        };
        let mut reported: Vec<facts::PlaceRoot> = Vec::new();
        for loan in facts
            .borrow
            .returned_loans
            .span_or_empty(borrow_state.returned_loans)
        {
            if loan.incoming_reference.is_valid() || reported.contains(&loan.root) {
                continue;
            }
            let storage = match loan.root {
                facts::PlaceRoot::Symbol(root) => {
                    if let Some((_, local)) = program.state_local_declaration(state, root) {
                        format!("the local `{}`", local.name)
                    } else if let Some(parameter) =
                        program.state_parameters(state).iter().find(|parameter| {
                            parameter.symbol == root
                                && !parameter.is_const
                                && validation::reference_shell(program, parameter.type_reference)
                                    .is_none()
                        })
                    {
                        format!("the owned input `{}`", parameter.name)
                    } else {
                        continue;
                    }
                }
                facts::PlaceRoot::Expression(expression) => {
                    let is_view = validation::expression_type_reference_in_state(
                        program,
                        state.symbol,
                        loan.statement_index,
                        expression,
                    )
                    .is_some_and(|reference| {
                        validation::reference_shell(program, reference).is_some()
                    });
                    // Quoted bytes already denote immutable image storage even
                    // when no explicit reference type was recorded for the literal.
                    let image_literal = loan.kind == checked_trees::BorrowAccessKind::Read
                        && matches!(
                            program.expression_table.expression(expression),
                            typed_trees::expression::ExpressionNode::String(_)
                        );
                    if (is_view || image_literal)
                        && super::persistent::is_state_independent_borrow_source(
                            program,
                            state,
                            expression,
                            &mut Vec::new(),
                        )
                    {
                        continue;
                    }
                    "temporary storage".to_owned()
                }
                facts::PlaceRoot::Unknown | facts::PlaceRoot::TypeReference(_) => {
                    diagnostics.push(Diagnostic::error(
                        "returned view has no captured storage origin",
                    ));
                    continue;
                }
            };
            reported.push(loan.root);
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` returns a view borrowing {storage}, which does not outlive \
                 the call; return a view of storage that outlives the invocation instead",
                state_subject(machine, state),
            )));
        }
    }
}

pub(super) fn state_subject<'a>(
    machine: &'a typed_trees::machine::Machine,
    state: &'a typed_trees::state::State,
) -> &'a str {
    if state.name.as_str() == "entry" || machine.name.as_str().ends_with(state.name.as_str()) {
        machine.name.as_str()
    } else {
        state.name.as_str()
    }
}
