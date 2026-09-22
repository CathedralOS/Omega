//! Owned External adoption: the owned route a device-described placement
//! takes once its schema/device correspondence is admitted.
//!
//! This is the owned sibling of [`bind_schema_correspondence_to_placement`]
//! and the External counterpart of the owned Stable/Atomic resident
//! adoption routes. An External placement has no resident-content grant —
//! another agent may change the storage — so this carrier joins the owned
//! placement admission to its exact admitted physical correspondence rather
//! than minting resident content custody. The route binds, views, projects,
//! and retires; it performs no device operation, observes no storage, and
//! grants no lowering authority.

use extents::Extent;

use crate::access_plan::diagnostic::{access_plan_rejection, into_validated_access};
use crate::placements::owned_resident_custody::{
    replay_owned_admission_resources, validate_resident_observation,
};
use crate::placements::placement_authority::PlacementAuthorityRef;
use crate::primitive_access::field_projection::project_admission_field;
use crate::{
    AccessFieldKey, AccessPlanDiagnostic, AdmittedSchemaDeviceCorrespondence, BorrowPolarity,
    ObservationModel, OwnedPlacementAdmission, PlacedFieldProjection, PlacedOccurrenceId,
    PlacementAdmissionId, PlacementResourceCompatibility, ResourceProfileReceiptId,
    ValidatedPlacementPlan,
};

/// Dormant owned carrier joining one owned placement admission to its exact
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
    pub(crate) admission: OwnedPlacementAdmission,
    pub(crate) correspondence: AdmittedSchemaDeviceCorrespondence,
}

/// One active owned view of device-described External placement content.
/// The placed occurrence is fresh for this view; the correspondence stays
/// bound through every projection and retirement replays the complete
/// retained authority before returning the dormant carrier.
#[derive(Debug)]
#[must_use = "active owned External placement retains linear custody and physical provenance"]
pub struct EstablishedOwnedExternalPlacement {
    pub(crate) admission: OwnedPlacementAdmission,
    pub(crate) correspondence: AdmittedSchemaDeviceCorrespondence,
    pub(crate) occurrence: PlacedOccurrenceId,
}

access_plan_rejection! {
/// Failed External adoption returns both non-Clone inputs unchanged.
    OwnedExternalAdoptionError {
        admission: OwnedPlacementAdmission,
        correspondence: AdmittedSchemaDeviceCorrespondence,
        diagnostic: AccessPlanDiagnostic,
    }
}

access_plan_rejection! {
/// Failed External view establishment preserves the complete dormant carrier
/// and the exact requested occurrence for corrected retry.
    OwnedExternalViewEstablishmentError {
        carrier: OwnedCorrespondedExternalAdmission,
        occurrence: PlacedOccurrenceId,
        diagnostic: AccessPlanDiagnostic,
    }
}

access_plan_rejection! {
/// Failed External retirement returns the complete active carrier; no
/// dormant claim is minted from drifted placement or correspondence
/// authority.
    OwnedExternalRetirementError {
        established: EstablishedOwnedExternalPlacement,
        diagnostic: AccessPlanDiagnostic,
    }
}

/// The External adopt route: join one owned placement admission to its exact
/// admitted schema/device correspondence.
///
/// Adoption independently binds the correspondence to the admission's exact
/// plan and resource-profile receipt, replays the retained placement
/// authority, and requires every accepted field to carry the External
/// observation model. Rejection returns both non-Clone inputs unchanged.
pub fn adopt_owned_external(
    admission: OwnedPlacementAdmission,
    correspondence: AdmittedSchemaDeviceCorrespondence,
) -> Result<OwnedCorrespondedExternalAdmission, OwnedExternalAdoptionError> {
    into_validated_access(
        (admission, correspondence),
        |(admission, correspondence)| {
            validate_owned_external_authority(admission, correspondence, "External adoption")
        },
        |(admission, correspondence), ()| OwnedCorrespondedExternalAdmission {
            admission,
            correspondence,
        },
        |(admission, correspondence), diagnostic| OwnedExternalAdoptionError {
            admission,
            correspondence,
            diagnostic,
        },
    )
}

fn validate_owned_external_authority(
    admission: &OwnedPlacementAdmission,
    correspondence: &AdmittedSchemaDeviceCorrespondence,
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
    validate_external_correspondence_binding(admission, correspondence, transition)?;
    validate_resident_observation(
        &admission.placement_plan,
        ObservationModel::External,
        transition,
    )
}

/// Independently replay the correspondence binding: the correspondence must
/// carry this admission's exact validated plan and resource-profile receipt
/// and replay its own admitted structure.
fn validate_external_correspondence_binding(
    admission: &OwnedPlacementAdmission,
    correspondence: &AdmittedSchemaDeviceCorrespondence,
    transition: &str,
) -> Result<(), AccessPlanDiagnostic> {
    correspondence.validate_structure().map_err(|diagnostic| {
        AccessPlanDiagnostic(format!(
            "{transition} could not replay the retained schema/device correspondence: {diagnostic}"
        ))
    })?;
    if correspondence.placement_plan() != &admission.placement_plan
        || correspondence.profile_receipt() != admission.profile_receipt
    {
        return Err(AccessPlanDiagnostic(format!(
            "{transition} schema correspondence does not bind the owned placement admission's exact plan and resource-profile receipt"
        )));
    }
    Ok(())
}

impl OwnedCorrespondedExternalAdmission {
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

    pub const fn correspondence(&self) -> &AdmittedSchemaDeviceCorrespondence {
        &self.correspondence
    }

    /// Separate the route back into its non-Clone inputs.
    pub fn into_parts(self) -> (OwnedPlacementAdmission, AdmittedSchemaDeviceCorrespondence) {
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
        occurrence: PlacedOccurrenceId,
    ) -> Result<EstablishedOwnedExternalPlacement, OwnedExternalViewEstablishmentError> {
        into_validated_access(
            self,
            |carrier| {
                validate_owned_external_authority(
                    &carrier.admission,
                    &carrier.correspondence,
                    "owned External view",
                )
            },
            |carrier, ()| EstablishedOwnedExternalPlacement {
                admission: carrier.admission,
                correspondence: carrier.correspondence,
                occurrence,
            },
            |carrier, diagnostic| OwnedExternalViewEstablishmentError {
                carrier,
                occurrence,
                diagnostic,
            },
        )
    }
}

impl EstablishedOwnedExternalPlacement {
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

    pub const fn correspondence(&self) -> &AdmittedSchemaDeviceCorrespondence {
        &self.correspondence
    }

    pub const fn occurrence(&self) -> PlacedOccurrenceId {
        self.occurrence
    }

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
            &self.admission.placement_plan,
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
        into_validated_access(
            self,
            |established| {
                validate_owned_external_authority(
                    &established.admission,
                    &established.correspondence,
                    "owned External retirement",
                )
            },
            |established, ()| OwnedCorrespondedExternalAdmission {
                admission: established.admission,
                correspondence: established.correspondence,
            },
            |established, diagnostic| OwnedExternalRetirementError {
                established,
                diagnostic,
            },
        )
    }

    /// Project one accepted External field through a shared borrow of this
    /// provider-corresponded owned placement.
    ///
    /// The returned accessor retains this entire carrier, including its
    /// schema/device correspondence, through any sealed primitive request
    /// derived from it.
    pub fn project<'view>(
        &'view self,
        key: AccessFieldKey,
    ) -> Result<PlacedFieldProjection<'view, 'view>, AccessPlanDiagnostic> {
        self.project_with(key, BorrowPolarity::Shared)
    }

    /// Exclusively project one accepted External field through an exclusive
    /// borrow of this provider-corresponded owned placement.
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
        project_admission_field(
            &self.admission,
            key,
            current_borrow,
            BorrowPolarity::Exclusive,
            Some(ObservationModel::External),
            PlacementAuthorityRef::OwnedCorrespondedExternal(self),
        )
    }
}
