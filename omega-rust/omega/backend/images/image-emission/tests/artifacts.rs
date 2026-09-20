//! Fixtures shared by the artifact tests: object and installation plans,
//! unit call accounts, cleanup plans and identities.
//! `provider_and_call_plans.rs`, `scalar_plans.rs` and
//! `dynamic_and_cleanup_plans.rs` hold the plan fixtures; this file keeps the
//! progress acceptance and the identity helpers.

#[path = "artifacts/acyclic_control_flow.rs"]
mod acyclic_control_flow;
#[path = "artifacts/dynamic_and_cleanup_plans.rs"]
mod dynamic_and_cleanup_plans;
#[path = "artifacts/fragment_container.rs"]
mod fragment_container;
#[path = "artifacts/hosted_exit_runtime.rs"]
mod hosted_exit_runtime;
#[path = "artifacts/hosted_receiver.rs"]
mod hosted_receiver;
#[path = "artifacts/installation_field_substitution_fields.rs"]
mod installation_field_substitution_fields;
#[path = "artifacts/installation_field_substitutions.rs"]
mod installation_field_substitutions;
#[path = "artifacts/installation_function_nested_custody.rs"]
mod installation_function_nested_custody;
#[path = "artifacts/installation_records.rs"]
mod installation_records;
#[path = "artifacts/installed_artifact.rs"]
mod installed_artifact;
#[path = "artifacts/macho_fixups.rs"]
mod macho_fixups;
#[path = "artifacts/macho_storage.rs"]
mod macho_storage;
#[path = "artifacts/object_custody.rs"]
mod object_custody;
#[path = "artifacts/object_replays.rs"]
mod object_replays;
#[path = "artifacts/provider_and_call_plans.rs"]
mod provider_and_call_plans;
#[path = "artifacts/provider_execution.rs"]
mod provider_execution;
#[path = "artifacts/requested_executable_route.rs"]
mod requested_executable_route;
#[path = "artifacts/scalar_call_reference.rs"]
mod scalar_call_reference;
#[path = "artifacts/scalar_plans.rs"]
mod scalar_plans;

use dynamic_and_cleanup_plans::{
    add_empty_unit_cleanup, continuation_unit_call_plan, dynamic_conformance_table_plan,
    dynamic_parameter_call_plan, edge_owned_cleanup_plan, forwarded_dynamic_descriptor_call_plan,
    forwarded_dynamic_parameter_call_plan, mixed_edge_owned_cleanup_plan, stored_dynamic_call_plan,
    structural_call_scalar_return_plan, two_call_edge_owned_cleanup_plan,
};
use provider_and_call_plans::{
    WriteExitProvider, admitted_x86_fma_provider, artifact_symbol, callback_private_plan,
    internal_call_plan, linux_foreign_call_plan, linux_write_line_exit_plan, port_effect_plan,
    refresh_x86_fma_identity, structural_return_plan, two_function_plan, windows_foreign_call_plan,
    x86_fma_plan,
};
use scalar_plans::{
    aarch64_words, account_aarch64_unit_call, account_x86_unit_call, conditional_tree,
    insert_aarch64_word, integer_return, promote_x86_cleanup_to_scalar, scalar_acyclic_plan,
    scalar_call_plan, scalar_conditional_call_plan, scalar_expression_condition_call_plan,
    scalar_expression_two_return_conditional_plan, scalar_mutation, scalar_three_leaf_cleanup_plan,
    scalar_two_return_conditional_plan,
};

use installation_evidence::ComponentProgressAcceptanceEvidence;
use semantic_vocabulary::{EdgeId, MachineId, OperationId};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

#[derive(Debug)]
struct TestComponentProgressAcceptance {
    manifest: u64,
    acceptance: u64,
}

impl ComponentProgressAcceptanceEvidence for TestComponentProgressAcceptance {
    fn component_progress_manifest_identity(&self) -> u64 {
        self.manifest
    }

    fn component_progress_acceptance_identity(&self) -> u64 {
        self.acceptance
    }
}

fn machine_id(raw: u64) -> MachineId {
    MachineId::new(raw).expect("machine")
}

fn operation_id(raw: u64) -> OperationId {
    OperationId::new(raw).expect("operation")
}

fn edge_id(raw: u64) -> EdgeId {
    EdgeId::new(raw).expect("edge")
}

fn identity() -> TerminalPsiIdentity {
    TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::CURRENT,
        program_fingerprint: SemanticFingerprint::from_bytes([9; 32]),
    }
}
