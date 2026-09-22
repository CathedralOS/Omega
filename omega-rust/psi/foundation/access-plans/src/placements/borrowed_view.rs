use extents::{ExtentLoan, LoanPolarity};

use crate::AdmittedResourceProfile;
use crate::ResourceProfileReceiptId;
use crate::access_plan::diagnostic::{access_plan_rejection, into_validated_access};
use crate::placements::placement_admission::validate_placement_admission;
use crate::placements::placement_authority::PlacementAuthorityRef;
use crate::primitive_access::field_projection::project_placed_field;
use crate::{
    AccessFieldKey, AccessPlanDiagnostic, BorrowPolarity, PlacedFieldProjection,
    PlacementAdmissionId,
};
use crate::{PlacementResourceCompatibility, ValidatedPlacementPlan};

/// A plan-qualified interpretation of one borrowed concrete range.
#[derive(Debug)]
pub struct PlacedView<'extent> {
    pub(crate) loan: ExtentLoan<'extent>,
    pub(crate) plan: ValidatedPlacementPlan,
    pub(crate) profile_receipt: ResourceProfileReceiptId,
    pub(crate) profile: AdmittedResourceProfile,
    pub(crate) resources: PlacementResourceCompatibility,
    pub(crate) admission: PlacementAdmissionId,
}

access_plan_rejection! {
/// Failed ordinary borrowed-view retirement preserves the complete
/// loan-bearing view for corrected retry.
    PlacedViewRetirementError<'extent> {
        view: PlacedView<'extent>,
        diagnostic: AccessPlanDiagnostic,
    }
}

impl<'extent> PlacedView<'extent> {
    pub const fn admission(&self) -> PlacementAdmissionId {
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
    pub fn retire(self) -> Result<ExtentLoan<'extent>, PlacedViewRetirementError<'extent>> {
        into_validated_access(
            self,
            |view| view.validate_authority("borrowed placed-view retirement"),
            |view, ()| view.loan,
            |view, diagnostic| PlacedViewRetirementError { view, diagnostic },
        )
    }

    /// Purely project one accepted field through a shared view borrow.
    ///
    /// Projection performs no memory event. The returned accessor remains
    /// tied to this placed view and exposes only named operation methods that
    /// create sealed lowering requests.
    pub fn project<'view>(
        &'view self,
        key: AccessFieldKey,
    ) -> Result<PlacedFieldProjection<'view, 'extent>, AccessPlanDiagnostic> {
        self.project_with(key, BorrowPolarity::Shared)
    }

    /// Purely project one accepted field through an exclusive view borrow.
    pub fn project_mut<'view>(
        &'view mut self,
        key: AccessFieldKey,
    ) -> Result<PlacedFieldProjection<'view, 'extent>, AccessPlanDiagnostic> {
        self.project_with(key, BorrowPolarity::Exclusive)
    }

    fn project_with<'view>(
        &'view self,
        key: AccessFieldKey,
        current_borrow: BorrowPolarity,
    ) -> Result<PlacedFieldProjection<'view, 'extent>, AccessPlanDiagnostic> {
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
            None,
            PlacementAuthorityRef::Borrowed(self),
        )
    }

    pub(super) fn validate_authority(&self, transition: &str) -> Result<(), AccessPlanDiagnostic> {
        if self.profile.receipt() != self.profile_receipt {
            return Err(AccessPlanDiagnostic(format!(
                "{transition} could not replay the exact admitted resource-profile receipt"
            )));
        }
        let replayed = validate_placement_admission(&self.loan, &self.plan, &self.profile)
            .map_err(|diagnostic| {
                AccessPlanDiagnostic(format!(
                    "{transition} could not replay the retained placement authority: {diagnostic}"
                ))
            })?;
        if replayed != self.resources {
            return Err(AccessPlanDiagnostic(format!(
                "{transition} replayed resource compatibility differs from the retained view"
            )));
        }
        Ok(())
    }
}
