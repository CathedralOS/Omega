#![forbid(unsafe_code)]
#![allow(
    clippy::obfuscated_if_else,
    reason = "valid-handle copy expressions use one uniform then/fallback idiom"
)]

//! Psi-owned typed source representation.
//!
//! Start at [`typed_trees::TypedTrees`]; its modules own the program's concepts.

pub mod typed_trees;

pub use typed_trees::calls::{boundary, dynamic_traits, service, signature};
pub use typed_trees::control_flow::{machine, state, statement};
pub use typed_trees::declarations::{
    constant, data, domain, measure, operator, trait_definition, visibility, wire,
};
pub use typed_trees::evidence::{
    byte_predicates, dependent_ranges, proof_only, proposition, ranking,
};
pub use typed_trees::inspection::snapshot;
pub use typed_trees::names::{identity, name};
pub use typed_trees::type_system::{type_identity, types};
pub use typed_trees::values::expression;

pub use language_semantics::declaration_selection::{
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
