pub(crate) mod representation_geometry;

/*
Semantic judgments for structural reference origins, placed loan regions, reborrow custody, and
reference access capabilities.
*/

mod boundary_results;
mod declared_results;
pub(crate) use boundary_results::provider_lifetime_debts;
pub use boundary_results::{
    boundary_descriptor_access, boundary_result_lifetimes, boundary_result_lifetimes_match,
    boundary_result_slots, validate_declared_boundary_reference_outputs,
    validate_provider_reference_outputs,
};
pub(crate) use declared_results::validate_result_lifetime_groups;
pub use declared_results::{
    DeclaredReferenceResult, callable_reference_result, declared_lifetime_path_is_static,
    public_reference_result, requirement_reference_result, selected_reference_result,
};

pub(crate) mod reference_completion;
pub(crate) mod reference_origins;
pub(crate) mod reference_storage_geometry;
pub(crate) mod reference_views;
pub(crate) mod resident_references;
