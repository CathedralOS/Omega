//! A boundary result and guarded mutable view compose through ordinary edges.

use super::*;
use abstract_operations_to_target_operations::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement,
};

fn reader() -> lowered_psi::LoweredPsi {
    let tokens = source_files_to_tokens::Lexer::new(include_str!("byte_input.omg"))
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "classify_bytes")
        .expect("read result and guarded destination share the ordinary state graph");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .unwrap();
    lowered
}

#[test]
fn bounded_byte_input_cycle_keeps_case_edges_in_replay() {
    use abstract_operations_to_abstract_operations::validation::{
        validate_psi_cycle_component_snapshot, validate_verified_psi_cycle_components,
    };
    let lowered = reader();
    let input = terminal_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        &terminal_codec::encode_module(&lowered.semantic_module).unwrap(),
        &terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    let verified = terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap();
    let custody = validate_verified_psi_cycle_components(&verified).unwrap();
    assert_eq!(custody.components().len(), 1);
    assert!(custody.ranking_certificates().certificates().is_empty());
    for edge in lowered.semantic_module.machines.iter().flat_map(|machine| {
        machine
            .blocks
            .iter()
            .flat_map(|block| match &block.terminator {
                terminal_psi::Terminator::StructuralCase { cases, .. } => {
                    cases.iter().map(|case| case.edge).collect::<Vec<_>>()
                }
                _ => Vec::new(),
            })
    }) {
        let mut missing = custody.snapshot().clone();
        let mut removed = false;
        for component in &mut missing.components {
            for edges in [&mut component.id.internal_edges, &mut component.exits] {
                edges.retain(|candidate| {
                    if candidate.edge == edge {
                        removed = true;
                        false
                    } else {
                        true
                    }
                });
            }
        }
        assert!(removed, "each case edge belongs to the cycle or its exits");
        assert!(
            validate_psi_cycle_component_snapshot(verified.input(), verified.unit(), &missing)
                .is_err(),
            "omitted case edge cannot retain component authority"
        );
    }
}

fn publish(target: NativeTarget) -> (image_emission::ExecutableImage, usize) {
    let lowered = reader();
    let [boundary] = lowered.semantic_module.boundary_machines.as_slice() else {
        panic!("one authored input leaf");
    };
    // Explicit test-owned settlement, not package acceptance inferred by name.
    let text = calls::stage_call_text_with_settlements(
        target,
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &[AdmittedBoundarySettlement {
            boundary: boundary.id,
            execution: AdmittedBoundaryExecution::CompilerBuiltin(
                target_operations::CompilerBuiltinExecution::HostedReadByte,
            ),
            realization: target_operations::HostedReadByteRealization.into(),
        }],
    );
    let source = std::sync::Arc::new(
        object_file::stage_optimized_relocation_free_object_container(text).unwrap(),
    );
    let object = image_emission::build_function_fragment_object_artifact(source.clone()).unwrap();
    image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
    let entry = object.entry_function().text_offset;
    let image = image_emission::emit_executable_image(&object, 3).unwrap();
    image_emission::validate_executable_image(&object, &image).unwrap();
    let record = image_emission::build_installation_record(
        &image,
        semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
    )
    .unwrap();
    let decoded = image_emission::decode_installation_record(
        &image_emission::encode_installation_record(&record).unwrap(),
    )
    .unwrap();
    image_emission::validate_installation_record(&decoded, &image).unwrap();
    let mut missing_frame = decoded.clone();
    missing_frame.functions_mut_for_test()[0].unit_stack = None;
    assert!(image_emission::validate_installation_record(&missing_frame, &image).is_err());
    assert_eq!(
        image_emission::derive_stack_demand(&object, lowered.semantic_module.entry).unwrap(),
        image_emission::derive_installation_stack_demand(
            &decoded,
            &image,
            lowered.semantic_module.entry
        )
        .unwrap()
    );
    let mut missing = object.clone();
    missing.clear_fragment_replay_for_test();
    assert!(image_emission::emit_executable_image(&missing, 3).is_err());
    (image, entry)
}

#[test]
fn bounded_byte_input_publishes_on_supported_hosted_targets() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let (image, _) = publish(target);
        assert!(!image.output().final_text_bytes.is_empty());
    }
}

#[test]
fn bounded_byte_input_executes_every_octet_eof_empty_and_repeated_views() {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    {
        let (image, entry) = publish(NativeTarget::host());
        native_function::assert_c_text(
            &image.output().final_text_bytes,
            entry,
            include_str!("byte_input.c"),
        );
    }
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    eprintln!("SKIP: byte input execution requires matching Linux or macOS ARM64 host");
}
