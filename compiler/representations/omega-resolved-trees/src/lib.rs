pub mod data;
pub mod expression;
pub mod identity;
pub mod invariant;
pub mod machine;
pub mod name;
pub mod platform;
pub mod snapshot;
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
    pub bodies: ResolvedBodyStorage,
    pub types: ResolvedTypeStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedTypeStorage {
    pub constraints: Arena<types::TypeConstraint>,
    pub references: types::TypeReferenceTable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedBodyStorage {
    pub expressions: expression::ExpressionTable,
    pub statements: statement::StatementTable,
}

impl Program {
    pub fn rebuild_tables(&mut self) {
        let tables = tables::ResolvedProgramTables::from_program_with_state_spans(self);
        self.tables.bodies.expressions = tables.bodies.expressions;
        self.tables.bodies.statements = tables.bodies.statements;
        self.tables.types.references = tables.types.references;
    }

    pub fn snapshot(&self) -> snapshot::ResolvedProgramSnapshot {
        snapshot::ResolvedProgramSnapshot::from_program(self)
    }

    pub fn snapshot_json(&self) -> Result<String, serde_json::Error> {
        self.snapshot().to_json()
    }

    pub fn snapshot_json_pretty(&self) -> Result<String, serde_json::Error> {
        self.snapshot().to_json_pretty()
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
