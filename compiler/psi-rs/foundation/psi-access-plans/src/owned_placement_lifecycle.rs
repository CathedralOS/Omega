use psi_extents::{
    Extent, ExtentContentCustodyReceiptId, ExtentContentValidityReceiptId,
    ProviderExistingContentGrant, ResidentClaimId,
};

use super::{
    AccessFieldKey, AccessPlanDiagnostic, BorrowPolarity, DormantOwnedResident,
    EstablishedOwnedPlacement, ObservationModel, OwnedPlacementAdmission, OwnedPlacementRejection,
    OwnedResidentRetirementError, OwnedResidentViewEstablishmentError, OwnedStableAdoptionError,
    PlacedFieldProjection, PlacedOccurrenceId, PlacementAdmissionId, PlacementAuthorityRef,
    PlacementResourceCompatibility, ResourceProfileReceiptId, ValidatedPlacementPlan,
    project_placed_field, validate_owned_resident_authority,
};

impl OwnedPlacementAdmission {
    pub const fn identity(&self) -> PlacementAdmissionId {
        self.identity
    }

    pub const fn profile_receipt(&self) -> ResourceProfileReceiptId {
        self.profile_receipt
    }

    pub const fn resources(&self) -> &PlacementResourceCompatibility {
        &self.resources
    }

    pub const fn extent(&self) -> &Extent {
        &self.extent
    }

    pub const fn placement_plan(&self) -> &ValidatedPlacementPlan {
        &self.placement_plan
    }

    /// Cancel permission-only admission without claiming content
    /// establishment, destruction, vacancy, or allocator release.
    pub fn withdraw(self) -> Extent {
        self.extent
    }
}

impl OwnedPlacementRejection {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (Extent, AccessPlanDiagnostic) {
        (self.extent, self.diagnostic)
    }
}

impl OwnedResidentViewEstablishmentError {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        DormantOwnedResident,
        PlacedOccurrenceId,
        AccessPlanDiagnostic,
    ) {
        (self.resident, self.occurrence, self.diagnostic)
    }
}

impl DormantOwnedResident {
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

    pub const fn validity_receipt(&self) -> ExtentContentValidityReceiptId {
        self.content.validity_receipt()
    }

    pub const fn custody_receipt(&self) -> ExtentContentCustodyReceiptId {
        self.content.custody_receipt()
    }

    pub const fn resident_claim(&self) -> ResidentClaimId {
        self.content.resident_claim()
    }

    /// Transfer dormant resident custody into one requested active placed
    /// occurrence after replaying the retained owned placement authority.
    /// The resident claim and provider receipts are forwarded unchanged; the
    /// occurrence issuer remains responsible for global freshness.
    pub fn view(
        self,
        occurrence: PlacedOccurrenceId,
    ) -> Result<EstablishedOwnedPlacement, OwnedResidentViewEstablishmentError> {
        if let Err(diagnostic) =
            validate_owned_resident_authority(&self.admission, &self.content, "owned resident view")
        {
            return Err(OwnedResidentViewEstablishmentError {
                resident: self,
                occurrence,
                diagnostic,
            });
        }
        Ok(EstablishedOwnedPlacement {
            admission: self.admission,
            content: self.content,
            occurrence,
        })
    }
}

impl OwnedResidentRetirementError {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (EstablishedOwnedPlacement, AccessPlanDiagnostic) {
        (self.established, self.diagnostic)
    }
}

impl EstablishedOwnedPlacement {
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

    pub const fn validity_receipt(&self) -> ExtentContentValidityReceiptId {
        self.content.validity_receipt()
    }

    pub const fn custody_receipt(&self) -> ExtentContentCustodyReceiptId {
        self.content.custody_receipt()
    }

    pub const fn resident_claim(&self) -> ResidentClaimId {
        self.content.resident_claim()
    }

    pub const fn occurrence(&self) -> PlacedOccurrenceId {
        self.occurrence
    }

    /// End this active owned view without destroying or moving out its
    /// content. The exact resident claim and provider receipts return to the
    /// dormant carrier; the active occurrence ends here.
    pub fn retire_resident(self) -> Result<DormantOwnedResident, OwnedResidentRetirementError> {
        if let Err(diagnostic) = validate_owned_resident_authority(
            &self.admission,
            &self.content,
            "resident-preserving retirement",
        ) {
            return Err(OwnedResidentRetirementError {
                established: self,
                diagnostic,
            });
        }
        Ok(DormantOwnedResident {
            admission: self.admission,
            content: self.content,
        })
    }

    /// Purely project one accepted Stable field through a shared borrow of
    /// this provider-established owned placement.
    ///
    /// The returned accessor retains this entire carrier, including its
    /// content-validity and custody evidence, through any sealed primitive
    /// request derived from it.
    pub fn project<'view>(
        &'view self,
        key: AccessFieldKey,
    ) -> Result<PlacedFieldProjection<'view, 'view>, AccessPlanDiagnostic> {
        self.project_with(key, BorrowPolarity::Shared)
    }

    /// Purely project one accepted Stable field through an exclusive borrow
    /// of this provider-established owned placement.
    pub fn project_mut<'view>(
        &'view mut self,
        key: AccessFieldKey,
    ) -> Result<PlacedFieldProjection<'view, 'view>, AccessPlanDiagnostic> {
        self.project_with(key, BorrowPolarity::Exclusive)
    }

    fn project_with<'view>(
        &'view self,
        key: AccessFieldKey,
        current_borrow: BorrowPolarity,
    ) -> Result<PlacedFieldProjection<'view, 'view>, AccessPlanDiagnostic> {
        project_placed_field(
            &self.admission.placement_plan,
            self.admission.profile_receipt,
            &self.admission.resources,
            self.admission.identity,
            self.admission.extent.base(),
            key,
            current_borrow,
            BorrowPolarity::Exclusive,
            Some(ObservationModel::Stable),
            PlacementAuthorityRef::EstablishedOwned(self),
        )
    }
}

impl OwnedStableAdoptionError {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        OwnedPlacementAdmission,
        ProviderExistingContentGrant,
        AccessPlanDiagnostic,
    ) {
        (self.admission, self.content, self.diagnostic)
    }
}
