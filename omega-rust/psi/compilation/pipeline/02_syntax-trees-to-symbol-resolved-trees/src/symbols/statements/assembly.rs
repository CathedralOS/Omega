//! Assembly instructions see their enclosing lexical prefix and their own
//! preceding temporaries. A sibling label never supplies a lexical binding.

use super::routing;
use crate::symbols::expressions::assign_statement_expression_symbols;
use crate::symbols::scope::MachineScope;
use arena::{Arena, HandleSpan};
use symbol_resolved_trees::{expression, name, signature, statement, types};
use symbols::{SymbolHandle, SymbolTable};

pub(super) struct AssemblyScopeRouting<'scope> {
    pub graphs: &'scope Arena<statement::AssemblyControlFlow>,
    pub blocks: &'scope Arena<statement::AssemblyBlock>,
    pub contracts: &'scope Arena<expression::ExpressionHandle>,
    pub statements: &'scope mut Arena<statement::Statement>,
    pub expressions: &'scope mut expression::ExpressionTable,
    pub child_types: &'scope mut Arena<types::TypeReference>,
    pub type_constraints: &'scope Arena<types::TypeConstraint>,
    pub path_members: &'scope mut Arena<name::DiagnosticName>,
    pub symbols: &'scope SymbolTable,
}

impl AssemblyScopeRouting<'_> {
    pub(super) fn assign(
        &mut self,
        machine: &MachineScope<'_>,
        parameters: &[signature::StateParameter],
        state: SymbolHandle,
        outer: HandleSpan<statement::Statement>,
    ) {
        let outer = self.statements.span_or_empty(outer).to_vec();
        for (index, statement) in outer.iter().enumerate() {
            let statement::Statement::Assembly(handle) = statement else {
                continue;
            };
            let graph = self.graphs.get(*handle);
            for block in self.blocks.span_or_empty(graph.blocks) {
                let prefix = &outer[..index];
                let entry = MachineScope {
                    prior_statements: prefix,
                    ..*machine
                };
                for expression in self.contracts.span_or_empty(block.requires) {
                    assign_statement_expression_symbols(
                        self.symbols,
                        &entry,
                        parameters,
                        state,
                        self.expressions,
                        self.child_types,
                        *expression,
                    );
                }
                let mut sequence = prefix.to_vec();
                sequence.extend_from_slice(self.statements.span_or_empty(block.instructions));
                for index in prefix.len()..sequence.len() {
                    let (prior, next) = sequence.split_at_mut(index);
                    let scope = MachineScope {
                        prior_statements: prior,
                        ..*machine
                    };
                    routing::assign_statement_symbols(
                        &scope,
                        parameters,
                        state,
                        self.expressions,
                        self.child_types,
                        self.type_constraints,
                        self.path_members,
                        &mut next[0],
                        self.symbols,
                    );
                }
                let exit = MachineScope {
                    prior_statements: &sequence,
                    ..*machine
                };
                for expression in self.contracts.span_or_empty(block.ensures) {
                    assign_statement_expression_symbols(
                        self.symbols,
                        &exit,
                        parameters,
                        state,
                        self.expressions,
                        self.child_types,
                        *expression,
                    );
                }
                self.statements
                    .span_mut_or_empty(block.instructions)
                    .clone_from_slice(&sequence[prefix.len()..]);
            }
        }
    }
}
