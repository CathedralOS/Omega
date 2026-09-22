//! Provider-backed resident custody for owned Atomic placements.
//!
//! This is the Atomic sibling of Stable resident adoption. It binds an exact
//! provider existing-content grant to an admitted Atomic-only placement,
//! activates one caller-supplied placed occurrence, and returns the unchanged
//! resident claim on retirement. It performs no atomic access or attempt.

use extents::{
    Extent, ExtentContentCustodyReceiptId, ExtentContentValidityReceiptId,
    ProviderExistingContentGrant, ResidentClaimId,
};

use crate::access_plan::diagnostic::into_validated_access;
use crate::placements::owned_resident_custody::replay_owned_admission_resources;
use crate::placements::owned_resident_custody::validate_owned_content_binding;
use crate::placements::owned_resident_custody::validate_resident_observation;
use crate::placements::placement_authority::PlacementAuthorityRef;
use crate::primitive_access::field_projection::project_placed_field;
use crate::{
    AccessFieldKey, AccessPlanDiagnostic, BorrowPolarity, ObservationModel,
    OwnedPlacementAdmission, PlacedFieldProjection, PlacedOccurrenceId, PlacementAdmissionId,
    PlacementResourceCompatibility, ResourceProfileReceiptId, ValidatedPlacementPlan,
};

/// Dormant provider-established content for one exact Atomic-only placement.
#[derive(Debug)]
#[must_use = "dormant Atomic resident content retains linear Extent and content custody"]
pub struct DormantOwnedAtomicResident {
    pub(crate) admission: OwnedPlacementAdmission,
    pub(crate) content: ProviderExistingContentGrant,
}

/// One active owned Atomic view carrying the same resident claim and one fresh
/// caller-supplied placed occurrence.
#[derive(Debug)]
#[must_use = "active owned Atomic placement retains linear resident custody"]
pub struct EstablishedOwnedAtomicPlacement {
    pub(super) admission: OwnedPlacementAdmission,
    pub(crate) content: ProviderExistingContentGrant,
    pub(super) occurrence: PlacedOccurrenceId,
}

/// Failed Atomic adoption returns both non-Clone inputs unchanged.
#[derive(Debug)]
pub struct OwnedAtomicAdoptionError {
    admission: OwnedPlacementAdmission,
    content: ProviderExistingContentGrant,
    diagnostic: AccessPlanDiagnostic,
}

impl OwnedAtomicAdoptionError {
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

/// Failed Atomic resident-view establishment returns the dormant resident and
/// exact requested occurrence for corrected retry.
#[derive(Debug)]
pub struct OwnedAtomicResidentViewEstablishmentError {
    resident: DormantOwnedAtomicResident,
    occurrence: PlacedOccurrenceId,
    diagnostic: AccessPlanDiagnostic,
}

impl OwnedAtomicResidentViewEstablishmentError {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        DormantOwnedAtomicResident,
        PlacedOccurrenceId,
        AccessPlanDiagnostic,
    ) {
        (self.resident, self.occurrence, self.diagnostic)
    }
}

/// Failed resident-preserving Atomic retirement returns the complete active
/// carrier without reconstructing custody from copied identities.
#[derive(Debug)]
pub struct OwnedAtomicResidentRetirementError {
    established: EstablishedOwnedAtomicPlacement,
    diagnostic: AccessPlanDiagnostic,
}

impl OwnedAtomicResidentRetirementError {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (EstablishedOwnedAtomicPlacement, AccessPlanDiagnostic) {
        (self.established, self.diagnostic)
    }
}

/// Establish provider-backed resident custody for one exact Atomic-only
/// placement. This consumes no checked result contract and performs no atomic
/// operation.
pub fn adopt_owned_atomic(
    admission: OwnedPlacementAdmission,
    content: ProviderExistingContentGrant,
) -> Result<DormantOwnedAtomicResident, OwnedAtomicAdoptionError> {
    into_validated_access(
        (admission, content),
        |(admission, content)| {
            validate_owned_atomic_resident_authority(admission, content, "Atomic adoption")
        },
        |(admission, content), ()| DormantOwnedAtomicResident { admission, content },
        |(admission, content), diagnostic| OwnedAtomicAdoptionError {
            admission,
            content,
            diagnostic,
        },
    )
}

impl DormantOwnedAtomicResident {
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

    /// Activate this exact resident claim through one requested occurrence.
    /// The occurrence issuer remains responsible for global freshness.
    pub fn view(
        self,
        occurrence: PlacedOccurrenceId,
    ) -> Result<EstablishedOwnedAtomicPlacement, OwnedAtomicResidentViewEstablishmentError> {
        into_validated_access(
            self,
            |resident| {
                validate_owned_atomic_resident_authority(
                    &resident.admission,
                    &resident.content,
                    "Atomic resident view",
                )
            },
            |resident, ()| EstablishedOwnedAtomicPlacement {
                admission: resident.admission,
                content: resident.content,
                occurrence,
            },
            |resident, diagnostic| OwnedAtomicResidentViewEstablishmentError {
                resident,
                occurrence,
                diagnostic,
            },
        )
    }
}

impl EstablishedOwnedAtomicPlacement {
    pub const fn admission(&self) -> PlacementAdmissionId {
        self.admission.identity
    }

    pub const fn placement_plan(&self) -> &ValidatedPlacementPlan {
        &self.admission.placement_plan
    }

    pub const fn profile_receipt(&self) -> ResourceProfileReceiptId {
        self.admission.profile_receipt
    }

    pub(super) const fn profile(
        &self,
    ) -> &crate::resources::resource_profile_admission::AdmittedResourceProfile {
        &self.admission.profile
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

    pub fn project<'view>(
        &'view self,
        key: AccessFieldKey,
    ) -> Result<PlacedFieldProjection<'view, 'view>, AccessPlanDiagnostic> {
        project_placed_field(
            &self.admission.placement_plan,
            self.admission.profile_receipt,
            &self.admission.resources,
            self.admission.identity,
            self.admission.extent.base(),
            key,
            BorrowPolarity::Shared,
            BorrowPolarity::Exclusive,
            Some(ObservationModel::Atomic),
            PlacementAuthorityRef::EstablishedOwnedAtomic(self),
        )
    }

    /// End this occurrence without performing an Atomic operation or changing
    /// its resident content. The exact provider grant returns dormant.
    pub fn retire_resident(
        self,
    ) -> Result<DormantOwnedAtomicResident, OwnedAtomicResidentRetirementError> {
        into_validated_access(
            self,
            |resident| {
                validate_owned_atomic_resident_authority(
                    &resident.admission,
                    &resident.content,
                    "Atomic resident-preserving retirement",
                )
            },
            |resident, ()| DormantOwnedAtomicResident {
                admission: resident.admission,
                content: resident.content,
            },
            |established, diagnostic| OwnedAtomicResidentRetirementError {
                established,
                diagnostic,
            },
        )
    }
}

pub(super) fn validate_owned_atomic_resident_authority(
    admission: &OwnedPlacementAdmission,
    content: &ProviderExistingContentGrant,
    transition: &str,
) -> Result<(), AccessPlanDiagnostic> {
    let resources = replay_owned_admission_resources(admission).map_err(|diagnostic| {
        AccessPlanDiagnostic(format!(
            "{transition} could not replay the retained placement authority: {diagnostic}"
        ))
    })?;
    if resources != admission.resources {
        return Err(AccessPlanDiagnostic(format!(
            "{transition} replayed resource compatibility differs from the retained admission"
        )));
    }
    validate_owned_content_binding(admission, content).map_err(|diagnostic| {
        AccessPlanDiagnostic(format!(
            "{transition} could not replay the retained provider content grant: {diagnostic}"
        ))
    })?;
    validate_resident_observation(
        &admission.placement_plan,
        ObservationModel::Atomic,
        transition,
    )
}
