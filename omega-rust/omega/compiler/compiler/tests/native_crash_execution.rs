//! Explicit crashes survive canonical publication and native execution.
//! The host caller observes a child process so a crash cannot kill the test runner.

use terminal_interpreter::{TerminalExecutionResult, TerminalScalarValue};
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};

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

#[test]
fn explicit_crashes_execute_after_source_removal() {
    for (cause_name, cause) in [
        ("Trap", terminal_psi::CrashCause::Trap),
        ("Abort", terminal_psi::CrashCause::Abort),
    ] {
        for selected_machine in ["choose", "wrapper", "fatal"] {
            let directory = std::env::temp_dir().join(format!(
                "omega-native-crash-{cause_name}-{selected_machine}-{}",
                std::process::id()
            ));
            std::fs::create_dir(&directory).unwrap();
            let source = directory.join("main.omg");
            std::fs::write(
                &source,
                format!(
                    r#"
            machine choose(value: bool, should_crash: bool) -> bool
            crashes {cause_name} should_crash
            {{ transition {{ !should_crash -> value }} crash {cause_name}; }}
            machine wrapper(value: bool, should_crash: bool) -> bool
            crashes {cause_name}
            {{ choose(value, should_crash) }}
            machine fatal(value: bool, should_crash: bool) -> bool
            crashes {cause_name}
            {{ crash {cause_name}; }}
        "#
                ),
            )
            .unwrap();
            let checked =
                compiler::compile_to_checked(compiler::CheckedCompileRequest::new(&source, None))
                    .expect("explicit crash source checks");
            let artifact = terminal_production::TerminalProductionRequest::new(
                &checked,
                TerminalMachineSelection::Name(selected_machine),
            )
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default(),
            ))
            .expect("publish crash-bearing caller and callee")
            .into_artifact();
            let artifact =
                terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
                    .expect("reload canonical crash artifact");
            drop(checked);
            std::fs::remove_file(&source).unwrap();
            std::fs::remove_dir(&directory).unwrap();
            for value in [false, true] {
                for should_crash in [false, true] {
                    let outcome = terminal_interpreter::interpret_terminal_artifact(
                        artifact.semantic_bytes(),
                        artifact.proof_bytes(),
                        &proof_admission::AdmissionProfile::default(),
                        &[
                            TerminalScalarValue::Boolean(value),
                            TerminalScalarValue::Boolean(should_crash),
                        ],
                    );
                    if should_crash || selected_machine == "fatal" {
                        assert!(matches!(outcome,
                            Err(terminal_interpreter::TerminalArtifactInterpretError::Execution(
                                terminal_interpreter::TerminalInterpretError::Crash(crash)
                            )) if crash.cause == cause
                        ));
                    } else {
                        assert_eq!(
                            outcome.unwrap(),
                            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(value))
                        );
                    }
                }
            }
            for native_target in [
                target::NativeTarget::linux_x64(),
                target::NativeTarget::windows_x64(),
                target::NativeTarget::linux_arm64(),
                target::NativeTarget::macos_arm64(),
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
            optimized, native_target, &[],
        ).expect("explicit no-successor crashes have native realization");
                let emission_source = physical.into_function_fragment_emission_source();
                let exit_contract = emission_source.exit_contract().contract();
                assert_eq!(
                    exit_contract
                        .functions
                        .iter()
                        .map(|function| function.crashes.len())
                        .sum::<usize>(),
                    1
                );
                assert!(
                    exit_contract
                        .functions
                        .iter()
                        .flat_map(|function| &function.crashes)
                        .all(|crash| crash.cause == cause)
                );
                if selected_machine == "fatal" {
                    assert!(
                        exit_contract
                            .functions
                            .iter()
                            .all(|function| function.returns.is_empty())
                    );
                }
                if native_target == target::NativeTarget::host() && selected_machine == "wrapper" {
                    assert_exit_mutations_reject(&emission_source);
                }
                let fragments =
                    machine_emission::stage_optimized_function_fragment_emission(emission_source)
                        .unwrap();
                let framed =
                    machine_emission::stage_function_fragment_frame_application(fragments).unwrap();
                let text =
                    machine_emission::stage_optimized_fixed_frame_text_section(framed).unwrap();
                let source = std::sync::Arc::new(
                    object_file::stage_optimized_relocation_free_object_container(text).unwrap(),
                );
                let object =
                    image_emission::build_function_fragment_object_artifact(source.clone())
                        .unwrap();
                image_emission::validate_function_fragment_object_artifact(&source, &object)
                    .unwrap();
                let image = image_emission::emit_direct_executable_image(&object, 0).unwrap();
                image_emission::validate_direct_executable_image(&object, &image).unwrap();
                #[cfg(any(
                    all(
                        target_os = "linux",
                        any(target_arch = "x86_64", target_arch = "aarch64")
                    ),
                    all(target_os = "macos", target_arch = "aarch64")
                ))]
                if native_target == target::NativeTarget::host() {
                    let driver = format!(
                        "#define CRASH_ONLY {}\n",
                        u8::from(selected_machine == "fatal")
                    ) + r#"
            #include <stdbool.h>
            #include <signal.h>
            #include <sys/resource.h>
            #include <sys/wait.h>
            #include <unistd.h>
            extern bool omega_entry(bool value, bool should_crash);
            int main(void) {
                if (!CRASH_ONLY && (omega_entry(false, false) || !omega_entry(true, false))) return 1;
                for (int value = 0; value < 2; ++value) {
                    pid_t child = fork();
                    if (child < 0) return 2;
                    if (child == 0) {
                        struct rlimit limit = {0, 0};
                        setrlimit(RLIMIT_CORE, &limit);
                        alarm(5);
                        omega_entry(value, true);
                        _exit(3);
                    }
                    int status;
                    if (waitpid(child, &status, 0) != child) return 4;
                    if (!WIFSIGNALED(status)) return 5;
                    if (WTERMSIG(status) != SIGILL && WTERMSIG(status) != SIGTRAP) return 6;
                }
                return 0;
            }
        "#;
                    native_function::assert_c_text(
                        &image.output().final_text_bytes,
                        object.entry_function().text_offset,
                        &driver,
                    );
                }
                #[cfg(not(any(
                    all(
                        target_os = "linux",
                        any(target_arch = "x86_64", target_arch = "aarch64")
                    ),
                    all(target_os = "macos", target_arch = "aarch64")
                )))]
                eprintln!("SKIP: native crash execution requires Linux x64/ARM64 or macOS ARM64");
            }
        }
    }
}

fn assert_exit_mutations_reject(
    source: &machine_emission::StagedOptimizedFunctionFragmentEmissionSource,
) {
    let realization = source.replay_for_test().fixed_frame();
    let current = realization.allocation().current();
    let validate = |contract: &machine_emission::ValidatedWholeFunctionExitContract| {
        machine_emission::validate_whole_function_exit_contract_for_layout(
            current.selected(),
            realization.machine(),
            current.register_environment().physical(),
            realization.encoding(),
            realization.baseline_layout(),
            realization.layout_optimization(),
            Some((realization.frame(), realization.protocol())),
            contract,
        )
    };
    validate(realization.exit_contract()).unwrap();
    for mutation in 0..6 {
        let mut changed = realization.exit_contract().clone();
        let record = changed.contract_mut();
        let function = record
            .functions
            .iter_mut()
            .find(|function| !function.crashes.is_empty())
            .unwrap();
        match mutation {
            0 => function.crashes.clear(),
            1 => function.crashes.push(function.crashes[0].clone()),
            2 => function.crashes[0].psi_edge = semantic_vocabulary::EdgeId::new(999).unwrap(),
            3 => {
                function.crashes[0].cause = match function.crashes[0].cause {
                    terminal_psi::CrashCause::Trap => terminal_psi::CrashCause::Abort,
                    terminal_psi::CrashCause::Abort => terminal_psi::CrashCause::Trap,
                }
            }
            4 => function.crashes[0].bytes[0] ^= 1,
            5 => function.crashes[0].offset += 1,
            _ => unreachable!(),
        }
        record.identity = machine_code::whole_function_exit_contract_identity(record);
        assert!(
            validate(&changed).is_err(),
            "reauthenticated crash exit mutation {mutation}"
        );
    }
}
