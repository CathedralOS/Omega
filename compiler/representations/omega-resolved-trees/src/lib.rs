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
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Program {
    pub storage: ProgramStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProgramStorage {
    pub data_definitions: Vec<data::DataDefinition>,
    pub invariant_definitions: Vec<invariant::InvariantDefinition>,
    pub machines: Vec<machine::Machine>,
    pub platforms: Vec<platform::Platform>,
    pub type_constraints: Arena<types::TypeConstraint>,
    pub expression_table: expression::ExpressionTable,
    pub statement_table: statement::StatementTable,
    pub type_reference_table: types::TypeReferenceTable,
    pub symbols: SymbolTable,
}

impl Program {
    pub fn rebuild_tables(&mut self) {
        let tables = tables::ResolvedProgramTables::from_program_with_state_spans(self);
        self.expression_table = tables.expressions;
        self.statement_table = tables.statements;
        self.type_reference_table = tables.type_references;
    }
}

impl Deref for Program {
    type Target = ProgramStorage;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl DerefMut for Program {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
}
