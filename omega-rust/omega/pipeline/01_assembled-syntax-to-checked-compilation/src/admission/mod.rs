//! Required checked-program trust settlement, independent of observation output.
//!
//! `admit_checked_compilation` reconstructs the trust obligations of one sealed
//! checked compilation and settles them against the owner's explicit
//! admissions without writing debug dumps.

use crate::CheckedCompilation;
use diagnostics::Diagnostic;
use trust_model::{TrustAdmission, TrustAdmissionSettlement};

/// Validated trust settlement for one checked compilation.
pub struct CheckedAdmission {
    settlement: TrustAdmissionSettlement,
}

impl CheckedAdmission {
    /// Retain the validated admission outcome.
    pub fn into_settlement(self) -> TrustAdmissionSettlement {
        self.settlement
    }
}

/// Reconstruct and validate trust against explicit owner admissions. This step
/// runs independently of report-file policy and grants no package-review authority.
pub fn admit_checked_compilation(
    checked: &CheckedCompilation,
    accepted: &[TrustAdmission],
) -> Result<CheckedAdmission, Vec<Diagnostic>> {
    let obligations = trust_model::reconstruct_trust_obligations(
        &checked.typed,
        checked,
        checked.root_grants(),
        checked.provider_plans(),
        checked.selected_provider_plans(),
        checked.accepted_template_classifications(),
        checked.package_identity().is_some(),
    )?;
    let settlement = trust_model::settle_trust_admissions(obligations, accepted)
        .map_err(|diagnostic| vec![diagnostic])?;
    let trust_report = trust_model::reconstruct_trust_report(
        checked,
        checked.root_grants(),
        checked.provider_plans(),
        checked.selected_provider_plans(),
        checked.accepted_template_classifications(),
    )?;
    trust_report
        .validate()
        .map_err(|diagnostic| vec![diagnostic])?;
    Ok(CheckedAdmission { settlement })
}
