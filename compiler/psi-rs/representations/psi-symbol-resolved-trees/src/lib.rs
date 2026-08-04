#![forbid(unsafe_code)]

//! Source-shaped Psi trees after name and symbol identity resolution.

pub mod data;
pub mod domain;
pub mod expression;
pub mod identity;
pub mod invariant;
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

pub use psi_arena::OrderedRootArena;
pub use snapshot::SymbolResolvedTreesSnapshot;
pub use symbol_resolved_trees::{
    SymbolResolvedBodyStorage, SymbolResolvedDeclarationStorage, SymbolResolvedRoots,
    SymbolResolvedTableStorage, SymbolResolvedTrees, SymbolResolvedTypeStorage,
};
