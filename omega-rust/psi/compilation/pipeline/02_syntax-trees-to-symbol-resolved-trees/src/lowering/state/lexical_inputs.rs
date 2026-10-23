//! One-shot lexical preparation for type operands and selected call results.
//!
//! These are declaration construction records, not executable statements. They
//! are never installed into a state's body, and the machine stays Deferred.
//! Ordinary body lowering shares the local declaration constructor; symbol
//! assignment shares its preceding-local scope and append-only allocation.

use crate::lowering::type_reference::lower_type_reference_handle;
use crate::resolution::lowerer::Lowerer;
use arena::{Handle, HandleSpan};
use diagnostics::Diagnostic;
use symbol_resolved_trees::{
    state::State,
    statement::{LocalData, LocalDataStorage, Statement},
    types::TypeReference,
};
use symbols::SymbolHandle;
use syntax_trees::{self as syntax, SyntaxTrees};

#[derive(Clone, Copy)]
pub(crate) enum LexicalInputDemand {
    TypeOperands,
    CallResults,
}

pub(crate) struct StateLexicalInputs {
    pub(crate) machine: SymbolHandle,
    pub(crate) state: Handle<State>,
    pub(crate) locals: Vec<Statement>,
    /// Each input sees only the authored locals preceding its statement.
    pub(crate) types: Vec<(usize, TypeReference)>,
    pub(crate) calls: Vec<(usize, symbol_resolved_trees::expression::ExpressionHandle)>,
}

pub(crate) fn lower_local_declaration(
    lowerer: &mut Lowerer,
    syntax: &SyntaxTrees,
    local: &syntax::statement::TableLocalData,
) -> Result<LocalData, Diagnostic> {
    // Parser-derived destructuring locals have no authored type. Unit is the
    // existing inferred-type marker, not an invented executable initializer.
    let type_reference = if local.type_reference.is_valid() {
        lower_type_reference_handle(lowerer, syntax, local.type_reference)?
    } else {
        TypeReference::Unit
    };
    Ok(LocalData {
        symbol: SymbolHandle::invalid(),
        name: crate::lowering::name::lower_name(syntax, &local.name),
        storage: LocalDataStorage {
            role: local.role,
            type_reference,
            initial_value: symbol_resolved_trees::expression::ExpressionHandle::invalid(),
            is_mutable: local.is_mutable,
            type_is_inferred: !local.type_reference.is_valid(),
            is_operand_capture: false,
            relevance: local.relevance,
        },
    })
}

pub(crate) fn lower_state_lexical_inputs(
    lowerer: &mut Lowerer,
    syntax: &SyntaxTrees,
    machine: SymbolHandle,
    state: Handle<State>,
    statements: HandleSpan<syntax::statement::StatementHandle>,
    demand: LexicalInputDemand,
) -> Result<StateLexicalInputs, Diagnostic> {
    let declaration = lowerer.symbol_resolved_trees.machine_state(state).clone();
    super::with_state_scope(lowerer, &declaration, |lowerer| {
        let mut inputs = StateLexicalInputs {
            machine,
            state,
            locals: Vec::new(),
            types: Vec::new(),
            calls: Vec::new(),
        };
        for &statement in syntax.items.statements(statements) {
            let local = match syntax.statements.statement(statement) {
                syntax::statement::StatementNode::LocalData(local) => Some(local),
                _ => None,
            };
            for reference in
                crate::preparation::generic_data::statement_type_inputs(syntax, statement)
            {
                if local.is_some_and(|local| local.type_reference == reference) {
                    continue;
                }
                inputs.types.push((
                    inputs.locals.len(),
                    lower_type_reference_handle(lowerer, syntax, reference)?,
                ));
            }
            if matches!(demand, LexicalInputDemand::CallResults) {
                for expression in
                    crate::preparation::generic_data::statement_call_inputs(syntax, statement)
                {
                    inputs.calls.push((
                        inputs.locals.len(),
                        crate::lowering::expression::lower_private_expression_into_table(
                            lowerer, syntax, expression,
                        )?,
                    ));
                }
            }
            // The declaration enters scope after every input of its own
            // statement, including inputs nested in its runtime initializer.
            if let Some(local) = local {
                inputs
                    .locals
                    .push(Statement::LocalData(lower_local_declaration(
                        lowerer, syntax, local,
                    )?));
            }
        }
        Ok(inputs)
    })
}
