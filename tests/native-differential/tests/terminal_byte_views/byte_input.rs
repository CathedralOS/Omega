//! A boundary result and guarded mutable view compose through ordinary edges.

use super::*;
use abstract_operations_to_target_operations::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement,
};

#[path = "byte_input/line_read.rs"]
mod line_read;

fn reader() -> lowered_psi::LoweredPsi {
    lower_reader(include_str!("byte_input.omg"), "classify_bytes")
}

fn lower_reader(source: &str, machine: &str) -> lowered_psi::LoweredPsi {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, machine)
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
    publish_reader(target, reader())
}

fn publish_reader(
    target: NativeTarget,
    lowered: lowered_psi::LoweredPsi,
) -> (image_emission::ExecutableImage, usize) {
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
    for (minimum, maximum) in [(1, 255), (0, 254)] {
        let mut narrowed = decoded.clone();
        let machine_code::BoundaryResultRecord::Structural(result) = &mut narrowed
            .boundary_settlements_mut_for_test()[0]
            .settlement
            .native_result
        else {
            panic!("owned byte result");
        };
        let terminal_psi::StructuralTypeShape::Sum { cases } = &mut result.declaration.shape else {
            panic!("byte result sum");
        };
        cases[1].fields[0].field_type = terminal_psi::StructuralFieldType::BoundedInteger(
            semantic_vocabulary::BoundedIntegerType::new(
                semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
                    .unwrap(),
                IntegerValue::Signed(minimum),
                IntegerValue::Signed(maximum),
            )
            .unwrap(),
        );
        assert!(
            image_emission::encode_installation_record(&narrowed).is_err(),
            "standalone installed realization rejects same-layout interval {minimum}..{maximum}"
        );
        assert!(image_emission::validate_installation_record(&narrowed, &image).is_err());
    }
    let mut missing_frame = decoded.clone();
    missing_frame.functions_mut_for_test()[0].unit_stack = None;
    missing_frame.functions_mut_for_test()[0].scalar_stack = None;
    assert!(image_emission::encode_installation_record(&missing_frame).is_err());
    assert!(image_emission::validate_installation_record(&missing_frame, &image).is_err());
    // Admitting a byte leaf in either return kind must not let metadata change
    // the enclosing function's independently validated frame role or geometry.
    let mut dual_frame = decoded.clone();
    let function = &mut dual_frame.functions_mut_for_test()[0];
    let originally_unit = function.unit_stack.is_some();
    if let Some(stack) = function.unit_stack {
        function.scalar_stack = Some(image_emission::ObjectScalarStack {
            local_peak_bytes: stack.local_peak_bytes,
            stack_alignment: stack.stack_alignment,
        });
    } else {
        let stack = function.scalar_stack.unwrap();
        function.unit_stack = Some(image_emission::ObjectUnitStack {
            frame_bytes: stack.local_peak_bytes,
            local_peak_bytes: stack.local_peak_bytes,
            stack_alignment: stack.stack_alignment,
        });
    }
    assert!(image_emission::encode_installation_record(&dual_frame).is_err());
    assert!(image_emission::validate_installation_record(&dual_frame, &image).is_err());
    let mut substituted_frame = dual_frame;
    if originally_unit {
        substituted_frame.functions_mut_for_test()[0].unit_stack = None;
    } else {
        substituted_frame.functions_mut_for_test()[0].scalar_stack = None;
    }
    assert!(image_emission::validate_installation_record(&substituted_frame, &image).is_err());
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
fn exact_byte_input_copy_publishes_on_supported_hosted_targets() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let lowered = lower_reader(include_str!("read_one.omg"), "read_one");
        let (image, _) = publish_reader(target, lowered);
        assert!(!image.output().final_text_bytes.is_empty());
    }
}

#[test]
fn exact_byte_input_copy_executes_all_octets_empty_view_and_eof() {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64"),
    ))]
    {
        let lowered = lower_reader(include_str!("read_one.omg"), "read_one");
        let (image, entry) = publish_reader(NativeTarget::host(), lowered);
        native_function::assert_c_text(
            &image.output().final_text_bytes,
            entry,
            include_str!("read_one.c"),
        );
    }
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64"),
    )))]
    eprintln!("SKIP: exact byte copy execution requires matching Linux or macOS ARM64 host");
}

#[test]
fn exact_byte_input_casts_compose_through_unsigned_intermediate_carriers() {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64"),
    ))]
    for intermediate in ["u16", "u32", "u64"] {
        let source = include_str!("read_one.omg").replace(
            "out[0] = value as u8;",
            &format!("out[0] = (value as {intermediate}) as u8;"),
        );
        let lowered = lower_reader(&source, "read_one");
        let (image, entry) = publish_reader(NativeTarget::host(), lowered);
        native_function::assert_c_text(
            &image.output().final_text_bytes,
            entry,
            include_str!("read_one.c"),
        );
    }
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64"),
    )))]
    eprintln!("SKIP: composed exact cast execution requires matching Linux or macOS ARM64 host");
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
