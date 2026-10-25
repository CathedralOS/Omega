//! Guard shared native stage sequencing without running another compiler.

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
        "stage_fixed_frame_function_relative_realization(",
        "emit_fragment_object(physical)",
    ] {
        assert_eq!(pipeline.matches(stage).count(), 1, "one call to {stage}");
        let position = remaining.find(stage).expect("stage follows its producer");
        remaining = &remaining[position + stage.len()..];
    }
}
