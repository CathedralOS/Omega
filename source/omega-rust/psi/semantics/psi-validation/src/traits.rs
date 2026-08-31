mod conformance;
mod data_conformance;
mod dynamic;
mod requirements;
mod shared;

pub use conformance::revalidate_top_level_requirement_realization;
pub(crate) use conformance::{
    GenericBoundRequirement, generic_bound_argument_matches, generic_bound_requirement_call,
    validate_external_leaf_native_shapes, validate_generic_conformance_bounds,
    validate_machine_trait_conformances, validate_trait_conformance_bounds,
};
pub(crate) use data_conformance::{arguments_for_declaring_trait, validate_conformances};
pub use dynamic::{
    DynamicConformanceSelection, collect_dynamic_conformance_selections,
    resolve_dynamic_call_targets,
};
pub(crate) use dynamic::{dynamic_requirement_call_error, dynamic_trait_symbol};
pub(crate) use requirements::validate_trait_requirements;
