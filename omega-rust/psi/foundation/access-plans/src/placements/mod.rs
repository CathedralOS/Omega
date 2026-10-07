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

/// The owned-placement carrier surface every admission-holding record
/// reports under the same names: the `OwnedPlacementAdmission`-held base
/// accessors — admission identity, placement plan, profile receipt, resource
/// compatibility, and extent — plus the carrier-specific tails each arm adds:
/// `occurrence` for placed carriers, `resident` for the provider content
/// receipts (validity/custody/claim), and `correspondence` for the retained
/// schema/device route. The `OwnedPlacementAdmission` leaf keeps its
/// direct-field accessors; every other carrier emits this block and keeps
/// divergent members (view establishment, retirement, adoption) in its impl.
macro_rules! owned_placement_accessors {
    (resident, occurrence) => {
        owned_placement_accessors!(resident);

        pub const fn occurrence(&self) -> PlacedOccurrenceId {
            self.occurrence
        }
    };
    (resident) => {
        owned_placement_accessors!();

        pub const fn validity_receipt(&self) -> ExtentContentValidityReceiptId {
            self.content.validity_receipt()
        }

        pub const fn custody_receipt(&self) -> ExtentContentCustodyReceiptId {
            self.content.custody_receipt()
        }

        pub const fn resident_claim(&self) -> ResidentClaimId {
            self.content.resident_claim()
        }
    };
    (correspondence, occurrence) => {
        owned_placement_accessors!(correspondence);

        pub const fn occurrence(&self) -> PlacedOccurrenceId {
            self.occurrence
        }
    };
    (correspondence) => {
        owned_placement_accessors!();

        pub const fn correspondence(&self) -> &AdmittedSchemaDeviceCorrespondence {
            &self.correspondence
        }
    };
    (occurrence) => {
        owned_placement_accessors!();

        pub const fn occurrence(&self) -> PlacedOccurrenceId {
            self.occurrence
        }
    };
    () => {
        pub const fn admission(&self) -> PlacementAdmissionId {
            self.admission.identity
        }

        pub const fn placement_plan(&self) -> &ValidatedPlacementPlan {
            &self.admission.placement_plan
        }

        pub const fn profile_receipt(&self) -> ResourceProfileReceiptId {
            self.admission.profile_receipt
        }

        pub const fn resources(&self) -> &PlacementResourceCompatibility {
            &self.admission.resources
        }

        pub const fn extent(&self) -> &Extent {
            &self.admission.extent
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
