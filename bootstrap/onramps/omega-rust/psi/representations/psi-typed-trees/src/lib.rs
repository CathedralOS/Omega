#![forbid(unsafe_code)]

//! Psi-owned typed source representation.

pub mod boundary;
pub mod byte_predicates;
pub mod constant;
pub mod data;
pub mod dependent_ranges;
pub mod domain;
pub mod dynamic_traits;
pub mod expression;
pub mod identity;
pub mod invariant;
pub mod machine;
pub mod measure;
pub mod name;
pub mod operator;
pub mod proof_only;
pub mod proposition;
pub mod ranking;
pub mod signature;
pub mod snapshot;
pub mod state;
pub mod statement;
pub mod trait_definition;
pub mod type_identity;
pub mod typed_trees;
pub mod types;
pub mod visibility;
pub mod wire;

pub use psi_language_semantics::declaration_selection::{
    AuthoredDeclarationSelection, AuthoredDeclarationSelectionExposure,
    AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionLateBinding,
    AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError,
    AuthoredDeclarationSelectionTarget, AuthoredDeclarationSelections,
    ResolvedAuthoredDeclarationSelection,
};
pub use snapshot::TypedTreesSnapshot;
pub use typed_trees::{
    PlanLaidBitField, PlanLaidBitFragment, PlanLaidIntegerField, PlanLaidLayout,
    PlanLaidRepeatedField, TypedTreeRoots, TypedTreeTables, TypedTrees,
};
