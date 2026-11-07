//! Source guards for shared native stage sequencing.
//!
//! The order of the representation transforms is not repeated here: each stage
//! consumes the previous stage's product, so `pipeline.rs` cannot run them out
//! of order and still type-check.

#[test]
fn terminal_application_preparation_owns_source_free_admission_order() {
    let source = include_str!("application.rs");
    let mut remaining = source;
    for operation in [
        "project_terminal_native_realization_proposal(&self.artifact",
        "if self.physical_entry_adapter",
        ".bind_physical_adapter(&self.artifact)",
        "RetainedTerminalArtifact::new_with_native_realization_proposal(",
        "proposal.with_target_provider_proposals(self.provider_proposals)",
    ] {
        let position = remaining
            .find(operation)
            .expect("application preparation order");
        remaining = &remaining[position + operation.len()..];
    }
    let validation = source
        .find("validate_package_terminal_authority_permission_custody(permissions.iter()")
        .unwrap();
    let construction = source
        .find("terminal_authority_permission_policy_with_rows(permissions, identities)")
        .unwrap();
    assert!(validation < construction);
}

#[test]
fn applications_and_callables_use_the_same_body_pipeline() {
    for caller in [
        include_str!("realization/object_emission.rs"),
        include_str!("realization/callable_objects.rs"),
    ] {
        for stage in [
            "optimize_abstract_operations(",
            "lower_optimized_to_target_operations(",
            "stage_optimized_instruction_selection(",
            "emit_fragment_object(",
        ] {
            assert!(!caller.contains(stage), "caller assembles {stage}");
        }
    }
}

#[test]
fn backend_joins_layout_products_without_driving_compiler_stages() {
    let backend =
        include_str!("../../backend/machine-emission/src/function_realization/fixed_frame.rs");
    for stage in [
        "stage_optimized_layout_independent_selected_form_encoding(",
        "stage_optimized_resolved_selected_form_layout(",
        "execute_resolved_layout_optimization(",
    ] {
        assert!(
            !backend.contains(stage),
            "backend assembles compiler stage {stage}"
        );
    }
}
