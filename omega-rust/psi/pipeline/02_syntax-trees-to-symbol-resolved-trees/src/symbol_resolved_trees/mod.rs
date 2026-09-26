//! Source-shaped Psi trees after name and symbol identity resolution.
//!
//! Start at `crate::symbol_resolved_trees::symbol_resolved_trees::SymbolResolvedTrees`; its modules own the
//! program's concepts: declarations, control flow, calls, names, evidence, and
//! inspection snapshots. Every name is a `SymbolHandle` here; types and
//! signatures are still unjudged and belong to the typing stage.

pub mod declaration_selection {
    pub use language_semantics::declaration_selection::{
        AuthoredDeclarationSelection, AuthoredDeclarationSelectionExposure,
        AuthoredDeclarationSelectionFinalizationError, AuthoredDeclarationSelectionIntrinsic,
        AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionLateBinding,
        AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError,
        AuthoredDeclarationSelectionSuffixRebase, AuthoredDeclarationSelectionSuffixRebaseError,
        AuthoredDeclarationSelectionTarget, AuthoredDeclarationSelections, BuildOperation,
        CollectionMeasure, CollectionViewOperation, CompilerDerivedSelectionPartition,
        ResolvedAuthoredDeclarationSelection,
    };
}
pub mod symbol_resolved_trees;

pub use crate::symbol_resolved_trees::symbol_resolved_trees::calls::signature;
pub use crate::symbol_resolved_trees::symbol_resolved_trees::control_flow::{
    machine, state, statement,
};
pub use crate::symbol_resolved_trees::symbol_resolved_trees::declarations::{
    constant, data, domain, measure, operator, trait_definition, wire,
};
pub use crate::symbol_resolved_trees::symbol_resolved_trees::evidence::{
    mathematical, proposition,
};
pub use crate::symbol_resolved_trees::symbol_resolved_trees::inspection::snapshot;
pub use crate::symbol_resolved_trees::symbol_resolved_trees::names::name;
pub use crate::symbol_resolved_trees::symbol_resolved_trees::storage::tables;
pub use crate::symbol_resolved_trees::symbol_resolved_trees::type_system::types;
pub use crate::symbol_resolved_trees::symbol_resolved_trees::values::expression;

pub use crate::symbol_resolved_trees::symbol_resolved_trees::{
    AuthoredSelectionExtensionFrontier, AuthoredSelectionExtensionRebaseError,
    AuthoredSelectionOccurrenceStore, SymbolResolvedBodyStorage, SymbolResolvedDeclarationStorage,
    SymbolResolvedRoots, SymbolResolvedTableStorage, SymbolResolvedTrees,
    SymbolResolvedTypeStorage,
};
pub use arena::OrderedRootArena;
pub use declaration_selection::{
    AuthoredDeclarationSelection, AuthoredDeclarationSelectionExposure,
    AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionLateBinding,
    AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError,
    AuthoredDeclarationSelectionSuffixRebase, AuthoredDeclarationSelectionSuffixRebaseError,
    AuthoredDeclarationSelectionTarget, AuthoredDeclarationSelections,
    ResolvedAuthoredDeclarationSelection,
};
pub use snapshot::SymbolResolvedTreesSnapshot;
