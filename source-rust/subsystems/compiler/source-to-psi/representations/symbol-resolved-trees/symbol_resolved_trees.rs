//! Symbol-resolved Omega trees between lexical name binding and typing.
//! The forest has ordered declaration roots, backing arenas and the symbol table
//! that gives each resolved occurrence its identity.

#![forbid(unsafe_code)]
#![allow(
    clippy::obfuscated_if_else,
    reason = "valid-handle copy expressions use one uniform then/fallback idiom"
)]

pub use crate as source_resolved;
use source_declarations::source_selections;
use source_symbol_table::{source_inputs, source_symbols, source_tables};
use source_symbols::SymbolTable;
use std::ops::{Deref, DerefMut};
use symbols::SymbolHandle;

macro_rules! deref_to_storage {
    ($wrapper:ty => $storage:ty) => {
        impl Deref for $wrapper {
            type Target = $storage;

            fn deref(&self) -> &Self::Target {
                &self.storage
            }
        }

        impl DerefMut for $wrapper {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.storage
            }
        }
    };
}

mod alias_bodies;
pub mod continuations;
mod nodes;
mod roots;
mod selections;
pub mod tables;

pub use nodes::{
    callable_values, calls, control_flow, declarations, evidence, machine_declarations, names,
    type_system, values,
};
pub use roots::SymbolResolvedRoots;
/// The statement spans of block values inside resolved statements.
pub mod block_values {
    pub use crate::nodes::state_bodies::{
        nested_block_statement_spans, statement_block_spans, statement_blocks,
    };
}
pub use selections::{
    AuthoredSelectionExtensionFrontier, AuthoredSelectionExtensionRebaseError,
    AuthoredSelectionOccurrenceStore,
};
pub use tables::{
    SymbolResolvedBodyStorage, SymbolResolvedDeclarationStorage, SymbolResolvedTypeStorage,
    lookup_indexes,
};

#[cfg(any(test, feature = "test-support"))]
pub mod storage {
    pub use super::tables;
}

#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub struct SymbolResolvedTrees {
    pub roots: SymbolResolvedRoots,
    pub tables: SymbolResolvedTableStorage,
    pub symbols: SymbolTable,
    pub service_reaches:
        crate::source_tables::ServiceReachTable<crate::trait_definition::TraitRequirement>,
    pub service_reach_rows: crate::source_tables::ServiceReachRowTable,
    pub authored_service_reach_rows:
        source_symbol_table::owner_rows::OwnerSortedRows<signature::AuthoredServiceReachRow>,
    pub semantic_domains: crate::source_tables::SemanticDomainTable,
    pub external_bindings: crate::source_tables::ExternalBindingTable,
    pub evidence_forwardings: Vec<statement::EvidenceForwarding>,
    pub generic_binding_overlaps: Vec<GenericBindingOverlap>,
    pub pending_token_shape_comparisons: Vec<PendingTokenShapeComparison>,
    pub resolution_indexes: lookup_indexes::ResolvedIndexes,
}

/// Typing decides whether this visible generic's bound denotes a value or a type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
pub struct GenericBindingOverlap {
    pub machine: SymbolHandle,
    pub parameter: SymbolHandle,
    pub binding: source_text::SourceSpan,
}

/// Dependent operand shapes must be compared after their types are lowered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
pub struct PendingTokenShapeComparison {
    pub earlier: SymbolHandle,
    pub later: SymbolHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub struct SymbolResolvedTableStorage {
    pub declarations: SymbolResolvedDeclarationStorage,
    pub bodies: SymbolResolvedBodyStorage,
    pub types: SymbolResolvedTypeStorage,
    authored_declaration_selections: AuthoredDeclarationSelections,
}

impl SymbolResolvedTrees {
    pub fn with_roots(
        roots: SymbolResolvedRoots,
        tables: SymbolResolvedTableStorage,
        symbols: SymbolTable,
    ) -> Self {
        Self {
            roots,
            tables,
            symbols,
            service_reaches: crate::source_tables::ServiceReachTable::default(),
            service_reach_rows: crate::source_tables::ServiceReachRowTable::default(),
            authored_service_reach_rows: Default::default(),
            semantic_domains: crate::source_tables::SemanticDomainTable::default(),
            external_bindings: crate::source_tables::ExternalBindingTable::default(),
            evidence_forwardings: Vec::new(),
            generic_binding_overlaps: Vec::new(),
            pending_token_shape_comparisons: Vec::new(),
            resolution_indexes: lookup_indexes::ResolvedIndexes::default(),
        }
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

pub use arena::OrderedRootArena;
pub use calls::signature;
pub use control_flow::{machine, state, statement};
#[cfg(any(test, feature = "test-support"))]
pub use declaration_selection::{
    AuthoredDeclarationSelection, ResolvedAuthoredDeclarationSelection,
};
pub use declaration_selection::{
    AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
    AuthoredDeclarationSelectionLateBinding, AuthoredDeclarationSelectionOccurrenceId,
    AuthoredDeclarationSelectionRecordError, AuthoredDeclarationSelectionSuffixRebase,
    AuthoredDeclarationSelectionSuffixRebaseError, AuthoredDeclarationSelectionTarget,
    AuthoredDeclarationSelections,
};
pub use declarations::{
    constant, data, domain, measure, operator, trait_definition, type_aliases, wire,
};
pub use evidence::{mathematical, proposition};
pub use names::name;
pub use type_system::types;
pub use values::expression;

pub mod declaration_selection {
    #[cfg(any(test, feature = "test-support"))]
    pub use crate::source_selections::{
        AuthoredDeclarationSelection, AuthoredDeclarationSelectionFinalizationError,
        CompilerDerivedSelectionPartition,
    };
    pub use crate::source_selections::{
        AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError,
        AuthoredDeclarationSelectionSuffixRebase, AuthoredDeclarationSelectionSuffixRebaseError,
        AuthoredDeclarationSelections,
    };
    pub use source_declarations::declaration_selection::AuthoredDeclarationSelectionExposure;
    #[cfg(any(test, feature = "test-support"))]
    pub use source_declarations::declaration_selection::AuthoredDeclarationSelectionIntrinsic;
    pub use source_declarations::declaration_selection::AuthoredDeclarationSelectionKind;
    pub use source_declarations::declaration_selection::AuthoredDeclarationSelectionLateBinding;
    pub use source_declarations::declaration_selection::AuthoredDeclarationSelectionTarget;
    #[cfg(any(test, feature = "test-support"))]
    pub use source_declarations::declaration_selection::BuildOperation;
    #[cfg(any(test, feature = "test-support"))]
    pub use source_declarations::declaration_selection::CollectionMeasure;
    #[cfg(any(test, feature = "test-support"))]
    pub use source_declarations::declaration_selection::ResolvedAuthoredDeclarationSelection;
}

#[cfg(test)]
#[path = "tests/resolved_records/mod.rs"]
mod tests;
