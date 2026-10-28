//! Guard shared native stage sequencing without running another compiler.

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
    for forbidden in [
        "checked_trees::",
        "typed_trees::",
        "provider_planning::",
        "package_compilation::",
    ] {
        assert!(
            !source.contains(forbidden),
            "application preparation imports {forbidden}"
        );
    }
}

#[test]
fn component_calls_are_request_data_not_an_alternate_compile_entrance() {
    let compiler = include_str!("compiler.rs");
    let request = include_str!("request.rs");
    let exports = include_str!("lib.rs");
    assert_eq!(compiler.matches("pub fn compile(").count(), 1);
    assert_eq!(compiler.matches("pub fn ").count(), 1);
    assert!(compiler.contains("crate::components::emit_components("));
    assert!(compiler.contains("EmittedComponent::admit_call"));
    assert!(compiler.contains("EmittedComponent::into_product"));
    assert!(compiler.contains("with_component_products(component_products)"));
    assert!(request.contains("pub components:"));
    assert!(exports.contains("pub use compiler::compile;"));
    assert!(!exports.contains("emit_component_exports"));
    assert!(!exports.contains("ComponentNativeCall"));
}

#[test]
fn applications_and_callables_use_the_same_body_pipeline() {
    for caller in [
        include_str!("realization/object_emission.rs"),
        include_str!("realization/callable_objects.rs"),
    ] {
        assert!(caller.contains("pipeline::lower_to_target("));
        assert!(caller.contains("pipeline::emit_target("));
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
fn body_pipeline_exposes_the_ordered_representation_transforms() {
    let pipeline = include_str!("pipeline.rs");
    let mut remaining = pipeline;
    for stage in [
        "optimize_abstract_operations(",
        "lower_optimized_to_target_operations(",
        "stage_optimized_instruction_selection(",
        "optimize_selected_instructions(selected)",
        "stage_register_allocation(selected)",
        "stage_optimized_post_allocation_machine_plan(",
        "stage_function_frame(",
        "stage_optimized_layout_independent_selected_form_encoding(",
        "stage_optimized_resolved_selected_form_layout(",
        "execute_resolved_layout_optimization(",
        "assemble_fixed_frame_realization(",
        "emit_fragment_object(physical)",
    ] {
        assert_eq!(pipeline.matches(stage).count(), 1, "one call to {stage}");
        let position = remaining.find(stage).expect("stage follows its producer");
        remaining = &remaining[position + stage.len()..];
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
    assert!(backend.contains("stage_whole_function_exit_contract_for_layout("));
    assert!(backend.contains("expected_fixed_frame_manifest("));
}
