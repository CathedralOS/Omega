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

use crate::placements::access_authority::AccessAuthorityRef;
use crate::placements::owned_resident_custody::validate_owned_admission_authority;
use crate::placements::owned_resident_custody::validate_owned_content_binding;
use crate::{
    AccessAdmissionId, AccessFieldKey, AccessFieldProjection, AccessOccurrenceId,
    AccessPlanDiagnostic, AccessResourceCompatibility, BorrowPolarity, ObservationModel,
    OwnedAccessAdmission, ResourceProfileReceiptId, ValidatedAccessPlan,
};

/// Dormant provider-established content for one exact Atomic-only placement.
#[derive(Debug)]
#[must_use = "dormant Atomic resident content retains linear Extent and content custody"]
pub struct DormantOwnedAtomicResident {
    pub(crate) admission: OwnedAccessAdmission,
    pub(crate) content: ProviderExistingContentGrant,
}

/// One active owned Atomic view carrying the same resident claim and one fresh
/// caller-supplied placed occurrence.
#[derive(Debug)]
#[must_use = "active owned Atomic placement retains linear resident custody"]
pub struct EstablishedOwnedAtomicAccess {
    pub(super) admission: OwnedAccessAdmission,
    pub(crate) content: ProviderExistingContentGrant,
    pub(super) occurrence: AccessOccurrenceId,
}

/// Failed Atomic adoption returns both non-Clone inputs unchanged.
#[derive(Debug)]
pub struct OwnedAtomicAdoptionError {
    admission: OwnedAccessAdmission,
    content: ProviderExistingContentGrant,
    diagnostic: AccessPlanDiagnostic,
}

impl OwnedAtomicAdoptionError {
    diagnostic_accessor!();

    pub fn into_parts(
        self,
    ) -> (
        OwnedAccessAdmission,
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
    occurrence: AccessOccurrenceId,
    diagnostic: AccessPlanDiagnostic,
}

impl OwnedAtomicResidentViewEstablishmentError {
    diagnostic_accessor!();

    pub fn into_parts(
        self,
    ) -> (
        DormantOwnedAtomicResident,
        AccessOccurrenceId,
        AccessPlanDiagnostic,
    ) {
        (self.resident, self.occurrence, self.diagnostic)
    }
}

/// Failed resident-preserving Atomic retirement returns the complete active
/// carrier without reconstructing custody from copied identities.
#[derive(Debug)]
pub struct OwnedAtomicResidentRetirementError {
    established: EstablishedOwnedAtomicAccess,
    diagnostic: AccessPlanDiagnostic,
}

impl OwnedAtomicResidentRetirementError {
    diagnostic_accessor!();

    pub fn into_parts(self) -> (EstablishedOwnedAtomicAccess, AccessPlanDiagnostic) {
        (self.established, self.diagnostic)
    }
}

/// Establish provider-backed resident custody for one exact Atomic-only
/// placement. This consumes no checked result contract and performs no atomic
/// operation.
pub fn adopt_owned_atomic(
    admission: OwnedAccessAdmission,
    content: ProviderExistingContentGrant,
) -> Result<DormantOwnedAtomicResident, OwnedAtomicAdoptionError> {
    if let Err(diagnostic) =
        validate_owned_atomic_resident_authority(&admission, &content, "Atomic adoption")
    {
        return Err(OwnedAtomicAdoptionError {
            admission,
            content,
            diagnostic,
        });
    }
    Ok(DormantOwnedAtomicResident { admission, content })
}

impl DormantOwnedAtomicResident {
    owned_view_accessors!(resident);

    /// Activate this exact resident claim through one requested occurrence.
    /// The occurrence issuer remains responsible for global freshness.
    pub fn view(
        self,
        occurrence: AccessOccurrenceId,
    ) -> Result<EstablishedOwnedAtomicAccess, OwnedAtomicResidentViewEstablishmentError> {
        if let Err(diagnostic) = validate_owned_atomic_resident_authority(
            &self.admission,
            &self.content,
            "Atomic resident view",
        ) {
            return Err(OwnedAtomicResidentViewEstablishmentError {
                resident: self,
                occurrence,
                diagnostic,
            });
        }
        Ok(EstablishedOwnedAtomicAccess {
            admission: self.admission,
            content: self.content,
            occurrence,
        })
    }
}

impl EstablishedOwnedAtomicAccess {
    owned_view_accessors!(resident, occurrence);

    pub(super) const fn profile(
        &self,
    ) -> &crate::resources::resource_profile_admission::AdmittedResourceProfile {
        &self.admission.profile
    }

    pub fn project<'view>(
        &'view self,
        key: AccessFieldKey,
    ) -> Result<AccessFieldProjection<'view, 'view>, AccessPlanDiagnostic> {
        self.admission.project_field(
            key,
            BorrowPolarity::Shared,
            BorrowPolarity::Exclusive,
            Some(ObservationModel::Atomic),
            AccessAuthorityRef::EstablishedOwnedAtomic(self),
        )
    }

    /// End this occurrence without performing an Atomic operation or changing
    /// its resident content. The exact provider grant returns dormant.
    pub fn retire_resident(
        self,
    ) -> Result<DormantOwnedAtomicResident, OwnedAtomicResidentRetirementError> {
        if let Err(diagnostic) = validate_owned_atomic_resident_authority(
            &self.admission,
            &self.content,
            "Atomic resident-preserving retirement",
        ) {
            return Err(OwnedAtomicResidentRetirementError {
                established: self,
                diagnostic,
            });
        }
        Ok(DormantOwnedAtomicResident {
            admission: self.admission,
            content: self.content,
        })
    }
}

pub(super) fn validate_owned_atomic_resident_authority(
    admission: &OwnedAccessAdmission,
    content: &ProviderExistingContentGrant,
    transition: &str,
) -> Result<(), AccessPlanDiagnostic> {
    validate_owned_admission_authority(
        admission,
        ObservationModel::Atomic,
        |transition| {
            validate_owned_content_binding(admission, content).map_err(|diagnostic| {
                AccessPlanDiagnostic(format!(
                    "{transition} could not replay the retained provider content grant: {diagnostic}"
                ))
            })
        },
        transition,
    )
}
