//! Borrowed views of provider-established resident content.
//!
//! The lender retains resident custody. This module binds one fresh active
//! occurrence to an exact whole-range `ExtentLoan` and forwards the lender's
//! claim and provider receipts through placed field access.

use super::{
    AccessFieldKey, AccessPlanDiagnostic, BorrowPolarity, DormantOwnedResident, ObservationModel,
    PlacedFieldProjection, PlacedOccurrenceId, PlacementAdmissionId, PlacementAuthorityRef,
    PlacementResourceCompatibility, ResourceProfileReceiptId, ValidatedPlacementPlan,
    project_placed_field,
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
        let plan = self.admission.placement_plan.clone();
        let profile_receipt = self.admission.profile_receipt;
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
    resources: PlacementResourceCompatibility,
    admission: PlacementAdmissionId,
    loan: ExtentLoan<'resident>,
    resident_claim: ResidentClaimId,
    occurrence: PlacedOccurrenceId,
    validity_receipt: ExtentContentValidityReceiptId,
    custody_receipt: ExtentContentCustodyReceiptId,
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

    /// Explicitly end this placed occurrence and release its exact loan. The
    /// lender's dormant resident claim and provider receipts remain unchanged.
    pub fn retire(self) {}
}
