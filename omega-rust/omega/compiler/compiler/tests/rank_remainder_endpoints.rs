//! Invocation-fixed remainder endpoints preserve both operands at state arrivals.

use compiler::{CheckedCompilation, CheckedCompileRequest, compile_to_checked};
use diagnostics::Diagnostic;
use package_compilation::{PackageCompilationInputs, PackageSourceBinding};
use semantic_vocabulary::PackageKeyIdentity;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_PROJECT: AtomicU64 = AtomicU64::new(0);

#[path = "../../../../../tests/native-differential/tests/common/native_function.rs"]
#[allow(dead_code)]
mod native_function;

const COUNTDOWN: &str = r#"
machine walk(remaining: u64 [0..=5], cap: u64 [0..=20], divisor: u64 [1..=5])
terminates by remaining in 0..(cap % divisor + 6);
-> u64 {
    transition { _ -> iterate(divisor, cap, remaining) }
    state iterate(width: u64 [1..=5], limit: u64 [0..=20], pending: u64 [0..=5]) {
        transition pending > 0 {
            true -> iterate(width - 0, limit - 0, pending - 1)
            false -> pending
        }
    }
}
machine recovered() -> u64 { walk(5, 17, 4) }
"#;

struct Project(PathBuf);

impl Project {
    fn new(source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-rank-remainder-{}-{}",
            std::process::id(),
            NEXT_PROJECT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).expect("create isolated project");
        fs::write(root.join("main.omg"), source).expect("write source");
        Self(root)
    }

    fn check(&self) -> Result<CheckedCompilation, Vec<Diagnostic>> {
        let package = PackageKeyIdentity::from_digest([1; 32]).unwrap();
        let inputs = PackageCompilationInputs::new_package(
            package,
            vec![PackageSourceBinding::new(
                package,
                "rank-remainder",
                self.0.clone(),
            )],
            Vec::new(),
        )
        .unwrap();
        compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(inputs),
            ..CheckedCompileRequest::new(&self.0.join("main.omg"), Some("macos_arm64"))
        })
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            fs::remove_dir_all(&self.0).expect("remove isolated project");
        }
    }
}

#[test]
fn named_state_remainder_endpoint_checks_and_interprets() {
    let project = Project::new(COUNTDOWN);
    let checked = project
        .check()
        .expect("variable-divisor endpoint forms and survives each arrival");
    let outcome = checked_interpreter::interpret_entry(
        &checked,
        "recovered",
        &[],
        checked_interpreter::InterpretOptions::default(),
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 0);
}

#[test]
fn runtime_remainder_endpoint_reaches_terminal_and_native_execution() {
    let project = Project::new(
        r#"
        machine walk(remaining: u64 [0..=5], cap: u64 [0..=20], divisor: u64 [1..=5])
        terminates by remaining in 0..(cap % divisor + 6);
        -> u64 {
            transition remaining > 0 {
                true -> walk(remaining - 1, cap - 0, divisor - 0)
                false -> remaining
            }
        }
    "#,
    );
    let checked = project.check().expect("runtime remainder endpoint checks");
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "walk")
        .produce_artifact()
        .expect("countdown reaches source-free Terminal");
    drop(checked);
    drop(project);
    assert_eq!(
        terminal_interpreter::interpret_terminal_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[5, 17, 4].map(|value| terminal_interpreter::TerminalScalarValue::Integer {
                scalar_type: semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    64
                )
                .unwrap(),
                value: semantic_vocabulary::IntegerValue::Unsigned(value),
            }),
        )
        .expect("source-free countdown executes"),
        terminal_interpreter::TerminalExecutionResult::Scalar(
            terminal_interpreter::TerminalScalarValue::Integer {
                scalar_type: semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    64
                )
                .unwrap(),
                value: semantic_vocabulary::IntegerValue::Unsigned(0),
            },
        ),
    );
    assert_native_countdown(&artifact);
}

fn assert_native_countdown(artifact: &terminal_codec::CanonicalTerminalArtifact) {
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
        "#include <stdint.h>\nextern uint64_t omega_entry(uint64_t, uint64_t, uint64_t);\nint main(void) { for (uint64_t remaining=0; remaining<=5; ++remaining) for (uint64_t cap=0; cap<=20; ++cap) for (uint64_t divisor=1; divisor<=5; ++divisor) if (omega_entry(remaining, cap, divisor) != 0) return 1; return 0; }",
    );
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    eprintln!("SKIP: native countdown execution requires a supported Linux or macOS host");
}
