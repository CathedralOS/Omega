//! Every integration test of this crate as one binary: each topic file is a module here,
//! shared support modules are declared once, and `cargo nextest run -p <crate> --test suite`
//! runs them all. A new `tests/<topic>.rs` joins by one `mod` line below.

// The integer prover runs scoped worker threads that allocate heavily; the
// macOS system allocator serializes them on its zone locks, and mimalloc's
// per-thread heaps do not. Test-only: the compiler binaries keep the system
// allocator.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;
// The front-end pipeline these tests run, shared with the crate's unit tests
// through `src/lib.rs`; see its module documentation.
#[path = "support/front_end.rs"]
mod front_end;

mod affine_result_calls;
mod affine_return_source;
mod anonymous_integer_landing;
mod atomic_access_events;
mod boolean_entry_requirements;
mod borrowed_computation_arguments;
mod borrowed_computation_custody;
mod borrowed_primitive_graph_reads;
mod borrowed_scalar_call_source;
mod borrowed_scalar_root;
mod boundary_result_domain_calls;
mod bounded_owner_locals;
mod byte_call_store_schedule;
mod byte_sequence_forwarding;
mod byte_sequence_length;
mod byte_sequence_read;
mod byte_sequence_subslice;
mod byte_write_parameter_roots;
mod call_operand_source_custody;
mod call_premise_terminal_boundary;
mod callable_multistate_unit;
mod case_guard_source;
mod case_payload_guard_observation;
mod case_payload_path_boundaries;
mod case_payload_record_member_read;
mod composed_call_admission;
mod composed_call_arguments_source;
mod composed_internal_call_arguments_source;
mod composed_unit_scalar_crashes_source;
mod constructed_field_replacement_source;
mod copied_call_operands;
mod crash_member_source;
mod direct_primitive_store_source;
mod entry_requirement_crash_coverage;
mod enum_view_reference_source;
mod evidence_identity_source;
mod exact_add_conjunction_source;
mod exact_affine_sibling_source;
mod exact_literal_division_source;
mod exact_shift_left_certificate_source;
mod field_rhs_computation_source;
mod float_entry_ranges_source;
mod general_partial_affine;
mod guarded_scalar_returns_source;
mod guarded_unit_completions;
mod integer_entry_ranges_source;
mod integer_policy_realization;
mod local_record_receivers_source;
mod local_source_ordinals;
mod mixed_case_establishment;
mod mixed_shift_source;
mod mixed_unit_crash_arguments_source;
mod mixed_unit_crash_requirements_source;
mod module_namespaces;
mod mutable_scalar_parameters;
mod nested_boundary_applications_source;
mod nested_call_arguments_source;
mod nested_custody;
mod nested_initializer_arguments_source;
mod nested_member_store_calls;
mod nested_scalar_store_calls;
mod nominal_affine_source;
mod owned_byte_literal_returns;
mod owned_local_element_reads;
mod owned_match_state_residuals;
mod owned_record_byte_field_read;
mod owned_record_return_source;
mod owned_scalar_cycles;
mod owned_scalar_graphs;
mod partial_affine_result_source;
mod partial_affine_source;
mod payloadless_case_return_source;
mod policy_cast_conversions_source;
mod post_state_guarantee_clauses;
mod primitive_local_borrow_custody;
mod primitive_local_unit_calls;
mod primitive_store_parameter_roots;
mod primitive_store_return_source;
mod program_local_root_introduction;
mod projected_case_observation;
mod provider_attachment_source;
mod provider_array_results;
mod provider_candidates;
mod provider_results;
mod receiver_call_source;
mod receiver_scalar_store_source;
mod receiver_state_graph_forwarding;
mod reference_result_source;
mod registered_callback_lifetime;
mod result_signature_borrowed_byte_view;
mod retained_borrow_custody;
mod retained_call_positions;
mod returned_record_case_field_store;
mod scalar_array_source;
mod scalar_boundary_arguments;
mod scalar_call_crash_substitution;
mod scalar_control_contracts;
mod scalar_empty_contract_custody;
mod scalar_graph_byte_view_parameter;
mod scalar_local_borrows;
mod scalar_return_calls_source;
mod scalar_storage_source;
mod scalar_unit_calls;
mod short_circuit_field_stores;
mod signed_wrapping_conversion_values;
mod slice_record_copy_source;
mod state_local_record_lifetimes;
mod structural_array_literals;
mod structural_return_source;
mod transition_structural_return;
mod trapping_operation_sites;
mod unit_boolean_requirements_source;
mod unit_plan_omissions;
mod unit_scalar_result_source;
mod unit_state_graph;
mod unit_tail_calls_source;
mod value_dispatch;

mod view_arrival_producers;
mod view_locals;

mod unit_local_refinements;
mod window_parameter_coordinates;
