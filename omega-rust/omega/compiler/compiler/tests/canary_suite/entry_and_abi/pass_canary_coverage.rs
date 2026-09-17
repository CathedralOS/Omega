#[cfg(windows)]
use crate::WINDOWS_HOST_PASS_CANARIES;
use crate::{
    ACTIVE_PASS_CANARIES, CANARY_UMBRELLA_LOCK, CHECKED_ONLY_PASS_CANARIES,
    CROSS_TARGET_PASS_CANARIES, ROOTED_BACKEND_PASS_CANARIES, ROOTED_TARGET_BACKEND_PASS_CANARIES,
    check_canary, compile_canary_without_output_for_target, compile_native_canary_without_output,
    compile_rooted_backend_canary_without_output,
    compile_rooted_backend_canary_without_output_for_target, exact_native_coverage, pass_canary,
    run_bounded_canary_jobs,
};

#[test]
fn pass_canaries_compile() {
    // COLLECT-ALL, not first-panic: a serial panic at the first failing
    // member masked every member ordered after it (this is the same
    // umbrella-masking pattern that hid a real interpreter bug behind the
    // differential's tick_count stop). One host-blocked cluster (e.g. the
    // efi members on a non-EFI-lowering host) must not exempt the rest of
    // the corpus from its compile check.
    let _umbrella = CANARY_UMBRELLA_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut failures: Vec<String> = Vec::new();
    let filter = std::env::var("OMEGA_PASS_CANARY_FILTER").ok();
    let selected = |canary_name: &&str| {
        filter.as_deref().is_none_or(|filter| {
            filter
                .split(',')
                .map(str::trim)
                .any(|candidate| !candidate.is_empty() && canary_name.contains(candidate))
        })
    };
    let mut selected_count = 0usize;

    let checked_only = CHECKED_ONLY_PASS_CANARIES
        .iter()
        .copied()
        .filter(selected)
        .collect::<Vec<_>>();
    selected_count += checked_only.len();
    failures.extend(
        run_bounded_canary_jobs(&checked_only, |canary_name| {
            let canary = pass_canary(canary_name);
            check_canary(&canary).err().map(|diagnostics| {
                format!(
                    "checked-only {}:\n{}",
                    canary.display(),
                    diagnostics
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            })
        })
        .into_iter()
        .flatten(),
    );

    let coverage_started = std::time::Instant::now();
    let exact_native_coverage = exact_native_coverage::ExactNativeCanaryCoverageIndex::discover()
        .unwrap_or_else(|diagnostic| {
            panic!("cannot audit dedicated exact-native canary coverage: {diagnostic}")
        });
    let coverage_elapsed = coverage_started.elapsed();

    let selected_cross_target = CROSS_TARGET_PASS_CANARIES
        .iter()
        .copied()
        .filter(|(canary_name, _)| selected(canary_name))
        .collect::<Vec<_>>();
    selected_count += selected_cross_target.len();
    let cross_target_elided_count = selected_cross_target
        .iter()
        .filter(|(canary_name, target)| {
            exact_native_coverage
                .unique_cross_target_owner(canary_name, target)
                .is_some()
        })
        .count();
    let cross_target = selected_cross_target
        .into_iter()
        .filter(|(canary_name, target)| {
            exact_native_coverage
                .unique_cross_target_owner(canary_name, target)
                .is_none()
        })
        .collect::<Vec<_>>();
    failures.extend(
        run_bounded_canary_jobs(&cross_target, |(canary_name, target)| {
            let canary = pass_canary(canary_name);
            compile_canary_without_output_for_target(&canary, target)
                .err()
                .map(|diagnostics| {
                    format!(
                        "cross-target {target} {}:\n{}",
                        canary.display(),
                        diagnostics
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join("\n")
                    )
                })
        })
        .into_iter()
        .flatten(),
    );

    let selected_rooted_target = ROOTED_TARGET_BACKEND_PASS_CANARIES
        .iter()
        .copied()
        .filter(|(canary_name, _)| selected(canary_name))
        .collect::<Vec<_>>();
    selected_count += selected_rooted_target.len();
    let rooted_target_elided_count = selected_rooted_target
        .iter()
        .filter(|(canary_name, target)| {
            exact_native_coverage
                .unique_rooted_target_owner(canary_name, target)
                .is_some()
        })
        .count();
    let rooted_target = selected_rooted_target
        .into_iter()
        .filter(|(canary_name, target)| {
            exact_native_coverage
                .unique_rooted_target_owner(canary_name, target)
                .is_none()
        })
        .collect::<Vec<_>>();
    failures.extend(
        run_bounded_canary_jobs(&rooted_target, |(canary_name, target)| {
            let canary = pass_canary(canary_name);
            compile_rooted_backend_canary_without_output_for_target(&canary, target)
                .err()
                .map(|diagnostics| {
                    format!(
                        "rooted target {target} {}:\n{}",
                        canary.display(),
                        diagnostics
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join("\n")
                    )
                })
        })
        .into_iter()
        .flatten(),
    );

    #[cfg(windows)]
    {
        let windows_host = WINDOWS_HOST_PASS_CANARIES
            .iter()
            .copied()
            .filter(selected)
            .collect::<Vec<_>>();
        selected_count += windows_host.len();
        failures.extend(
            run_bounded_canary_jobs(&windows_host, |canary_name| {
                let canary = pass_canary(canary_name);
                compile_native_canary_without_output(&canary)
                    .err()
                    .map(|diagnostics| {
                        format!(
                            "windows-host {}:\n{}",
                            canary.display(),
                            diagnostics
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join("\n")
                        )
                    })
            })
            .into_iter()
            .flatten(),
        );
    }
    let selected_active = ACTIVE_PASS_CANARIES
        .iter()
        .copied()
        .filter(selected)
        .collect::<Vec<_>>();
    selected_count += selected_active.len();
    let rooted_elided_count = selected_active
        .iter()
        .filter(|canary_name| {
            ROOTED_BACKEND_PASS_CANARIES.contains(canary_name)
                && exact_native_coverage
                    .unique_rooted_owner(canary_name)
                    .is_some()
        })
        .count();
    let direct_elided_count = selected_active
        .iter()
        .filter(|canary_name| {
            !ROOTED_BACKEND_PASS_CANARIES.contains(canary_name)
                && exact_native_coverage
                    .unique_direct_owner(canary_name)
                    .is_some()
        })
        .count();
    let active = selected_active
        .into_iter()
        .filter(|canary_name| {
            if ROOTED_BACKEND_PASS_CANARIES.contains(canary_name) {
                exact_native_coverage
                    .unique_rooted_owner(canary_name)
                    .is_none()
            } else {
                exact_native_coverage
                    .unique_direct_owner(canary_name)
                    .is_none()
            }
        })
        .collect::<Vec<_>>();
    if std::env::var_os("OMEGA_PASS_CANARY_REPORT_COUNTS").is_some() {
        eprintln!(
            "pass-canary coverage: selected-active={} rooted-exact-native-elided={} direct-exact-native-elided={} active-compiled={} cross-target-elided={} cross-target-compiled={} rooted-target-elided={} rooted-target-compiled={} source-files={} source-bytes={} scan-micros={}",
            active.len() + rooted_elided_count + direct_elided_count,
            rooted_elided_count,
            direct_elided_count,
            active.len(),
            cross_target_elided_count,
            cross_target.len(),
            rooted_target_elided_count,
            rooted_target.len(),
            exact_native_coverage.source_file_count(),
            exact_native_coverage.source_byte_count(),
            coverage_elapsed.as_micros(),
        );
    }
    failures.extend(
        run_bounded_canary_jobs(&active, |canary_name| {
            let canary = pass_canary(canary_name);
            let result = if ROOTED_BACKEND_PASS_CANARIES.contains(canary_name) {
                compile_rooted_backend_canary_without_output(&canary)
            } else {
                compile_native_canary_without_output(&canary)
            };
            result.err().map(|diagnostics| {
                format!(
                    "{}:\n{}",
                    canary.display(),
                    diagnostics
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            })
        })
        .into_iter()
        .flatten(),
    );

    assert!(
        filter.is_none() || selected_count > 0,
        "OMEGA_PASS_CANARY_FILTER matched no active pass canaries"
    );
    assert!(
        failures.is_empty(),
        "{} pass canary(ies) failed to compile:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

#[test]
fn discovered_exact_native_coverage_is_consistent() {
    let started = std::time::Instant::now();
    let coverage = exact_native_coverage::ExactNativeCanaryCoverageIndex::discover()
        .expect("canary test sources should form one exact-native coverage index");
    let elapsed = started.elapsed();
    let rooted_active = ROOTED_BACKEND_PASS_CANARIES
        .iter()
        .copied()
        .filter(|canary| ACTIVE_PASS_CANARIES.contains(canary))
        .collect::<Vec<_>>();
    let uniquely_rooted = rooted_active
        .iter()
        .filter(|canary| coverage.unique_rooted_owner(canary).is_some())
        .count();
    let direct_active = ACTIVE_PASS_CANARIES
        .iter()
        .copied()
        .filter(|canary| !ROOTED_BACKEND_PASS_CANARIES.contains(canary))
        .collect::<Vec<_>>();
    let uniquely_direct = direct_active
        .iter()
        .filter(|canary| coverage.unique_direct_owner(canary).is_some())
        .count();
    let uniquely_cross_target = CROSS_TARGET_PASS_CANARIES
        .iter()
        .filter(|(canary, target)| coverage.unique_cross_target_owner(canary, target).is_some())
        .count();
    let uniquely_rooted_target = ROOTED_TARGET_BACKEND_PASS_CANARIES
        .iter()
        .filter(|(canary, target)| {
            coverage
                .unique_rooted_target_owner(canary, target)
                .is_some()
        })
        .count();
    assert_eq!(
        uniquely_rooted,
        exact_native_coverage::EXPECTED_UNIQUE_ROOTED_ACTIVE_COVERAGE,
        "the discovered rooted duplicate-elision cohort must change deliberately after auditing every added or removed owner"
    );
    assert_eq!(
        uniquely_direct,
        exact_native_coverage::EXPECTED_UNIQUE_DIRECT_ACTIVE_COVERAGE,
        "the discovered direct duplicate-elision cohort must change deliberately after auditing every added or removed owner"
    );
    assert_eq!(
        uniquely_cross_target,
        exact_native_coverage::EXPECTED_UNIQUE_CROSS_TARGET_COVERAGE,
        "the discovered cross-target duplicate-elision cohort must change deliberately after auditing every exact fixture/target owner"
    );
    assert_eq!(
        uniquely_rooted_target,
        exact_native_coverage::EXPECTED_UNIQUE_ROOTED_TARGET_COVERAGE,
        "the discovered rooted-target duplicate-elision cohort must change deliberately after auditing every exact fixture/target owner"
    );
    assert!(coverage.source_file_count() >= 25);
    assert!(coverage.source_byte_count() > 1_000_000);
    assert!(coverage.test_body_count() > coverage.qualifying_test_count());
    for canary in rooted_active
        .iter()
        .filter(|canary| coverage.unique_rooted_owner(canary).is_some())
    {
        let fixture = pass_canary(canary);
        assert!(
            fixture.join("main.omg").is_file() && fixture.join("build.omg").is_file(),
            "{} must retain both its source and authored root",
            canary
        );
    }
    for canary in direct_active
        .iter()
        .filter(|canary| coverage.unique_direct_owner(canary).is_some())
    {
        assert!(
            pass_canary(canary).join("main.omg").is_file(),
            "{} must retain its compiled source fixture",
            canary
        );
    }
    let positive = coverage
        .unique_rooted_owner("arithmetic/runtime_unsigned_modulo_call_argument_exit")
        .expect("known rooted exact-native owner should be discovered");
    assert_eq!(positive.expected_status, 70);
    assert_eq!(
        positive.test_name,
        "runtime_unsigned_modulo_call_argument_exit_canary_runs"
    );
    assert!(
        positive
            .source_path
            .ends_with("canary_suite/arithmetic_and_data/enum_and_comparison_canaries.rs")
    );
    // The linear-transfer fixture was the corpus example of an ambiguous
    // rooted owner until its second, dump-reading test was removed with the
    // debug dumps; it is now uniquely owned and elided like every other rooted
    // fixture. Synthetic ambiguity stays covered by the source-index unit test.
    let linear_transfer = coverage
        .unique_rooted_owner("ownership/linear_transfer_and_consume")
        .expect("the linear-transfer fixture keeps exactly one rooted exact-native owner");
    assert_eq!(
        linear_transfer.test_name,
        "backend_report_renders_ownership_summary_events"
    );
    #[cfg(windows)]
    {
        let rooted_windows_positive = coverage
            .unique_rooted_owner("host/runtime_user32_key_state_exit")
            .expect("known rooted Windows exact-native owner should be discovered");
        assert_eq!(rooted_windows_positive.expected_status, 70);
        assert_eq!(
            rooted_windows_positive.test_name,
            "runtime_user32_key_state_exit_canary_runs"
        );
    }
    #[cfg(not(windows))]
    assert_eq!(
        coverage.rooted_owner_count("host/runtime_user32_key_state_exit"),
        0
    );
    assert_eq!(
        coverage.direct_owner_count("host/runtime_user32_key_state_exit"),
        0
    );
    assert_eq!(
        coverage.direct_owner_count("traits/boundary_trait_effects_host_call"),
        0
    );
    let cross_target_positive = coverage
        .unique_cross_target_owner("targets/sysv_small_result_entry", "linux_x86_64")
        .expect("known exact cross-target owner should be discovered");
    assert_eq!(
        cross_target_positive.test_name,
        "sysv_small_result_entry_loads_rax_and_rdx"
    );
    assert!(
        cross_target_positive
            .source_path
            .ends_with("canary_suite/entry_and_abi/sysv_entry_abi.rs")
    );
    assert_eq!(
        coverage.cross_target_owner_count("build/receiver_bound_program_entry", "windows_x86_64"),
        0,
        "known cross-target control without a dedicated exact owner must remain in the umbrella"
    );
    let rooted_target_positive = coverage
        .unique_rooted_target_owner("providers/external_leaf_syscall_compile", "linux_arm64")
        .expect("known exact rooted-target owner should be discovered");
    assert_eq!(
        rooted_target_positive.test_name,
        "external_leaf_syscall_reaches_linux_x64_backend"
    );
    assert_eq!(
        coverage.rooted_target_owner_count("time/runtime_time_host_native_exit", "windows_x86_64"),
        0,
        "known rooted-target control without a dedicated exact owner must remain in the umbrella"
    );
    eprintln!(
        "exact-native coverage index: files={} bytes={} test-bodies={} qualifying-tests={} qualifying-target-compiles={} unique-rooted-active={} unique-direct-active={} unique-cross-target={} unique-rooted-target={} scan-micros={}",
        coverage.source_file_count(),
        coverage.source_byte_count(),
        coverage.test_body_count(),
        coverage.qualifying_test_count(),
        coverage.qualifying_target_compile_count(),
        uniquely_rooted,
        uniquely_direct,
        uniquely_cross_target,
        uniquely_rooted_target,
        elapsed.as_micros(),
    );
}
