//! An assembly fact statement: the fact's places resolve inside the state's
//! value scope.

use super::{StatementOutputs, StatementScope};
use crate::proof_contracts::proof_facts;
use diagnostics::Diagnostic;
use typed_trees::statement::StatementNode;

pub(super) fn validate(
    scope: &StatementScope<'_>,
    outputs: &mut StatementOutputs<'_>,
    statement: &StatementNode,
) {
    let Some(fact) = statement.as_assembly_fact() else {
        unreachable!("dispatched assembly_fact_statements")
    };
    let StatementScope {
        program,
        machine,
        state_name,
        current_state: state,
        symbols,
        writable_roots,
        ..
    } = *scope;
    let diagnostics = &mut *outputs.diagnostics;
    if let Some(state) = state {
        for expression in program.tables.proof_fact_expression_roots(fact.fact) {
            crate::value_custody::locals::StateValueScope {
                program,
                machine,
                state,
                symbols,
                prior_statements: writable_roots.statements,
                context: "asm assertion",
            }
            .expression(expression, diagnostics);
        }
    }
    let proof_fact = program.proof_facts.get(fact.fact);
    if let Some(state) = state {
        proof_facts::validate_proof_facts(
            program,
            std::slice::from_ref(proof_fact),
            diagnostics,
            proof_facts::ProofFactOwner::StatementContract {
                machine,
                state,
                statement_index: scope.statement_index,
                kind: "asm",
            },
        );
    }
    if let typed_trees::domain::ProofFact::Expression(expression) = proof_fact
        && !proof_facts::is_boolean_asm_fact_expression(program, machine, state, *expression)
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{state_name}` asm fact `{}` is not boolean-shaped",
            machine.name,
            program.expression_table.display_name(*expression)
        )));
    }
}
