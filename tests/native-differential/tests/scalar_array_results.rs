//! Primitive arrays retain real native payloads through construction and result calls.

use native_realization::{compiler_baseline_request_v1, optimize_artifact_sections};
use optimization_core::OptimizationSelections;
use proof_admission::AdmissionProfile;
use target::NativeTarget;

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

#[path = "scalar_array_results/admission.rs"]
mod admission;

fn produce(
    source: &str,
    entry: &str,
) -> Result<
    terminal_codec::CanonicalTerminalArtifact,
    terminal_production::TerminalArtifactProductionError,
> {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
    terminal_production::produce_terminal_artifact(&checked, entry)
}

fn target_plan(
    source: &str,
    entry: &str,
    target: NativeTarget,
) -> Result<
    abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations,
    abstract_operations_to_target_operations::LoweringError,
> {
    let artifact = produce(source, entry)
        .unwrap_or_else(|error| panic!("array Terminal production {entry}: {error:?}"));
    let artifact =
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap();
    let selections = OptimizationSelections::default();
    let optimized = optimize_artifact_sections(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &AdmissionProfile::default(),
        compiler_baseline_request_v1(&selections),
    )
    .unwrap();
    abstract_operations_to_target_operations::lower_optimized_to_target_operations(
        optimized, target,
    )
}

fn publish(
    source: &str,
    entry: &str,
    target: NativeTarget,
) -> (image_emission::ExecutableImage, usize) {
    let target_plan = target_plan(source, entry, target)
        .unwrap_or_else(|error| panic!("array target lowering {entry} on {target:?}: {error:?}"));
    let post_terminal = target_plan.optimized().selections().project_post_terminal();
    let physical = native_realization::stage_optimized_verified_physical_pipeline(
        target_plan,
        post_terminal.selections(),
    )
    .unwrap_or_else(|error| panic!("array native selection {entry} on {target:?}: {error:?}"));
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
    let offset = object.entry_function().text_offset;
    let mut stripped = object.clone();
    stripped.clear_fragment_replay_for_test();
    assert!(image_emission::emit_executable_image(&stripped, 3).is_err());
    let image = image_emission::emit_executable_image(&object, 3).unwrap();
    image_emission::validate_executable_image(&object, &image).unwrap();
    let record = image_emission::build_installation_record(
        &image,
        semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
    )
    .unwrap();
    let bytes = image_emission::encode_installation_record(&record).unwrap();
    let decoded = image_emission::decode_installation_record(&bytes).unwrap();
    image_emission::validate_installation_record(&decoded, &image).unwrap();
    assert_eq!(
        image_emission::encode_installation_record(&decoded).unwrap(),
        bytes
    );
    for function_index in 0..decoded.functions().len() {
        let mut changed = decoded.clone();
        changed.functions_mut_for_test()[function_index].parameter_abi = None;
        assert!(image_emission::validate_installation_record(&changed, &image).is_err());
    }
    (image, offset)
}

#[test]
fn native_array_shapes_preserve_leaf_bits_and_row_major_order() {
    // Observe every logical leaf, including values spanning both result registers.
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        for (entry, c_type, count, expected) in [
            ("byte", "uint8_t", 1, "{255}"),
            ("half", "uint16_t", 1, "{65535}"),
            ("word", "uint32_t", 1, "{4294967295U}"),
            ("wide", "uint64_t", 1, "{18446744073709551615ULL}"),
            (
                "pair",
                "uint64_t",
                2,
                "{18446744073709551615ULL, 9223372036854775809ULL}",
            ),
            ("nested", "uint16_t", 4, "{65535, 1, 2, 32768}"),
            ("boolean", "uint8_t", 2, "{1, 0}"),
        ] {
            let source = include_str!("scalar_array_results/shapes.omg");
            if entry == "pair" && target == NativeTarget::windows_x64() {
                continue; // The existing Microsoft hidden-pointer result path is unfinished.
            }
            let (image, offset) = publish(source, entry, target);
            #[cfg(any(
                all(
                    target_os = "linux",
                    any(target_arch = "x86_64", target_arch = "aarch64")
                ),
                all(target_os = "macos", target_arch = "aarch64")
            ))]
            if target == NativeTarget::host() {
                let driver = format!(
                    "#include <stdint.h>\n typedef struct {{ {c_type} values[{count}]; }} Row;\n extern Row omega_entry(void);\n int main(void) {{ const {c_type} expected[{count}] = {expected}; Row row = omega_entry(); for (unsigned element = 0; element < {count}; ++element) if (row.values[element] != expected[element]) return 1; return 0; }}"
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
                let _ = (image, offset, c_type, count, expected);
                eprintln!(
                    "SKIP: native array shape execution requires the Linux/macOS host harness"
                );
            }
        }
    }
}

const SOURCE: &str = "
    machine computed_row(value: u8) -> [u8; 2] { [value, 7u8 + 2u8] }
    machine called_row(value: u8) -> [u8; 2] { computed_row(value) }
    machine bound_row(value: u8) -> [u8; 2] { let row: [u8; 2] = called_row(value); row }
";

#[test]
fn constructed_array_results_publish_through_ordinary_calls() {
    for entry in ["computed_row", "called_row", "bound_row"] {
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            publish(SOURCE, entry, target);
        }
    }
}

#[test]
fn native_array_results_preserve_every_byte() {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    for entry in ["computed_row", "called_row", "bound_row"] {
        let (image, offset) = publish(SOURCE, entry, NativeTarget::host());
        native_function::assert_c_text(
            &image.output().final_text_bytes,
            offset,
            r"
            #include <stdint.h>
            typedef struct { uint8_t values[2]; } Row;
            extern Row omega_entry(uint8_t);
            int main(void) {
                for (unsigned value = 0; value < 256; ++value) {
                    Row row = omega_entry((uint8_t)value);
                    if (row.values[0] != value || row.values[1] != 9) return 1;
                }
                return 0;
            }
        ",
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
        "SKIP: native array callable execution requires the existing Linux/macOS host harness"
    );
}
