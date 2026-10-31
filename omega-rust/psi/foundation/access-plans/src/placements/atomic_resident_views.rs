//! Borrowed views of provider-established Atomic resident content.
//!
//! The lender retains the exact resident claim and provider receipts. Each
//! active carrier owns one whole-range loan and one caller-supplied placed
//! occurrence; retirement releases only that loan and never remints custody or
//! performs an Atomic attempt.

use extents::{
    ExtentContentCustodyReceiptId, ExtentContentValidityReceiptId, ExtentLoan, LoanPolarity,
    ProviderExistingContentGrant, ResidentClaimId,
};

use crate::placements::access_admission::validate_access_admission;
use crate::placements::access_authority::AccessAuthorityRef;
use crate::placements::owned_atomic_resident_custody::validate_owned_atomic_resident_authority;
use crate::placements::owned_resident_custody::{
    validate_provider_content_binding, validate_resident_observation,
};
use crate::primitive_access::field_projection::project_access_field;
use crate::{
    AccessAdmissionId, AccessFieldKey, AccessFieldProjection, AccessOccurrenceId,
    AccessPlanDiagnostic, AccessResourceCompatibility, AdmittedResourceProfile, BorrowPolarity,
    DormantOwnedAtomicResident, ObservationModel, ResourceProfileReceiptId, ValidatedAccessPlan,
};

impl DormantOwnedAtomicResident {
    /// Borrow the whole Atomic resident range through a shared placed
    /// occurrence. Atomic operation permissions remain exactly those admitted
    /// by the placement; shared borrowing adds none.
    pub fn borrow_view(
        &self,
        occurrence: AccessOccurrenceId,
    ) -> Result<EstablishedBorrowedAtomicResidentAccess<'_>, AccessPlanDiagnostic> {
        validate_owned_atomic_resident_authority(
            &self.admission,
            &self.content,
            "borrowed Atomic resident shared-view establishment",
        )?;
        let length = self.admission.extent.length();
        let loan = self
            .admission
            .extent
            .loan(0, length)
            .map_err(|diagnostic| {
                AccessPlanDiagnostic(format!(
                    "borrowed Atomic resident whole-range loan rejected: {diagnostic}"
                ))
            })?;
        Ok(EstablishedBorrowedAtomicResidentAccess {
            plan: self.admission.access_plan.clone(),
            lender_plan: &self.admission.access_plan,
            profile_receipt: self.admission.profile_receipt,
            profile: &self.admission.profile,
            resources: &self.admission.resources,
            admission: self.admission.identity,
            lender_admission: &self.admission.identity,
            loan,
            content: &self.content,
            occurrence,
        })
    }

    /// Borrow the whole Atomic resident range through an exclusive placed
    /// occurrence. Exclusivity changes only the retained loan polarity and
    /// grants no Atomic operation absent from the placement.
    pub fn borrow_view_mut(
        &mut self,
        occurrence: AccessOccurrenceId,
    ) -> Result<EstablishedBorrowedAtomicResidentAccess<'_>, AccessPlanDiagnostic> {
        validate_owned_atomic_resident_authority(
            &self.admission,
            &self.content,
            "borrowed Atomic resident exclusive-view establishment",
        )?;
        let plan = self.admission.access_plan.clone();
        let lender_plan = &self.admission.access_plan;
        let profile_receipt = self.admission.profile_receipt;
        let profile = &self.admission.profile;
        let resources = &self.admission.resources;
        let admission = self.admission.identity;
        let lender_admission = &self.admission.identity;
        let content = &self.content;
        let length = self.admission.extent.length();
        let loan = self
            .admission
            .extent
            .loan_mut(0, length)
            .map_err(|diagnostic| {
                AccessPlanDiagnostic(format!(
                    "borrowed Atomic resident whole-range loan rejected: {diagnostic}"
                ))
            })?;
        Ok(EstablishedBorrowedAtomicResidentAccess {
            plan,
            lender_plan,
            profile_receipt,
            profile,
            resources,
            admission,
            lender_admission,
            loan,
            content,
            occurrence,
        })
    }
}

/// One active borrowed view of provider-established Atomic resident content.
///
/// The lender continues to own the claim. This non-Clone carrier retains the
/// exact whole-range loan, placement/profile authority, provider content
/// grant, and active occurrence until explicit retirement.
#[derive(Debug)]
#[must_use = "active borrowed Atomic content retains an exact resident loan"]
pub struct EstablishedBorrowedAtomicResidentAccess<'resident> {
    plan: ValidatedAccessPlan,
    lender_plan: &'resident ValidatedAccessPlan,
    profile_receipt: ResourceProfileReceiptId,
    profile: &'resident AdmittedResourceProfile,
    resources: &'resident AccessResourceCompatibility,
    admission: AccessAdmissionId,
    lender_admission: &'resident AccessAdmissionId,
    loan: ExtentLoan<'resident>,
    content: &'resident ProviderExistingContentGrant,
    occurrence: AccessOccurrenceId,
}

/// Failed borrowed Atomic retirement returns the complete active carrier.
/// No loan is released and no resident identity or receipt is reconstructed.
#[derive(Debug)]
pub struct BorrowedAtomicResidentRetirementError<'resident> {
    established: EstablishedBorrowedAtomicResidentAccess<'resident>,
    diagnostic: AccessPlanDiagnostic,
}

impl<'resident> BorrowedAtomicResidentRetirementError<'resident> {
    diagnostic_accessor!();

    pub fn into_parts(
        self,
    ) -> (
        EstablishedBorrowedAtomicResidentAccess<'resident>,
        AccessPlanDiagnostic,
    ) {
        (self.established, self.diagnostic)
    }
}

impl<'resident> EstablishedBorrowedAtomicResidentAccess<'resident> {
    pub(super) const fn profile(&self) -> &AdmittedResourceProfile {
        self.profile
    }

    pub(super) const fn resources(&self) -> &AccessResourceCompatibility {
        self.resources
    }

    resident_view_accessors!();

    fn project_with<'view>(
        &'view self,
        key: AccessFieldKey,
        current_borrow: BorrowPolarity,
    ) -> Result<AccessFieldProjection<'view, 'resident>, AccessPlanDiagnostic> {
        self.validate_lender_binding("borrowed Atomic resident projection")?;
        let source_loan = BorrowPolarity::from(self.loan.polarity());
        project_access_field(
            &self.plan,
            self.profile_receipt,
            self.resources,
            self.admission,
            self.loan.base(),
            key,
            current_borrow,
            source_loan,
            Some(ObservationModel::Atomic),
            AccessAuthorityRef::BorrowedAtomicResident(self),
        )
    }

    fn validate_retirement_authority(&self) -> Result<(), AccessPlanDiagnostic> {
        self.validate_lender_binding("borrowed Atomic resident retirement")?;
        if self.profile.receipt() != self.profile_receipt {
            return Err(AccessPlanDiagnostic(
                "borrowed Atomic resident retirement profile receipt differs from its retained admitted profile"
                    .into(),
            ));
        }
        let replayed = validate_access_admission(&self.loan, &self.plan, self.profile)
            .map_err(|diagnostic| {
                AccessPlanDiagnostic(format!(
                    "borrowed Atomic resident retirement could not replay the retained access authority: {diagnostic}"
                ))
            })?;
        if &replayed != self.resources {
            return Err(AccessPlanDiagnostic(
                "borrowed Atomic resident retirement replayed resource compatibility differs from the retained active carrier"
                    .into(),
            ));
        }
        validate_provider_content_binding(&self.plan, &self.loan, self.content).map_err(
            |diagnostic| {
                AccessPlanDiagnostic(format!(
                    "borrowed Atomic resident retirement could not replay the retained provider content grant: {diagnostic}"
                ))
            },
        )?;
        validate_resident_observation(
            &self.plan,
            ObservationModel::Atomic,
            "borrowed Atomic resident retirement",
        )
    }

    pub(super) fn validate_lender_binding(
        &self,
        transition: &str,
    ) -> Result<(), AccessPlanDiagnostic> {
        if &self.plan != self.lender_plan
            || self.profile_receipt != self.profile.receipt()
            || &self.admission != self.lender_admission
        {
            return Err(AccessPlanDiagnostic(format!(
                "{transition} retained placement/profile authority differs from the exact lender"
            )));
        }
        Ok(())
    }

    /// End this occurrence and release its exact loan. The lender's dormant
    /// claim and provider receipts remain unchanged.
    pub fn retire(self) -> Result<(), BorrowedAtomicResidentRetirementError<'resident>> {
        if let Err(diagnostic) = self.validate_retirement_authority() {
            return Err(BorrowedAtomicResidentRetirementError {
                established: self,
                diagnostic,
            });
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn replace_plan_for_test(
        &mut self,
        plan: ValidatedAccessPlan,
    ) -> ValidatedAccessPlan {
        std::mem::replace(&mut self.plan, plan)
    }

    #[cfg(test)]
    pub(crate) fn replace_admission_for_test(
        &mut self,
        admission: AccessAdmissionId,
    ) -> AccessAdmissionId {
        std::mem::replace(&mut self.admission, admission)
    }
}
