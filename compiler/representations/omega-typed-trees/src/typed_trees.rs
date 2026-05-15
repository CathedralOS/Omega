use crate::{data, expression, invariant, machine, platform, tables, types};
use omega_core::arena::Arena;
use omega_core::symbols::SymbolTable;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypedTrees {
    pub data_definitions: Vec<data::DataDefinition>,
    pub invariant_definitions: Vec<invariant::InvariantDefinition>,
    pub machines: Vec<machine::Machine>,
    pub platforms: Vec<platform::Platform>,
    pub type_constraints: Arena<types::TypeConstraint>,
    pub expression_table: expression::ExpressionTable,
    pub statement_table: crate::statement::StatementTable,
    pub type_reference_table: types::TypeReferenceTable,
    pub symbols: SymbolTable,
}

impl TypedTrees {
    pub fn rebuild_tables(&mut self) {
        let tables = tables::TypedProgramTables::from_typed_trees_with_state_spans(self);
        self.expression_table = tables.expressions;
        self.statement_table = tables.statements;
        self.type_reference_table = tables.type_references;
    }
}
