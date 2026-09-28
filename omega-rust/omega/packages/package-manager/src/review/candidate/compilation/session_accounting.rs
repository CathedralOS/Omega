//! Aggregate reconciliation between retained review usage and its sponsor.

use super::super::CompileResolvedPackageReviewsError;
use checked_interpreter::BuildEvaluationSponsor;

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
    sponsor: &BuildEvaluationSponsor,
) -> Result<(), CompileResolvedPackageReviewsError> {
    verify_fuel(usages, discarded, sponsor)?;
    verify_build_log(usages, discarded, sponsor)?;
    verify_filesystem_attempts(usages, discarded, sponsor)?;
    verify_live_filesystem_handles(usages, discarded, sponsor)?;
    verify_live_cells(usages, discarded, sponsor)?;
    verify_live_text_bytes(usages, discarded, sponsor)?;
    verify_results(usages, discarded, sponsor)
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
    sponsor: &BuildEvaluationSponsor,
) -> Result<(), CompileResolvedPackageReviewsError> {
    let reported = reported_usages(usages, discarded).and_then(|usages| {
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
    sponsor: &BuildEvaluationSponsor,
) -> Result<(), CompileResolvedPackageReviewsError> {
    let reported = reported_usages(usages, discarded).and_then(|usages| {
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
    sponsor: &BuildEvaluationSponsor,
) -> Result<(), CompileResolvedPackageReviewsError> {
    let reported_cells = reported_usages(usages, discarded).and_then(|usages| {
        usages
            .iter()
            .try_fold(0_u64, |total, usage| total.checked_add(usage.result_cells))
    });
    let reported_text_bytes = reported_usages(usages, discarded).and_then(|usages| {
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
