//! Borrowed views of provider-established Atomic resident content.
//!
//! The lender retains the exact resident claim and provider receipts. Each
//! active carrier owns one whole-range loan and one caller-supplied placed
//! occurrence; retirement releases only that loan and never remints custody or
//! performs an Atomic attempt.

use psi_extents::{
    ExtentContentCustodyReceiptId, ExtentContentValidityReceiptId, ExtentLoan, LoanPolarity,
    ProviderExistingContentGrant, ResidentClaimId,
};

use super::owned_atomic_resident_custody::validate_owned_atomic_resident_authority;
use super::owned_resident_custody::{
    validate_provider_content_binding, validate_resident_observation,
};
use super::{
    AccessFieldKey, AccessPlanDiagnostic, AdmittedResourceProfile, BorrowPolarity,
    DormantOwnedAtomicResident, ObservationModel, PlacedFieldProjection, PlacedOccurrenceId,
    PlacementAdmissionId, PlacementAuthorityRef, PlacementResourceCompatibility,
    ResourceProfileReceiptId, ValidatedPlacementPlan, project_placed_field,
    validate_placement_admission,
};

impl DormantOwnedAtomicResident {
    /// Borrow the whole Atomic resident range through a shared placed
    /// occurrence. Atomic operation permissions remain exactly those admitted
    /// by the placement; shared borrowing adds none.
    pub fn borrow_view(
        &self,
        occurrence: PlacedOccurrenceId,
    ) -> Result<EstablishedBorrowedAtomicResidentPlacement<'_>, AccessPlanDiagnostic> {
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
        Ok(EstablishedBorrowedAtomicResidentPlacement {
            plan: self.admission.placement_plan.clone(),
            lender_plan: &self.admission.placement_plan,
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
        occurrence: PlacedOccurrenceId,
    ) -> Result<EstablishedBorrowedAtomicResidentPlacement<'_>, AccessPlanDiagnostic> {
        validate_owned_atomic_resident_authority(
            &self.admission,
            &self.content,
            "borrowed Atomic resident exclusive-view establishment",
        )?;
        let plan = self.admission.placement_plan.clone();
        let lender_plan = &self.admission.placement_plan;
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
        Ok(EstablishedBorrowedAtomicResidentPlacement {
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
pub struct EstablishedBorrowedAtomicResidentPlacement<'resident> {
    plan: ValidatedPlacementPlan,
    lender_plan: &'resident ValidatedPlacementPlan,
    profile_receipt: ResourceProfileReceiptId,
    profile: &'resident AdmittedResourceProfile,
    resources: &'resident PlacementResourceCompatibility,
    admission: PlacementAdmissionId,
    lender_admission: &'resident PlacementAdmissionId,
    loan: ExtentLoan<'resident>,
    content: &'resident ProviderExistingContentGrant,
    occurrence: PlacedOccurrenceId,
}

/// Failed borrowed Atomic retirement returns the complete active carrier.
/// No loan is released and no resident identity or receipt is reconstructed.
#[derive(Debug)]
pub struct BorrowedAtomicResidentRetirementError<'resident> {
    established: EstablishedBorrowedAtomicResidentPlacement<'resident>,
    diagnostic: AccessPlanDiagnostic,
}

impl<'resident> BorrowedAtomicResidentRetirementError<'resident> {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        EstablishedBorrowedAtomicResidentPlacement<'resident>,
        AccessPlanDiagnostic,
    ) {
        (self.established, self.diagnostic)
    }
}

impl<'resident> EstablishedBorrowedAtomicResidentPlacement<'resident> {
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
        self.profile
    }

    pub(super) const fn resources(&self) -> &PlacementResourceCompatibility {
        self.resources
    }

    pub(super) const fn loan(&self) -> &ExtentLoan<'resident> {
        &self.loan
    }

    pub(super) const fn content(&self) -> &'resident ProviderExistingContentGrant {
        self.content
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
        self.content.resident_claim()
    }

    pub const fn occurrence(&self) -> PlacedOccurrenceId {
        self.occurrence
    }

    pub const fn validity_receipt(&self) -> ExtentContentValidityReceiptId {
        self.content.validity_receipt()
    }

    pub const fn custody_receipt(&self) -> ExtentContentCustodyReceiptId {
        self.content.custody_receipt()
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
        self.validate_lender_binding("borrowed Atomic resident projection")?;
        let source_loan = match self.loan.polarity() {
            LoanPolarity::Shared => BorrowPolarity::Shared,
            LoanPolarity::Exclusive => BorrowPolarity::Exclusive,
        };
        project_placed_field(
            &self.plan,
            self.profile_receipt,
            self.resources,
            self.admission,
            self.loan.base(),
            key,
            current_borrow,
            source_loan,
            Some(ObservationModel::Atomic),
            PlacementAuthorityRef::BorrowedAtomicResident(self),
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
        let replayed = validate_placement_admission(&self.loan, &self.plan, self.profile)
            .map_err(|diagnostic| {
                AccessPlanDiagnostic(format!(
                    "borrowed Atomic resident retirement could not replay the retained placement authority: {diagnostic}"
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
    pub(super) fn replace_plan_for_test(
        &mut self,
        plan: ValidatedPlacementPlan,
    ) -> ValidatedPlacementPlan {
        std::mem::replace(&mut self.plan, plan)
    }

    #[cfg(test)]
    pub(super) fn replace_admission_for_test(
        &mut self,
        admission: PlacementAdmissionId,
    ) -> PlacementAdmissionId {
        std::mem::replace(&mut self.admission, admission)
    }

    #[cfg(test)]
    pub(super) fn replace_content_for_test(
        &mut self,
        content: &'resident ProviderExistingContentGrant,
    ) -> &'resident ProviderExistingContentGrant {
        std::mem::replace(&mut self.content, content)
    }
}
