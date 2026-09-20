//! Generic endpoint contracts survive source checking, portable replay and
//! native execution. The source is removed before either consumer runs.

use compiler::{CheckedCompileRequest, compile_to_checked};
use package_compilation::{PackageCompilationInputs, PackageSourceBinding};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, PackageKeyIdentity};
use std::sync::atomic::{AtomicU64, Ordering};
use terminal_interpreter::{TerminalExecutionResult, TerminalScalarValue};

#[path = "../../../../../tests/native-differential/tests/common/native_function.rs"]
#[allow(dead_code)]
mod native_function;

fn checked_source(source: &str) -> compiler::CheckedCompilation {
    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "omega-bounded-slice-selectors-{}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir(&root).expect("isolated source directory");
    let main = root.join("main.omg");
    std::fs::write(&main, source).expect("generic endpoint source");
    let package = PackageKeyIdentity::from_digest([1; 32]).unwrap();
    let inputs = PackageCompilationInputs::new_package(
        package,
        vec![PackageSourceBinding::new(
            package,
            "bounded-endpoints",
            root.clone(),
        )],
        Vec::new(),
    )
    .unwrap();
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&main, Some("linux_x86_64"))
    })
    .unwrap_or_else(|diagnostics| panic!("bounded endpoint source: {diagnostics:#?}"));
    std::fs::remove_file(&main).unwrap();
    std::fs::remove_dir(&root).unwrap();
    assert!(!root.exists());
    checked
}

#[test]
fn bounded_generic_endpoints_execute_after_source_removal() {
    let checked = checked_source(
        r#"
        machine endpoint<const N: u64>() -> u64 [0..=3]
        requires N <= 3
        { N }
        machine main() -> u64 {
            let first: u64 = endpoint<2>();
            let second: u64 = endpoint<3>();
            transition first == 2 && second == 3 { true -> 7 false -> 0 }
        }
    "#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "main")
        .produce_artifact()
        .expect("bounded endpoint calls publish Terminal");
    drop(checked);
    assert_eq!(
        terminal_interpreter::interpret_terminal_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
        )
        .expect("source-free endpoint execution"),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            value: IntegerValue::Unsigned(7),
        }),
    );

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
    let image = image_emission::emit_executable_image(&object, 3).unwrap();
    image_emission::validate_executable_image(&object, &image).unwrap();
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
        "#include <stdint.h>\nextern uint64_t omega_entry(void);\nint main(void) { return omega_entry() == 7 ? 0 : 1; }",
    );
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    eprintln!("SKIP: native function execution requires a supported Linux or macOS host");
}

#[test]
fn inferred_slice_selectors_check_but_await_structural_control_plan() {
    let checked = checked_source(
        r#"
        data Main {}
        machine Main::endpoint<const N: u64>(&self, witness: &[u8; N]) -> u64 [0..=3]
        requires N <= 3
        { N }
        machine Main::window(&self, items: &[i32; 4]) -> u64 {
            let pair: [u8; 2] = [0, 0];
            let triple: [u8; 3] = [0, 0, 0];
            let pair_view: &[i32] = items[..self.endpoint(&pair)];
            let triple_view: &[i32] = items[..self.endpoint(&triple)];
            transition pair_view.len == 2 && triple_view.len == 3 { true -> 7 false -> 0 }
        }
        machine main() -> u64 {
            let selector: Main = Main {};
            let values: [i32; 4] = [11, 7, 6, 22];
            selector.window(&values)
        }
    "#,
    );
    // Keep the original slice customer visible beside its working scalar
    // dependency. Checked/interpreter success must not masquerade as native
    // slice support; replace this assertion with execution when lowering lands.
    let error = terminal_production::TerminalProductionRequest::new(&checked, "main")
        .produce_artifact()
        .expect_err("slice control-plan production remains unfinished");
    assert!(
        format!("{error:?}")
            .contains("machine has no source-independent checked scalar control plan"),
        "{error:?}",
    );
}
