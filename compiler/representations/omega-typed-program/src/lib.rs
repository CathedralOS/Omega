pub mod data;
pub mod expression;
pub mod identity;
pub mod invariant;
pub mod machine;
pub mod name;
pub mod platform;
pub mod signature;
pub mod state;
pub mod statement;
pub mod tables;
pub mod types;

use omega_core::arena::Arena;
use omega_core::symbols::SymbolTable;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Program {
    pub data_definitions: Vec<data::DataDefinition>,
    pub invariant_definitions: Vec<invariant::InvariantDefinition>,
    pub machines: Vec<machine::Machine>,
    pub platforms: Vec<platform::Platform>,
    pub type_constraints: Arena<types::TypeConstraint>,
    pub expression_table: expression::ExpressionTable,
    pub type_reference_table: types::TypeReferenceTable,
    pub symbols: SymbolTable,
}

impl Program {
    pub fn rebuild_tables(&mut self) {
        let tables = tables::TypedProgramTables::from_program(self);
        self.expression_table = tables.expressions;
        self.type_reference_table = tables.type_references;
    }
}
