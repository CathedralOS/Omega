//! Aggregate reconciliation between retained review usage and its sponsor.

use super::super::{CompileResolvedPackageReviewsError, CompilerIssuedPackageReview};
use checked_interpreter::BuildEvaluationSponsor;

/// `discarded` carries sponsored evaluations that ran inside a compile whose
/// verdict no retained review reports — for example a component-closure
/// rejection the caller answered and compiled again. Their consumption is
/// real session accounting and reconciles beside the retained reviews.
pub(super) fn verify_build_session_accounting(
    reviews: &[CompilerIssuedPackageReview],
    discarded: &[build_evaluation::BuildEvaluationUsage],
    sponsor: &BuildEvaluationSponsor,
) -> Result<(), CompileResolvedPackageReviewsError> {
    verify_fuel(reviews, discarded, sponsor)?;
    verify_build_log(reviews, discarded, sponsor)?;
    verify_filesystem_attempts(reviews, discarded, sponsor)?;
    verify_live_filesystem_handles(reviews, discarded, sponsor)?;
    verify_live_cells(reviews, discarded, sponsor)?;
    verify_live_text_bytes(reviews, discarded, sponsor)?;
    verify_results(reviews, discarded, sponsor)
}

/// Reported usage of every evaluation the sponsor consumed: retained reviews
/// plus discarded attempts. `None` when any review carries no usage at all,
/// which the verifiers surface as a mismatch rather than silently dropping.
fn reported_usages<'a>(
    reviews: &'a [CompilerIssuedPackageReview],
    discarded: &'a [build_evaluation::BuildEvaluationUsage],
) -> Option<Vec<build_evaluation::BuildEvaluationUsage>> {
    reviews
        .iter()
        .map(|review| review.build_evaluation_usage())
        .collect::<Option<Vec<_>>>()
        .map(|usages| {
            usages
                .into_iter()
                .chain(discarded.iter().copied())
                .collect()
        })
}

/// Session-observed peaks of every evaluation the sponsor consumed. Retained
/// reviews that carry no usage contribute nothing here because the sponsor's
/// peaks are the session's, not the review's.
fn observed_usages<'a>(
    reviews: &'a [CompilerIssuedPackageReview],
    discarded: &'a [build_evaluation::BuildEvaluationUsage],
) -> impl Iterator<Item = build_evaluation::BuildEvaluationUsage> + 'a {
    reviews
        .iter()
        .filter_map(|review| review.build_evaluation_usage())
        .chain(discarded.iter().copied())
}

fn verify_fuel(
    reviews: &[CompilerIssuedPackageReview],
    discarded: &[build_evaluation::BuildEvaluationUsage],
    sponsor: &BuildEvaluationSponsor,
) -> Result<(), CompileResolvedPackageReviewsError> {
    let reported = reported_usages(reviews, discarded).and_then(|usages| {
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
    reviews: &[CompilerIssuedPackageReview],
    discarded: &[build_evaluation::BuildEvaluationUsage],
    sponsor: &BuildEvaluationSponsor,
) -> Result<(), CompileResolvedPackageReviewsError> {
    let reported = reported_usages(reviews, discarded).and_then(|usages| {
        usages
            .iter()
            .try_fold(0_u64, |total, usage| total.checked_add(usage.build_log_bytes))
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
    reviews: &[CompilerIssuedPackageReview],
    discarded: &[build_evaluation::BuildEvaluationUsage],
    sponsor: &BuildEvaluationSponsor,
) -> Result<(), CompileResolvedPackageReviewsError> {
    let reported = reported_usages(reviews, discarded).and_then(|usages| {
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
    reviews: &[CompilerIssuedPackageReview],
    discarded: &[build_evaluation::BuildEvaluationUsage],
    sponsor: &BuildEvaluationSponsor,
) -> Result<(), CompileResolvedPackageReviewsError> {
    let reported_peak = Some(
        observed_usages(reviews, discarded)
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
    reviews: &[CompilerIssuedPackageReview],
    discarded: &[build_evaluation::BuildEvaluationUsage],
    sponsor: &BuildEvaluationSponsor,
) -> Result<(), CompileResolvedPackageReviewsError> {
    let reported_invocation_peak = Some(
        observed_usages(reviews, discarded).map(|usage| usage.peak_live_cells)
            .max()
            .unwrap_or(0),
    );
    let reported_session_peak = observed_usages(reviews, discarded)
        .map(|usage| usage.session_peak_live_cells)
        .max();
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
    reviews: &[CompilerIssuedPackageReview],
    discarded: &[build_evaluation::BuildEvaluationUsage],
    sponsor: &BuildEvaluationSponsor,
) -> Result<(), CompileResolvedPackageReviewsError> {
    let reported_invocation_peak = Some(
        observed_usages(reviews, discarded).map(|usage| usage.peak_live_text_bytes)
            .max()
            .unwrap_or(0),
    );
    let reported_session_peak = observed_usages(reviews, discarded)
        .map(|usage| usage.session_peak_live_text_bytes)
        .max();
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
    reviews: &[CompilerIssuedPackageReview],
    discarded: &[build_evaluation::BuildEvaluationUsage],
    sponsor: &BuildEvaluationSponsor,
) -> Result<(), CompileResolvedPackageReviewsError> {
    let reported_cells = reported_usages(reviews, discarded).and_then(|usages| {
        usages
            .iter()
            .try_fold(0_u64, |total, usage| total.checked_add(usage.result_cells))
    });
    let reported_text_bytes = reported_usages(reviews, discarded).and_then(|usages| {
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
