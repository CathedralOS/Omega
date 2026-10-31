//! Borrowed views of provider-established resident content.
//!
//! The lender retains resident custody. This module binds one fresh active
//! occurrence to an exact whole-range `ExtentLoan` and forwards the lender's
//! claim and provider receipts through access field access.

use crate::placements::access_admission::validate_access_admission;
use crate::placements::access_authority::AccessAuthorityRef;
use crate::placements::owned_resident_custody::validate_owned_resident_authority;
use crate::placements::owned_resident_custody::validate_provider_content_binding;
use crate::placements::owned_resident_custody::validate_resident_observation;
use crate::primitive_access::field_projection::project_access_field;
use crate::{
    AccessAdmissionId, AccessFieldKey, AccessFieldProjection, AccessOccurrenceId,
    AccessPlanDiagnostic, AccessResourceCompatibility, AdmittedResourceProfile, BorrowPolarity,
    DormantOwnedResident, ObservationModel, ResourceProfileReceiptId, ValidatedAccessPlan,
};
use extents::{
    ExtentContentCustodyReceiptId, ExtentContentValidityReceiptId, ExtentLoan, LoanPolarity,
    ProviderExistingContentGrant, ResidentClaimId,
};

impl DormantOwnedResident {
    /// Borrow the whole resident range through a shared placed occurrence.
    ///
    /// The lender keeps the exact resident claim and provider receipts. The
    /// returned carrier retains a whole-range shared loan and may therefore
    /// authorize only operations compatible with shared placed access.
    pub fn borrow_view(
        &self,
        occurrence: AccessOccurrenceId,
    ) -> Result<EstablishedBorrowedResidentAccess<'_>, AccessPlanDiagnostic> {
        validate_owned_resident_authority(
            &self.admission,
            &self.content,
            "borrowed resident shared-view establishment",
        )?;
        let length = self.admission.extent.length();
        let loan = self
            .admission
            .extent
            .loan(0, length)
            .map_err(|diagnostic| {
                AccessPlanDiagnostic(format!(
                    "borrowed resident whole-range loan rejected: {diagnostic}"
                ))
            })?;
        Ok(EstablishedBorrowedResidentAccess {
            plan: self.admission.access_plan.clone(),
            profile_receipt: self.admission.profile_receipt,
            profile: self.admission.profile.clone(),
            resources: self.admission.resources.clone(),
            admission: self.admission.identity,
            loan,
            content: &self.content,
            occurrence,
        })
    }

    /// Borrow the whole resident range through an exclusive placed
    /// occurrence. Ending the returned carrier releases only this loan; it
    /// does not retire, replace, or remint the lender's resident claim.
    pub fn borrow_view_mut(
        &mut self,
        occurrence: AccessOccurrenceId,
    ) -> Result<EstablishedBorrowedResidentAccess<'_>, AccessPlanDiagnostic> {
        validate_owned_resident_authority(
            &self.admission,
            &self.content,
            "borrowed resident exclusive-view establishment",
        )?;
        let plan = self.admission.access_plan.clone();
        let profile_receipt = self.admission.profile_receipt;
        let profile = self.admission.profile.clone();
        let resources = self.admission.resources.clone();
        let admission = self.admission.identity;
        let content = &self.content;
        let length = self.admission.extent.length();
        let loan = self
            .admission
            .extent
            .loan_mut(0, length)
            .map_err(|diagnostic| {
                AccessPlanDiagnostic(format!(
                    "borrowed resident whole-range loan rejected: {diagnostic}"
                ))
            })?;
        Ok(EstablishedBorrowedResidentAccess {
            plan,
            profile_receipt,
            profile,
            resources,
            admission,
            loan,
            content,
            occurrence,
        })
    }
}

/// One active borrowed view of provider-established Stable resident content.
///
/// The lender continues to own the exact resident claim. This carrier retains
/// a whole-range `ExtentLoan`, the same claim and provider receipts, and one
/// fresh active placed occurrence. Consuming it ends the loan without creating
/// or retiring resident custody.
#[derive(Debug)]
#[must_use = "active borrowed placed content retains an exact resident loan"]
pub struct EstablishedBorrowedResidentAccess<'resident> {
    plan: ValidatedAccessPlan,
    profile_receipt: ResourceProfileReceiptId,
    profile: AdmittedResourceProfile,
    resources: AccessResourceCompatibility,
    admission: AccessAdmissionId,
    loan: ExtentLoan<'resident>,
    content: &'resident ProviderExistingContentGrant,
    occurrence: AccessOccurrenceId,
}

/// Failed borrowed-resident retirement returns the complete active carrier.
/// No loan is released and no resident identity or provider receipt is
/// reconstructed from copied fields on rejection.
#[derive(Debug)]
pub struct BorrowedResidentRetirementError<'resident> {
    established: EstablishedBorrowedResidentAccess<'resident>,
    diagnostic: AccessPlanDiagnostic,
}

impl<'resident> BorrowedResidentRetirementError<'resident> {
    diagnostic_accessor!();

    pub fn into_parts(
        self,
    ) -> (
        EstablishedBorrowedResidentAccess<'resident>,
        AccessPlanDiagnostic,
    ) {
        (self.established, self.diagnostic)
    }
}

impl<'resident> EstablishedBorrowedResidentAccess<'resident> {
    pub(super) const fn profile(&self) -> &AdmittedResourceProfile {
        &self.profile
    }

    pub(super) const fn resources(&self) -> &AccessResourceCompatibility {
        &self.resources
    }

    resident_view_accessors!();

    fn project_with<'view>(
        &'view self,
        key: AccessFieldKey,
        current_borrow: BorrowPolarity,
    ) -> Result<AccessFieldProjection<'view, 'resident>, AccessPlanDiagnostic> {
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
            Some(ObservationModel::Stable),
            AccessAuthorityRef::BorrowedResident(self),
        )
    }

    fn validate_retirement_authority(&self) -> Result<(), AccessPlanDiagnostic> {
        if self.profile.receipt() != self.profile_receipt {
            return Err(AccessPlanDiagnostic(
                "borrowed resident retirement profile receipt differs from its retained admitted profile"
                    .into(),
            ));
        }
        let replayed = validate_access_admission(&self.loan, &self.plan, &self.profile)
            .map_err(|diagnostic| {
                AccessPlanDiagnostic(format!(
                    "borrowed resident retirement could not replay the retained access authority: {diagnostic}"
                ))
            })?;
        if replayed != self.resources {
            return Err(AccessPlanDiagnostic(
                "borrowed resident retirement replayed resource compatibility differs from the retained active carrier"
                    .into(),
            ));
        }
        validate_provider_content_binding(&self.plan, &self.loan, self.content).map_err(
            |diagnostic| {
                AccessPlanDiagnostic(format!(
                    "borrowed resident retirement could not replay the retained provider content grant: {diagnostic}"
                ))
            },
        )?;
        validate_resident_observation(
            &self.plan,
            ObservationModel::Stable,
            "borrowed resident retirement",
        )
    }

    /// Explicitly end this placed occurrence and release its exact loan. The
    /// lender's dormant resident claim and provider receipts remain unchanged.
    pub fn retire(self) -> Result<(), BorrowedResidentRetirementError<'resident>> {
        if let Err(diagnostic) = self.validate_retirement_authority() {
            return Err(BorrowedResidentRetirementError {
                established: self,
                diagnostic,
            });
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn replace_profile_for_test(
        &mut self,
        profile: AdmittedResourceProfile,
    ) -> AdmittedResourceProfile {
        std::mem::replace(&mut self.profile, profile)
    }
}
