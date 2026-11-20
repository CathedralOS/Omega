//! Executable statements and lexical declarations of one resolved state.
use crate::source_resolved::SymbolResolvedTrees;
use crate::source_resolved::state::State;
use crate::source_resolved::statement::{Statement, StatementNode, TableLocalData};
use crate::source_resolved::types::TypeReference;
use symbols::SymbolHandle;

impl SymbolResolvedTrees {
    /// The executable statements published by resolution, in evaluation order.
    pub fn state_body(&self, state: &State) -> &[StatementNode] {
        self.statement_table().statements(state.statement_nodes)
    }

    /// Local instructions and their declared types, including lexical operand owners.
    pub fn state_locals(
        &self,
        state: &State,
    ) -> impl Iterator<Item = (&TableLocalData, &TypeReference)> {
        self.state_body(state)
            .iter()
            .zip(self.construction_statements(state.storage.construction_statements))
            .filter_map(|(instruction, declaration)| declared_local(instruction, declaration))
    }

    pub fn state_local(
        &self,
        state: &State,
        symbol: SymbolHandle,
    ) -> Option<(&TableLocalData, &TypeReference)> {
        if !symbol.is_valid() {
            return None;
        }
        let declarations = self.construction_statements(state.storage.construction_statements);
        let position = self
            .resolution_indexes
            .local_statement_position(declarations, symbol)? as usize;
        let (local, declared_type) = declared_local(
            self.state_body(state).get(position)?,
            declarations.get(position)?,
        )?;
        (local.symbol == symbol).then_some((local, declared_type))
    }
}

// Body publication projects each declaration at the same ordinal. A local's
// instruction supplies execution metadata. Its declaration retains the rich
// type tree, which can mention source-scoped operands.
fn declared_local<'a>(
    instruction: &'a StatementNode,
    declaration: &'a Statement,
) -> Option<(&'a TableLocalData, &'a TypeReference)> {
    match (instruction, declaration) {
        (StatementNode::LocalData(local), Statement::LocalData(declared))
            if local.symbol == declared.symbol =>
        {
            Some((local, &declared.type_reference))
        }
        _ => None,
    }
}
