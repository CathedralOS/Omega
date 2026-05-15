use crate::{expression, snapshot, statement, tables, types};
use omega_core::arena::Arena;
use omega_core::symbols::SymbolTable;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolResolvedTrees {
    pub roots: SymbolResolvedRoots,
    pub tables: SymbolResolvedTableStorage,
    pub symbols: SymbolTable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolResolvedRoots {
    pub data_definitions: Vec<crate::data::DataDefinition>,
    pub invariant_definitions: Vec<crate::invariant::InvariantDefinition>,
    pub machines: Vec<crate::machine::Machine>,
    pub platforms: Vec<crate::platform::Platform>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolResolvedTableStorage {
    pub bodies: SymbolResolvedBodyStorage,
    pub types: SymbolResolvedTypeStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolResolvedTypeStorage {
    pub constraints: Arena<types::TypeConstraint>,
    pub references: types::TypeReferenceTable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolResolvedBodyStorage {
    pub expressions: expression::ExpressionTable,
    pub statements: statement::StatementTable,
}

impl SymbolResolvedTrees {
    pub fn rebuild_tables(&mut self) {
        let tables =
            tables::SymbolResolvedTreeTables::from_symbol_resolved_trees_with_state_spans(self);
        self.tables.bodies.expressions = tables.bodies.expressions;
        self.tables.bodies.statements = tables.bodies.statements;
        self.tables.types.references = tables.types.references;
    }

    pub fn snapshot(&self) -> snapshot::SymbolResolvedTreesSnapshot {
        snapshot::SymbolResolvedTreesSnapshot::from_symbol_resolved_trees(self)
    }

    pub fn snapshot_json(&self) -> Result<String, serde_json::Error> {
        self.snapshot().to_json()
    }

    pub fn snapshot_json_pretty(&self) -> Result<String, serde_json::Error> {
        self.snapshot().to_json_pretty()
    }
}

impl Deref for SymbolResolvedTrees {
    type Target = SymbolResolvedRoots;

    fn deref(&self) -> &Self::Target {
        &self.roots
    }
}

impl DerefMut for SymbolResolvedTrees {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.roots
    }
}
