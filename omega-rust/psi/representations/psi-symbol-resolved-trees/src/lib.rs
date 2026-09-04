#![forbid(unsafe_code)]
#![allow(
    clippy::obfuscated_if_else,
    reason = "valid-handle copy expressions use one uniform then/fallback idiom"
)]

//! Source-shaped Psi trees after name and symbol identity resolution.

pub mod constant;
pub mod data;
pub mod declaration_selection {
    pub use psi_language_semantics::declaration_selection::*;
}
pub mod domain;
pub mod expression;
pub mod identity;
pub mod machine;
pub mod measure;
pub mod name;
pub mod operator;
pub mod proposition;
pub mod signature;
pub mod snapshot;
pub mod state;
pub mod statement;
pub mod symbol_resolved_trees;
pub mod tables;
pub mod trait_definition;
pub mod types;
pub mod wire;

pub use declaration_selection::{
    AuthoredDeclarationSelection, AuthoredDeclarationSelectionExposure,
    AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionLateBinding,
    AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError,
    AuthoredDeclarationSelectionSuffixRebase, AuthoredDeclarationSelectionSuffixRebaseError,
    AuthoredDeclarationSelectionTarget, AuthoredDeclarationSelections,
    ResolvedAuthoredDeclarationSelection,
};
pub use psi_arena::OrderedRootArena;
pub use snapshot::SymbolResolvedTreesSnapshot;
pub use symbol_resolved_trees::{
    AuthoredSelectionExtensionFrontier, AuthoredSelectionExtensionRebaseError,
    AuthoredSelectionOccurrenceStore, SymbolResolvedBodyStorage, SymbolResolvedDeclarationStorage,
    SymbolResolvedRoots, SymbolResolvedTableStorage, SymbolResolvedTrees,
    SymbolResolvedTypeStorage,
};
