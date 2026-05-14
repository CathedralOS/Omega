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
    pub roots: ResolvedRoots,
    pub tables: ResolvedTableStorage,
    pub symbols: SymbolTable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedRoots {
    pub data_definitions: Vec<data::DataDefinition>,
    pub invariant_definitions: Vec<invariant::InvariantDefinition>,
    pub machines: Vec<machine::Machine>,
    pub platforms: Vec<platform::Platform>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedTableStorage {
    pub type_constraints: Arena<types::TypeConstraint>,
    pub expression_table: expression::ExpressionTable,
    pub statement_table: statement::StatementTable,
    pub type_reference_table: types::TypeReferenceTable,
}

impl Program {
    pub fn rebuild_tables(&mut self) {
        let tables = tables::ResolvedProgramTables::from_program_with_state_spans(self);
        self.tables.expression_table = tables.expressions;
        self.tables.statement_table = tables.statements;
        self.tables.type_reference_table = tables.type_references;
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

impl Deref for ProgramStorage {
    type Target = ResolvedRoots;

    fn deref(&self) -> &Self::Target {
        &self.roots
    }
}

impl DerefMut for ProgramStorage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.roots
    }
}
