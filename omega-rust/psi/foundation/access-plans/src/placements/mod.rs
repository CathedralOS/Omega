//! Placing a plan over a concrete range: the access plan, admission and
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
        pub const fn admission(&self) -> AccessAdmissionId {
            self.admission
        }

        pub const fn access_plan(&self) -> &ValidatedAccessPlan {
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

        pub const fn occurrence(&self) -> AccessOccurrenceId {
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
        ) -> Result<AccessFieldProjection<'view, 'resident>, AccessPlanDiagnostic> {
            self.project_with(key, BorrowPolarity::Shared)
        }

        pub fn project_mut<'view>(
            &'view mut self,
            key: AccessFieldKey,
        ) -> Result<AccessFieldProjection<'view, 'resident>, AccessPlanDiagnostic> {
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
/// reports under the same names: the `OwnedAccessAdmission`-held base
/// accessors — admission identity, access plan, profile receipt, resource
/// compatibility, and extent — plus the carrier-specific tails each arm adds:
/// `occurrence` for placed carriers, `resident` for the provider content
/// receipts (validity/custody/claim), and `correspondence` for the retained
/// schema/device route. The `OwnedAccessAdmission` leaf keeps its
/// direct-field accessors; every other carrier emits this block and keeps
/// divergent members (view establishment, retirement, adoption) in its impl.
macro_rules! owned_view_accessors {
    (resident, occurrence) => {
        owned_view_accessors!(resident);

        pub const fn occurrence(&self) -> AccessOccurrenceId {
            self.occurrence
        }
    };
    (resident) => {
        owned_view_accessors!();

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
        owned_view_accessors!(correspondence);

        pub const fn occurrence(&self) -> AccessOccurrenceId {
            self.occurrence
        }
    };
    (correspondence) => {
        owned_view_accessors!();

        pub const fn correspondence(&self) -> &AdmittedSchemaDeviceCorrespondence {
            &self.correspondence
        }
    };
    (occurrence) => {
        owned_view_accessors!();

        pub const fn occurrence(&self) -> AccessOccurrenceId {
            self.occurrence
        }
    };
    () => {
        pub const fn admission(&self) -> AccessAdmissionId {
            self.admission.identity
        }

        pub const fn access_plan(&self) -> &ValidatedAccessPlan {
            &self.admission.access_plan
        }

        pub const fn profile_receipt(&self) -> ResourceProfileReceiptId {
            self.admission.profile_receipt
        }

        pub const fn resources(&self) -> &AccessResourceCompatibility {
            &self.admission.resources
        }

        pub const fn extent(&self) -> &Extent {
            &self.admission.extent
        }
    };
}

pub(crate) mod access_admission;
pub(crate) mod access_authority;
pub(crate) mod access_plan;
pub(crate) mod atomic_resident_views;
pub(crate) mod borrowed_view;
pub(crate) mod owned_access_lifecycle;
pub(crate) mod owned_atomic_resident_custody;
pub(crate) mod owned_external_correspondence;
pub(crate) mod owned_resident_custody;
pub(crate) mod resident_views;
pub(crate) mod schema_correspondence;

/// The shared/exclusive field-projection pair every placed carrier exposes:
/// both delegate to the carrier's own `project_with` borrow-polarity
/// implementation. `$extent` is the placement extent lifetime the returned
/// projection borrows for.
macro_rules! field_projection_accessors {
    ($extent:lifetime) => {
        /// Project one accepted field through a shared borrow.
        ///
        /// The returned accessor retains this carrier's access authority
        /// through any sealed primitive request derived from it.
        pub fn project<'view>(
            &'view self,
            key: AccessFieldKey,
        ) -> Result<AccessFieldProjection<'view, $extent>, AccessPlanDiagnostic> {
            self.project_with(key, BorrowPolarity::Shared)
        }

        /// Project one accepted field through an exclusive borrow.
        pub fn project_mut<'view>(
            &'view mut self,
            key: AccessFieldKey,
        ) -> Result<AccessFieldProjection<'view, $extent>, AccessPlanDiagnostic> {
            self.project_with(key, BorrowPolarity::Exclusive)
        }
    };
}
pub(crate) use field_projection_accessors;
