//! Nested record results retain their original homes through receiver calls.
use proof_admission::AdmissionProfile;
use target::NativeTarget;
use terminal_codec::CanonicalTerminalArtifact;

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

const SOURCE: &str = include_str!("../../omega/pass/structural/local_record_receivers/main.omg");

fn produce() -> CanonicalTerminalArtifact {
    let tokens = source_files_to_tokens::Lexer::new(SOURCE)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "observe")
        .produce_artifact()
        .unwrap();
    let artifact = CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
    assert!(
        module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                operation.kind,
                terminal_psi::OperationKind::EstablishRecord { .. }
            ))
    );
    artifact
}

fn publish(
    artifact: &CanonicalTerminalArtifact,
    target: NativeTarget,
) -> (image_emission::ExecutableImage, usize) {
    let selections = optimization_core::OptimizationSelections::default();
    let optimized = native_realization::optimize_artifact_sections(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &AdmissionProfile::default(),
        native_realization::compiler_baseline_request_v1(&selections),
    )
    .expect("record optimizer admission");
    let post_terminal = optimized.selections().project_post_terminal();
    let targeted = abstract_operations_to_target_operations::lower_optimized_to_target_operations(
        optimized, target,
    )
    .unwrap_or_else(|error| panic!("record lowering on {target:?}: {error:#?}"));
    let physical = native_realization::stage_optimized_verified_physical_pipeline(
        targeted,
        post_terminal.selections(),
    )
    .unwrap_or_else(|error| panic!("record realization on {target:?}: {error:#?}"));
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
    let mut stripped = object.clone();
    stripped.clear_fragment_replay_for_test();
    assert!(image_emission::emit_executable_image(&stripped, 3).is_err());
    let entry_offset = object.entry_function().text_offset;
    let image = image_emission::emit_executable_image(&object, 3).unwrap();
    image_emission::validate_executable_image(&object, &image).unwrap();
    (image, entry_offset)
}

#[test]
fn interpreted_nested_receivers_preserve_original_referents_across_suspension() {
    use terminal_interpreter::{
        TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
        TerminalStructuralPrimitiveValue, TerminalStructuralValue,
    };
    let artifact = produce();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let integer = |value| TerminalScalarValue::Integer {
        scalar_type: semantic_vocabulary::IntegerType::new(
            semantic_vocabulary::IntegerSign::Unsigned,
            64,
        )
        .unwrap(),
        value: semantic_vocabulary::IntegerValue::Unsigned(value),
    };
    for choose_first in [false, true] {
        for initial_fuel in [0, 10_000] {
            let structural_arguments = entry
                .structural_parameters
                .iter()
                .enumerate()
                .map(|(index, parameter)| TerminalStructuralValue {
                    opaque_identity: 91 + index as u64,
                    structural_type: parameter.structural_type,
                    qualifications: Vec::new(),
                    path: Vec::new(),
                })
                .collect::<Vec<_>>();
            let mut execution =
                TerminalExecution::start_artifact_with_structural_arguments_and_primitive_values(
                    artifact.semantic_bytes(),
                    artifact.proof_bytes(),
                    &AdmissionProfile::default(),
                    &[TerminalScalarValue::Boolean(choose_first)],
                    &structural_arguments,
                    &[
                        TerminalStructuralPrimitiveValue {
                            argument_index: 0,
                            value: integer(999),
                        },
                        TerminalStructuralPrimitiveValue {
                            argument_index: 1,
                            value: integer(888),
                        },
                    ],
                )
                .unwrap();
            let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(initial_fuel);
            let mut completed = false;
            let mut trace = Vec::new();
            for _ in 0..2_048 {
                let status = execution.resume(&mut fuel).unwrap();
                trace.push(execution.structural_primitive_values()[1].value);
                match status {
                    TerminalExecutionStatus::Complete(result) => {
                        assert_eq!(result, TerminalExecutionResult::Unit);
                        completed = true;
                        break;
                    }
                    TerminalExecutionStatus::SponsorExhausted(_) => fuel.replenish(1).unwrap(),
                    other => panic!("record interpreter: {other:?}"),
                }
            }
            assert!(completed);
            assert_eq!(
                execution.structural_primitive_values()[0].value,
                integer(if choose_first { 100 } else { 200 })
            );
            assert_eq!(
                execution.structural_primitive_values()[1].value,
                integer(29)
            );
            if initial_fuel == 0 {
                trace.dedup();
                assert_eq!(trace, [integer(888), integer(17), integer(29)]);
            }
        }
    }
}

#[test]
fn nested_local_receivers_publish_on_four_targets() {
    let artifact = produce();
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let (image, _) = publish(&artifact, target);
        assert!(!image.output().final_text_bytes.is_empty());
    }
}

#[test]
fn native_nested_mutation_preserves_snapshots_other_homes_and_field_order() {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    {
        let (image, entry_offset) = publish(&produce(), NativeTarget::host());
        native_function::assert_c_text(
            &image.output().final_text_bytes,
            entry_offset,
            r#"
#include <stdint.h>
#include <unistd.h>
extern void omega_entry(_Bool choose_first, uint64_t *output, uint64_t *trace);
int main(void) {
    alarm(10);
    for (unsigned choice = 0; choice < 2; ++choice) {
        uint64_t output[] = { 123, 999, 456 };
        uint64_t trace[] = { 321, 888, 654 };
        omega_entry(choice != 0, &output[1], &trace[1]);
        if (output[1] != (choice ? 100 : 200)) return 1;
        if (trace[1] != 29) return 2;
        if (output[0] != 123 || output[2] != 456) return 3;
        if (trace[0] != 321 || trace[2] != 654) return 4;
    }
    return 0;
}
"#,
        );
    }
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    eprintln!(
        "SKIP: record receiver C runtime requires Linux x86-64/AArch64 or macOS AArch64; Windows has publication coverage only"
    );
}
