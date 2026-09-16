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
    let StatementNode::AssemblyFact(fact) = statement else {
        unreachable!("dispatched assembly_fact_statements")
    };
    let StatementScope {
        program,
        machine,
        state_name,
        current_state,
        machine_symbols,
        symbols,
        writable_roots,
        ..
    } = *scope;
    let diagnostics = &mut *outputs.diagnostics;
    let state = current_state;
    if let Some(state) = state {
        crate::value_custody::locals::StateValueScope {
            program,
            machine,
            state,
            machine_symbols,
            symbols,
            prior_statements: writable_roots.statements,
            context: "asm assertion",
        }
        .expression(fact.expression, diagnostics);
    }
    if !proof_facts::is_boolean_asm_fact_expression(program, machine, state, fact.expression) {
        let kind = match fact.kind {
            typed_trees::statement::AssemblyFactKind::Requires => "requires",
            typed_trees::statement::AssemblyFactKind::Ensures => "ensures",
        };
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{state_name}` asm `{kind}` fact `{}` is not boolean-shaped",
            machine.name,
            program.expression_table.display_name(fact.expression),
        )));
    }
}
