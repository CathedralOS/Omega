//! Canonical exact association ordering for callable policy rows.

use crate::record::{PackagePolicyCallableConformance, PackageReviewOperatorRealization};
use diagnostics::Diagnostic;

pub(super) fn finish(
    subject: &str,
    conformances: &mut [PackagePolicyCallableConformance],
    operator_realizations: &mut [PackageReviewOperatorRealization],
    requirement_realizations: &mut [crate::record::PackageReviewRequirementRealization],
) -> Result<(), Vec<Diagnostic>> {
    conformances.sort();
    if conformances.windows(2).any(|rows| rows[0] == rows[1]) {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable {subject} repeats an exact policy conformance application"
        ))]);
    }
    operator_realizations.sort();
    if operator_realizations
        .windows(2)
        .any(|rows| rows[0] == rows[1])
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{subject}` contains a duplicate exact operator realization"
        ))]);
    }
    requirement_realizations.sort();
    if requirement_realizations
        .windows(2)
        .any(|rows| rows[0] == rows[1])
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{subject}` contains a duplicate exact top-level requirement realization"
        ))]);
    }
    Ok(())
}
