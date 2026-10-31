//! Owned External adoption: the owned route a device-described placement
//! takes once its schema/device correspondence is admitted.
//!
//! This is the owned sibling of [`bind_schema_correspondence_to_access_plan`]
//! and the External counterpart of the owned Stable/Atomic resident
//! adoption routes. An External placement has no resident-content grant —
//! another agent may change the storage — so this carrier joins the owned
//! access admission to its exact admitted physical correspondence rather
//! than minting resident content custody. The route binds, views, projects,
//! and retires; it performs no device operation, observes no storage, and
//! grants no lowering authority.

use extents::Extent;

use crate::placements::access_authority::AccessAuthorityRef;
use crate::placements::owned_resident_custody::{
    validate_owned_admission_authority, validate_resident_observation,
};
use crate::{
    AccessAdmissionId, AccessFieldKey, AccessFieldProjection, AccessOccurrenceId,
    AccessPlanDiagnostic, AccessResourceCompatibility, AdmittedSchemaDeviceCorrespondence,
    BorrowPolarity, ObservationModel, OwnedAccessAdmission, ResourceProfileReceiptId,
    ValidatedAccessPlan,
};

/// Dormant owned carrier joining one owned access admission to its exact
/// admitted schema/device correspondence.
///
/// This is permission to establish placed External views of the named
/// device, not evidence of established content — External storage may
/// change under another agent, so no resident claim, validity, or custody
/// receipt exists here. `view` opens one caller-supplied placed occurrence;
/// `withdraw` cancels the whole route back to the owned admission.
#[derive(Debug)]
#[must_use = "owned corresponded admission retains linear Extent and physical provenance"]
pub struct OwnedCorrespondedExternalAdmission {
    pub(crate) admission: OwnedAccessAdmission,
    pub(crate) correspondence: AdmittedSchemaDeviceCorrespondence,
}

/// One active owned view of device-described External placement content.
/// The placed occurrence is fresh for this view; the correspondence stays
/// bound through every projection and retirement replays the complete
/// retained authority before returning the dormant carrier.
#[derive(Debug)]
#[must_use = "active owned External placement retains linear custody and physical provenance"]
pub struct EstablishedOwnedExternalAccess {
    pub(crate) admission: OwnedAccessAdmission,
    pub(crate) correspondence: AdmittedSchemaDeviceCorrespondence,
    pub(crate) occurrence: AccessOccurrenceId,
}

/// Failed External adoption returns both non-Clone inputs unchanged.
#[derive(Debug)]
pub struct OwnedExternalAdoptionError {
    admission: OwnedAccessAdmission,
    correspondence: AdmittedSchemaDeviceCorrespondence,
    diagnostic: AccessPlanDiagnostic,
}

impl OwnedExternalAdoptionError {
    diagnostic_accessor!();

    pub fn into_parts(
        self,
    ) -> (
        OwnedAccessAdmission,
        AdmittedSchemaDeviceCorrespondence,
        AccessPlanDiagnostic,
    ) {
        (self.admission, self.correspondence, self.diagnostic)
    }
}

/// Failed External view establishment preserves the complete dormant carrier
/// and the exact requested occurrence for corrected retry.
#[derive(Debug)]
pub struct OwnedExternalViewEstablishmentError {
    carrier: OwnedCorrespondedExternalAdmission,
    occurrence: AccessOccurrenceId,
    diagnostic: AccessPlanDiagnostic,
}

impl OwnedExternalViewEstablishmentError {
    diagnostic_accessor!();

    pub fn into_parts(
        self,
    ) -> (
        OwnedCorrespondedExternalAdmission,
        AccessOccurrenceId,
        AccessPlanDiagnostic,
    ) {
        (self.carrier, self.occurrence, self.diagnostic)
    }
}

/// Failed External retirement returns the complete active carrier; no
/// dormant claim is minted from drifted placement or correspondence
/// authority.
#[derive(Debug)]
pub struct OwnedExternalRetirementError {
    established: EstablishedOwnedExternalAccess,
    diagnostic: AccessPlanDiagnostic,
}

impl OwnedExternalRetirementError {
    diagnostic_accessor!();

    pub fn into_parts(self) -> (EstablishedOwnedExternalAccess, AccessPlanDiagnostic) {
        (self.established, self.diagnostic)
    }
}

/// The External adopt route: join one owned access admission to its exact
/// admitted schema/device correspondence.
///
/// Adoption independently binds the correspondence to the admission's exact
/// plan and resource-profile receipt, replays the retained placement
/// authority, and requires every accepted field to carry the External
/// observation model. Rejection returns both non-Clone inputs unchanged.
pub fn adopt_owned_external(
    admission: OwnedAccessAdmission,
    correspondence: AdmittedSchemaDeviceCorrespondence,
) -> Result<OwnedCorrespondedExternalAdmission, OwnedExternalAdoptionError> {
    if let Err(diagnostic) =
        validate_owned_external_authority(&admission, &correspondence, "External adoption")
    {
        return Err(OwnedExternalAdoptionError {
            admission,
            correspondence,
            diagnostic,
        });
    }
    Ok(OwnedCorrespondedExternalAdmission {
        admission,
        correspondence,
    })
}

fn validate_owned_external_authority(
    admission: &OwnedAccessAdmission,
    correspondence: &AdmittedSchemaDeviceCorrespondence,
    transition: &str,
) -> Result<(), AccessPlanDiagnostic> {
    validate_owned_admission_authority(
        admission,
        ObservationModel::External,
        |transition| {
            validate_external_correspondence_binding(admission, correspondence, transition)
        },
        transition,
    )
}

/// Independently replay the correspondence binding: the correspondence must
/// carry this admission's exact validated plan and resource-profile receipt
/// and replay its own admitted structure.
fn validate_external_correspondence_binding(
    admission: &OwnedAccessAdmission,
    correspondence: &AdmittedSchemaDeviceCorrespondence,
    transition: &str,
) -> Result<(), AccessPlanDiagnostic> {
    correspondence.validate_structure().map_err(|diagnostic| {
        AccessPlanDiagnostic(format!(
            "{transition} could not replay the retained schema/device correspondence: {diagnostic}"
        ))
    })?;
    if correspondence.access_plan() != &admission.access_plan
        || correspondence.profile_receipt() != admission.profile_receipt
    {
        return Err(AccessPlanDiagnostic(format!(
            "{transition} schema correspondence does not bind the owned access admission's exact plan and resource-profile receipt"
        )));
    }
    Ok(())
}

impl OwnedCorrespondedExternalAdmission {
    owned_view_accessors!(correspondence);

    /// Separate the route back into its non-Clone inputs.
    pub fn into_parts(self) -> (OwnedAccessAdmission, AdmittedSchemaDeviceCorrespondence) {
        (self.admission, self.correspondence)
    }

    /// Cancel the External adopt route without claiming content
    /// establishment, destruction, vacancy, or allocator release. Withdrawal
    /// returns the underlying extent and the separately admitted
    /// correspondence, which remains usable with a later matching placement.
    pub fn withdraw(self) -> (Extent, AdmittedSchemaDeviceCorrespondence) {
        (self.admission.withdraw(), self.correspondence)
    }

    /// Transfer the dormant corresponded admission into one requested active
    /// placed occurrence after replaying the complete retained authority.
    /// The occurrence issuer remains responsible for global freshness.
    pub fn view(
        self,
        occurrence: AccessOccurrenceId,
    ) -> Result<EstablishedOwnedExternalAccess, OwnedExternalViewEstablishmentError> {
        if let Err(diagnostic) = validate_owned_external_authority(
            &self.admission,
            &self.correspondence,
            "owned External view",
        ) {
            return Err(OwnedExternalViewEstablishmentError {
                carrier: self,
                occurrence,
                diagnostic,
            });
        }
        Ok(EstablishedOwnedExternalAccess {
            admission: self.admission,
            correspondence: self.correspondence,
            occurrence,
        })
    }
}

impl EstablishedOwnedExternalAccess {
    owned_view_accessors!(correspondence, occurrence);

    /// Replay the retained schema/device correspondence binding for the
    /// sealed-request authority hooks.
    pub(crate) fn validate_correspondence(
        &self,
        transition: &str,
    ) -> Result<(), AccessPlanDiagnostic> {
        validate_external_correspondence_binding(&self.admission, &self.correspondence, transition)
    }

    /// Replay the retained External observation roster for the
    /// sealed-request authority hooks. An External placement holds no
    /// resident-content grant; its admitted-observation replay is the
    /// content-side check this route has.
    pub(crate) fn validate_external_observation(
        &self,
        transition: &str,
    ) -> Result<(), AccessPlanDiagnostic> {
        validate_resident_observation(
            &self.admission.access_plan,
            ObservationModel::External,
            transition,
        )
    }

    /// End this active owned view. The exact owned admission and
    /// correspondence return to the dormant carrier; the active occurrence
    /// ends here. No content custody is established or destroyed.
    pub fn retire(
        self,
    ) -> Result<OwnedCorrespondedExternalAdmission, OwnedExternalRetirementError> {
        if let Err(diagnostic) = validate_owned_external_authority(
            &self.admission,
            &self.correspondence,
            "owned External retirement",
        ) {
            return Err(OwnedExternalRetirementError {
                established: self,
                diagnostic,
            });
        }
        Ok(OwnedCorrespondedExternalAdmission {
            admission: self.admission,
            correspondence: self.correspondence,
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
            Some(ObservationModel::External),
            AccessAuthorityRef::OwnedCorrespondedExternal(self),
        )
    }
}
