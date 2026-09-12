//! Required checked-program trust settlement, independent of observation output.

use crate::CheckedCompilation;
use artifacts::TrustReport;
use diagnostics::Diagnostic;
use trust_model::{TrustAdmission, TrustAdmissionSettlement};

/// Validated trust evidence tied to the exact checked program it describes.
/// Borrowing the program prevents observation from pairing this evidence with
/// another compilation or mutating its subject before use.
pub struct CheckedAdmission<'checked> {
    checked: &'checked CheckedCompilation,
    trust_report: TrustReport,
    settlement: TrustAdmissionSettlement,
}

impl CheckedAdmission<'_> {
    pub(crate) fn checked(&self) -> &CheckedCompilation {
        self.checked
    }

    pub(crate) fn trust_report(&self) -> &TrustReport {
        &self.trust_report
    }

    /// Retain the admission outcome after consuming any requested observations.
    pub fn into_settlement(self) -> TrustAdmissionSettlement {
        self.settlement
    }
}

/// Reconstruct and validate trust against explicit owner admissions. This step
/// runs independently of report-file policy and grants no package-review authority.
pub fn admit_checked_compilation<'checked>(
    checked: &'checked CheckedCompilation,
    accepted: &[TrustAdmission],
) -> Result<CheckedAdmission<'checked>, Vec<Diagnostic>> {
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
    Ok(CheckedAdmission {
        checked,
        trust_report,
        settlement,
    })
}
