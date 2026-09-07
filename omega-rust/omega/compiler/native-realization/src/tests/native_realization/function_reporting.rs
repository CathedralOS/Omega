//! Function reports describe current selected spans and frame bytes, not a
//! second proof or an ISA-instruction count.

use std::sync::Arc;

use optimization_core::OptimizationSelections;
use target::NativeTarget;

fn publish(
    source: &str,
    target: NativeTarget,
) -> (
    Arc<object_file::StagedOptimizedRelocationFreeObjectContainer>,
    image_emission::ObjectArtifact,
) {
    let checked = crate::tests::fixtures::checked_source::checked(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main")
        .expect("lower checked source");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
    let input = terminal_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        &semantic,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("verified abstract input");
    let selections = OptimizationSelections::default();
    let optimized = crate::optimize_verified_abstract_input(
        input,
        crate::compiler_baseline_request_v1(&selections),
    )
    .expect("ordinary abstract optimization");
    let targeted =
        abstract_operations_to_target_operations::lower_validated_abstract_to_target_operations(
            optimized,
            target,
            &[],
            None,
            &[],
            &[],
        )
        .expect("ordinary target lowering");
    let physical = crate::stage_optimized_verified_physical_pipeline(
        targeted,
        selections.project_post_terminal().selections(),
    )
    .expect("ordinary physical pipeline");
    let emitted = machine_emission::stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .expect("selected fragments");
    let applied = machine_emission::stage_function_fragment_frame_application(emitted)
        .expect("applied frame");
    let text =
        machine_emission::stage_optimized_fixed_frame_text_section(applied).expect("placed text");
    let source = Arc::new(
        object_file::stage_optimized_relocation_free_object_container(text)
            .expect("object container"),
    );
    let object = image_emission::build_function_fragment_object_artifact(Arc::clone(&source))
        .expect("compiler object");
    (source, object)
}

const CALL_SOURCE: &str = r#"
    data Main {}
    machine Main::identity(value: u64) -> u64
    requires value == value
    ensures result == value
    { transition { _ -> value } }
    machine Main::main() { let result: u64 = Main::identity(72623859790382856u64); }
"#;

#[test]
fn current_function_reports_cover_selected_spans_and_inserted_frames_on_every_hosted_target() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let (source, object) = publish(CALL_SOURCE, target);
        let image = image_emission::emit_executable_image(&object, 3).expect("final image");
        image_emission::validate_executable_image(&object, &image).expect("independent replay");
        let report = image
            .output()
            .compiler_function_validation
            .expect("current function report");
        let applied = source.source().source().application();
        let spans: Vec<_> = applied
            .fragments
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .collect();
        assert_eq!(report.function_count, 2);
        assert_eq!(report.instruction_count, spans.len());
        assert_eq!(
            report.zero_width_instruction_count,
            spans.iter().filter(|row| row.bytes.is_empty()).count()
        );
        let prologue_bytes: u64 = applied
            .functions
            .iter()
            .map(|row| row.prologue_byte_count)
            .sum();
        let epilogue_bytes: u64 = applied
            .functions
            .iter()
            .flat_map(|row| &row.epilogues)
            .map(|row| row.byte_count)
            .sum();
        assert!(
            prologue_bytes > 0 && epilogue_bytes > 0,
            "nonleaf frames are exercised: {target:?}"
        );
        assert_eq!(report.frame_prologue_byte_count as u64, prologue_bytes);
        assert_eq!(report.frame_epilogue_byte_count as u64, epilogue_bytes);
        if target.architecture == target::Architecture::Aarch64 {
            assert!(
                spans.iter().any(|row| row.bytes.len() > 4),
                "multi-ISA-instruction selected materialization is exercised"
            );
        }
        assert_eq!(report.boundary_contract_report_fingerprint, None);
        assert_eq!(
            report.final_region_binding_report_fingerprint,
            image
                .output()
                .executable_regions
                .inventory_report_fingerprint
        );
    }
}

#[test]
fn current_function_report_requires_source_replay_and_rejects_reauthenticated_summary_changes() {
    let (_, object) = publish(CALL_SOURCE, NativeTarget::linux_x64());
    let image = image_emission::emit_executable_image(&object, 3).unwrap();
    let original = image.output().compiler_function_validation.unwrap();
    let mut missing = object.clone();
    missing.clear_fragment_replay_for_test();
    assert!(image_emission::validate_executable_image(&missing, &image).is_err());
    // A mechanical image without compiler source must not recover its report
    // from otherwise matching functions or machine-code bytes.
    assert!(
        image_emission::emit_executable_image(&missing, 3)
            .unwrap()
            .output()
            .compiler_function_validation
            .is_none()
    );

    for field in 0..11 {
        let mut changed = image.clone();
        let report = changed
            .output_mut_for_test()
            .compiler_function_validation
            .as_mut()
            .unwrap();
        match field {
            0 => report.function_count += 1,
            1 => report.instruction_count += 1,
            2 => report.zero_width_instruction_count += 1,
            3 => report.frame_prologue_byte_count += 1,
            4 => report.frame_epilogue_byte_count += 1,
            5 => report.fragment_manifest_report_fingerprint ^= 1,
            6 => report.frame_application_report_fingerprint ^= 1,
            7 => report.final_region_binding_report_fingerprint ^= 1,
            8 => report.final_text_validation_report_fingerprint ^= 1,
            9 => report.boundary_contract_report_fingerprint = Some(0),
            _ => {
                changed.output_mut_for_test().compiler_function_validation = None;
            }
        }
        if let Some(report) = changed.output().compiler_function_validation {
            assert_ne!(report.evidence_digest(), original.evidence_digest());
            let _ = report.evidence_report_fingerprint();
        }
        assert!(
            image_emission::validate_executable_image(&object, &changed).is_err(),
            "field {field} must be independently rederived"
        );
    }
    let mut changed = object.clone();
    changed.text_bytes_mut_for_test()[0] ^= 1;
    assert!(image_emission::validate_executable_image(&changed, &image).is_err());
    let mut changed = object.clone();
    changed.functions_mut_for_test()[0].byte_count += 1;
    assert!(image_emission::validate_executable_image(&changed, &image).is_err());
}
