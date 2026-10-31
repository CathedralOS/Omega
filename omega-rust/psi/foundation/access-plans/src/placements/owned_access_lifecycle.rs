use extents::{
    Extent, ExtentContentCustodyReceiptId, ExtentContentValidityReceiptId,
    ProviderExistingContentGrant, ResidentClaimId,
};

use crate::AdmittedResourceProfile;
use crate::placements::access_authority::AccessAuthorityRef;
use crate::placements::owned_resident_custody::validate_owned_resident_authority;
use crate::primitive_access::field_projection::project_access_field;
use crate::{
    AccessAdmissionId, AccessFieldKey, AccessFieldProjection, AccessOccurrenceId,
    AccessPlanDiagnostic, AccessResourceCompatibility, BorrowPolarity, ObservationModel,
    ResourceProfileReceiptId, ValidatedAccessPlan,
};

/// One accepted whole-range access admission that retains the exact owned
/// Extent checked against provider supply.
///
/// This is permission to establish placed content, not evidence that content
/// already exists. A later explicit Stable initialize/validate/adopt or
/// External adopt route must consume this carrier. Withdrawing it therefore
/// returns only the original granted Extent and establishes no `Vacant` fact.
#[derive(Debug)]
#[must_use = "an owned access admission retains linear Extent authority"]
pub struct OwnedAccessAdmission {
    pub(crate) identity: AccessAdmissionId,
    pub(crate) access_plan: ValidatedAccessPlan,
    pub(crate) profile_receipt: ResourceProfileReceiptId,
    pub(crate) profile: AdmittedResourceProfile,
    pub(crate) resources: AccessResourceCompatibility,
    pub(crate) extent: Extent,
}

/// Failed owned admission returns the exact moved Extent rather than losing
/// or reconstructing its authority account.
#[derive(Debug)]
pub struct OwnedAccessRejection {
    pub(crate) extent: Extent,
    pub(crate) diagnostic: AccessPlanDiagnostic,
}

/// Dormant provider-validated Stable content whose exact Extent authority and
/// resident claim are retained by the accepted access admission.
///
/// This is the first content-establishing owned carrier. It deliberately has
/// neither field projection nor a route back to a bare Extent. An explicit
/// view transition creates one fresh active placed occurrence; checked
/// destruction or move-out must land before another retirement route can
/// establish `Vacant` and release storage authority.
#[derive(Debug)]
#[must_use = "dormant resident content retains linear Extent and content custody"]
pub struct DormantOwnedResident {
    pub(crate) admission: OwnedAccessAdmission,
    pub(crate) content: ProviderExistingContentGrant,
}

/// Failed owned resident-view establishment preserves the complete dormant
/// content authority and the exact requested occurrence for corrected retry.
#[derive(Debug)]
pub struct OwnedResidentViewEstablishmentError {
    resident: DormantOwnedResident,
    occurrence: AccessOccurrenceId,
    diagnostic: AccessPlanDiagnostic,
}

/// One active owned view of provider-established Stable resident content.
/// The occurrence is fresh for this view while `resident_claim` remains the
/// identity of the same dormant content across view/retirement cycles.
#[derive(Debug)]
#[must_use = "active owned placed content retains linear resident custody"]
pub struct EstablishedOwnedAccess {
    pub(crate) admission: OwnedAccessAdmission,
    pub(crate) content: ProviderExistingContentGrant,
    pub(crate) occurrence: AccessOccurrenceId,
}

/// Failed resident-preserving retirement returns the complete active carrier;
/// no dormant claim is minted from drifted access authority.
#[derive(Debug)]
pub struct OwnedResidentRetirementError {
    established: EstablishedOwnedAccess,
    diagnostic: AccessPlanDiagnostic,
}

/// Failed Stable adoption preserves both linear inputs for a corrected retry
/// or explicit cancellation.
#[derive(Debug)]
pub struct OwnedStableAdoptionError {
    pub(crate) admission: OwnedAccessAdmission,
    pub(crate) content: ProviderExistingContentGrant,
    pub(crate) diagnostic: AccessPlanDiagnostic,
}

impl OwnedAccessAdmission {
    pub const fn identity(&self) -> AccessAdmissionId {
        self.identity
    }

    pub const fn profile_receipt(&self) -> ResourceProfileReceiptId {
        self.profile_receipt
    }

    pub const fn resources(&self) -> &AccessResourceCompatibility {
        &self.resources
    }

    pub const fn extent(&self) -> &Extent {
        &self.extent
    }

    pub const fn access_plan(&self) -> &ValidatedAccessPlan {
        &self.access_plan
    }

    /// Project one access field over this admission's authority account: the
    /// plan, profile, resources, identity, and extent base it carries forward.
    /// Each owned carrier supplies only its borrow poles, required observation,
    /// and authority receipt.
    pub(crate) fn project_field<'view>(
        &'view self,
        key: AccessFieldKey,
        current_borrow: BorrowPolarity,
        source_loan: BorrowPolarity,
        required_observation: Option<ObservationModel>,
        authority: AccessAuthorityRef<'view, 'view>,
    ) -> Result<AccessFieldProjection<'view, 'view>, AccessPlanDiagnostic> {
        project_access_field(
            &self.access_plan,
            self.profile_receipt,
            &self.resources,
            self.identity,
            self.extent.base(),
            key,
            current_borrow,
            source_loan,
            required_observation,
            authority,
        )
    }

    /// Cancel permission-only admission without claiming content
    /// establishment, destruction, vacancy, or allocator release.
    pub fn withdraw(self) -> Extent {
        self.extent
    }
}

impl OwnedAccessRejection {
    diagnostic_accessor!();

    pub fn into_parts(self) -> (Extent, AccessPlanDiagnostic) {
        (self.extent, self.diagnostic)
    }
}

impl OwnedResidentViewEstablishmentError {
    diagnostic_accessor!();

    pub fn into_parts(
        self,
    ) -> (
        DormantOwnedResident,
        AccessOccurrenceId,
        AccessPlanDiagnostic,
    ) {
        (self.resident, self.occurrence, self.diagnostic)
    }
}

impl DormantOwnedResident {
    owned_view_accessors!(resident);

    /// Transfer dormant resident custody into one requested active placed
    /// occurrence after replaying the retained owned access authority.
    /// The resident claim and provider receipts are forwarded unchanged; the
    /// occurrence issuer remains responsible for global freshness.
    pub fn view(
        self,
        occurrence: AccessOccurrenceId,
    ) -> Result<EstablishedOwnedAccess, OwnedResidentViewEstablishmentError> {
        if let Err(diagnostic) =
            validate_owned_resident_authority(&self.admission, &self.content, "owned resident view")
        {
            return Err(OwnedResidentViewEstablishmentError {
                resident: self,
                occurrence,
                diagnostic,
            });
        }
        Ok(EstablishedOwnedAccess {
            admission: self.admission,
            content: self.content,
            occurrence,
        })
    }
}

impl OwnedResidentRetirementError {
    diagnostic_accessor!();

    pub fn into_parts(self) -> (EstablishedOwnedAccess, AccessPlanDiagnostic) {
        (self.established, self.diagnostic)
    }
}

impl EstablishedOwnedAccess {
    owned_view_accessors!(resident, occurrence);

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

    crate::placements::field_projection_accessors!('view);

    fn project_with<'view>(
        &'view self,
        key: AccessFieldKey,
        current_borrow: BorrowPolarity,
    ) -> Result<AccessFieldProjection<'view, 'view>, AccessPlanDiagnostic> {
        self.admission.project_field(
            key,
            current_borrow,
            BorrowPolarity::Exclusive,
            Some(ObservationModel::Stable),
            AccessAuthorityRef::EstablishedOwned(self),
        )
    }
}

impl OwnedStableAdoptionError {
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
