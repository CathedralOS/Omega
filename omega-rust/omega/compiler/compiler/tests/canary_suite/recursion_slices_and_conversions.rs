//! Fixtures shared by the recursion, slice and conversion canaries.

#[path = "recursion_slices_and_conversions/dynamic_indices_and_slice_transitions.rs"]
mod dynamic_indices_and_slice_transitions;
#[path = "../fixture_rosters/recursion_slices_and_conversions.rs"]
pub(super) mod fixture_roster;
#[path = "recursion_slices_and_conversions/loops_and_indexed_arrays.rs"]
mod loops_and_indexed_arrays;
#[path = "recursion_slices_and_conversions/numeric_conversions.rs"]
mod numeric_conversions;
#[path = "recursion_slices_and_conversions/recursion_and_declared_domains.rs"]
mod recursion_and_declared_domains;
#[path = "recursion_slices_and_conversions/string_concat_and_mutable_parameters.rs"]
mod string_concat_and_mutable_parameters;
#[path = "recursion_slices_and_conversions/subslice_parameters_and_ranges.rs"]
mod subslice_parameters_and_ranges;
#[path = "recursion_slices_and_conversions/wait_wake_boundary.rs"]
mod wait_wake_boundary;
