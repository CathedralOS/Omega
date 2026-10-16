#![forbid(unsafe_code)]
#![allow(
    clippy::obfuscated_if_else,
    reason = "valid-handle copy expressions use one uniform then/fallback idiom"
)]

//! Psi-owned typed source representation.
//!
//! Start at `typed_trees::TypedTrees`; its modules own the program's concepts:
//! declarations, control flow, calls with their signatures and boundaries, the
//! type system with closed numerics and type identities, evidence, and
//! inspection snapshots. Contracts, ownership, and proofs are checked later.

pub mod references;
pub mod typed_trees;

pub use typed_trees::calls::{
    boundary, dynamic_traits, finite_family, service, signature, static_applications,
};
pub use typed_trees::constant_recipes;
pub use typed_trees::control_flow::{machine, state, statement};
pub use typed_trees::declaration_index;
pub use typed_trees::declarations::{
    boundary_completions, calling_vocabulary, constant, data, domain, invocations, measure,
    operator, product_operand, trait_definition, visibility, wire,
};
pub use typed_trees::evidence::{
    byte_predicates, dependent_ranges, mathematical, proof_only, proposition, ranking,
};
pub use typed_trees::inspection::snapshot;
pub use typed_trees::names::{identity, name};
pub use typed_trees::sum_reachability::{
    RecordSumProfile, SumReachability, SumTraversalFault, record_sum_profile,
};
pub use typed_trees::type_system::{closed_numeric, type_identity, types};
pub use typed_trees::values::const_integer;
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
    BoundaryCompletionDeclaration, FrozenTypedTrees, PlacedScalarAccessorDeclaration,
    PlacedScalarAccessorOperation, PlanLaidBitField, PlanLaidBitFragment, PlanLaidGeometry,
    PlanLaidIntegerField, PlanLaidLayout, PlanLaidRealizations, PlanLaidRepeatedField,
    PlanLaidSchemaArgument, PlanLaidSchemaField, PlanLaidSchemaFieldKind, PlanLaidTargetLayout,
    ProgramIdentity, TypedTreeRoots, TypedTreeTables, TypedTrees,
    closed_conformance_applications_of, machine_specialization_matches_template_identity,
};
