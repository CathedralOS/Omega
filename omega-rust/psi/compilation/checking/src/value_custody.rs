//! Validating values and their custody: literals, constants, data and
//! struct literals, places and placed views, recasts, borrows, locals,
//! cleanup, expression and reference types, wire and intrinsic boundaries.

pub(crate) mod activation_storage;
pub(crate) mod atomic_fences;
pub(crate) mod atomic_operations;
pub(crate) mod claim_frontier;
pub(crate) mod cleanup;
pub(crate) mod const_expressions;
pub(crate) mod constants;
pub(crate) mod content_classes;
pub(crate) mod content_conservation;
pub(crate) mod content_projections;
pub(crate) mod data;
pub(crate) mod destructure;
pub(crate) mod dispatch_extraction;
mod executable_value_occurrences;
pub(crate) mod expression_types;
pub mod field_domains;
pub use executable_value_occurrences::{
    collect_executable_value_origins, visit_executable_value_occurrences,
    visit_statement_value_occurrences,
};
pub(crate) mod intrinsic_boundaries;
pub(crate) mod literals;
pub(crate) mod locals;
pub(crate) mod owned_value_source;
pub(crate) mod partial_affine_residuals;
pub(crate) mod permission_provenance;
pub(crate) mod placed_views;
pub use placed_views::placed_atomic_observing_results;
pub(crate) mod places;
pub(crate) mod plan_laid;
pub(crate) mod qualification_casts;
pub(crate) mod recasts;
pub(crate) mod receiving_values;
pub(crate) mod reference_access;
pub(crate) mod reference_local_establishment;
pub(crate) mod reference_shells;
pub(crate) mod resident_content;
pub(crate) mod scalar_case_constructor;
pub(crate) mod scalar_representation_range;
pub(crate) mod source_liveness;
pub(crate) mod storage_contents;
pub(crate) mod struct_literals;
pub(crate) mod structural_identity;
pub(crate) mod transition_operand_moves;
pub(crate) mod type_properties;
pub(crate) mod type_references;
pub(crate) mod type_shapes;
pub(crate) mod wire;
pub(crate) mod write_only_borrows;

pub mod completed_storage;

pub(crate) mod static_inline_storage;
