#![forbid(unsafe_code)]
#![allow(
    clippy::obfuscated_if_else,
    reason = "valid-handle copy expressions use one uniform then/fallback idiom"
)]

//! Source-shaped Psi trees after name and symbol identity resolution.
//!
//! Start at [`symbol_resolved_trees::SymbolResolvedTrees`]; its modules own the program's concepts.

pub mod declaration_selection {
    pub use language_semantics::declaration_selection::*;
}
pub mod symbol_resolved_trees;

pub use symbol_resolved_trees::calls::signature;
pub use symbol_resolved_trees::control_flow::{machine, state, statement};
pub use symbol_resolved_trees::declarations::{
    constant, data, domain, measure, operator, trait_definition, wire,
};
pub use symbol_resolved_trees::evidence::proposition;
pub use symbol_resolved_trees::inspection::snapshot;
pub use symbol_resolved_trees::names::{identity, name};
pub use symbol_resolved_trees::storage::tables;
pub use symbol_resolved_trees::type_system::types;
pub use symbol_resolved_trees::values::expression;

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
pub use symbol_resolved_trees::{
    AuthoredSelectionExtensionFrontier, AuthoredSelectionExtensionRebaseError,
    AuthoredSelectionOccurrenceStore, SymbolResolvedBodyStorage, SymbolResolvedDeclarationStorage,
    SymbolResolvedRoots, SymbolResolvedTableStorage, SymbolResolvedTrees,
    SymbolResolvedTypeStorage,
};
