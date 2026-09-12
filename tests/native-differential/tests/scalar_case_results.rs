//! Ordinary scalar-sum returns retain tags, full-width payloads, and caller storage.
use native_realization::{compiler_baseline_request_v1, optimize_artifact_sections};
use optimization_core::OptimizationSelections;
use proof_admission::AdmissionProfile;
use target::NativeTarget;
use terminal_codec::CanonicalTerminalArtifact;

#[path = "scalar_case_results/admission.rs"]
mod admission;

#[path = "scalar_case_results/source_custody.rs"]
mod source_custody;

#[path = "scalar_case_results/owned_state.rs"]
mod owned_state;

#[path = "scalar_case_results/membership.rs"]
mod membership;

#[path = "scalar_case_results/package_membership.rs"]
mod package_membership;

#[path = "scalar_case_results/owned_selection.rs"]
mod owned_selection;
#[path = "scalar_case_results/records.rs"]
mod records;

#[cfg(any(
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(target_os = "macos", target_arch = "aarch64")
))]
#[path = "common/native_function.rs"]
#[allow(dead_code)]
mod native_function;

fn produce(entry: &str) -> CanonicalTerminalArtifact {
    produce_source(entry, include_str!("scalar_case_results/choose.omg"))
}
fn produce_source(entry: &str, source: &str) -> CanonicalTerminalArtifact {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize scalar-case source");
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse scalar-case source");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("resolve scalar-case source");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type scalar-case source");
    let checked =
        typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check scalar-case source");
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, entry)
        .produce_artifact()
        .expect("publish scalar-case Terminal");
    let artifact = CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap();
    terminal_verifier::verify_module(
        &terminal_codec::decode_module(artifact.semantic_bytes()).unwrap(),
        &terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap(),
        &AdmissionProfile::default(),
    )
    .expect("independently verify reloaded scalar-case Terminal");
    artifact
}

fn publish(
    artifact: &CanonicalTerminalArtifact,
    target: NativeTarget,
) -> (image_emission::ExecutableImage, usize) {
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let selections = OptimizationSelections::new([]).unwrap();
    let optimized = optimize_artifact_sections(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &AdmissionProfile::default(),
        compiler_baseline_request_v1(&selections),
    )
    .unwrap_or_else(|error| panic!("admit scalar-case source on {target:?}: {error:#?}"));
    let post_terminal = optimized.selections().project_post_terminal();
    let target_operations =
        abstract_operations_to_target_operations::lower_optimized_to_target_operations(
            optimized, target,
        )
        .unwrap_or_else(|error| panic!("lower scalar-case source on {target:?}: {error:#?}"));
    let physical = native_realization::stage_optimized_verified_physical_pipeline(
        target_operations,
        post_terminal.selections(),
    )
    .unwrap_or_else(|error| panic!("select scalar-case source on {target:?}: {error:#?}"));
    let fragments = machine_emission::stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .unwrap();
    let framed = machine_emission::stage_function_fragment_frame_application(fragments).unwrap();
    let text = machine_emission::stage_optimized_fixed_frame_text_section(framed).unwrap();
    machine_emission::validate_optimized_fixed_frame_text_section(&text).unwrap();
    let source = std::sync::Arc::new(
        object_file::stage_optimized_relocation_free_object_container(text).unwrap(),
    );
    let object = image_emission::build_function_fragment_object_artifact(source.clone()).unwrap();
    image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
    assert_eq!(object.functions().len(), module.machines.len());
    let entry = object.entry_function().text_offset;
    let mut stripped = object.clone();
    stripped.clear_fragment_replay_for_test();
    assert!(
        image_emission::emit_executable_image(&stripped, 3).is_err(),
        "aggregate result publication requires its retained physical replay"
    );
    let image = image_emission::emit_executable_image(&object, 3).unwrap();
    image_emission::validate_executable_image(&object, &image).unwrap();
    let record = image_emission::build_installation_record(
        &image,
        semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
    )
    .unwrap();
    let bytes = image_emission::encode_installation_record(&record).unwrap();
    let decoded = image_emission::decode_installation_record(&bytes).unwrap();
    assert_eq!(
        image_emission::encode_installation_record(&decoded).unwrap(),
        bytes
    );
    image_emission::validate_installation_record(&decoded, &image).unwrap();
    if let Some(result_machine) = module
        .machines
        .iter()
        .find(|machine| machine.result.structural().is_some())
    {
        admission::installation_cannot_change_call_or_result(&decoded, &image, result_machine.id);
    } else {
        // Local construction has no function-result ABI. Keep the call/result
        // corruption controls required for every actual returning fixture.
        assert!(decoded.functions().iter().all(|function| {
            function
                .parameter_abi
                .as_ref()
                .is_none_or(|abi| abi.call_plan.result.is_none())
        }));
    }
    assert_eq!(
        image_emission::derive_stack_demand(&object, module.entry).unwrap(),
        image_emission::derive_installation_stack_demand(&decoded, &image, module.entry).unwrap(),
    );
    (image, entry)
}

#[test]
fn source_scalar_case_results_reach_canonical_terminal() {
    for entry in ["choose", "collect"] {
        let _ = produce(entry);
    }
}

#[test]
fn scalar_case_returns_and_calls_publish_on_direct_aggregate_targets() {
    for entry in ["choose", "collect"] {
        let artifact = produce(entry);
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
        ] {
            publish(&artifact, target);
        }
    }
}

#[test]
fn scalar_case_runtime_preserves_every_tag_and_full_width_payload() {
    assert_host("choose", include_str!("scalar_case_results/choose.c"));
}

#[test]
fn scalar_case_call_dispatch_writes_original_borrowed_storage() {
    assert_host("collect", include_str!("scalar_case_results/collect.c"));
}

#[test]
fn scalar_case_borrowed_callee_loop_preserves_storage_and_result() {
    let artifact = produce_source("collect", include_str!("scalar_case_results/borrowed.omg"));
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let (image, offset) = publish(&artifact, target);
        #[cfg(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(target_os = "macos", target_arch = "aarch64")
        ))]
        if target == NativeTarget::host() {
            let driver = format!(
                "#define CALLEE_FILLS_VIEW 1\n{}",
                include_str!("scalar_case_results/collect.c")
            );
            native_function::assert_c_text(&image.output().final_text_bytes, offset, &driver);
        }
        #[cfg(not(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(target_os = "macos", target_arch = "aarch64")
        )))]
        {
            let _ = (image, offset);
            eprintln!(
                "SKIP: borrowed aggregate-result runtime requires a matching direct-return host; cross-target publication was checked"
            );
        }
    }
}

fn assert_host(entry: &str, driver: &str) {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    {
        let (image, offset) = publish(&produce(entry), NativeTarget::host());
        native_function::assert_c_text(&image.output().final_text_bytes, offset, driver);
    }
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    {
        let _ = (entry, driver);
        eprintln!(
            "SKIP: direct aggregate native runtime requires Linux x64/ARM64 or macOS ARM64; Windows indirect return remains unsupported"
        );
    }
}
