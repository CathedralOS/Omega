//! Aggregate reconciliation between retained review usage and its sponsor.

use super::super::{CompileResolvedPackageReviewsError, CompilerIssuedPackageReview};
use psi_checked_interpreter::BuildEvaluationSponsor;

pub(super) fn verify_build_session_accounting(
    reviews: &[CompilerIssuedPackageReview],
    sponsor: &BuildEvaluationSponsor,
) -> Result<(), CompileResolvedPackageReviewsError> {
    verify_fuel(reviews, sponsor)?;
    verify_build_log(reviews, sponsor)?;
    verify_filesystem_attempts(reviews, sponsor)?;
    verify_live_filesystem_handles(reviews, sponsor)?;
    verify_live_cells(reviews, sponsor)?;
    verify_live_text_bytes(reviews, sponsor)?;
    verify_results(reviews, sponsor)
}

fn verify_fuel(
    reviews: &[CompilerIssuedPackageReview],
    sponsor: &BuildEvaluationSponsor,
) -> Result<(), CompileResolvedPackageReviewsError> {
    let reported = reviews.iter().try_fold(0_u64, |total, review| {
        let usage = review.build_evaluation_usage()?;
        total
            .checked_add(usage.fuel_units)
            .and_then(|total| total.checked_add(usage.replay_fuel_units))
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
    sponsor: &BuildEvaluationSponsor,
) -> Result<(), CompileResolvedPackageReviewsError> {
    let reported = reviews.iter().try_fold(0_u64, |total, review| {
        let usage = review.build_evaluation_usage()?;
        total
            .checked_add(usage.build_log_bytes)
            .and_then(|total| total.checked_add(usage.replay_build_log_bytes))
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
    sponsor: &BuildEvaluationSponsor,
) -> Result<(), CompileResolvedPackageReviewsError> {
    let reported = reviews.iter().try_fold(0_u64, |total, review| {
        let usage = review.build_evaluation_usage()?;
        total
            .checked_add(usage.filesystem_operation_attempts)
            .and_then(|total| total.checked_add(usage.replay_filesystem_operation_attempts))
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
    sponsor: &BuildEvaluationSponsor,
) -> Result<(), CompileResolvedPackageReviewsError> {
    let reported_peak = Some(
        reviews
            .iter()
            .filter_map(|review| review.build_evaluation_usage())
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
    sponsor: &BuildEvaluationSponsor,
) -> Result<(), CompileResolvedPackageReviewsError> {
    let reported_invocation_peak = Some(
        reviews
            .iter()
            .filter_map(|review| review.build_evaluation_usage())
            .flat_map(|usage| [usage.peak_live_cells, usage.replay_peak_live_cells])
            .max()
            .unwrap_or(0),
    );
    let reported_session_peak = reviews
        .iter()
        .filter_map(|review| review.build_evaluation_usage())
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
    sponsor: &BuildEvaluationSponsor,
) -> Result<(), CompileResolvedPackageReviewsError> {
    let reported_invocation_peak = Some(
        reviews
            .iter()
            .filter_map(|review| review.build_evaluation_usage())
            .flat_map(|usage| {
                [
                    usage.peak_live_text_bytes,
                    usage.replay_peak_live_text_bytes,
                ]
            })
            .max()
            .unwrap_or(0),
    );
    let reported_session_peak = reviews
        .iter()
        .filter_map(|review| review.build_evaluation_usage())
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
    sponsor: &BuildEvaluationSponsor,
) -> Result<(), CompileResolvedPackageReviewsError> {
    let reported_cells = reviews.iter().try_fold(0_u64, |total, review| {
        let usage = review.build_evaluation_usage()?;
        total
            .checked_add(usage.result_cells)
            .and_then(|total| total.checked_add(usage.replay_result_cells))
    });
    let reported_text_bytes = reviews.iter().try_fold(0_u64, |total, review| {
        let usage = review.build_evaluation_usage()?;
        total
            .checked_add(usage.result_text_bytes)
            .and_then(|total| total.checked_add(usage.replay_result_text_bytes))
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
