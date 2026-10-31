//! A reference local declared without an initializer holds no descriptor until
//! a whole store establishes it. Terminal Psi's store vocabulary
//! (wiki/spec/terminal-psi/structural_access.md, "Store vocabulary") forbids
//! reading the home, lending it to a call and forwarding it on a state edge
//! before a whole store into it on every path reaching that use. A state body
//! is one statement sequence, so every path reaching a statement passes every
//! statement before it; the judgment is statement order.

use super::source_liveness::{expression_uses_symbol, statement_uses_symbol};
use crate::frozen_program::expression_table;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::statement::StatementNode;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn validate(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut unestablished: Vec<(SymbolHandle, &str)> = Vec::new();
    for statement in program.statement_table.statements(state.statement_nodes) {
        let stored = match statement {
            StatementNode::Assignment(assignment) => whole_local_target(program, assignment.target),
            _ => None,
        };
        unestablished.retain(|(symbol, name)| {
            let used = match statement {
                StatementNode::Assignment(assignment) if stored == Some(*symbol) => {
                    expression_uses_symbol(program, assignment.value, *symbol)
                }
                _ => statement_uses_symbol(program, statement, *symbol),
            };
            if used {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{}` uses reference local `{name}` before a store \
                     establishes it; a reference declared without an initializer holds no \
                     descriptor until it is assigned",
                    machine.name,
                    state.name.as_str(),
                )));
                return false;
            }
            stored != Some(*symbol)
        });
        if let Some(local) = statement.as_local_data()
            && !local.initial_value.is_valid()
            && is_reference(program, local.type_reference)
        {
            unestablished.push((local.symbol, local.name.as_str()));
        }
    }
}

/// The local a whole-local assignment target names: a bare name with no
/// member path. A field or element store does not establish the descriptor.
fn whole_local_target(
    program: &dyn crate::CheckingInput,
    target: ExpressionHandle,
) -> Option<SymbolHandle> {
    let path = expression_table(program).name_path_node(target)?;
    (expression_table(program)
        .name_path_members(path.members)
        .len()
        == 1
        && path.symbol.is_valid())
    .then_some(path.symbol)
}

fn is_reference(
    program: &dyn crate::CheckingInput,
    mut type_reference: TypeReferenceHandle,
) -> bool {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Reference { .. } => return true,
            _ => return false,
        }
    }
}
