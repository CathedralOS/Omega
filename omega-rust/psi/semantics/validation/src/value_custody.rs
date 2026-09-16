//! Validating values and their custody: literals, constants, data and
//! struct literals, places and placed views, recasts, borrows, locals,
//! cleanup, expression and reference types, wire and intrinsic boundaries.

pub(crate) mod atomic_operations;
pub(crate) mod cleanup;
pub(crate) mod constants;
pub(crate) mod content_conservation;
pub(crate) mod content_projections;
pub(crate) mod data;
pub(crate) mod destructure;
pub(crate) mod expression_types;
pub(crate) mod intrinsic_boundaries;
pub(crate) mod literals;
pub(crate) mod locals;
pub(crate) mod owned_value_source;
pub(crate) mod permission_provenance;
pub(crate) mod placed_views;
pub(crate) mod places;
pub(crate) mod plan_laid;
pub(crate) mod recasts;
pub(crate) mod record_local_disposition;
pub(crate) mod scalar_case_constructor;
pub(crate) mod scalar_representation_range;
pub(crate) mod storage_contents;
pub(crate) mod struct_literals;
pub(crate) mod type_references;
pub(crate) mod wire;
pub(crate) mod write_only_borrows;
