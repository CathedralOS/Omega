//! Every integration test of this crate as one binary: each topic file is a module here,
//! shared support modules are declared once, and `cargo nextest run -p <crate> --test suite`
//! runs them all. A new `tests/<topic>.rs` joins by one `mod` line below.

mod assembly_value_scope;
mod cast_result_ranges;
mod cast_result_types;
mod cleanup_ownership;
mod declared_operator_results;
mod evaluated_via;
mod float_operator_meaning;
mod integer_call_carriers;
mod integer_call_result_bounds;
mod literal_destinations;
mod local_dynamic_coercions;
mod match_values;
mod native_carrier_identity;
mod nested_initializer_calls;
mod program_validation;
mod quotient_terminal_bridge;
mod rational_float_destinations;
mod reference_field_types;
mod relevance;
mod resolved_receiver_calls;
mod selective_value_calls;
mod state_value_scope;
mod trait_machine_identity;
