//! Provider-issued correspondence between one nominal placement and one
//! stable physical device instance.
//!
//! Resource-profile compatibility answers whether a range can support a
//! placement. This module deliberately carries the separate admitted claim
//! that the placement describes the named device. It performs no device read,
//! access admission, content establishment, or field access.

use crate::placements::access_admission::validate_access_admission;
use crate::placements::access_authority::AccessAuthorityRef;
use crate::primitive_access::field_projection::project_access_field;
use crate::{
    AccessAdmission, AccessAdmissionId, AccessFieldKey, AccessFieldProjection,
    AccessPlanDiagnostic, AccessPlanId, AccessView, AdmittedResourceProfile, BorrowPolarity,
    ResourceProfileReceiptId, ValidatedAccessPlan, place,
};
use extents::ExtentLoan;

use crate::normalized_identity;

normalized_identity!(
    SchemaCorrespondenceProviderId,
    "schema-correspondence provider identity"
);
normalized_identity!(StableDeviceInstanceId, "stable device-instance identity");
normalized_identity!(
    SchemaCorrespondenceSourceId,
    "schema-correspondence provenance-source identity"
);
normalized_identity!(
    RuntimeDeviceRevisionObservationId,
    "runtime device-revision observation identity"
);
normalized_identity!(
    DeviceRevisionPredicateId,
    "device-revision predicate identity"
);

/// Provider-issued evidence for one runtime revision observation.
///
/// The observed word is ordinary data. This non-Clone carrier is the admitted
/// evidence that the observation belongs to one stable device instance and
/// the same resource-profile grant later named by correspondence.
#[derive(Debug)]
#[must_use = "runtime revision evidence retains its provider/device/grant binding"]
pub struct RuntimeDeviceRevisionEvidence {
    observation: RuntimeDeviceRevisionObservationId,
    predicate: DeviceRevisionPredicateId,
    provider: SchemaCorrespondenceProviderId,
    device: StableDeviceInstanceId,
    profile_receipt: ResourceProfileReceiptId,
    observed_revision: u64,
}

impl RuntimeDeviceRevisionEvidence {
    pub fn from_admitted_provider(
        observation: RuntimeDeviceRevisionObservationId,
        predicate: DeviceRevisionPredicateId,
        provider: SchemaCorrespondenceProviderId,
        device: StableDeviceInstanceId,
        profile_receipt: ResourceProfileReceiptId,
        observed_revision: u64,
    ) -> Self {
        Self {
            observation,
            predicate,
            provider,
            device,
            profile_receipt,
            observed_revision,
        }
    }

    pub const fn observation(&self) -> RuntimeDeviceRevisionObservationId {
        self.observation
    }

    pub const fn predicate(&self) -> DeviceRevisionPredicateId {
        self.predicate
    }

    pub const fn provider(&self) -> SchemaCorrespondenceProviderId {
        self.provider
    }

    pub const fn device(&self) -> StableDeviceInstanceId {
        self.device
    }

    pub const fn profile_receipt(&self) -> ResourceProfileReceiptId {
        self.profile_receipt
    }

    pub const fn observed_revision(&self) -> u64 {
        self.observed_revision
    }
}

/// Provider-only authority to assert that one nominal placement describes one
/// exact stable device instance.
///
/// This remains separate from `AccessResourceCompatibility`: compatibility
/// cannot manufacture physical meaning. Optional revision evidence must name
/// the same provider, device, and resource-profile grant before this carrier
/// can be formed.
#[derive(Debug)]
#[must_use = "schema correspondence retains admitted provider/device provenance"]
pub struct SchemaDeviceCorrespondenceGrant {
    provider: SchemaCorrespondenceProviderId,
    device: StableDeviceInstanceId,
    source: SchemaCorrespondenceSourceId,
    placement: ValidatedAccessPlan,
    profile_receipt: ResourceProfileReceiptId,
    revision: Option<RuntimeDeviceRevisionEvidence>,
}

impl SchemaDeviceCorrespondenceGrant {
    pub fn from_admitted_provider(
        provider: SchemaCorrespondenceProviderId,
        device: StableDeviceInstanceId,
        source: SchemaCorrespondenceSourceId,
        placement: &ValidatedAccessPlan,
        profile_receipt: ResourceProfileReceiptId,
        revision: Option<RuntimeDeviceRevisionEvidence>,
    ) -> Result<Self, SchemaDeviceCorrespondenceGrantError> {
        if revision.as_ref().is_some_and(|revision| {
            revision.provider != provider
                || revision.device != device
                || revision.profile_receipt != profile_receipt
        }) {
            return Err(SchemaDeviceCorrespondenceGrantError {
                revision,
                diagnostic: AccessPlanDiagnostic(
                    "runtime revision evidence does not bind the correspondence provider, stable device instance, and resource-profile grant"
                        .into(),
                ),
            });
        }
        Ok(Self {
            provider,
            device,
            source,
            placement: placement.clone(),
            profile_receipt,
            revision,
        })
    }

    /// Independently join this admitted physical claim to the exact validated
    /// placement and admitted storage profile. Rejection returns the complete
    /// non-Clone grant for repair or retry and establishes no correspondence.
    pub fn admit(
        self,
        placement: &ValidatedAccessPlan,
        profile: &AdmittedResourceProfile,
    ) -> Result<AdmittedSchemaDeviceCorrespondence, SchemaDeviceCorrespondenceAdmissionError> {
        admit_schema_device_correspondence(self, placement, profile.receipt())
    }
}

/// Failed provider-grant formation returns optional runtime evidence intact.
#[derive(Debug)]
pub struct SchemaDeviceCorrespondenceGrantError {
    revision: Option<RuntimeDeviceRevisionEvidence>,
    diagnostic: AccessPlanDiagnostic,
}

impl SchemaDeviceCorrespondenceGrantError {
    diagnostic_accessor!();

    pub fn into_parts(self) -> (Option<RuntimeDeviceRevisionEvidence>, AccessPlanDiagnostic) {
        (self.revision, self.diagnostic)
    }
}

/// Admitted physical correspondence, intentionally distinct from storage
/// compatibility and content validity.
#[derive(Debug)]
#[must_use = "admitted schema correspondence retains physical provenance"]
pub struct AdmittedSchemaDeviceCorrespondence {
    provider: SchemaCorrespondenceProviderId,
    device: StableDeviceInstanceId,
    source: SchemaCorrespondenceSourceId,
    placement: ValidatedAccessPlan,
    profile_receipt: ResourceProfileReceiptId,
    revision: Option<RuntimeDeviceRevisionEvidence>,
}

/// Exact inert facts behind one admitted schema/device correspondence.
///
/// This context is deliberately not correspondence, placement, or device
/// authority. It retains the complete admitted structure so a later provider
/// receipt can be bound to more than the compact device and placement
/// identities, without making the non-Clone correspondence itself reusable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaDeviceCorrespondenceReceiptContext {
    provider: SchemaCorrespondenceProviderId,
    device: StableDeviceInstanceId,
    source: SchemaCorrespondenceSourceId,
    placement: ValidatedAccessPlan,
    profile_receipt: ResourceProfileReceiptId,
    revision: Option<RuntimeDeviceRevisionReceiptContext>,
}

impl SchemaDeviceCorrespondenceReceiptContext {
    /// Nominal provider identity only; this does not grant correspondence or
    /// device authority.
    pub const fn provider(&self) -> SchemaCorrespondenceProviderId {
        self.provider
    }

    /// Nominal stable-device identity only; this does not grant
    /// correspondence or device authority.
    pub const fn device(&self) -> StableDeviceInstanceId {
        self.device
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeDeviceRevisionReceiptContext {
    observation: RuntimeDeviceRevisionObservationId,
    predicate: DeviceRevisionPredicateId,
    provider: SchemaCorrespondenceProviderId,
    device: StableDeviceInstanceId,
    profile_receipt: ResourceProfileReceiptId,
    observed_revision: u64,
}

impl From<&RuntimeDeviceRevisionEvidence> for RuntimeDeviceRevisionReceiptContext {
    fn from(revision: &RuntimeDeviceRevisionEvidence) -> Self {
        Self {
            observation: revision.observation,
            predicate: revision.predicate,
            provider: revision.provider,
            device: revision.device,
            profile_receipt: revision.profile_receipt,
            observed_revision: revision.observed_revision,
        }
    }
}

impl AdmittedSchemaDeviceCorrespondence {
    pub const fn provider(&self) -> SchemaCorrespondenceProviderId {
        self.provider
    }

    pub const fn device(&self) -> StableDeviceInstanceId {
        self.device
    }

    pub const fn source(&self) -> SchemaCorrespondenceSourceId {
        self.source
    }

    pub const fn placement(&self) -> AccessPlanId {
        self.placement.identity()
    }

    pub const fn access_plan(&self) -> &ValidatedAccessPlan {
        &self.placement
    }

    pub const fn profile_receipt(&self) -> ResourceProfileReceiptId {
        self.profile_receipt
    }

    pub const fn revision(&self) -> Option<&RuntimeDeviceRevisionEvidence> {
        self.revision.as_ref()
    }

    /// Export the complete inert structure for exact provider-receipt
    /// binding. Cloning this context grants no correspondence, placement, or
    /// device authority.
    pub fn receipt_context(&self) -> SchemaDeviceCorrespondenceReceiptContext {
        SchemaDeviceCorrespondenceReceiptContext {
            provider: self.provider,
            device: self.device,
            source: self.source,
            placement: self.placement.clone(),
            profile_receipt: self.profile_receipt,
            revision: self
                .revision
                .as_ref()
                .map(RuntimeDeviceRevisionReceiptContext::from),
        }
    }

    pub(crate) fn validate_structure(&self) -> Result<(), AccessPlanDiagnostic> {
        if self.revision.as_ref().is_some_and(|revision| {
            revision.provider != self.provider
                || revision.device != self.device
                || revision.profile_receipt != self.profile_receipt
        }) {
            return Err(AccessPlanDiagnostic(
                "admitted schema correspondence could not replay its runtime revision provider, stable device instance, and resource-profile grant"
                    .into(),
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn replace_access_plan_for_test(&mut self, placement: AccessPlanId) -> AccessPlanId {
        std::mem::replace(&mut self.placement.identity, placement)
    }

    #[cfg(test)]
    pub(super) fn replace_profile_receipt_for_test(
        &mut self,
        receipt: ResourceProfileReceiptId,
    ) -> ResourceProfileReceiptId {
        std::mem::replace(&mut self.profile_receipt, receipt)
    }
}

/// Failed correspondence admission returns the exact provider grant rather
/// than reducing it to copied provider/device/receipt identities.
#[derive(Debug)]
pub struct SchemaDeviceCorrespondenceAdmissionError {
    grant: SchemaDeviceCorrespondenceGrant,
    diagnostic: AccessPlanDiagnostic,
}

impl SchemaDeviceCorrespondenceAdmissionError {
    diagnostic_accessor!();

    pub fn into_parts(self) -> (SchemaDeviceCorrespondenceGrant, AccessPlanDiagnostic) {
        (self.grant, self.diagnostic)
    }
}

fn admit_schema_device_correspondence(
    grant: SchemaDeviceCorrespondenceGrant,
    placement: &ValidatedAccessPlan,
    profile_receipt: ResourceProfileReceiptId,
) -> Result<AdmittedSchemaDeviceCorrespondence, SchemaDeviceCorrespondenceAdmissionError> {
    if grant.revision.as_ref().is_some_and(|revision| {
        revision.provider != grant.provider
            || revision.device != grant.device
            || revision.profile_receipt != grant.profile_receipt
    }) {
        return Err(SchemaDeviceCorrespondenceAdmissionError {
            grant,
            diagnostic: AccessPlanDiagnostic(
                "schema correspondence could not replay its runtime revision provider, stable device instance, and resource-profile grant"
                    .into(),
            ),
        });
    }
    if grant.placement != *placement {
        return Err(SchemaDeviceCorrespondenceAdmissionError {
            grant,
            diagnostic: AccessPlanDiagnostic(
                "schema correspondence does not name the exact validated placement".into(),
            ),
        });
    }
    if grant.profile_receipt != profile_receipt {
        return Err(SchemaDeviceCorrespondenceAdmissionError {
            grant,
            diagnostic: AccessPlanDiagnostic(
                "schema correspondence does not name the exact admitted resource-profile grant"
                    .into(),
            ),
        });
    }
    let SchemaDeviceCorrespondenceGrant {
        provider,
        device,
        source,
        placement,
        profile_receipt,
        revision,
    } = grant;
    Ok(AdmittedSchemaDeviceCorrespondence {
        provider,
        device,
        source,
        placement,
        profile_receipt,
        revision,
    })
}

/// One borrowed access admission joined to its separate admitted physical
/// correspondence. The fields remain distinct: correspondence contributes no
/// storage compatibility, loan, content, or access authority.
#[derive(Debug)]
#[must_use = "corresponded access admission retains loan and physical provenance"]
pub struct SchemaCorrespondedAccessAdmission<'extent> {
    admission: AccessAdmission<'extent>,
    correspondence: AdmittedSchemaDeviceCorrespondence,
}

impl<'extent> SchemaCorrespondedAccessAdmission<'extent> {
    pub const fn admission(&self) -> AccessAdmissionId {
        self.admission.identity
    }

    pub const fn correspondence(&self) -> &AdmittedSchemaDeviceCorrespondence {
        &self.correspondence
    }

    pub fn into_parts(self) -> (AccessAdmission<'extent>, AdmittedSchemaDeviceCorrespondence) {
        (self.admission, self.correspondence)
    }

    /// Cancel the permission-only placement while preserving the separately
    /// admitted correspondence for a later matching placement.
    pub fn withdraw(self) -> (ExtentLoan<'extent>, AdmittedSchemaDeviceCorrespondence) {
        (self.admission.withdraw(), self.correspondence)
    }

    /// Establish the borrowed view only after independently replaying both the
    /// correspondence relation and the retained access admission. The
    /// resulting carrier retains the exact correspondence through every
    /// projection derived from it.
    pub fn establish_view(
        self,
    ) -> Result<
        SchemaCorrespondedAccessView<'extent>,
        SchemaCorrespondedPlaceEstablishmentError<'extent>,
    > {
        if let Err(diagnostic) =
            validate_schema_correspondence_access_binding(&self.admission, &self.correspondence)
        {
            return Err(SchemaCorrespondedPlaceEstablishmentError {
                bound: self,
                diagnostic,
            });
        }
        let Self {
            admission,
            correspondence,
        } = self;
        match place(admission) {
            Ok(view) => Ok(SchemaCorrespondedAccessView {
                view,
                correspondence,
            }),
            Err(rejection) => {
                let (admission, diagnostic) = rejection.into_parts();
                Err(SchemaCorrespondedPlaceEstablishmentError {
                    bound: Self {
                        admission,
                        correspondence,
                    },
                    diagnostic,
                })
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn replace_correspondence_access_plan_for_test(
        &mut self,
        placement: AccessPlanId,
    ) -> AccessPlanId {
        self.correspondence.replace_access_plan_for_test(placement)
    }
}

/// Established borrowed view retaining its separate physical correspondence.
///
#[derive(Debug)]
#[must_use = "corresponded access view retains loan and physical provenance"]
pub struct SchemaCorrespondedAccessView<'extent> {
    view: AccessView<'extent>,
    correspondence: AdmittedSchemaDeviceCorrespondence,
}

impl<'extent> SchemaCorrespondedAccessView<'extent> {
    pub const fn admission(&self) -> AccessAdmissionId {
        self.view.admission()
    }

    pub const fn base(&self) -> u64 {
        self.view.base()
    }

    pub const fn length(&self) -> u64 {
        self.view.length()
    }

    pub const fn correspondence(&self) -> &AdmittedSchemaDeviceCorrespondence {
        &self.correspondence
    }

    /// End this corresponded access view after independently replaying both
    /// its exact borrowed access authority and physical correspondence.
    /// Success returns the original loan and non-Clone correspondence as
    /// distinct values; rejection returns this complete view for repair and
    /// retry. No content or device operation is established.
    pub fn retire(
        self,
    ) -> Result<
        (ExtentLoan<'extent>, AdmittedSchemaDeviceCorrespondence),
        SchemaCorrespondedPlaceRetirementError<'extent>,
    > {
        if let Err(diagnostic) = self.validate_retirement() {
            return Err(SchemaCorrespondedPlaceRetirementError {
                view: self,
                diagnostic,
            });
        }
        let Self {
            view,
            correspondence,
        } = self;
        Ok((view.loan, correspondence))
    }

    crate::placements::field_projection_accessors!('extent);

    fn project_with<'view>(
        &'view self,
        key: AccessFieldKey,
        current_borrow: BorrowPolarity,
    ) -> Result<AccessFieldProjection<'view, 'extent>, AccessPlanDiagnostic> {
        let source_loan = BorrowPolarity::from(self.view.loan.polarity());
        project_access_field(
            &self.view.plan,
            self.view.profile_receipt,
            &self.view.resources,
            self.view.admission,
            self.view.loan.base(),
            key,
            current_borrow,
            source_loan,
            None,
            AccessAuthorityRef::CorrespondedBorrowed(self),
        )
    }

    pub(super) const fn view(&self) -> &AccessView<'extent> {
        &self.view
    }

    pub(super) fn validate_correspondence(&self) -> Result<(), AccessPlanDiagnostic> {
        self.correspondence.validate_structure()?;
        if self.correspondence.placement != self.view.plan
            || self.correspondence.profile_receipt != self.view.profile_receipt
        {
            return Err(AccessPlanDiagnostic(
                "corresponded access view could not replay its exact placement and resource-profile correspondence"
                    .into(),
            ));
        }
        Ok(())
    }

    fn validate_retirement(&self) -> Result<(), AccessPlanDiagnostic> {
        self.validate_correspondence()?;
        self.view
            .validate_authority("corresponded access-view retirement")
    }

    #[cfg(test)]
    pub(crate) fn replace_correspondence_access_plan_for_test(
        &mut self,
        placement: AccessPlanId,
    ) -> AccessPlanId {
        self.correspondence.replace_access_plan_for_test(placement)
    }

    #[cfg(test)]
    pub(crate) fn replace_view_profile_receipt_for_test(
        &mut self,
        receipt: ResourceProfileReceiptId,
    ) -> ResourceProfileReceiptId {
        std::mem::replace(&mut self.view.profile_receipt, receipt)
    }

    #[cfg(test)]
    pub(crate) fn replace_correspondence_profile_receipt_for_test(
        &mut self,
        receipt: ResourceProfileReceiptId,
    ) -> ResourceProfileReceiptId {
        self.correspondence
            .replace_profile_receipt_for_test(receipt)
    }
}

/// Failed corresponded-view retirement preserves the complete loan-bearing
/// view and its non-Clone physical provenance for corrected retry.
#[derive(Debug)]
pub struct SchemaCorrespondedPlaceRetirementError<'extent> {
    view: SchemaCorrespondedAccessView<'extent>,
    diagnostic: AccessPlanDiagnostic,
}

impl<'extent> SchemaCorrespondedPlaceRetirementError<'extent> {
    diagnostic_accessor!();

    pub fn into_parts(self) -> (SchemaCorrespondedAccessView<'extent>, AccessPlanDiagnostic) {
        (self.view, self.diagnostic)
    }
}

/// Failed corresponded-view establishment returns the complete bound carrier;
/// its loan and physical provenance remain available for repair or withdrawal.
#[derive(Debug)]
pub struct SchemaCorrespondedPlaceEstablishmentError<'extent> {
    bound: SchemaCorrespondedAccessAdmission<'extent>,
    diagnostic: AccessPlanDiagnostic,
}

impl<'extent> SchemaCorrespondedPlaceEstablishmentError<'extent> {
    diagnostic_accessor!();

    pub fn into_parts(
        self,
    ) -> (
        SchemaCorrespondedAccessAdmission<'extent>,
        AccessPlanDiagnostic,
    ) {
        (self.bound, self.diagnostic)
    }
}

/// Failed placement/correspondence binding returns both exact non-Clone
/// inputs. No loan is released and no physical meaning is attached to storage.
#[derive(Debug)]
pub struct SchemaCorrespondenceAccessBindingError<'extent> {
    admission: AccessAdmission<'extent>,
    correspondence: AdmittedSchemaDeviceCorrespondence,
    diagnostic: AccessPlanDiagnostic,
}

impl<'extent> SchemaCorrespondenceAccessBindingError<'extent> {
    diagnostic_accessor!();

    pub fn into_parts(
        self,
    ) -> (
        AccessAdmission<'extent>,
        AdmittedSchemaDeviceCorrespondence,
        AccessPlanDiagnostic,
    ) {
        (self.admission, self.correspondence, self.diagnostic)
    }
}

/// Join one correspondence fact to the exact borrowed access admission it
/// describes. This is a custody relation only; it establishes no access view,
/// content qualification, field access, or device operation.
pub fn bind_schema_correspondence_to_access_plan<'extent>(
    admission: AccessAdmission<'extent>,
    correspondence: AdmittedSchemaDeviceCorrespondence,
) -> Result<
    SchemaCorrespondedAccessAdmission<'extent>,
    SchemaCorrespondenceAccessBindingError<'extent>,
> {
    let diagnostic = validate_schema_correspondence_access_binding(&admission, &correspondence);
    if let Err(diagnostic) = diagnostic {
        return Err(SchemaCorrespondenceAccessBindingError {
            admission,
            correspondence,
            diagnostic,
        });
    }
    Ok(SchemaCorrespondedAccessAdmission {
        admission,
        correspondence,
    })
}

fn validate_schema_correspondence_access_binding(
    admission: &AccessAdmission<'_>,
    correspondence: &AdmittedSchemaDeviceCorrespondence,
) -> Result<(), AccessPlanDiagnostic> {
    correspondence.validate_structure()?;
    if correspondence.placement != admission.access_plan
        || correspondence.profile_receipt != admission.profile_receipt
        || admission.profile.receipt() != admission.profile_receipt
    {
        return Err(AccessPlanDiagnostic(
            "schema correspondence does not bind the access admission's exact plan and resource-profile receipt"
                .into(),
        ));
    }
    let replayed = validate_access_admission(
        &admission.loan,
        &admission.access_plan,
        &admission.profile,
    )
    .map_err(|diagnostic| {
        AccessPlanDiagnostic(format!(
            "schema correspondence could not replay the retained access admission: {diagnostic}"
        ))
    })?;
    if replayed != admission.resources {
        return Err(AccessPlanDiagnostic(
            "schema correspondence replayed resource compatibility differs from the retained access admission"
                .into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
