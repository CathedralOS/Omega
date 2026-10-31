use extents::ExtentLoan;

use crate::AdmittedResourceProfile;
use crate::ResourceProfileReceiptId;
use crate::placements::access_admission::validate_access_admission;
use crate::placements::access_authority::AccessAuthorityRef;
use crate::primitive_access::field_projection::project_access_field;
use crate::{
    AccessAdmissionId, AccessFieldKey, AccessFieldProjection, AccessPlanDiagnostic, BorrowPolarity,
};
use crate::{AccessResourceCompatibility, ValidatedAccessPlan};

/// A plan-qualified interpretation of one borrowed concrete range.
#[derive(Debug)]
pub struct AccessView<'extent> {
    pub(crate) loan: ExtentLoan<'extent>,
    pub(crate) plan: ValidatedAccessPlan,
    pub(crate) profile_receipt: ResourceProfileReceiptId,
    pub(crate) profile: AdmittedResourceProfile,
    pub(crate) resources: AccessResourceCompatibility,
    pub(crate) admission: AccessAdmissionId,
}

/// Failed ordinary borrowed-view retirement preserves the complete
/// loan-bearing view for corrected retry.
#[derive(Debug)]
pub struct AccessViewRetirementError<'extent> {
    view: AccessView<'extent>,
    diagnostic: AccessPlanDiagnostic,
}

impl<'extent> AccessView<'extent> {
    pub const fn admission(&self) -> AccessAdmissionId {
        self.admission
    }

    pub const fn base(&self) -> u64 {
        self.loan.base()
    }

    pub const fn length(&self) -> u64 {
        self.loan.length()
    }

    /// End this ordinary borrowed view after independently replaying its exact
    /// loan, placement, admitted profile, receipt, and resource compatibility.
    /// Success returns the original loan; rejection returns this complete view
    /// for repair and retry. No content, vacancy, or destruction is claimed.
    pub fn retire(self) -> Result<ExtentLoan<'extent>, AccessViewRetirementError<'extent>> {
        if let Err(diagnostic) = self.validate_authority("borrowed access-view retirement") {
            return Err(AccessViewRetirementError {
                view: self,
                diagnostic,
            });
        }
        Ok(self.loan)
    }

    crate::placements::field_projection_accessors!('extent);

    fn project_with<'view>(
        &'view self,
        key: AccessFieldKey,
        current_borrow: BorrowPolarity,
    ) -> Result<AccessFieldProjection<'view, 'extent>, AccessPlanDiagnostic> {
        let source_loan = BorrowPolarity::from(self.loan.polarity());
        project_access_field(
            &self.plan,
            self.profile_receipt,
            &self.resources,
            self.admission,
            self.loan.base(),
            key,
            current_borrow,
            source_loan,
            None,
            AccessAuthorityRef::Borrowed(self),
        )
    }

    pub(super) fn validate_authority(&self, transition: &str) -> Result<(), AccessPlanDiagnostic> {
        if self.profile.receipt() != self.profile_receipt {
            return Err(AccessPlanDiagnostic(format!(
                "{transition} could not replay the exact admitted resource-profile receipt"
            )));
        }
        let replayed = validate_access_admission(&self.loan, &self.plan, &self.profile).map_err(
            |diagnostic| {
                AccessPlanDiagnostic(format!(
                    "{transition} could not replay the retained access authority: {diagnostic}"
                ))
            },
        )?;
        if replayed != self.resources {
            return Err(AccessPlanDiagnostic(format!(
                "{transition} replayed resource compatibility differs from the retained view"
            )));
        }
        Ok(())
    }
}

impl<'extent> AccessViewRetirementError<'extent> {
    diagnostic_accessor!();

    pub fn into_parts(self) -> (AccessView<'extent>, AccessPlanDiagnostic) {
        (self.view, self.diagnostic)
    }
}
