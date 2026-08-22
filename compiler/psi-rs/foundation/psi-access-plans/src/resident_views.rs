//! Borrowed views of provider-established resident content.
//!
//! The lender retains resident custody. This module binds one fresh active
//! occurrence to an exact whole-range `ExtentLoan` and forwards the lender's
//! claim and provider receipts through placed field access.

use super::{
    AccessFieldKey, AccessPlanDiagnostic, AdmittedResourceProfile, BorrowPolarity,
    DormantOwnedResident, ObservationModel, PlacedFieldProjection, PlacedOccurrenceId,
    PlacementAdmissionId, PlacementAuthorityRef, PlacementResourceCompatibility,
    ResourceProfileReceiptId, ValidatedPlacementPlan, project_placed_field,
    validate_owned_resident_authority, validate_placement_admission,
};
use psi_extents::{
    ExtentContentCustodyReceiptId, ExtentContentValidityReceiptId, ExtentLoan, LoanPolarity,
    ResidentClaimId,
};

impl DormantOwnedResident {
    /// Borrow the whole resident range through a shared placed occurrence.
    ///
    /// The lender keeps the exact resident claim and provider receipts. The
    /// returned carrier retains a whole-range shared loan and may therefore
    /// authorize only operations compatible with shared placed access.
    pub fn borrow_view(
        &self,
        occurrence: PlacedOccurrenceId,
    ) -> Result<EstablishedBorrowedResidentPlacement<'_>, AccessPlanDiagnostic> {
        validate_owned_resident_authority(
            &self.admission,
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
        Ok(EstablishedBorrowedResidentPlacement {
            plan: self.admission.placement_plan.clone(),
            profile_receipt: self.admission.profile_receipt,
            profile: self.admission.profile.clone(),
            resources: self.admission.resources.clone(),
            admission: self.admission.identity,
            loan,
            resident_claim: self.resident_claim,
            occurrence,
            validity_receipt: self.validity_receipt,
            custody_receipt: self.custody_receipt,
        })
    }

    /// Borrow the whole resident range through an exclusive placed
    /// occurrence. Ending the returned carrier releases only this loan; it
    /// does not retire, replace, or remint the lender's resident claim.
    pub fn borrow_view_mut(
        &mut self,
        occurrence: PlacedOccurrenceId,
    ) -> Result<EstablishedBorrowedResidentPlacement<'_>, AccessPlanDiagnostic> {
        validate_owned_resident_authority(
            &self.admission,
            "borrowed resident exclusive-view establishment",
        )?;
        let plan = self.admission.placement_plan.clone();
        let profile_receipt = self.admission.profile_receipt;
        let profile = self.admission.profile.clone();
        let resources = self.admission.resources.clone();
        let admission = self.admission.identity;
        let resident_claim = self.resident_claim;
        let validity_receipt = self.validity_receipt;
        let custody_receipt = self.custody_receipt;
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
        Ok(EstablishedBorrowedResidentPlacement {
            plan,
            profile_receipt,
            profile,
            resources,
            admission,
            loan,
            resident_claim,
            occurrence,
            validity_receipt,
            custody_receipt,
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
pub struct EstablishedBorrowedResidentPlacement<'resident> {
    plan: ValidatedPlacementPlan,
    profile_receipt: ResourceProfileReceiptId,
    profile: AdmittedResourceProfile,
    resources: PlacementResourceCompatibility,
    admission: PlacementAdmissionId,
    loan: ExtentLoan<'resident>,
    resident_claim: ResidentClaimId,
    occurrence: PlacedOccurrenceId,
    validity_receipt: ExtentContentValidityReceiptId,
    custody_receipt: ExtentContentCustodyReceiptId,
}

/// Failed borrowed-resident retirement returns the complete active carrier.
/// No loan is released and no resident identity or provider receipt is
/// reconstructed from copied fields on rejection.
#[derive(Debug)]
pub struct BorrowedResidentRetirementError<'resident> {
    established: EstablishedBorrowedResidentPlacement<'resident>,
    diagnostic: AccessPlanDiagnostic,
}

impl<'resident> BorrowedResidentRetirementError<'resident> {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        EstablishedBorrowedResidentPlacement<'resident>,
        AccessPlanDiagnostic,
    ) {
        (self.established, self.diagnostic)
    }
}

impl<'resident> EstablishedBorrowedResidentPlacement<'resident> {
    pub const fn admission(&self) -> PlacementAdmissionId {
        self.admission
    }

    pub const fn placement_plan(&self) -> &ValidatedPlacementPlan {
        &self.plan
    }

    pub const fn profile_receipt(&self) -> ResourceProfileReceiptId {
        self.profile_receipt
    }

    pub(super) const fn profile(&self) -> &AdmittedResourceProfile {
        &self.profile
    }

    pub(super) const fn loan(&self) -> &ExtentLoan<'resident> {
        &self.loan
    }

    pub(super) const fn resources(&self) -> &PlacementResourceCompatibility {
        &self.resources
    }

    pub const fn base(&self) -> u64 {
        self.loan.base()
    }

    pub const fn length(&self) -> u64 {
        self.loan.length()
    }

    pub const fn loan_polarity(&self) -> LoanPolarity {
        self.loan.polarity()
    }

    pub const fn resident_claim(&self) -> ResidentClaimId {
        self.resident_claim
    }

    pub const fn occurrence(&self) -> PlacedOccurrenceId {
        self.occurrence
    }

    pub const fn validity_receipt(&self) -> ExtentContentValidityReceiptId {
        self.validity_receipt
    }

    pub const fn custody_receipt(&self) -> ExtentContentCustodyReceiptId {
        self.custody_receipt
    }

    pub fn project<'view>(
        &'view self,
        key: AccessFieldKey,
    ) -> Result<PlacedFieldProjection<'view, 'resident>, AccessPlanDiagnostic> {
        self.project_with(key, BorrowPolarity::Shared)
    }

    pub fn project_mut<'view>(
        &'view mut self,
        key: AccessFieldKey,
    ) -> Result<PlacedFieldProjection<'view, 'resident>, AccessPlanDiagnostic> {
        self.project_with(key, BorrowPolarity::Exclusive)
    }

    fn project_with<'view>(
        &'view self,
        key: AccessFieldKey,
        current_borrow: BorrowPolarity,
    ) -> Result<PlacedFieldProjection<'view, 'resident>, AccessPlanDiagnostic> {
        let source_loan = match self.loan.polarity() {
            LoanPolarity::Shared => BorrowPolarity::Shared,
            LoanPolarity::Exclusive => BorrowPolarity::Exclusive,
        };
        project_placed_field(
            &self.plan,
            self.profile_receipt,
            &self.resources,
            self.admission,
            self.loan.base(),
            key,
            current_borrow,
            source_loan,
            Some(ObservationModel::Stable),
            PlacementAuthorityRef::BorrowedResident(self),
        )
    }

    fn validate_retirement_authority(&self) -> Result<(), AccessPlanDiagnostic> {
        if self.profile.receipt() != self.profile_receipt {
            return Err(AccessPlanDiagnostic(
                "borrowed resident retirement profile receipt differs from its retained admitted profile"
                    .into(),
            ));
        }
        let replayed = validate_placement_admission(&self.loan, &self.plan, &self.profile)
            .map_err(|diagnostic| {
                AccessPlanDiagnostic(format!(
                    "borrowed resident retirement could not replay the retained placement authority: {diagnostic}"
                ))
            })?;
        if replayed != self.resources {
            return Err(AccessPlanDiagnostic(
                "borrowed resident retirement replayed resource compatibility differs from the retained active carrier"
                    .into(),
            ));
        }
        Ok(())
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
    pub(super) fn replace_profile_for_test(
        &mut self,
        profile: AdmittedResourceProfile,
    ) -> AdmittedResourceProfile {
        std::mem::replace(&mut self.profile, profile)
    }
}
