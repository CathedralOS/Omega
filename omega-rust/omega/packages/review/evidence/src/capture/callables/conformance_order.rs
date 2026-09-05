//! Canonical exact association ordering for legacy and policy projections.

use crate::capture::source::ProjectedReviewRow;
use crate::record::{
    PackagePolicyCallableConformance, PackageReviewCallableConformance,
    PackageReviewExternalExecutableSupply, PackageReviewOperatorRealization,
};
use psi_diagnostics::Diagnostic;

pub(super) fn finish(
    subject: &str,
    projected: &mut [PackageReviewCallableConformance],
    operator_realizations: &mut [PackageReviewOperatorRealization],
    external_executable_supply: &mut [ProjectedReviewRow<PackageReviewExternalExecutableSupply>],
    policy: Option<&mut Vec<PackagePolicyCallableConformance>>,
) -> Result<(), Vec<Diagnostic>> {
    projected.sort();
    if policy.is_none() && projected.windows(2).any(|rows| rows[0] == rows[1]) {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` contains a duplicate exact trait realization",
            subject
        ))]);
    }
    operator_realizations.sort();
    if operator_realizations
        .windows(2)
        .any(|rows| rows[0] == rows[1])
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` contains a duplicate exact operator realization",
            subject
        ))]);
    }
    external_executable_supply.sort_by(|left, right| left.row.cmp(&right.row));
    if external_executable_supply
        .windows(2)
        .any(|rows| rows[0].row == rows[1].row)
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed external callable `{}` contains duplicate executable-supply identity",
            subject
        ))]);
    }
    if let Some(policy) = policy {
        policy.sort();
        if policy.windows(2).any(|rows| rows[0] == rows[1]) {
            return Err(vec![Diagnostic::error(format!(
                "reviewed callable {subject} repeats an exact policy conformance application"
            ))]);
        }
    }
    Ok(())
}
