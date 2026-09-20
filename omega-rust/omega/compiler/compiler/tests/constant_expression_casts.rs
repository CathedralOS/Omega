//! Explicit scalar conversions retain their landing inside constant expressions.

use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_PROJECT: AtomicU64 = AtomicU64::new(0);

#[path = "../../../../../tests/native-differential/tests/common/native_function.rs"]
#[allow(dead_code)]
mod native_function;

struct Sources(std::path::PathBuf);

impl Sources {
    fn new(files: &[(&str, &str)]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-constant-expression-casts-{}-{}",
            std::process::id(),
            NEXT_PROJECT.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir(&root).unwrap();
        for (name, contents) in files {
            std::fs::write(root.join(name), contents).unwrap();
        }
        Self(root)
    }
}

impl Drop for Sources {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn artifact(files: &[(&str, &str)], expected: u128) -> terminal_codec::CanonicalTerminalArtifact {
    let sources = Sources::new(files);
    let path = sources.0.join("main.omg");
    let checked = compiler::compile_to_checked(compiler::CheckedCompileRequest::new(&path, None))
        .expect("an ordinary total integer conversion forms a constant");
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "read")
        .produce_artifact()
        .expect("converted constant reaches Terminal");
    drop(checked);
    drop(sources);
    assert!(!path.exists());
    assert_eq!(
        terminal_interpreter::interpret_terminal_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
        )
        .unwrap(),
        terminal_interpreter::TerminalExecutionResult::Scalar(
            terminal_interpreter::TerminalScalarValue::Integer {
                scalar_type: semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    64,
                )
                .unwrap(),
                value: semantic_vocabulary::IntegerValue::Unsigned(expected),
            }
        ),
    );
    terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap()
}

#[test]
fn direct_constant_casts_reach_source_free_terminal_execution() {
    for (expression, expected) in [
        ("7u8 as u64", 7),
        ("(7u8 as u16) as u64", 7),
        ("(7 / 2 * 2) as u64", 7),
        ("(250u8 as u64) + 10", 260),
        ("retain(7u8 as u64)", 7),
        ("small() as u64", 250),
        ("BASE as u64", 7),
        (
            "(match true { true -> 7u8, false -> never(false) }) as u64",
            7,
        ),
    ] {
        let source = format!(
            "const BASE: u8 = 7;
             machine small() -> u8 {{ 250 }}
             machine retain(value: u64) -> u64 {{ value }}
             machine never(ready: bool) -> u8 requires ready; {{ 1 }}
             const VALUE: u64 = {expression}; machine read() -> u64 {{ VALUE }}"
        );
        drop(artifact(&[("main.omg", &source)], expected));
    }
}

#[test]
fn cast_call_results_form_exact_generic_indices() {
    drop(artifact(
        &[(
            "main.omg",
            "machine small() -> u8 { 7 }
             domain<const N: u64> u64::Indexed<N>;
             machine consume(value: u64 in Indexed<7>) -> u64 { value as u64 }
             machine read() -> u64 {
                 let value: u64 in Indexed<(small() as u64)> = 7 as u64 in Indexed<7>;
                 consume(value)
             }",
        )],
        7,
    ));
}

#[test]
fn constant_casts_retain_imported_operand_selection() {
    drop(artifact(
        &[
            ("settings.omg", "module settings; pub const BASE: u8 = 7;"),
            (
                "main.omg",
                "use settings::BASE; const VALUE: u64 = BASE as u64;
            machine read() -> u64 { VALUE }",
            ),
        ],
        7,
    ));
}

#[test]
fn constant_casts_cannot_erase_operand_width_or_invent_conversion_evidence() {
    for expression in [
        "(250u8 + 10) as u64",
        "true as u64",
        "1.5f32 as u64",
        "256u64 as u8",
        "1u8 as u64 in Wrapping",
        "match true { true -> 7u64, false -> true as u64 }",
        "(7 / 2) as u64",
    ] {
        let source = format!("const VALUE: u64 = {expression}; machine read() -> u64 {{ VALUE }}");
        let sources = Sources::new(&[("main.omg", &source)]);
        let result = compiler::compile_to_checked(compiler::CheckedCompileRequest::new(
            &sources.0.join("main.omg"),
            None,
        ));
        assert!(result.is_err(), "invalid conversion admitted: {expression}");
    }
}

#[test]
fn converted_constant_executes_natively_after_source_removal() {
    let artifact = artifact(
        &[(
            "main.omg",
            "machine small() -> u8 { 250 }
         machine retain(value: u64) -> u64 { value }
         const VALUE: u64 = retain((small() as u16) as u64) + 10;
         machine read() -> u64 { VALUE }",
        )],
        260,
    );
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ] {
        let selections = optimization_core::OptimizationSelections::new([]).unwrap();
        let optimized = native_realization::optimize_artifact_sections(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            native_realization::compiler_baseline_request_v1(&selections),
        )
        .unwrap();
        let physical = native_realization::stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized, target, &[],
        ).unwrap();
        let fragments = machine_emission::stage_optimized_function_fragment_emission(
            physical.into_function_fragment_emission_source(),
        )
        .unwrap();
        let framed =
            machine_emission::stage_function_fragment_frame_application(fragments).unwrap();
        let text = machine_emission::stage_optimized_fixed_frame_text_section(framed).unwrap();
        let source = std::sync::Arc::new(
            object_file::stage_optimized_relocation_free_object_container(text).unwrap(),
        );
        let object =
            image_emission::build_function_fragment_object_artifact(source.clone()).unwrap();
        image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
        let image = image_emission::emit_executable_image(&object, 0).unwrap();
        image_emission::validate_executable_image(&object, &image).unwrap();
        #[cfg(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(target_os = "macos", target_arch = "aarch64"),
        ))]
        if target == target::NativeTarget::host() {
            native_function::assert_c_text(
                &image.output().final_text_bytes,
                object.entry_function().text_offset,
                "#include <stdint.h>\nextern uint64_t omega_entry(void);\nint main(void) { return omega_entry() == 260 ? 0 : 1; }",
            );
        }
    }
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64"),
    )))]
    eprintln!("SKIP: native constant execution requires a supported Linux or macOS host");
}
