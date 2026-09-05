//! Render findings and load exact project choices in the install/update flow.

use super::model::{PackageCommandError, failure};
use super::state;
use crate::operations::{PackageChangeReview, PackageFileTransaction};
use crate::review::{
    PackagePolicyResolution, PackagePolicyReviewError, recover_package_policy_review,
    render_package_policy_review,
};
use platform_custody::record_file::{RecordFileRoot, RootRecordRead};
use std::path::PathBuf;

pub(super) struct Choices<'a> {
    pub blocked: bool,
    pub resolutions: Vec<PackagePolicyResolution>,
    pub reads: Vec<RootRecordRead<'a>>,
    pub report: String,
    pub paths: Vec<PathBuf>,
}

pub(super) fn prepare<'a>(
    files: &'a RecordFileRoot,
    transaction: &PackageFileTransaction,
    reviews: &[PackageChangeReview],
    resume: bool,
    missing_baseline: bool,
) -> Result<Choices<'a>, PackageCommandError> {
    let mut result = Choices {
        blocked: false,
        resolutions: Vec::new(),
        reads: Vec::new(),
        report: String::new(),
        paths: Vec::new(),
    };
    if missing_baseline {
        result
            .report
            .push_str("No accepted lock baseline: reviewing the complete candidate graph.\n");
    }
    let mut total_bytes = 0usize;
    for review in reviews {
        let name = state::review_name(review.target());
        if !resume {
            let text = render_package_policy_review(review.changes(), state::LIMITS.maximum_bytes)
                .map_err(failure)?;
            total_bytes = total_bytes
                .checked_add(text.len())
                .filter(|total| *total <= state::LIMITS.maximum_bytes)
                .ok_or_else(|| {
                    failure("combined package review documents exceed the byte limit")
                })?;
            state::write(files, &name, &text)?;
        }
        let read = state::read(files, &name)?.ok_or_else(|| {
            failure(format!(
                "missing review document {name}; discard the proposal and regenerate findings"
            ))
        })?;
        if resume {
            total_bytes = total_bytes
                .checked_add(read.bytes().len())
                .filter(|total| *total <= state::LIMITS.maximum_bytes)
                .ok_or_else(|| {
                    failure("combined package review documents exceed the byte limit")
                })?;
        }
        let text = state::text(&read)?;
        match recover_package_policy_review(review.changes(), text, state::LIMITS.maximum_bytes) {
            Ok(resolution) => {
                if !resolution.all_required_changes_accepted() {
                    result.blocked = true;
                    result.report.push_str(&format!(
                        "{}: one or more changes were rejected; this proposal cannot publish.\n",
                        review.target().target_name()
                    ));
                }
                result.resolutions.push(resolution);
            }
            Err(PackagePolicyReviewError::UnresolvedDecision(_)) => {
                result.blocked = true;
            }
            Err(error) => {
                return Err(failure(format!(
                    "cannot resume {name}: {error}; accepted project files are unchanged"
                )));
            }
        }
        result.reads.push(read);
        result
            .paths
            .push(state::review_path(transaction, review.target()));
        result.report.push_str(&format!(
            "{}: checked {} source packages.\n",
            review.target().target_name(),
            review.source_closure().custodies().len()
        ));
        for package in review.changes().packages() {
            if package.audit_recommended() {
                let identity = package.key().identity().digest()[..6]
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
                result.report.push_str(&format!(
                    "Audit recommended: {} [{identity}].\n",
                    package.key().name().as_str(),
                ));
            }
        }
        if !missing_baseline && review.changes().source_subject_changed() {
            result.report.push_str("Source pins changed. Previous source was not loaded; use standalone candidate review or obtain the source diff separately.\n");
        }
    }
    Ok(result)
}
