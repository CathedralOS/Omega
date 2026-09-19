//! Every integration test of this crate as one binary: each topic file is a module here,
//! shared support modules are declared once, and `cargo nextest run -p <crate> --test suite`
//! runs them all. A new `tests/<topic>.rs` joins by one `mod` line below.

mod anonymous_array_landing;
mod anonymous_integer_landing;
mod anonymous_window_landing;
mod borrowed_restoration;
mod borrowed_subslices;
mod build_arguments;
mod const_values;
mod constant_array_projection;
mod field_ranking_arrivals;
mod projected_results;
mod raw_literal_bytes;
mod resolved_state_execution;
mod runtime_value_generics;
mod structural_equality;
mod token_bound_machine_calls;
mod trait_operators;
mod unsigned_ranking_views;
mod value_dispatch;
