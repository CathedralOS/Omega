//! Every integration test of this crate as one binary: each topic file is a module here,
//! shared support modules are declared once, and `cargo nextest run -p <crate> --test suite`
//! runs them all. A new `tests/<topic>.rs` joins by one `mod` line below.

mod borrowed_storage_windows;
mod byte_sequence_literal;
mod dynamic_descriptor_join;
mod dynamic_dispatch;
mod natural_native;
mod partial_affine_call_results;
mod primitive_locals;
mod proof_section_admission;
mod provider_installation;
mod ranked_native;
mod scalar_affine_cleanup;
mod scalar_array_construction;
mod scalar_boundary_arguments;
mod structural_byte_sequence_index;
mod structural_byte_sequence_store;
mod structural_leaf_copy;
mod structural_scalar_fields;
mod write_only_primitive_store;
