//! Aggregate reconciliation between retained review usage and its sponsor.

use super::super::CompileResolvedPackageReviewsError;
use checked_interpreter::{BuildEvaluationConsumptionReceipt, BuildEvaluationSponsor};

/// `usages` carries the evaluation usage of every compile this session's
/// sponsor actually ran — one entry per freshly compiled occurrence, `None`
/// where the compile ran no build machine. An occurrence carried forward from
/// a prior pass contributes nothing: its evaluation was consumed and
/// reconciled by the session that ran it, so a pass that reuses every
/// occurrence reconciles as the empty session it was.
///
/// `discarded` carries sponsored evaluations that ran inside a compile whose
/// verdict no retained review reports — for example a component-closure
/// rejection the caller answered and compiled again. Their consumption is
/// real session accounting and reconciles beside the retained usages.
pub(super) fn verify_build_session_accounting(
    usages: &[Option<build_evaluation::BuildEvaluationUsage>],
    discarded: &[build_evaluation::BuildEvaluationUsage],
    consumed: &[BuildEvaluationConsumptionReceipt],
    sponsor: &BuildEvaluationSponsor,
) -> Result<(), CompileResolvedPackageReviewsError> {
    sponsor
        .verify_retained_consumptions(consumed)
        .map_err(
            |_| CompileResolvedPackageReviewsError::BuildEvaluationAccountingMismatch {
                reported: None,
                sponsored: sponsor.consumed_fuel_units(),
            },
        )?;
    verify_fuel(usages, discarded, consumed, sponsor)?;
    verify_build_log(usages, discarded, consumed, sponsor)?;
    verify_filesystem_attempts(usages, discarded, sponsor)?;
    verify_live_filesystem_handles(usages, discarded, sponsor)?;
    verify_live_cells(usages, discarded, sponsor)?;
    verify_live_text_bytes(usages, discarded, sponsor)?;
    verify_results(usages, discarded, consumed, sponsor)
}

/// Reported usage of every evaluation the sponsor consumed: retained usages
/// plus discarded attempts. `None` when any fresh compile carried no usage at
/// all, which the verifiers surface as a mismatch rather than silently
/// dropping.
fn reported_usages<'a>(
    usages: &'a [Option<build_evaluation::BuildEvaluationUsage>],
    discarded: &'a [build_evaluation::BuildEvaluationUsage],
) -> Option<Vec<build_evaluation::BuildEvaluationUsage>> {
    usages
        .iter()
        .copied()
        .collect::<Option<Vec<_>>>()
        .map(|usages| {
            usages
                .into_iter()
                .chain(discarded.iter().copied())
                .collect()
        })
}

/// Cumulative policy accounts include current-account charges for retained
/// computations. These are kept distinct from actual execution observations;
/// in particular they never enter the live-resource peak reconciliation below.
fn policy_usages(
    usages: &[Option<build_evaluation::BuildEvaluationUsage>],
    discarded: &[build_evaluation::BuildEvaluationUsage],
    consumed: &[BuildEvaluationConsumptionReceipt],
) -> Option<Vec<build_evaluation::BuildEvaluationUsage>> {
    reported_usages(usages, discarded).map(|mut usages| {
        usages.extend(
            consumed
                .iter()
                .map(BuildEvaluationConsumptionReceipt::producing_usage),
        );
        usages
    })
}

/// Session-observed peaks of every evaluation the sponsor consumed. Compiles
/// that reported no usage contribute nothing here because the sponsor's
/// peaks are the session's, not the compile's.
fn observed_usages<'a>(
    usages: &'a [Option<build_evaluation::BuildEvaluationUsage>],
    discarded: &'a [build_evaluation::BuildEvaluationUsage],
) -> impl Iterator<Item = build_evaluation::BuildEvaluationUsage> + 'a {
    usages
        .iter()
        .filter_map(|usage| *usage)
        .chain(discarded.iter().copied())
}

fn verify_fuel(
    usages: &[Option<build_evaluation::BuildEvaluationUsage>],
    discarded: &[build_evaluation::BuildEvaluationUsage],
    consumed: &[BuildEvaluationConsumptionReceipt],
    sponsor: &BuildEvaluationSponsor,
) -> Result<(), CompileResolvedPackageReviewsError> {
    let reported = policy_usages(usages, discarded, consumed).and_then(|usages| {
        usages
            .iter()
            .try_fold(0_u64, |total, usage| total.checked_add(usage.fuel_units))
    });
    let sponsored = sponsor.consumed_fuel_units();
    if reported != Some(sponsored) {
        return Err(
            CompileResolvedPackageReviewsError::BuildEvaluationAccountingMismatch {
                reported,
                sponsored,
            },
        );
    }
    Ok(())
}

fn verify_build_log(
    usages: &[Option<build_evaluation::BuildEvaluationUsage>],
    discarded: &[build_evaluation::BuildEvaluationUsage],
    consumed: &[BuildEvaluationConsumptionReceipt],
    sponsor: &BuildEvaluationSponsor,
) -> Result<(), CompileResolvedPackageReviewsError> {
    let reported = policy_usages(usages, discarded, consumed).and_then(|usages| {
        usages.iter().try_fold(0_u64, |total, usage| {
            total.checked_add(usage.build_log_bytes)
        })
    });
    let sponsored = sponsor.consumed_build_log_bytes();
    if reported != Some(sponsored) {
        return Err(
            CompileResolvedPackageReviewsError::BuildLogAccountingMismatch {
                reported,
                sponsored,
            },
        );
    }
    Ok(())
}

fn verify_filesystem_attempts(
    usages: &[Option<build_evaluation::BuildEvaluationUsage>],
    discarded: &[build_evaluation::BuildEvaluationUsage],
    sponsor: &BuildEvaluationSponsor,
) -> Result<(), CompileResolvedPackageReviewsError> {
    let reported = reported_usages(usages, discarded).and_then(|usages| {
        usages.iter().try_fold(0_u64, |total, usage| {
            total.checked_add(usage.filesystem_operation_attempts)
        })
    });
    let sponsored = sponsor.consumed_filesystem_operation_attempts();
    if reported != Some(sponsored) {
        return Err(
            CompileResolvedPackageReviewsError::BuildFilesystemAttemptAccountingMismatch {
                reported,
                sponsored,
            },
        );
    }
    Ok(())
}

fn verify_live_filesystem_handles(
    usages: &[Option<build_evaluation::BuildEvaluationUsage>],
    discarded: &[build_evaluation::BuildEvaluationUsage],
    sponsor: &BuildEvaluationSponsor,
) -> Result<(), CompileResolvedPackageReviewsError> {
    let reported_peak = Some(
        observed_usages(usages, discarded)
            .map(|usage| usage.session_peak_live_filesystem_handles)
            .max()
            .unwrap_or(0),
    );
    let sponsored_live = sponsor.live_filesystem_handles();
    let sponsored_peak = sponsor.peak_live_filesystem_handles();
    if reported_peak != Some(sponsored_peak) || sponsored_live != 0 {
        return Err(
            CompileResolvedPackageReviewsError::BuildLiveFilesystemHandleAccountingMismatch {
                reported_peak,
                sponsored_peak,
                sponsored_live,
            },
        );
    }
    Ok(())
}

fn verify_live_cells(
    usages: &[Option<build_evaluation::BuildEvaluationUsage>],
    discarded: &[build_evaluation::BuildEvaluationUsage],
    sponsor: &BuildEvaluationSponsor,
) -> Result<(), CompileResolvedPackageReviewsError> {
    let reported_invocation_peak = Some(
        observed_usages(usages, discarded)
            .map(|usage| usage.peak_live_cells)
            .max()
            .unwrap_or(0),
    );
    // An entirely carried session ran nothing: its reported peak is the empty
    // session's zero, matching the sponsor's untouched peak.
    let reported_session_peak = Some(
        observed_usages(usages, discarded)
            .map(|usage| usage.session_peak_live_cells)
            .max()
            .unwrap_or(0),
    );
    let sponsored_live = sponsor.live_cells();
    let sponsored_peak = sponsor.peak_live_cells();
    if reported_invocation_peak != Some(sponsored_peak)
        || reported_session_peak != Some(sponsored_peak)
        || sponsored_live != 0
    {
        return Err(
            CompileResolvedPackageReviewsError::BuildLiveCellAccountingMismatch {
                reported_invocation_peak,
                reported_session_peak,
                sponsored_peak,
                sponsored_live,
            },
        );
    }
    Ok(())
}

fn verify_live_text_bytes(
    usages: &[Option<build_evaluation::BuildEvaluationUsage>],
    discarded: &[build_evaluation::BuildEvaluationUsage],
    sponsor: &BuildEvaluationSponsor,
) -> Result<(), CompileResolvedPackageReviewsError> {
    let reported_invocation_peak = Some(
        observed_usages(usages, discarded)
            .map(|usage| usage.peak_live_text_bytes)
            .max()
            .unwrap_or(0),
    );
    let reported_session_peak = Some(
        observed_usages(usages, discarded)
            .map(|usage| usage.session_peak_live_text_bytes)
            .max()
            .unwrap_or(0),
    );
    let sponsored_live = sponsor.live_text_bytes();
    let sponsored_peak = sponsor.peak_live_text_bytes();
    if reported_invocation_peak != Some(sponsored_peak)
        || reported_session_peak != Some(sponsored_peak)
        || sponsored_live != 0
    {
        return Err(
            CompileResolvedPackageReviewsError::BuildLiveTextByteAccountingMismatch {
                reported_invocation_peak,
                reported_session_peak,
                sponsored_peak,
                sponsored_live,
            },
        );
    }
    Ok(())
}

fn verify_results(
    usages: &[Option<build_evaluation::BuildEvaluationUsage>],
    discarded: &[build_evaluation::BuildEvaluationUsage],
    consumed: &[BuildEvaluationConsumptionReceipt],
    sponsor: &BuildEvaluationSponsor,
) -> Result<(), CompileResolvedPackageReviewsError> {
    let reported_cells = policy_usages(usages, discarded, consumed).and_then(|usages| {
        usages
            .iter()
            .try_fold(0_u64, |total, usage| total.checked_add(usage.result_cells))
    });
    let reported_text_bytes = policy_usages(usages, discarded, consumed).and_then(|usages| {
        usages.iter().try_fold(0_u64, |total, usage| {
            total.checked_add(usage.result_text_bytes)
        })
    });
    let sponsored_cells = sponsor.consumed_result_cells();
    let sponsored_text_bytes = sponsor.consumed_result_text_bytes();
    if reported_cells != Some(sponsored_cells) || reported_text_bytes != Some(sponsored_text_bytes)
    {
        return Err(
            CompileResolvedPackageReviewsError::BuildResultCustodyAccountingMismatch {
                reported_cells,
                sponsored_cells,
                reported_text_bytes,
                sponsored_text_bytes,
            },
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sponsor() -> BuildEvaluationSponsor {
        BuildEvaluationSponsor::new(
            checked_interpreter::BuildEvaluationSponsorLimits::new(
                1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000,
                1_000_000,
            )
            .unwrap(),
        )
    }

    struct BuildFixture(std::path::PathBuf);
    impl BuildFixture {
        fn new() -> Self {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "omega-build-charge-reconciliation-{}-{stamp}",
                std::process::id()
            ));
            std::fs::create_dir(&root).unwrap();
            std::fs::write(root.join("main.omg"), "machine main() {}\n").unwrap();
            std::fs::write(root.join("build.omg"), "machine build(builder: &mut Build) { Build::package(builder, \"accounting_fixture\"); builder.log.write_line(\"accounting witness\"); }\n").unwrap();
            Self(root)
        }
        fn compile(
            &self,
            sponsor: &BuildEvaluationSponsor,
            target: Option<&str>,
        ) -> compilation_products::CheckedCompileOutcome {
            compiler::compile_checked(compilation_products::CheckedCompileRequest {
                evaluation_sponsor: Some(sponsor.clone()),
                ..compilation_products::CheckedCompileRequest::new(&self.0.join("main.omg"), target)
            })
        }
        fn usage(
            &self,
            sponsor: &BuildEvaluationSponsor,
        ) -> build_evaluation::BuildEvaluationUsage {
            let outcome = self.compile(sponsor, None);
            assert!(outcome.result.is_ok(), "{:?}", outcome.result.err());
            let compilation_products::BuildEvaluationAccounting::Executed(usage) =
                outcome.build_accounting
            else {
                panic!("fresh request must execute Build");
            };
            usage
        }
    }
    impl Drop for BuildFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn session_accounting_reconciles_cache_only_without_historical_peaks() {
        let fixture = BuildFixture::new();
        let producing = sponsor();
        let usage = fixture.usage(&producing);
        let consuming = sponsor();
        let receipt = consuming.charge_retained_pure_usage(usage).unwrap();
        verify_build_session_accounting(&[], &[], &[receipt.clone()], &consuming).unwrap();
        assert_eq!(receipt.producing_usage(), usage);
        assert_eq!(consuming.peak_live_cells(), 0);
        assert_eq!(consuming.peak_live_text_bytes(), 0);
        assert!(
            verify_build_session_accounting(&[Some(usage)], &[], &[receipt], &consuming).is_err(),
            "the producing execution must not also be reported as current execution"
        );
    }

    #[test]
    fn session_accounting_reconciles_cached_and_fresh_work_in_both_orders() {
        let fixture = BuildFixture::new();
        let usage = fixture.usage(&sponsor());
        for cached_first in [true, false] {
            let current = sponsor();
            let (receipt, actual) = if cached_first {
                let receipt = current.charge_retained_pure_usage(usage).unwrap();
                (receipt, fixture.usage(&current))
            } else {
                let actual = fixture.usage(&current);
                (current.charge_retained_pure_usage(usage).unwrap(), actual)
            };
            verify_build_session_accounting(&[Some(actual)], &[], &[receipt.clone()], &current)
                .unwrap();
            // A later product/review rejection moves only the actual result's
            // disposition; the current-account reservation remains payable.
            verify_build_session_accounting(&[], &[actual], &[receipt], &current).unwrap();
            assert_eq!(
                current.consumed_fuel_units(),
                usage.fuel_units + actual.fuel_units
            );
        }
    }

    #[test]
    fn failed_final_settlement_retains_consumed_charge_and_retry_disposition() {
        let fixture = BuildFixture::new();
        std::fs::write(
            fixture.0.join("main.omg"),
            "data Main {} machine Main::main(&mut self) {}\n",
        )
        .unwrap();
        std::fs::write(fixture.0.join("build.omg"),
            "machine build(builder: &mut Build) { Build::application(builder, \"accounting_fixture\"); builder.roots.bind(windows_x86_64::ProgramEntry, Main::main); }\n").unwrap();
        let package = semantic_vocabulary::PackageKeyIdentity::from_digest([93; 32]).unwrap();
        let inputs = package_compilation::PackageCompilationInputs::new(
            package,
            package_compilation::BuildDeclarationKind::Application,
            vec![package_compilation::PackageSourceBinding::new(
                package,
                "accounting_fixture",
                fixture.0.clone(),
            )],
            Vec::new(),
        )
        .unwrap();
        let owner = build_evaluation::PureBuildCompilation::default();
        let compile = |account: &BuildEvaluationSponsor, target| {
            compiler::compile_checked_reusing_builds(
                compilation_products::CheckedCompileRequest {
                    evaluation_sponsor: Some(account.clone()),
                    package_inputs: Some(inputs.clone()),
                    ..compilation_products::CheckedCompileRequest::new(
                        &fixture.0.join("main.omg"),
                        target,
                    )
                },
                &owner,
            )
        };
        let producing = sponsor();
        let produced = compile(&producing, Some("linux_x86_64"));
        let errors = produced
            .result
            .err()
            .expect("missing target row fails after Build");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("no bound required root")),
            "{errors:?}"
        );
        let compilation_products::BuildEvaluationAccounting::Executed(usage) =
            produced.build_accounting
        else {
            panic!("failed product must retain completed Build execution");
        };
        verify_build_session_accounting(&[], &[usage], &[], &producing).unwrap();
        let current = sponsor();
        let consumed = compile(&current, Some("linux_x86_64"));
        assert!(
            consumed.result.is_err(),
            "consumption cannot admit the missing target row"
        );
        let compilation_products::BuildEvaluationAccounting::Consumed(first) =
            consumed.build_accounting
        else {
            panic!("second failed product must consume the completed Pure Build");
        };
        verify_build_session_accounting(&[], &[], &[first.clone()], &current).unwrap();
        let retried = compile(&current, None);
        assert!(retried.result.is_ok(), "{:?}", retried.result.err());
        let compilation_products::BuildEvaluationAccounting::Consumed(second) =
            retried.build_accounting
        else {
            panic!("retry consumes the same value with its own current-attempt charge");
        };
        verify_build_session_accounting(&[], &[], &[first, second], &current).unwrap();
        assert_eq!(current.peak_live_cells(), 0);
        assert_eq!(current.consumed_fuel_units(), 2 * usage.fuel_units);
    }
}
