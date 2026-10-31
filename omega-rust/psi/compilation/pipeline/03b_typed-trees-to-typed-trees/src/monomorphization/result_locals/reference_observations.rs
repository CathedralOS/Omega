//! Separate a call's produced reference from the scalar its consumer reads.
//!
//! A scalar destination does not change a selected callee's result signature.
//! Retain the exact call once in a reference local, then observe that local at
//! the original scalar binding. Ordinary checking establishes and retires the
//! temporary loan before subsequent effects. This also applies to authored
//! scalar locals; it is not a special calling convention for index selectors.
//!
//! Replacing a statement span invalidates statement call-site handles. The
//! specialization driver invokes this only before collecting selections or
//! after consuming them, restarting the round when an insertion was necessary.

use arena::HandleSpan;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::{StatementNode, TableLocalData};

pub(in crate::monomorphization) fn materialize(
    program: &mut TypedTrees,
) -> Result<Vec<SymbolHandle>, Vec<diagnostics::Diagnostic>> {
    let mut pending = Vec::new();
    for machine in program.machines() {
        for (state_index, state) in program.machine_states(machine).iter().enumerate() {
            let mut insertions = Vec::new();
            for (offset, local) in super::local_bindings(program, state.statement_nodes) {
                if local.relevance.is_erased()
                    || local.role != language_core::LocalBindingRole::Value
                    || program
                        .primitive_type_reference(local.type_reference)
                        .is_none()
                    || !matches!(
                        program.expression_table.expression(local.initial_value),
                        ExpressionNode::Call(_)
                    )
                {
                    continue;
                }
                let Some(actual) = validation::expression_result_type_reference(
                    program,
                    machine,
                    state,
                    local.initial_value,
                ) else {
                    continue;
                };
                if validation::reference_shell(program, actual).is_some()
                    && typed_trees::operator::scalar_operand_read_type(
                        program,
                        actual,
                        local.type_reference,
                    ) != actual
                {
                    insertions.push((offset, actual));
                }
            }
            if !insertions.is_empty() {
                pending.push((machine.clone(), state_index, state.clone(), insertions));
            }
        }
    }
    let mut changed = Vec::new();
    for (machine, state_index, state, insertions) in pending {
        let statements = program
            .statement_table
            .statements(state.statement_nodes)
            .to_vec();
        let mut body = HandleSpan::empty();
        for (offset, mut statement) in statements.into_iter().enumerate() {
            if let Some((_, actual)) = insertions.iter().find(|(position, ..)| *position == offset)
            {
                let StatementNode::LocalData(local) = &mut statement else {
                    unreachable!();
                };
                let name = format!("__reference_result#{}", local.symbol.arena_index());
                let children = program.symbols.append_generated_children_from(
                    state.symbol,
                    [(local.symbol, SymbolKind::Local, name.as_str())],
                );
                let symbol = symbols::SymbolTableBuilder::child_handles(children)
                    .next()
                    .expect("one generated reference result");
                let name = typed_trees::name::Identifier::generated(name);
                let reference = super::super::body_rewriting::insert_subject_name(
                    &mut program.expression_table,
                    name.clone(),
                    symbol,
                );
                let source_span = program.expression_table.source_span(local.initial_value);
                program
                    .expression_table
                    .set_source_span(reference, source_span);
                program.statement_table.push_statement(
                    &mut body,
                    StatementNode::LocalData(TableLocalData {
                        symbol,
                        name,
                        type_reference: *actual,
                        initial_value: local.initial_value,
                        type_is_inferred: true,
                        ..Default::default()
                    }),
                );
                local.initial_value = reference;
            }
            program.statement_table.push_statement(&mut body, statement);
        }
        program.machine_states_mut(&machine)[state_index].statement_nodes = body;
        changed.push(machine.symbol);
    }
    Ok(changed)
}
