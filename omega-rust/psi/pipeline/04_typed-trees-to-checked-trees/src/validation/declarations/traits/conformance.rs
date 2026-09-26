//!
//! `bounds.rs` checks conformance bounds, `machine_conformance.rs` validates
//! each machine's conformances, `trait_applications.rs` their application
//! obligations, `signature_matching.rs` the signature match and
//! `external_shapes.rs` the external native shapes; `generic_operator.rs`
//! stays as it was.

mod bounds;
mod external_shapes;
mod generic_operator;
mod machine_conformance;
mod signature_matching;
#[cfg(test)]
mod tests;
mod trait_applications;

pub(crate) use bounds::{
    GenericBoundRequirement, generic_bound_argument_matches, generic_bound_requirement_call,
    named_conformance_argument_matches, validate_generic_conformance_bounds,
    validate_trait_conformance_bounds,
};
pub(crate) use external_shapes::{
    validate_external_leaf_native_shapes, validate_external_via_expression,
};
pub use generic_operator::generic_bound_operator_requirement;
pub(crate) use machine_conformance::validate_machine_trait_conformances;
pub use machine_conformance::{
    compose_forwarded_trait_arguments, revalidate_top_level_requirement_realization,
};
pub(crate) use signature_matching::validate_machine_state_satisfies_trait_signature_with_arguments;
pub(crate) use trait_applications::validate_trait_application_obligations;
