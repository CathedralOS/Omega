//! Selected operator ceilings survive entry disproof and enclosing scalar calls.
//! Reload Terminal and remove source before interpreting and emitting it, so
//! independent consumers must establish safety from retained entry contracts.

use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use std::path::PathBuf;
use terminal_interpreter::{TerminalExecutionResult, TerminalScalarValue};

#[cfg(any(
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(target_os = "macos", target_arch = "aarch64")
))]
#[path = "../../../../../tests/native-differential/tests/common/native_function.rs"]
#[allow(dead_code)]
mod native_function;

struct SourceTree(PathBuf);

impl Drop for SourceTree {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).expect("remove guarded operator source tree");
    }
}

#[test]
fn guarded_operator_executes_after_source_removal() {
    assert_source_free_execution(
        "safe",
        &[
            (&[0], false),
            (&[1], true),
            (&[2], false),
            (&[i32::MAX], false),
        ],
        false,
        "#include <stdint.h>\n#include <stdbool.h>\nextern bool omega_entry(int32_t value);\nint main(void) { return !omega_entry(0) && omega_entry(1) && !omega_entry(2) && !omega_entry(INT32_MAX) ? 0 : 1; }",
    );
}

#[test]
fn inferred_operator_ceiling_executes_through_wrapper_after_source_removal() {
    assert_source_free_execution(
        "wrapper",
        &[
            (&[0, 0], true),
            (&[1, 2], false),
            (&[i32::MIN, 0], false),
            (&[i32::MAX, i32::MAX], true),
        ],
        true,
        "#include <stdint.h>\n#include <stdbool.h>\nextern bool omega_entry(int32_t left, int32_t right);\nint main(void) { return omega_entry(0, 0) && !omega_entry(1, 2) && !omega_entry(INT32_MIN, 0) && omega_entry(INT32_MAX, INT32_MAX) ? 0 : 1; }",
    );
}

fn assert_source_free_execution(
    selected_machine: &str,
    cases: &[(&[i32], bool)],
    has_crash_ceiling: bool,
    native_driver: &str,
) {
    let directory = std::env::temp_dir().join(format!(
        "omega-guarded-operator-{selected_machine}-{}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).expect("create guarded operator source tree");
    let tree = SourceTree(directory.clone());
    std::fs::write(
        directory.join("main.omg"),
        include_str!("../../../../../tests/omega/pass/operators/crash_routes/main.omg"),
    )
    .unwrap();
    let checked = compiler::compile_to_checked(compiler::CheckedCompileRequest::new(
        &directory.join("main.omg"),
        None,
    ))
    .expect("the unchanged guarded-operator customer checks");
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, selected_machine)
        .produce_artifact()
        .expect("selected operator and enclosing call retain verified crash contracts");
    let artifact = terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
        .expect("reload independent Terminal artifact");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    assert!(!module.operation_crash_contracts.is_empty());
    assert!(
        module.operation_crash_contracts.iter().all(|contract| {
            !contract.published_routes.is_empty() && !contract.crash_continuations.is_empty()
        }),
        "invocation disproof must not erase the published or substituted ceiling"
    );
    assert_eq!(
        module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap()
            .contract
            .crash_routes
            .is_empty(),
        !has_crash_ceiling,
    );
    drop(checked);
    drop(tree);
    assert!(!directory.exists());

    for (arguments, expected) in cases {
        let arguments = arguments
            .iter()
            .map(|value| TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                value: IntegerValue::Signed(i128::from(*value)),
            })
            .collect::<Vec<_>>();
        let result = terminal_interpreter::interpret_terminal_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &arguments,
        )
        .expect("source-free guarded operator executes");
        assert_eq!(
            result,
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(*expected))
        );
    }

    let selections = optimization_core::OptimizationSelections::new([]).unwrap();
    let optimized = native_realization::optimize_artifact_sections(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        native_realization::compiler_baseline_request_v1(&selections),
    )
    .unwrap();
    let physical =
        native_realization::stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target::NativeTarget::host(),
            &[],
        )
        .unwrap();
    let fragments = machine_emission::stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .unwrap();
    let framed = machine_emission::stage_function_fragment_frame_application(fragments).unwrap();
    let text = machine_emission::stage_optimized_fixed_frame_text_section(framed).unwrap();
    let source = std::sync::Arc::new(
        object_file::stage_optimized_relocation_free_object_container(text).unwrap(),
    );
    let object = image_emission::build_function_fragment_object_artifact(source.clone()).unwrap();
    image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
    let image = image_emission::emit_direct_executable_image(&object, 0).unwrap();
    image_emission::validate_direct_executable_image(&object, &image).unwrap();
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    native_function::assert_c_text(
        &image.output().final_text_bytes,
        object.entry_function().text_offset,
        native_driver,
    );
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    {
        let _ = native_driver;
        eprintln!(
            "SKIP: native guarded operator execution requires Linux x64/ARM64 or macOS ARM64"
        );
    }
}
