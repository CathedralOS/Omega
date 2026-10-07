//! Placing a plan over a concrete range: the placement plan, admission and
//! authority, owned and borrowed resident custody, stable and atomic
//! resident views, and schema correspondence.

/// The placed-occurrence view surface every borrowed resident placement
/// reports under the same names — admission and plan identity, the retained
/// profile/loan/content handles, the whole-range loan geometry and its
/// provider receipts, occurrence identity, and the shared/exclusive
/// projection entrypoints. The stable and atomic carriers emit this block;
/// their divergent members (`profile`, `resources`, lender validation and
/// retirement) stay in each impl.
macro_rules! resident_view_accessors {
    () => {
        pub const fn admission(&self) -> PlacementAdmissionId {
            self.admission
        }

        pub const fn placement_plan(&self) -> &ValidatedPlacementPlan {
            &self.plan
        }

        pub const fn profile_receipt(&self) -> ResourceProfileReceiptId {
            self.profile_receipt
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

        #[cfg(test)]
        pub(crate) fn replace_content_for_test(
            &mut self,
            content: &'resident ProviderExistingContentGrant,
        ) -> &'resident ProviderExistingContentGrant {
            std::mem::replace(&mut self.content, content)
        }
    };
}

pub(crate) mod atomic_resident_views;
pub(crate) mod borrowed_view;
pub(crate) mod owned_atomic_resident_custody;
pub(crate) mod owned_external_correspondence;
pub(crate) mod owned_placement_lifecycle;
pub(crate) mod owned_resident_custody;
pub(crate) mod placement_admission;
pub(crate) mod placement_authority;
pub(crate) mod placement_plan;
pub(crate) mod resident_views;
pub(crate) mod schema_correspondence;
