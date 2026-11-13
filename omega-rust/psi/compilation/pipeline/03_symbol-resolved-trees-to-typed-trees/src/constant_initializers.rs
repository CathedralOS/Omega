//! A constant position requests an ordinary zero-argument computation.
//! Retain that computation as typed machine input; only structural discharge
//! and invocation admission may authorize running it. The authored expression
//! remains separate from the value subsequently materialized by evaluation.

use arena::Handle;
use diagnostics::Diagnostic;
use symbols::SymbolKind;
use typed_trees::{TypedTrees, constant::ConstDeclaration, machine::Machine, state::State};
use typed_trees::statement::{StatementNode, TableTransition, TransitionTargetNode};

pub(crate) fn retain_entry(
    program: &mut TypedTrees,
    declaration: Handle<ConstDeclaration>,
) -> Result<(), Diagnostic> {
    let constant = program.tables.const_declarations.get(declaration).clone();
    if constant.evaluated_value.is_some() || constant.pending_target_observation {
        return Ok(());
    }
    if !program.expression_table.expression_is_valid(constant.materialized_initializer) {
        return Err(Diagnostic::error("constant lost its retained initializer"));
    }
    let symbol = program.symbols.insert_generated_root_from(
        constant.symbol, SymbolKind::Machine, "@constant-initializer",
    );
    let state_symbol = program.symbols.insert_generated_children(
        symbol, [(SymbolKind::State, "@constant-initializer")],
    ).start();
    let mut machine = Machine {
        symbol,
        name: typed_trees::name::Identifier::generated("@constant-initializer"),
        ..Default::default()
    };
    let mut state = State {
        symbol: state_symbol,
        name: machine.name.clone(),
        return_type: constant.declared_type,
        ..Default::default()
    };
    let target = program.statement_table.insert_transition_target(
        TransitionTargetNode::Value(constant.materialized_initializer),
    );
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Transition(TableTransition {
            target,
            source_span: constant.initializer_source_span,
            ..Default::default()
        }),
    );
    program.push_machine_state(&mut machine, state);
    let entry = program.tables.machines.append_to_span(&mut program.roots.machines, machine);
    let constant = program.tables.const_declarations.get_mut(declaration);
    constant.evaluation_entry = entry;
    constant.evaluation_expression = constant.materialized_initializer;
    Ok(())
}
