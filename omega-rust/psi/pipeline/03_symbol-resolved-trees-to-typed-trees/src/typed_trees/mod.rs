//! Psi-owned typed source representation.
//!
//! Start at `crate::typed_trees::TypedTrees`; its modules own the program's concepts:
//! declarations, control flow, calls with their signatures and boundaries, the
//! type system with closed numerics and type identities, evidence, and
//! inspection snapshots. Contracts, ownership, and proofs are checked later.

pub mod typed_trees;

pub use crate::typed_trees::typed_trees::calls::{
    boundary, dynamic_traits, finite_family, service, signature,
};
pub use crate::typed_trees::typed_trees::control_flow::{machine, state, statement};
pub use crate::typed_trees::typed_trees::declarations::{
    constant, data, domain, measure, operator, trait_definition, visibility, wire,
};
pub use crate::typed_trees::typed_trees::evidence::{
    byte_predicates, dependent_ranges, mathematical, proof_only, proposition, ranking,
};
pub use crate::typed_trees::typed_trees::inspection::snapshot;
pub use crate::typed_trees::typed_trees::names::{identity, name};
pub use crate::typed_trees::typed_trees::type_system::{closed_numeric, type_identity, types};
pub use crate::typed_trees::typed_trees::values::expression;

pub use crate::typed_trees::typed_trees::{
    PlanLaidBitField, PlanLaidBitFragment, PlanLaidIntegerField, PlanLaidLayout,
    PlanLaidRepeatedField, ProgramIdentity, TypedTreeRoots, TypedTreeTables, TypedTrees,
};
pub use language_semantics::declaration_selection::{
    AuthoredDeclarationSelection, AuthoredDeclarationSelectionExposure,
    AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionLateBinding,
    AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError,
    AuthoredDeclarationSelectionTarget, AuthoredDeclarationSelections,
    ResolvedAuthoredDeclarationSelection,
};
pub use snapshot::TypedTreesSnapshot;
