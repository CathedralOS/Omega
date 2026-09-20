//! Every integration test of this crate as one binary: each topic file is a module here,
//! shared support modules are declared once, and `cargo nextest run -p <crate> --test suite`
//! runs them all. A new `tests/<topic>.rs` joins by one `mod` line below.

mod capability_conflicts;

mod build_named_inputs;
mod candidate_policy_retention;
mod dependency_generated_sources;
mod dependency_purposes;
mod locked_source_checking;
mod locked_source_recovery;
mod named_workspace_install;
mod offline_package_commands;
mod opaque_boundary_agreement;
mod package_capability_conflicts;
mod package_evidence_fixtures;
mod package_inspection;
mod package_lineage_spoofing;
mod package_policy_changes;
mod package_reconstruction_question;
mod provider_selection_conflicts;
mod remote_fixtures;
mod repository_build_declarations;
mod semantic_binding_review;
mod source_closure_subject;
mod source_closure_text;
mod source_diff_commands;
mod standard_library_package_resolution;
mod symbolic_boundary_application_closure;
mod token_binding_revision;
