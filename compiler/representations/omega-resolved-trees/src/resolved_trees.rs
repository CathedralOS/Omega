use crate::{expression, snapshot, statement, tables, types};
use omega_core::arena::Arena;
use omega_core::symbols::SymbolTable;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedTrees {
    pub roots: ResolvedRoots,
    pub tables: ResolvedTableStorage,
    pub symbols: SymbolTable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedRoots {
    pub data_definitions: Vec<crate::data::DataDefinition>,
    pub invariant_definitions: Vec<crate::invariant::InvariantDefinition>,
    pub machines: Vec<crate::machine::Machine>,
    pub platforms: Vec<crate::platform::Platform>,
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

impl ResolvedTrees {
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

impl Deref for ResolvedTrees {
    type Target = ResolvedRoots;

    fn deref(&self) -> &Self::Target {
        &self.roots
    }
}

impl DerefMut for ResolvedTrees {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.roots
    }
}
