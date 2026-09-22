use extents::{
    Extent, ExtentContentCustodyReceiptId, ExtentContentValidityReceiptId,
    ProviderExistingContentGrant, ResidentClaimId,
};

use crate::AdmittedResourceProfile;
use crate::access_plan::diagnostic::into_validated_access;
use crate::placements::owned_resident_custody::validate_owned_resident_authority;
use crate::placements::placement_authority::PlacementAuthorityRef;
use crate::primitive_access::field_projection::project_placed_field;
use crate::{
    AccessFieldKey, AccessPlanDiagnostic, BorrowPolarity, ObservationModel, PlacedFieldProjection,
    PlacedOccurrenceId, PlacementAdmissionId, PlacementResourceCompatibility,
    ResourceProfileReceiptId, ValidatedPlacementPlan,
};

/// One accepted whole-range placement admission that retains the exact owned
/// Extent checked against provider supply.
///
/// This is permission to establish placed content, not evidence that content
/// already exists. A later explicit Stable initialize/validate/adopt or
/// External adopt route must consume this carrier. Withdrawing it therefore
/// returns only the original granted Extent and establishes no `Vacant` fact.
#[derive(Debug)]
#[must_use = "an owned placement admission retains linear Extent authority"]
pub struct OwnedPlacementAdmission {
    pub(crate) identity: PlacementAdmissionId,
    pub(crate) placement_plan: ValidatedPlacementPlan,
    pub(crate) profile_receipt: ResourceProfileReceiptId,
    pub(crate) profile: AdmittedResourceProfile,
    pub(crate) resources: PlacementResourceCompatibility,
    pub(crate) extent: Extent,
}

/// Failed owned admission returns the exact moved Extent rather than losing
/// or reconstructing its authority account.
#[derive(Debug)]
pub struct OwnedPlacementRejection {
    pub(crate) extent: Extent,
    pub(crate) diagnostic: AccessPlanDiagnostic,
}

/// Dormant provider-validated Stable content whose exact Extent authority and
/// resident claim are retained by the accepted placement admission.
///
/// This is the first content-establishing owned carrier. It deliberately has
/// neither field projection nor a route back to a bare Extent. An explicit
/// view transition creates one fresh active placed occurrence; checked
/// destruction or move-out must land before another retirement route can
/// establish `Vacant` and release storage authority.
#[derive(Debug)]
#[must_use = "dormant resident content retains linear Extent and content custody"]
pub struct DormantOwnedResident {
    pub(crate) admission: OwnedPlacementAdmission,
    pub(crate) content: ProviderExistingContentGrant,
}

/// Failed owned resident-view establishment preserves the complete dormant
/// content authority and the exact requested occurrence for corrected retry.
#[derive(Debug)]
pub struct OwnedResidentViewEstablishmentError {
    resident: DormantOwnedResident,
    occurrence: PlacedOccurrenceId,
    diagnostic: AccessPlanDiagnostic,
}

/// One active owned view of provider-established Stable resident content.
/// The occurrence is fresh for this view while `resident_claim` remains the
/// identity of the same dormant content across view/retirement cycles.
#[derive(Debug)]
#[must_use = "active owned placed content retains linear resident custody"]
pub struct EstablishedOwnedPlacement {
    pub(crate) admission: OwnedPlacementAdmission,
    pub(crate) content: ProviderExistingContentGrant,
    pub(crate) occurrence: PlacedOccurrenceId,
}

/// Failed resident-preserving retirement returns the complete active carrier;
/// no dormant claim is minted from drifted placement authority.
#[derive(Debug)]
pub struct OwnedResidentRetirementError {
    established: EstablishedOwnedPlacement,
    diagnostic: AccessPlanDiagnostic,
}

/// Failed Stable adoption preserves both linear inputs for a corrected retry
/// or explicit cancellation.
#[derive(Debug)]
pub struct OwnedStableAdoptionError {
    pub(crate) admission: OwnedPlacementAdmission,
    pub(crate) content: ProviderExistingContentGrant,
    pub(crate) diagnostic: AccessPlanDiagnostic,
}

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
        into_validated_access(
            self,
            |resident| {
                validate_owned_resident_authority(
                    &resident.admission,
                    &resident.content,
                    ObservationModel::Stable,
                    "owned resident view",
                )
            },
            |resident, ()| EstablishedOwnedPlacement {
                admission: resident.admission,
                content: resident.content,
                occurrence,
            },
            |resident, diagnostic| OwnedResidentViewEstablishmentError {
                resident,
                occurrence,
                diagnostic,
            },
        )
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
        into_validated_access(
            self,
            |resident| {
                validate_owned_resident_authority(
                    &resident.admission,
                    &resident.content,
                    ObservationModel::Stable,
                    "resident-preserving retirement",
                )
            },
            |resident, ()| DormantOwnedResident {
                admission: resident.admission,
                content: resident.content,
            },
            |established, diagnostic| OwnedResidentRetirementError {
                established,
                diagnostic,
            },
        )
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
