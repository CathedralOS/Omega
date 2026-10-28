//! Typed-to-typed elaboration before immutable checking.
//!
//! Start at preparation.rs: it owns specialization, exact call rebinding and
//! invocation admission before sealing the ordinary typed representation.
//! Readonly source judgments live in validation; this package owns mutation.
//! The 03b directory inserts this transform between typing and checking
//! without renumbering the existing downstream stages.

mod assembly_control;
mod assignment_destinations;
mod conformance;
mod constructed_results;
mod monomorphization;
mod operators;
mod preparation;

pub use preparation::{
    PreparationAdmission, PreparationRequest, PreparedProductRequirement, TypedPreparation,
    prepare_constant_execution, prepare_product_operand_application,
    prepare_product_requirement_operand, prepare_retained_product_application, prepare_typed_trees,
    specialize_static_machine_calls,
};
